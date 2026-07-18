// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Message adapter from the backend lyrics output boundary to the frontend.

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

    fn set_lyrics_document(&self, document: LyricsDocument) {
        let _ = self.sender.send(AppMsg::SetLyricsDocument(document));
    }

    fn show_lyrics(&self, frame: LyricsFrame) {
        let _ = self.sender.send(AppMsg::ShowLyrics(frame));
    }

    fn show_status(&self, key: Text) {
        let _ = self.sender.send(AppMsg::ShowStatus(key));
    }
}
