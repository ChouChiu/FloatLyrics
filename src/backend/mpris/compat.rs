// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Player-specific metadata hints layered on top of standard MPRIS.

use floatlyrics_lyrics::lyrics::{LyricsLookupHint, LyricsProvider};

use super::{MPRIS_BUS_PREFIX, model::MprisMetadata};

pub(super) fn lyrics_lookup_hint(
    identity: &str,
    bus_name: &str,
    metadata: &MprisMetadata,
    source_url: Option<&str>,
) -> Option<LyricsLookupHint> {
    let identity = identity.trim().to_ascii_lowercase();

    if matches!(identity.as_str(), "electronncm" | "qcm")
        || bus_matches(bus_name, "musicfox")
        || bus_matches(bus_name, "neteasecloudmusicgtk4")
    {
        return metadata
            .track_id
            .as_deref()
            .and_then(last_path_component)
            .and_then(netease_hint);
    }

    if identity == "feeluown" {
        let source_url = source_url?;
        if let Some(song_id) = source_url.strip_prefix("fuo://netease/songs/") {
            return netease_hint(song_id);
        }
        if let Some(song_id) = source_url.strip_prefix("fuo://qqmusic/songs/") {
            return qq_music_hint(song_id);
        }
    }

    if identity == "yesplaymusic" {
        return source_url
            .and_then(|url| url.strip_prefix("/trackid/"))
            .and_then(netease_hint);
    }

    None
}

fn bus_matches(bus_name: &str, expected_suffix: &str) -> bool {
    let suffix = bus_name
        .strip_prefix(MPRIS_BUS_PREFIX)
        .unwrap_or(bus_name)
        .to_ascii_lowercase();
    suffix == expected_suffix
        || suffix
            .strip_prefix(expected_suffix)
            .is_some_and(|remainder| remainder.starts_with('.'))
}

fn last_path_component(value: &str) -> Option<&str> {
    value.trim_end_matches('/').rsplit('/').next()
}

fn netease_hint(song_id: &str) -> Option<LyricsLookupHint> {
    let song_id = song_id.trim();
    (!song_id.is_empty() && song_id.bytes().all(|byte| byte.is_ascii_digit())).then(|| {
        LyricsLookupHint {
            provider: LyricsProvider::NetEase,
            provider_track_id: song_id.to_string(),
        }
    })
}

fn qq_music_hint(song_id: &str) -> Option<LyricsLookupHint> {
    let song_id = song_id.trim();
    (!song_id.is_empty() && song_id.bytes().all(|byte| byte.is_ascii_alphanumeric())).then(|| {
        LyricsLookupHint {
            provider: LyricsProvider::QqMusic,
            provider_track_id: song_id.to_string(),
        }
    })
}
