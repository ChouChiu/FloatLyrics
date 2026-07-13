// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use std::{path::Path, str::FromStr};

use crate::lyrics::LyricsProvider;
use floatlyrics_core::track::TrackMetadata;

mod schema;

/// Persistence boundary used by lyrics controllers.
///
/// Implementations may use SQLite, another database, or an in-memory test
/// double. Methods use domain values so callers do not depend on SQL details.
pub trait LyricsCache {
    /// Records the latest track metadata and returns its stable fingerprint.
    ///
    /// # Errors
    /// Returns an error when serialization or persistence fails.
    fn upsert_track(&self, track: &TrackMetadata) -> anyhow::Result<String>;

    /// Stores a reusable lyrics document and returns its backend identifier.
    ///
    /// # Errors
    /// Returns an error when serialization or persistence fails.
    fn insert_lyrics(&self, lyrics: LyricsInsert<'_>) -> anyhow::Result<i64>;

    /// Associates a track fingerprint with manually selected lyrics.
    ///
    /// # Errors
    /// Returns an error when either identifier is invalid or persistence fails.
    fn bind_manual_match(&self, track_fingerprint: &str, lyrics_id: i64) -> anyhow::Result<()>;

    /// Loads the best cached lyrics, preferring a manual selection.
    ///
    /// Providers are considered in `provider_order` after the manual mapping.
    ///
    /// # Errors
    /// Returns an error when stored data is invalid or cannot be read.
    fn lyrics_for_track(
        &self,
        track_fingerprint: &str,
        provider_order: &[LyricsProvider],
    ) -> anyhow::Result<Option<CachedLyrics>>;

    /// Records an automatic provider result and returns its backend identifier.
    ///
    /// # Errors
    /// Returns an error when serialization or persistence fails.
    fn insert_provider_result(&self, result: ProviderResultInsert<'_>) -> anyhow::Result<i64>;
}

/// Lyrics document returned by a cache lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedLyrics {
    /// Whether this document was explicitly selected for the track.
    pub manually_selected: bool,
    /// Backend-specific row identifier.
    pub id: i64,
    /// Provider that supplied the document.
    pub provider: LyricsProvider,
    /// Provider-specific track identifier, when available.
    pub provider_track_id: Option<String>,
    /// Title stored with the lyrics.
    pub title: String,
    /// Artists stored with the lyrics.
    pub artists: Vec<String>,
    /// Original lyrics payload.
    pub raw_lyrics: String,
}

/// SQLite-backed implementation of [`LyricsCache`].
pub struct Cache {
    conn: Connection,
}

impl Cache {
    /// Opens or creates a cache at `path` and applies its schema.
    ///
    /// # Errors
    /// Returns an error if the directory, database, or schema cannot be created.
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

    /// Opens an isolated in-memory cache.
    ///
    /// # Errors
    /// Returns an error if SQLite cannot initialize the database or schema.
    pub fn open_memory() -> Result<Self> {
        let cache = Self {
            conn: Connection::open_in_memory().context("opening in-memory database")?,
        };
        cache.migrate()?;
        Ok(cache)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(schema::MIGRATION)?;
        let has_content_unique_index: bool = self.conn.query_row(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM sqlite_master
                WHERE type = 'index' AND name = 'provider_results_content_unique'
            )
            "#,
            [],
            |row| row.get(0),
        )?;
        if !has_content_unique_index {
            self.conn
                .execute_batch(schema::PROVIDER_RESULTS_CONTENT_UNIQUE_MIGRATION)
                .context("creating provider result content index")?;
        }
        Ok(())
    }

    fn upsert_track(&self, track: &TrackMetadata) -> Result<String> {
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

    fn insert_lyrics(&self, lyrics: LyricsInsert<'_>) -> Result<i64> {
        let artists_json = serde_json::to_string(lyrics.artists)?;
        let content_hash =
            floatlyrics_core::track::track_fingerprint(lyrics.title, lyrics.artists, None, None)
                + ":"
                + &floatlyrics_core::digest::sha256_hex(lyrics.raw_lyrics);

        self.conn.execute(
            r#"
            INSERT OR IGNORE INTO lyrics
                (provider, provider_track_id, title, artists_json, raw_lyrics, content_hash)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                lyrics.provider.as_str(),
                lyrics.provider_track_id,
                lyrics.title,
                artists_json,
                lyrics.raw_lyrics,
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

    fn bind_manual_match(&self, track_fingerprint: &str, lyrics_id: i64) -> Result<()> {
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

    fn lyrics_for_track(
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
                |row| row_to_cached_lyrics(row, true),
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
                |row| row_to_cached_lyrics(row, false),
            )
            .optional()
            .context("loading cached provider result")
    }

    fn insert_provider_result(&self, result: ProviderResultInsert<'_>) -> Result<i64> {
        let artists_json = serde_json::to_string(result.artists)?;
        self.conn.query_row(
            r#"
            INSERT INTO provider_results
                (track_fingerprint, provider, provider_track_id, title, artists_json, score, raw_lyrics)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT DO UPDATE SET
                title = excluded.title,
                artists_json = excluded.artists_json,
                score = excluded.score,
                created_at = CURRENT_TIMESTAMP
            RETURNING id
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
            |row| row.get(0),
        )
        .context("storing provider result")
    }
}

impl LyricsCache for Cache {
    fn upsert_track(&self, track: &TrackMetadata) -> anyhow::Result<String> {
        Cache::upsert_track(self, track)
    }

    fn insert_lyrics(&self, lyrics: LyricsInsert<'_>) -> anyhow::Result<i64> {
        Cache::insert_lyrics(self, lyrics)
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

/// Borrowed input for storing a manually reusable lyrics document.
#[derive(Debug, Clone, Copy)]
pub struct LyricsInsert<'a> {
    /// Provider that supplied the lyrics.
    pub provider: LyricsProvider,
    /// Provider-specific track identifier, when available.
    pub provider_track_id: Option<&'a str>,
    /// Track title reported by the provider.
    pub title: &'a str,
    /// Track artists reported by the provider.
    pub artists: &'a [String],
    /// Original lyrics payload.
    pub raw_lyrics: &'a str,
}

/// Borrowed input for recording an automatic provider search result.
#[derive(Debug, Clone, Copy)]
pub struct ProviderResultInsert<'a> {
    /// Fingerprint of the requested track.
    pub track_fingerprint: &'a str,
    /// Provider that handled the request.
    pub provider: LyricsProvider,
    /// Provider-specific track identifier, when available.
    pub provider_track_id: Option<&'a str>,
    /// Title returned by the provider.
    pub title: &'a str,
    /// Artists returned by the provider.
    pub artists: &'a [String],
    /// Provider match score; higher values are preferred.
    pub score: f64,
    /// Original lyrics payload, or `None` for an unsuccessful result.
    pub raw_lyrics: Option<&'a str>,
}

fn row_to_cached_lyrics(
    row: &rusqlite::Row<'_>,
    manually_selected: bool,
) -> rusqlite::Result<CachedLyrics> {
    let provider_raw: String = row.get(1)?;
    let artists_json: String = row.get(4)?;
    Ok(CachedLyrics {
        manually_selected,
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

#[cfg(test)]
#[path = "test/cache_test.rs"]
mod tests;
