// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Coordinates backend playback events, lyrics retrieval, and caching.

mod handle;
mod loading;
mod player;
mod presentation;
mod track;

use std::{
    rc::Rc,
    sync::{Arc, mpsc},
};

use floatlyrics_core::i18n::Text;

use crate::shared::{presentation::PlayerControl, runtime::LyricsRuntimeConfig};

use super::{
    cache::CacheService,
    model::{LyricsDisplayState, PlaybackSnapshot},
    mpris::{PlayerLyricsHintEvent, PlayerWatcherEvent},
};
use handle::{ControllerCommand, PlaybackProjection};
use loading::{
    LyricsCacheApplyContext, LyricsCacheEvent, LyricsFetchApplyContext, LyricsFetchEvent,
    RomanizationEvent, apply_lyrics_cache_event, apply_lyrics_fetch_event,
    apply_romanization_event,
};
use player::{apply_player_lyrics_hint, credit_artists, handle_player_event};
use presentation::{publish_song_info, refresh_lyrics_display};
use track::{ensure_lyrics_loaded, set_track_offset, sync_lyrics_document};

pub(crate) use handle::ControllerHandle;

pub(crate) use presentation::LyricsView;

#[derive(Default)]
struct ControllerState {
    latest: Option<Arc<PlaybackSnapshot>>,
    /// Fingerprint of the current track, recomputed when the track changes so
    /// the tick does not hash the metadata again.
    current_fingerprint: Option<String>,
    lyrics: LyricsDisplayState,
    lyrics_generation: u64,
    document_dirty: bool,
    document_revision: u64,
    seek_pending: bool,
    track_offset_ms: i64,
    track_offset_fingerprint: Option<String>,
    track_offset_generation: u64,
    lyrics_hint: Option<floatlyrics_lyrics::lyrics::LyricsLookupHint>,
    /// Last control state published to the view, so it is not resent per tick.
    player_control: Option<PlayerControl>,
    /// Track fingerprint and provider billing already reconciled for the view.
    ///
    /// Keeps the credited restatement to one per pair rather than one per frame,
    /// and is forgotten whenever the player's own metadata is published again,
    /// so the restatement follows that too.
    credited_billing: Option<(String, Vec<String>)>,
    /// Status shown while no player is connected, restated to a new view.
    ///
    /// `None` until a player disconnects or fails, which reads as
    /// [`Text::OpenPlayer`].
    idle_status: Option<Text>,
}

impl ControllerState {
    fn reload_lyrics(&mut self) {
        self.lyrics.track_fingerprint = None;
        self.document_dirty = true;
    }

    fn refresh_lyrics_presentation(&mut self) {
        self.document_dirty = true;
    }
}

struct ControllerContext<'a> {
    runtime: &'a tokio::runtime::Handle,
    floating: &'a dyn LyricsView,
    cache: &'a CacheService,
    config: &'a LyricsRuntimeConfig,
    cache_sender: &'a mpsc::Sender<LyricsCacheEvent>,
}

/// Decoupled controller: owns playback state and exposes a [`Controller::tick`] method
/// that the caller drives from the GTK main loop (or from tests).
pub(crate) struct Controller {
    receiver: mpsc::Receiver<PlayerWatcherEvent>,
    hint_receiver: mpsc::Receiver<PlayerLyricsHintEvent>,
    cache_receiver: mpsc::Receiver<LyricsCacheEvent>,
    lyrics_receiver: mpsc::Receiver<LyricsFetchEvent>,
    pending_fetch: Option<tokio::task::JoinHandle<()>>,
    romanization_receiver: mpsc::Receiver<RomanizationEvent>,
    command_receiver: mpsc::Receiver<ControllerCommand>,
    command_sender: mpsc::Sender<ControllerCommand>,
    state: ControllerState,
    playback: PlaybackProjection,
    services: Services,
}

/// What the controller works through, kept apart from its state so both can be
/// borrowed at once.
struct Services {
    floating: Rc<dyn LyricsView>,
    cache: CacheService,
    config: LyricsRuntimeConfig,
    runtime: tokio::runtime::Handle,
    lyrics_sender: mpsc::Sender<LyricsFetchEvent>,
    cache_sender: mpsc::Sender<LyricsCacheEvent>,
    romanization_sender: mpsc::Sender<RomanizationEvent>,
}

impl Services {
    fn context(&self) -> ControllerContext<'_> {
        ControllerContext {
            runtime: &self.runtime,
            floating: self.floating.as_ref(),
            cache: &self.cache,
            config: &self.config,
            cache_sender: &self.cache_sender,
        }
    }
}

impl Controller {
    pub(super) fn new(
        receiver: mpsc::Receiver<PlayerWatcherEvent>,
        hint_receiver: mpsc::Receiver<PlayerLyricsHintEvent>,
        runtime: tokio::runtime::Handle,
        floating: Rc<dyn LyricsView>,
        cache: CacheService,
        config: LyricsRuntimeConfig,
    ) -> Self {
        let (lyrics_sender, lyrics_receiver) = mpsc::channel();
        let (cache_sender, cache_receiver) = mpsc::channel();
        let (romanization_sender, romanization_receiver) = mpsc::channel();
        let (command_sender, command_receiver) = mpsc::channel();
        let state = ControllerState {
            document_dirty: true,
            ..ControllerState::default()
        };
        let playback = PlaybackProjection::default();
        Self {
            receiver,
            hint_receiver,
            cache_receiver,
            lyrics_receiver,
            pending_fetch: None,
            romanization_receiver,
            command_receiver,
            command_sender,
            state,
            playback: playback.clone(),
            services: Services {
                floating,
                cache,
                config,
                runtime,
                lyrics_sender,
                cache_sender,
                romanization_sender,
            },
        }
    }

    /// Returns a lightweight handle used by settings and manual-search to
    /// query current track and trigger a lyrics reload.
    pub(crate) fn handle(&self) -> ControllerHandle {
        ControllerHandle {
            commands: self.command_sender.clone(),
            playback: self.playback.clone(),
        }
    }

    pub(crate) fn current_fingerprint(&self) -> Option<&str> {
        self.state.current_fingerprint.as_deref()
    }

    /// Replaces the runtime preferences used by subsequent controller ticks.
    pub(crate) fn update_config(&mut self, config: LyricsRuntimeConfig) {
        self.services.config = config;
    }

    pub(crate) fn reload_lyrics(&mut self) {
        self.state.reload_lyrics();
    }

    pub(crate) fn refresh_lyrics_presentation(&mut self) {
        self.state.refresh_lyrics_presentation();
    }

    /// Sends subsequent output to `view` and restates the current playback to it.
    ///
    /// A view replaced at runtime has seen none of the earlier notifications, so
    /// the track, its offset, and the player control state are published again
    /// here, and the lyrics document and credited billing on the next tick.
    pub(crate) fn set_view(&mut self, view: Rc<dyn LyricsView>) {
        self.services.floating = view;
        let view = self.services.floating.as_ref();
        let state = &mut self.state;
        state.document_dirty = true;
        state.credited_billing = None;
        view.set_track_offset(state.track_offset_ms);
        view.set_player_control(state.player_control.as_ref());
        let Some(snapshot) = state.latest.as_deref() else {
            view.set_track_metadata(None);
            view.set_song_info("FloatLyrics");
            view.show_status(state.idle_status.unwrap_or(Text::OpenPlayer));
            return;
        };
        match &snapshot.state.track {
            Some(track) => {
                view.set_track_metadata(Some(track));
                publish_song_info(&snapshot.state, view);
            }
            None => {
                view.set_track_metadata(None);
                view.set_song_info("FloatLyrics");
                view.show_status(Text::WaitingForMetadata);
            }
        }
    }

    /// Process one frame: drain incoming events, check for new lyrics,
    /// and refresh the display. Call from the GTK tick callback.
    pub(crate) fn tick(&mut self) {
        self.apply_commands();
        self.apply_player_events();
        if (self.state.current_fingerprint.is_none()
            || self.state.current_fingerprint != self.state.lyrics.track_fingerprint)
            && let Some(request) = self.pending_fetch.take()
        {
            request.abort();
        }
        self.apply_cache_results();
        self.apply_fetch_results();
        self.apply_romanization_results();
        self.present();
        self.state.seek_pending = false;
    }

    fn apply_commands(&mut self) {
        for command in self.command_receiver.try_iter() {
            let (offset_ms, persist_unchanged) = match command {
                ControllerCommand::ReloadLyrics => {
                    self.state.reload_lyrics();
                    continue;
                }
                ControllerCommand::AdjustTrackOffset(delta_ms) => {
                    (self.state.track_offset_ms.saturating_add(delta_ms), false)
                }
                ControllerCommand::ResetTrackOffset => (0, true),
            };
            set_track_offset(
                &mut self.state,
                &self.services.cache,
                self.services.floating.as_ref(),
                offset_ms,
                persist_unchanged,
            );
        }
    }

    fn apply_player_events(&mut self) {
        let ctx = self.services.context();
        for event in self.receiver.try_iter() {
            handle_player_event(&event, &ctx, &mut self.state, &self.playback);
        }
        for event in self.hint_receiver.try_iter() {
            apply_player_lyrics_hint(event, &mut self.state);
        }
    }

    fn apply_cache_results(&mut self) {
        for event in self.cache_receiver.try_iter() {
            let Some(snapshot) = self.state.latest.clone() else {
                continue;
            };
            let applied = apply_lyrics_cache_event(
                event,
                &mut LyricsCacheApplyContext {
                    current_generation: self.state.lyrics_generation,
                    snapshot: &snapshot,
                    state: &mut self.state.lyrics,
                    track_offset_ms: &mut self.state.track_offset_ms,
                    current_offset_generation: self.state.track_offset_generation,
                    config: &self.services.config,
                    runtime: &self.services.runtime,
                    lyrics_sender: &self.services.lyrics_sender,
                    pending_fetch: &mut self.pending_fetch,
                    romanization_sender: &self.services.romanization_sender,
                },
            );
            self.state.document_dirty |= applied;
            self.services
                .floating
                .set_track_offset(self.state.track_offset_ms);
        }
    }

    fn apply_fetch_results(&mut self) {
        for event in self.lyrics_receiver.try_iter() {
            let Some(snapshot) = self.state.latest.clone() else {
                continue;
            };
            let applied = apply_lyrics_fetch_event(
                event,
                &mut LyricsFetchApplyContext {
                    current_generation: self.state.lyrics_generation,
                    snapshot: &snapshot,
                    state: &mut self.state.lyrics,
                    runtime: &self.services.runtime,
                    cache: &self.services.cache,
                    config: &self.services.config,
                    cache_sender: &self.services.cache_sender,
                },
            );
            self.state.document_dirty |= applied;
        }
    }

    fn apply_romanization_results(&mut self) {
        for event in self.romanization_receiver.try_iter() {
            self.state.document_dirty |= apply_romanization_event(
                event,
                &mut self.state.lyrics,
                self.services.config.chinese_romanization,
            );
        }
    }

    /// Loads, reconciles, and draws the lyrics of the current track.
    fn present(&mut self) {
        let Some(snapshot) = self.state.latest.clone() else {
            return;
        };
        let ctx = self.services.context();
        let state = &mut self.state;
        ensure_lyrics_loaded(&ctx, state);
        credit_artists(state, ctx.floating);
        sync_lyrics_document(state, &snapshot, ctx.config, ctx.floating);
        refresh_lyrics_display(
            &snapshot,
            ctx.floating,
            ctx.config,
            &state.lyrics,
            state.seek_pending,
            state.track_offset_ms,
        );
    }
}

#[cfg(test)]
#[path = "../test/controller_state_test.rs"]
mod tests;
