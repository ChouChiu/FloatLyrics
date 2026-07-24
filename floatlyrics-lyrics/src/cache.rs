// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Lyrics persistence facade separating domain contracts from SQLite details.

mod model;
mod schema;
mod sqlite;

pub use model::{
    CachedLyrics, LyricsCache, LyricsInsert, ProviderResultInsert, TRACK_OFFSET_MS_MAX,
    TRACK_OFFSET_MS_MIN,
};
pub use sqlite::Cache;
