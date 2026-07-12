// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Provider-neutral state converted from MPRIS properties and signals.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Instant};
use zvariant::{OwnedObjectPath, OwnedValue};

use floatlyrics_core::track::TrackMetadata;

/// State change emitted by the background MPRIS watcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotifyWatcherEvent {
    /// A matching player appeared with its initial state.
    Connected(SpotifyPlayerState),
    /// Metadata or playback status changed.
    Updated(SpotifyPlayerState),
    /// A new authoritative playback position sample arrived.
    PositionUpdated {
        /// Identity of the sampled track, when known.
        track_identity: Option<String>,
        /// Sampled position in milliseconds.
        position_ms: u64,
        /// Local monotonic time at which the sample was taken.
        sampled_at: Instant,
    },
    /// The active matching player disappeared.
    Disconnected,
    /// The watcher stopped because of a fatal error.
    Error(String),
}

/// Latest known state for one Spotify-compatible MPRIS player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotifyPlayerState {
    /// D-Bus well-known name of the player instance.
    pub bus_name: String,
    /// Current playback status.
    pub playback_status: PlaybackStatus,
    /// Playback position in milliseconds, when known.
    pub position_ms: Option<u64>,
    /// Current track metadata, when known.
    pub track: Option<TrackMetadata>,
}

/// Typed subset of MPRIS metadata used by FloatLyrics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpotifyMetadata {
    /// Track title.
    pub title: String,
    /// Track artists.
    pub artists: Vec<String>,
    /// Album title, when supplied.
    pub album: Option<String>,
    /// Track length in microseconds, when supplied.
    pub length_us: Option<u64>,
    /// MPRIS track object path, when supplied.
    pub track_id: Option<String>,
}

/// Extracts the metadata fields used by FloatLyrics from MPRIS properties.
///
/// Returns `None` when required title metadata is absent or has an unexpected
/// D-Bus type.
pub fn spotify_metadata_from_mpris(
    metadata: &HashMap<String, OwnedValue>,
) -> Option<SpotifyMetadata> {
    Some(SpotifyMetadata {
        title: string_value(metadata.get("xesam:title")?)?,
        artists: string_vec_value(metadata.get("xesam:artist")?).unwrap_or_default(),
        album: metadata.get("xesam:album").and_then(string_value),
        length_us: metadata.get("mpris:length").and_then(u64_value),
        track_id: metadata.get("mpris:trackid").and_then(object_path_value),
    })
}

fn string_value(value: &OwnedValue) -> Option<String> {
    String::try_from(value.try_clone().ok()?).ok()
}

fn string_vec_value(value: &OwnedValue) -> Option<Vec<String>> {
    Vec::<String>::try_from(value.try_clone().ok()?).ok()
}

fn u64_value(value: &OwnedValue) -> Option<u64> {
    i64::try_from(value.try_clone().ok()?).ok()?.try_into().ok()
}

fn object_path_value(value: &OwnedValue) -> Option<String> {
    OwnedObjectPath::try_from(value.try_clone().ok()?)
        .ok()
        .map(|path| path.to_string())
}

impl SpotifyMetadata {
    /// Validates and converts MPRIS metadata into shared track metadata.
    ///
    /// # Errors
    /// Returns an error when the title or usable artist list is empty.
    pub fn into_track_metadata(self) -> Result<TrackMetadata> {
        if self.title.trim().is_empty() {
            anyhow::bail!("Spotify metadata did not include a title");
        }
        if self.artists.is_empty() {
            anyhow::bail!("Spotify metadata did not include artists");
        }

        let track = TrackMetadata {
            title: self.title.trim().to_string(),
            artists: self
                .artists
                .into_iter()
                .map(|artist| artist.trim().to_string())
                .filter(|artist| !artist.is_empty())
                .collect::<Vec<_>>(),
            album: self
                .album
                .map(|album| album.trim().to_string())
                .filter(|album| !album.is_empty()),
            duration_ms: self.length_us.map(|value| value / 1_000),
            mpris_track_id: self.track_id,
        };

        if track.artists.is_empty() {
            anyhow::bail!("Spotify metadata did not include usable artists");
        }

        Ok(track)
    }
}

/// Normalized MPRIS playback status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackStatus {
    /// Playback is advancing.
    Playing,
    /// Playback is paused at the current position.
    Paused,
    /// Playback is stopped.
    Stopped,
}

impl TryFrom<&str> for PlaybackStatus {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "Playing" => Ok(Self::Playing),
            "Paused" => Ok(Self::Paused),
            "Stopped" => Ok(Self::Stopped),
            other => anyhow::bail!("unknown MPRIS playback status: {other}"),
        }
    }
}
