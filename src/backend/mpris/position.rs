// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Backend playback position synchronization helpers.

use floatlyrics_core::track::TrackMetadata;

use super::model::SpotifyPlayerState;

pub(super) fn position_us_to_ms(position_us: i64) -> Option<u64> {
    if position_us >= 0 {
        Some(position_us as u64 / 1_000)
    } else {
        None
    }
}

pub(super) fn player_track_identity(state: &SpotifyPlayerState) -> Option<String> {
    state.track.as_ref().map(TrackMetadata::playback_identity)
}
