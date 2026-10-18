// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! The narrow, cloneable view of the controller handed to the user interface.

use std::{cell::RefCell, rc::Rc, sync::mpsc};

use floatlyrics_core::track::TrackMetadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ControllerCommand {
    ReloadLyrics,
    AdjustTrackOffset(i64),
    ResetTrackOffset,
}

#[derive(Clone, Default)]
pub(super) struct PlaybackProjection {
    current_track: Rc<RefCell<Option<TrackMetadata>>>,
}

impl PlaybackProjection {
    pub(super) fn set_current_track(&self, track: Option<TrackMetadata>) {
        self.current_track.replace(track);
    }

    fn current_track(&self) -> Option<TrackMetadata> {
        self.current_track.borrow().clone()
    }
}

#[derive(Clone)]
pub(crate) struct ControllerHandle {
    pub(super) commands: mpsc::Sender<ControllerCommand>,
    pub(super) playback: PlaybackProjection,
}

impl ControllerHandle {
    pub(crate) fn reload_lyrics(&self) {
        let _ = self.commands.send(ControllerCommand::ReloadLyrics);
    }

    pub(crate) fn current_track(&self) -> Option<TrackMetadata> {
        self.playback.current_track()
    }

    pub(crate) fn adjust_track_offset(&self, delta_ms: i64) {
        let _ = self
            .commands
            .send(ControllerCommand::AdjustTrackOffset(delta_ms));
    }

    pub(crate) fn reset_track_offset(&self) {
        let _ = self.commands.send(ControllerCommand::ResetTrackOffset);
    }
}
