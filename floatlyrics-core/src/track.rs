// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Track metadata shared by playback, search, and persistence layers.

use serde::{Deserialize, Serialize};

use crate::digest::sha256_hex;

/// Provider-neutral metadata describing a playable track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackMetadata {
    /// Display title.
    pub title: String,
    /// Ordered display artist names.
    pub artists: Vec<String>,
    /// Album title, when supplied by the player.
    pub album: Option<String>,
    /// Track duration in milliseconds, when known.
    pub duration_ms: Option<u64>,
    /// MPRIS object path or identifier, when supplied by the player.
    pub mpris_track_id: Option<String>,
}

impl TrackMetadata {
    /// Computes the stable, metadata-derived cache fingerprint for this track.
    pub fn fingerprint(&self) -> String {
        track_fingerprint(
            &self.title,
            &self.artists,
            self.album.as_deref(),
            self.duration_ms,
        )
    }

    /// Joins artist names for display and provider queries.
    pub fn display_artist(&self) -> String {
        self.artists.join(", ")
    }

    /// Returns the player identifier, falling back to [`Self::fingerprint`].
    pub fn playback_identity(&self) -> String {
        self.mpris_track_id
            .clone()
            .unwrap_or_else(|| self.fingerprint())
    }
}

/// Computes a stable SHA-256 fingerprint from canonical track metadata.
///
/// Artist order and insignificant whitespace or case do not affect the result.
/// Duration is rounded to the nearest second so small player differences still
/// address the same cache entry.
pub fn track_fingerprint(
    title: &str,
    artists: &[String],
    album: Option<&str>,
    duration_ms: Option<u64>,
) -> String {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(canonicalize(title).as_bytes());
    canonical.push(0);

    let mut canonical_artists = artists
        .iter()
        .map(|artist| canonicalize(artist))
        .collect::<Vec<_>>();
    canonical_artists.sort();
    canonical.extend_from_slice(canonical_artists.join(";").as_bytes());
    canonical.push(0);

    if let Some(album) = album {
        canonical.extend_from_slice(canonicalize(album).as_bytes());
    }
    canonical.push(0);

    if let Some(duration_ms) = duration_ms {
        let rounded_seconds = duration_ms.saturating_add(500) / 1000;
        canonical.extend_from_slice(rounded_seconds.to_string().as_bytes());
    }

    sha256_hex(canonical)
}

fn canonicalize(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
#[path = "test/track_test.rs"]
mod tests;
