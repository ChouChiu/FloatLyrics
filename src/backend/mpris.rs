// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Backend MPRIS facade.
//!
//! D-Bus watching, metadata models, and position synchronization are isolated
//! behind focused modules.

mod compat;
mod control;
mod model;
mod position;
mod watcher;

pub(crate) use control::{MediaCommand, MediaControlHandle};
pub use model::{
    MprisMetadata, PlaybackStatus, PlayerState, PlayerWatcherEvent, metadata_from_mpris,
};
pub use watcher::{MPRIS_BUS_PREFIX, PlayerSelection, mpris_player_names};
pub(crate) use watcher::{PlayerLyricsHintEvent, spawn_player_watcher};

#[cfg(test)]
#[path = "../test/mpris_test.rs"]
mod tests;
