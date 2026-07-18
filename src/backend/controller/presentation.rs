// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Backend output boundary and conversion from playback state to view messages.

use floatlyrics_core::i18n::Text;

use crate::shared::{
    presentation::{LyricsDocument, LyricsFrame},
    runtime::LyricsRuntimeConfig,
};

use crate::backend::{
    model::{LyricsDisplayState, PlaybackSnapshot, effective_position_ms, lyrics_frame},
    mpris::{PlaybackStatus, SpotifyPlayerState},
};

/// Output boundary implemented by the frontend overlay adapter.
pub(crate) trait LyricsView {
    fn set_song_info(&self, value: &str);
    fn set_lyrics_document(&self, document: LyricsDocument);
    fn show_lyrics(&self, frame: LyricsFrame);
    fn show_status(&self, key: Text);
}

pub(super) fn refresh_lyrics_display(
    snapshot: &PlaybackSnapshot,
    view: &dyn LyricsView,
    config: &LyricsRuntimeConfig,
    lyrics_state: &LyricsDisplayState,
    seeking: bool,
) {
    if snapshot.state.track.is_some() {
        update_track_display(
            &snapshot.state,
            view,
            config,
            lyrics_state,
            effective_position_ms(snapshot),
            seeking,
        );
    }
}

pub(super) fn update_track_display(
    state: &SpotifyPlayerState,
    view: &dyn LyricsView,
    config: &LyricsRuntimeConfig,
    lyrics_state: &LyricsDisplayState,
    position_ms: Option<u64>,
    seeking: bool,
) {
    let Some(track) = &state.track else {
        return;
    };

    view.set_song_info(&format!("{} - {}", track.title, track.display_artist()));
    view.show_lyrics(lyrics_frame(
        lyrics_state,
        config,
        position_ms,
        state.playback_status == PlaybackStatus::Playing,
        seeking,
        config.language,
    ));
}
