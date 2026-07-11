// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use std::{path::Path, str::FromStr};

use crate::lyrics::LyricsProvider;
use floatlyrics_core::track::TrackMetadata;

/// Trait abstracting lyrics cache operations.
/// Enables decoupling controller and manual-search from the concrete [`Cache`] type.
pub trait LyricsCache {
    fn upsert_track(&self, track: &TrackMetadata) -> anyhow::Result<String>;

    fn insert_lyrics(
        &self,
        provider: LyricsProvider,
        provider_track_id: Option<&str>,
        title: &str,
        artists: &[String],
        raw_lyrics: &str,
    ) -> anyhow::Result<i64>;

    fn bind_manual_match(&self, track_fingerprint: &str, lyrics_id: i64) -> anyhow::Result<()>;

    fn lyrics_for_track(
        &self,
        track_fingerprint: &str,
        provider_order: &[LyricsProvider],
    ) -> anyhow::Result<Option<CachedLyrics>>;

    fn insert_provider_result(&self, result: ProviderResultInsert<'_>) -> anyhow::Result<i64>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedLyrics {
    pub id: i64,
    pub provider: LyricsProvider,
    pub provider_track_id: Option<String>,
    pub title: String,
    pub artists: Vec<String>,
    pub raw_lyrics: String,
}

pub struct Cache {
    conn: Connection,
}

impl Cache {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating database directory {}", parent.display()))?;
        }

        let conn = Connection::open(path)
            .with_context(|| format!("opening database {}", path.display()))?;
        let cache = Self { conn };
        cache.migrate()?;
        Ok(cache)
    }

    pub fn open_memory() -> Result<Self> {
        let cache = Self {
            conn: Connection::open_in_memory().context("opening in-memory database")?,
        };
        cache.migrate()?;
        Ok(cache)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS tracks (
                fingerprint TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                artists_json TEXT NOT NULL,
                album TEXT,
                duration_ms INTEGER,
                mpris_track_id TEXT,
                last_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS lyrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                provider TEXT NOT NULL,
                provider_track_id TEXT,
                title TEXT NOT NULL,
                artists_json TEXT NOT NULL,
                raw_lyrics TEXT NOT NULL,
                content_hash TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS manual_matches (
                track_fingerprint TEXT PRIMARY KEY REFERENCES tracks(fingerprint) ON DELETE CASCADE,
                lyrics_id INTEGER NOT NULL REFERENCES lyrics(id) ON DELETE CASCADE,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS provider_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                track_fingerprint TEXT NOT NULL REFERENCES tracks(fingerprint) ON DELETE CASCADE,
                provider TEXT NOT NULL,
                provider_track_id TEXT,
                title TEXT NOT NULL,
                artists_json TEXT NOT NULL,
                score REAL NOT NULL DEFAULT 0,
                raw_lyrics TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS provider_results_track_provider_score
                ON provider_results(track_fingerprint, provider, score DESC, id DESC);
            "#,
        )?;
        Ok(())
    }

    pub fn upsert_track(&self, track: &TrackMetadata) -> Result<String> {
        let fingerprint = track.fingerprint();
        let artists_json = serde_json::to_string(&track.artists)?;
        let duration_ms: Option<i64> = track.duration_ms.and_then(|value| value.try_into().ok());
        self.conn.execute(
            r#"
            INSERT INTO tracks (fingerprint, title, artists_json, album, duration_ms, mpris_track_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(fingerprint) DO UPDATE SET
                title = excluded.title,
                artists_json = excluded.artists_json,
                album = excluded.album,
                duration_ms = excluded.duration_ms,
                mpris_track_id = excluded.mpris_track_id,
                last_seen_at = CURRENT_TIMESTAMP
            "#,
            params![
                fingerprint,
                track.title,
                artists_json,
                track.album,
                duration_ms,
                track.mpris_track_id,
            ],
        )?;
        Ok(fingerprint)
    }

    pub fn insert_lyrics(
        &self,
        provider: LyricsProvider,
        provider_track_id: Option<&str>,
        title: &str,
        artists: &[String],
        raw_lyrics: &str,
    ) -> Result<i64> {
        let artists_json = serde_json::to_string(artists)?;
        let content_hash = floatlyrics_core::track::track_fingerprint(title, artists, None, None)
            + ":"
            + &hash_content(raw_lyrics);

        self.conn.execute(
            r#"
            INSERT OR IGNORE INTO lyrics
                (provider, provider_track_id, title, artists_json, raw_lyrics, content_hash)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                provider.as_str(),
                provider_track_id,
                title,
                artists_json,
                raw_lyrics,
                content_hash
            ],
        )?;

        let id = self.conn.query_row(
            "SELECT id FROM lyrics WHERE content_hash = ?1",
            params![content_hash],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn bind_manual_match(&self, track_fingerprint: &str, lyrics_id: i64) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO manual_matches (track_fingerprint, lyrics_id)
            VALUES (?1, ?2)
            ON CONFLICT(track_fingerprint) DO UPDATE SET
                lyrics_id = excluded.lyrics_id,
                created_at = CURRENT_TIMESTAMP
            "#,
            params![track_fingerprint, lyrics_id],
        )?;
        Ok(())
    }

    pub fn lyrics_for_track(
        &self,
        track_fingerprint: &str,
        provider_order: &[LyricsProvider],
    ) -> Result<Option<CachedLyrics>> {
        if let Some(manual) = self.manual_lyrics_for_track(track_fingerprint)? {
            return Ok(Some(manual));
        }

        for provider in provider_order {
            if let Some(cached) = self.latest_provider_result(track_fingerprint, *provider)? {
                return Ok(Some(cached));
            }
        }

        Ok(None)
    }

    fn manual_lyrics_for_track(&self, track_fingerprint: &str) -> Result<Option<CachedLyrics>> {
        self.conn
            .query_row(
                r#"
                SELECT lyrics.id, lyrics.provider, lyrics.provider_track_id, lyrics.title,
                       lyrics.artists_json, lyrics.raw_lyrics
                FROM manual_matches
                JOIN lyrics ON lyrics.id = manual_matches.lyrics_id
                WHERE manual_matches.track_fingerprint = ?1
                "#,
                params_from_iter([track_fingerprint]),
                row_to_cached_lyrics,
            )
            .optional()
            .context("loading manual lyrics match")
    }

    fn latest_provider_result(
        &self,
        track_fingerprint: &str,
        provider: LyricsProvider,
    ) -> Result<Option<CachedLyrics>> {
        self.conn
            .query_row(
                r#"
                SELECT id, provider, provider_track_id, title, artists_json, raw_lyrics
                FROM provider_results
                WHERE track_fingerprint = ?1 AND provider = ?2 AND raw_lyrics IS NOT NULL
                ORDER BY score DESC, id DESC
                LIMIT 1
                "#,
                params_from_iter([track_fingerprint, provider.as_str()]),
                row_to_cached_lyrics,
            )
            .optional()
            .context("loading cached provider result")
    }

    pub fn insert_provider_result(&self, result: ProviderResultInsert<'_>) -> Result<i64> {
        let artists_json = serde_json::to_string(result.artists)?;
        self.conn.execute(
            r#"
            INSERT INTO provider_results
                (track_fingerprint, provider, provider_track_id, title, artists_json, score, raw_lyrics)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                result.track_fingerprint,
                result.provider.as_str(),
                result.provider_track_id,
                result.title,
                artists_json,
                result.score,
                result.raw_lyrics
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }
}

impl LyricsCache for Cache {
    fn upsert_track(&self, track: &TrackMetadata) -> anyhow::Result<String> {
        Cache::upsert_track(self, track)
    }

    fn insert_lyrics(
        &self,
        provider: LyricsProvider,
        provider_track_id: Option<&str>,
        title: &str,
        artists: &[String],
        raw_lyrics: &str,
    ) -> anyhow::Result<i64> {
        Cache::insert_lyrics(
            self,
            provider,
            provider_track_id,
            title,
            artists,
            raw_lyrics,
        )
    }

    fn bind_manual_match(&self, track_fingerprint: &str, lyrics_id: i64) -> anyhow::Result<()> {
        Cache::bind_manual_match(self, track_fingerprint, lyrics_id)
    }

    fn lyrics_for_track(
        &self,
        track_fingerprint: &str,
        provider_order: &[LyricsProvider],
    ) -> anyhow::Result<Option<CachedLyrics>> {
        Cache::lyrics_for_track(self, track_fingerprint, provider_order)
    }

    fn insert_provider_result(&self, result: ProviderResultInsert<'_>) -> anyhow::Result<i64> {
        Cache::insert_provider_result(self, result)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProviderResultInsert<'a> {
    pub track_fingerprint: &'a str,
    pub provider: LyricsProvider,
    pub provider_track_id: Option<&'a str>,
    pub title: &'a str,
    pub artists: &'a [String],
    pub score: f64,
    pub raw_lyrics: Option<&'a str>,
}

fn row_to_cached_lyrics(row: &rusqlite::Row<'_>) -> rusqlite::Result<CachedLyrics> {
    let provider_raw: String = row.get(1)?;
    let artists_json: String = row.get(4)?;
    Ok(CachedLyrics {
        id: row.get(0)?,
        provider: LyricsProvider::from_str(&provider_raw).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(err))
        })?,
        provider_track_id: row.get(2)?,
        title: row.get(3)?,
        artists: serde_json::from_str(&artists_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(err))
        })?,
        raw_lyrics: row.get(5)?,
    })
}

fn hash_content(content: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
#[path = "test/cache_test.rs"]
mod tests;
