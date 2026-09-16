// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Backend state converted from MPRIS properties and signals.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Instant};
use zvariant::{OwnedObjectPath, OwnedValue, Value};

use floatlyrics_core::track::TrackMetadata;

/// State change emitted by the background MPRIS watcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerWatcherEvent {
    /// A matching player appeared with its initial state.
    Connected(PlayerState),
    /// Metadata or playback status changed.
    Updated(PlayerState),
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

/// Latest known state for one MPRIS player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState {
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
pub struct MprisMetadata {
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
    /// Album cover URL supplied by the player, when available.
    #[serde(default)]
    pub art_url: Option<String>,
}

/// Extracts the metadata fields used by FloatLyrics from MPRIS properties.
///
/// Returns `None` when required title metadata is absent or has an unexpected
/// D-Bus type.
pub fn metadata_from_mpris(metadata: &HashMap<String, OwnedValue>) -> Option<MprisMetadata> {
    Some(MprisMetadata {
        title: string_value(metadata.get("xesam:title")?)?,
        artists: metadata
            .get("xesam:artist")
            .and_then(string_vec_value)
            .unwrap_or_default(),
        album: metadata.get("xesam:album").and_then(string_value),
        length_us: metadata.get("mpris:length").and_then(u64_value),
        track_id: metadata.get("mpris:trackid").and_then(track_id_value),
        art_url: metadata.get("mpris:artUrl").and_then(string_value),
    })
}

pub(super) fn source_url_from_mpris(metadata: &HashMap<String, OwnedValue>) -> Option<String> {
    metadata.get("xesam:url").and_then(string_value)
}

fn string_value(value: &OwnedValue) -> Option<String> {
    String::try_from(value.try_clone().ok()?).ok()
}

fn string_vec_value(value: &OwnedValue) -> Option<Vec<String>> {
    Vec::<String>::try_from(value.try_clone().ok()?).ok()
}

/// Reads an unsigned integer whichever integer type the player sent.
///
/// The specification types `mpris:length` as an `int64`, but Spotify and other
/// players send a `uint64`, and zbus converts to the exact variant only — a
/// strict `i64` read silently drops the length, which is what sizes a listener's
/// progress bar against the playback clock.
fn u64_value(value: &OwnedValue) -> Option<u64> {
    match &**value {
        Value::I64(number) => u64::try_from(*number).ok(),
        Value::U64(number) => Some(*number),
        Value::I32(number) => u64::try_from(*number).ok(),
        Value::U32(number) => Some(u64::from(*number)),
        _ => None,
    }
}

fn object_path_value(value: &OwnedValue) -> Option<String> {
    OwnedObjectPath::try_from(value.try_clone().ok()?)
        .ok()
        .map(|path| path.to_string())
}

fn track_id_value(value: &OwnedValue) -> Option<String> {
    object_path_value(value).or_else(|| string_value(value))
}

impl MprisMetadata {
    /// Validates and converts MPRIS metadata into shared track metadata.
    ///
    /// # Errors
    /// Returns an error when the title is empty.
    pub fn into_track_metadata(self) -> Result<TrackMetadata> {
        if self.title.trim().is_empty() {
            anyhow::bail!("MPRIS metadata did not include a title");
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
            art_url: self
                .art_url
                .map(|url| url.trim().to_string())
                .filter(|url| !url.is_empty()),
        };

        Ok(track)
    }
}

/// Compatibility alias for the former Spotify-specific event name.
pub type SpotifyWatcherEvent = PlayerWatcherEvent;
/// Compatibility alias for the former Spotify-specific player-state name.
pub type SpotifyPlayerState = PlayerState;
/// Compatibility alias for the former Spotify-specific metadata name.
pub type SpotifyMetadata = MprisMetadata;

/// Compatibility wrapper for the former Spotify-specific conversion function.
pub fn spotify_metadata_from_mpris(
    metadata: &HashMap<String, OwnedValue>,
) -> Option<MprisMetadata> {
    metadata_from_mpris(metadata)
}

/// Normalized MPRIS playback status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
