// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Backend output boundary and conversion from playback state to view messages.

use std::sync::Arc;

use floatlyrics_core::i18n::Text;
use floatlyrics_core::track::TrackMetadata;

use crate::shared::{
    presentation::{LyricsDocument, LyricsFrame, PlayerControl},
    runtime::LyricsRuntimeConfig,
};

use crate::backend::{
    model::{LyricsDisplayState, PlaybackSnapshot, effective_position_ms, lyrics_frame},
    mpris::{PlaybackStatus, PlayerState},
};

/// Output boundary implemented by the frontend overlay adapter and by the
/// AMLL WebSocket sender.
pub(crate) trait LyricsView {
    fn set_song_info(&self, value: &str);
    /// Publishes structured track metadata, or `None` when playback stopped.
    ///
    /// Implementations that only render text may ignore this notification.
    fn set_track_metadata(&self, track: Option<&TrackMetadata>);
    fn set_track_offset(&self, offset_ms: i64);
    /// Publishes volume, loop and shuffle mode, and the capabilities of the
    /// active player, or `None` while no player is connected.
    ///
    /// Only the AMLL sender consumes this: the overlay and the tray read the
    /// same state from the controller's playback projection.
    fn set_player_control(&self, control: Option<&PlayerControl>);
    fn set_lyrics_document(&self, document: LyricsDocument);
    /// Publishes the playback clock of the current track.
    ///
    /// Progress is published for every frame of a playing track, whether or not
    /// its lyrics are known, because a listener draws its own progress bar from
    /// it. `position_ms` is the player's own position: the lyric offset only
    /// calibrates the lyrics a view renders.
    fn set_playback(&self, position_ms: Option<u64>, playing: bool);
    /// Publishes the pinned line. The frame is shared rather than moved so the
    /// adapter can forward it to the GTK thread without copying its content.
    fn show_lyrics(&self, frame: Arc<LyricsFrame>);
    fn show_status(&self, key: Text);
}

pub(super) fn refresh_lyrics_display(
    snapshot: &PlaybackSnapshot,
    view: &dyn LyricsView,
    config: &LyricsRuntimeConfig,
    lyrics_state: &LyricsDisplayState,
    seeking: bool,
    track_offset_ms: i64,
) {
    if snapshot.state.track.is_none() {
        return;
    }
    let position_ms = effective_position_ms(snapshot);
    let playing = snapshot.state.playback_status == PlaybackStatus::Playing;
    // The playback clock is published before the lyrics, and independently of
    // them: a listener needs it even while no lyric line is available.
    view.set_playback(position_ms, playing);
    update_track_display(
        view,
        config,
        lyrics_state,
        position_ms,
        playing,
        seeking,
        track_offset_ms,
    );
}

pub(super) fn update_track_display(
    view: &dyn LyricsView,
    config: &LyricsRuntimeConfig,
    lyrics_state: &LyricsDisplayState,
    position_ms: Option<u64>,
    playing: bool,
    seeking: bool,
    track_offset_ms: i64,
) {
    view.show_lyrics(Arc::new(lyrics_frame(
        lyrics_state,
        config,
        position_ms,
        playing,
        seeking,
        config.language,
        track_offset_ms,
    )));
}

/// Publishes the song info of `track`, which only changes with the track.
pub(super) fn publish_song_info(state: &PlayerState, view: &dyn LyricsView) {
    let Some(track) = &state.track else {
        return;
    };
    let song_info = if track.artists.is_empty() {
        track.title.clone()
    } else {
        format!("{} - {}", track.title, track.display_artist())
    };
    view.set_song_info(&song_info);
}
