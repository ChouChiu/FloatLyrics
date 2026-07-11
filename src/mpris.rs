// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Spotify MPRIS facade.
//!
//! The public API is stable while D-Bus watching, metadata models, and position
//! synchronization are isolated behind focused modules.

mod model;
mod position;
mod watcher;

pub use model::{
    PlaybackStatus, SpotifyMetadata, SpotifyPlayerState, SpotifyWatcherEvent,
    spotify_metadata_from_mpris,
};
pub use watcher::{
    SPOTIFY_MPRIS_PREFIX, is_spotify_mpris_name, spawn_spotify_watcher,
    spawn_spotify_watcher_with_prefix, spotify_mpris_names,
};

#[cfg(test)]
mod tests {
    use super::position::position_us_to_ms;
    use super::*;
    use std::collections::HashMap;
    use zvariant::{OwnedValue, Value};

    fn owned(value: impl Into<Value<'static>>) -> OwnedValue {
        OwnedValue::try_from(value.into()).unwrap()
    }

    #[test]
    fn filters_spotify_mpris_names_only() {
        assert!(is_spotify_mpris_name("org.mpris.MediaPlayer2.spotify"));
        assert!(is_spotify_mpris_name(
            "org.mpris.MediaPlayer2.spotify.instance123"
        ));
        assert!(!is_spotify_mpris_name("org.mpris.MediaPlayer2.vlc"));
        assert!(!is_spotify_mpris_name("org.example.spotify"));
    }

    #[test]
    fn converts_spotify_metadata_to_internal_track() {
        let track = SpotifyMetadata {
            title: " Track ".to_string(),
            artists: vec![" Alice ".to_string(), "Bob".to_string()],
            album: Some(" Album ".to_string()),
            length_us: Some(215_500_000),
            track_id: Some("/org/mpris/MediaPlayer2/Track/42".to_string()),
        }
        .into_track_metadata()
        .unwrap();

        assert_eq!(track.title, "Track");
        assert_eq!(track.artists, vec!["Alice", "Bob"]);
        assert_eq!(track.album.as_deref(), Some("Album"));
        assert_eq!(track.duration_ms, Some(215_500));
    }

    #[test]
    fn parses_mpris_metadata_map() {
        let mut metadata = HashMap::new();
        metadata.insert("xesam:title".to_string(), owned("Song"));
        metadata.insert(
            "xesam:artist".to_string(),
            owned(vec!["Alice".to_string(), "Bob".to_string()]),
        );
        metadata.insert("xesam:album".to_string(), owned("Album"));
        metadata.insert("mpris:length".to_string(), owned(215_000_000_i64));
        metadata.insert(
            "mpris:trackid".to_string(),
            owned(zvariant::ObjectPath::try_from("/org/mpris/MediaPlayer2/Track/1").unwrap()),
        );

        let parsed = spotify_metadata_from_mpris(&metadata).unwrap();

        assert_eq!(parsed.title, "Song");
        assert_eq!(parsed.artists, vec!["Alice", "Bob"]);
        assert_eq!(parsed.album.as_deref(), Some("Album"));
        assert_eq!(parsed.length_us, Some(215_000_000));
        assert_eq!(
            parsed.track_id.as_deref(),
            Some("/org/mpris/MediaPlayer2/Track/1")
        );
    }

    #[test]
    fn converts_mpris_position_to_milliseconds() {
        assert_eq!(position_us_to_ms(12_345_678), Some(12_345));
        assert_eq!(position_us_to_ms(-1), None);
    }
}
