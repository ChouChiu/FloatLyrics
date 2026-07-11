// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Instant};
use zvariant::{OwnedObjectPath, OwnedValue};

use floatlyrics_core::track::TrackMetadata;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotifyWatcherEvent {
    Connected(SpotifyPlayerState),
    Updated(SpotifyPlayerState),
    PositionUpdated {
        track_identity: Option<String>,
        position_ms: u64,
        sampled_at: Instant,
    },
    Disconnected,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotifyPlayerState {
    pub bus_name: String,
    pub playback_status: PlaybackStatus,
    pub position_ms: Option<u64>,
    pub track: Option<TrackMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpotifyMetadata {
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub length_us: Option<u64>,
    pub track_id: Option<String>,
}

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
    u64::try_from(value.try_clone().ok()?)
        .ok()
        .or_else(|| i64::try_from(value.try_clone().ok()?).ok()?.try_into().ok())
}

fn object_path_value(value: &OwnedValue) -> Option<String> {
    OwnedObjectPath::try_from(value.try_clone().ok()?)
        .ok()
        .map(|path| path.to_string())
}

impl SpotifyMetadata {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
    Unknown(String),
}

impl From<&str> for PlaybackStatus {
    fn from(value: &str) -> Self {
        match value {
            "Playing" => Self::Playing,
            "Paused" => Self::Paused,
            "Stopped" => Self::Stopped,
            other => Self::Unknown(other.to_string()),
        }
    }
}
