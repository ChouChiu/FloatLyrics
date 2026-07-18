// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Provider-neutral cache contracts and persistence input models.

use floatlyrics_core::track::TrackMetadata;

use crate::lyrics::LyricsProvider;

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
