// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Message adapter from the backend lyrics output boundary to the frontend.

use std::sync::Arc;

use floatlyrics_core::i18n::Text;

use crate::{
    backend::LyricsView,
    shared::presentation::{LyricsDocument, LyricsFrame},
};

use super::super::AppMsg;

/// Message-only handle to the overlay component state.
///
/// Keeping GTK widgets out of the playback controller makes `AppModel::update`
/// the single place where the concrete view is mutated.
#[derive(Clone)]
pub(in crate::frontend) struct OverlaySender {
    sender: relm4::Sender<AppMsg>,
}

impl OverlaySender {
    pub(in crate::frontend) fn new(sender: relm4::Sender<AppMsg>) -> Self {
        Self { sender }
    }
}

impl LyricsView for OverlaySender {
    fn set_song_info(&self, value: &str) {
        let _ = self.sender.send(AppMsg::SetSongInfo(value.to_string()));
    }

    fn set_track_metadata(&self, _track: Option<&floatlyrics_core::track::TrackMetadata>) {}

    fn set_track_offset(&self, offset_ms: i64) {
        let _ = self.sender.send(AppMsg::SetTrackOffset(offset_ms));
    }

    /// The overlay reads the same state from the controller's playback
    /// projection on every tick, so no message is needed here.
    fn set_player_control(&self, _control: Option<&crate::shared::presentation::PlayerControl>) {}

    fn set_lyrics_document(&self, document: LyricsDocument) {
        let _ = self.sender.send(AppMsg::SetLyricsDocument(document));
    }

    /// The overlay renders lyrics from the frames it receives, so it needs no
    /// separate playback clock.
    fn set_playback(&self, _position_ms: Option<u64>, _playing: bool) {}

    fn show_lyrics(&self, frame: Arc<LyricsFrame>) {
        let _ = self.sender.send(AppMsg::ShowLyrics(frame));
    }

    fn show_status(&self, key: Text) {
        let _ = self.sender.send(AppMsg::ShowStatus(key));
    }
}
