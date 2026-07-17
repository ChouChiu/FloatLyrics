// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! SQLite schema owned by the cache implementation.

pub(super) const MIGRATION: &str = r#"
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
"#;

pub(super) const PROVIDER_RESULTS_CONTENT_UNIQUE_MIGRATION: &str = r#"
    DELETE FROM provider_results
    WHERE id NOT IN (
        SELECT MAX(id)
        FROM provider_results
        GROUP BY
            track_fingerprint,
            provider,
            COALESCE(provider_track_id, ''),
            COALESCE(raw_lyrics, '')
    );

    CREATE UNIQUE INDEX IF NOT EXISTS provider_results_content_unique
        ON provider_results (
            track_fingerprint,
            provider,
            COALESCE(provider_track_id, ''),
            COALESCE(raw_lyrics, '')
        );
"#;
