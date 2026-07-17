// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Backend Spotify MPRIS facade.
//!
//! D-Bus watching, metadata models, and position synchronization are isolated
//! behind focused modules.

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
#[path = "../test/mpris_test.rs"]
mod tests;
