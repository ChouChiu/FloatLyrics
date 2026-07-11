// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackMetadata {
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub mpris_track_id: Option<String>,
}

impl TrackMetadata {
    pub fn fingerprint(&self) -> String {
        track_fingerprint(
            &self.title,
            &self.artists,
            self.album.as_deref(),
            self.duration_ms,
        )
    }

    pub fn display_artist(&self) -> String {
        self.artists.join(", ")
    }

    pub fn playback_identity(&self) -> String {
        self.mpris_track_id
            .clone()
            .unwrap_or_else(|| self.fingerprint())
    }
}

pub fn track_fingerprint(
    title: &str,
    artists: &[String],
    album: Option<&str>,
    duration_ms: Option<u64>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonicalize(title).as_bytes());
    hasher.update(b"\0");

    let mut canonical_artists = artists
        .iter()
        .map(|artist| canonicalize(artist))
        .collect::<Vec<_>>();
    canonical_artists.sort();
    hasher.update(canonical_artists.join(";").as_bytes());
    hasher.update(b"\0");

    if let Some(album) = album {
        hasher.update(canonicalize(album).as_bytes());
    }
    hasher.update(b"\0");

    if let Some(duration_ms) = duration_ms {
        let rounded_seconds = duration_ms.saturating_add(500) / 1000;
        hasher.update(rounded_seconds.to_string().as_bytes());
    }

    format!("{:x}", hasher.finalize())
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
