// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Coordinates backend playback events, lyrics retrieval, and caching.

mod loading;
mod presentation;

use std::{cell::RefCell, rc::Rc, sync::mpsc, time::Instant};

use floatlyrics_core::{i18n::Text, track::TrackMetadata};
use floatlyrics_lyrics::cache::{TRACK_OFFSET_MS_MAX, TRACK_OFFSET_MS_MIN};

use crate::shared::runtime::LyricsRuntimeConfig;

use super::{
    cache::CacheService,
    model::{
        LyricsDisplayState, PlaybackSnapshot, apply_position_sample, effective_position_ms,
        lyrics_document, playback_jump_detected,
    },
    mpris::{PlayerLyricsHintEvent, PlayerState, PlayerWatcherEvent},
};
use loading::{
    LyricsCacheApplyContext, LyricsCacheEvent, LyricsFetchApplyContext, LyricsFetchEvent,
    LyricsLoadContext, RomanizationEvent, apply_lyrics_cache_event, apply_lyrics_fetch_event,
    apply_romanization_event, load_lyrics_for_track,
};
use presentation::{refresh_lyrics_display, update_track_display};

pub(crate) use presentation::LyricsView;

#[derive(Default)]
struct ControllerState {
    latest: Option<PlaybackSnapshot>,
    lyrics: LyricsDisplayState,
    lyrics_generation: u64,
    document_dirty: bool,
    document_revision: u64,
    seek_pending: bool,
    track_offset_ms: i64,
    track_offset_fingerprint: Option<String>,
    track_offset_generation: u64,
    lyrics_hint: Option<floatlyrics_lyrics::lyrics::LyricsLookupHint>,
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
    floating: &'a dyn LyricsView,
    cache: &'a CacheService,
    config: &'a LyricsRuntimeConfig,
    runtime: &'a tokio::runtime::Handle,
    lyrics_sender: &'a mpsc::Sender<LyricsFetchEvent>,
    cache_sender: &'a mpsc::Sender<LyricsCacheEvent>,
    romanization_sender: &'a mpsc::Sender<RomanizationEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControllerCommand {
    ReloadLyrics,
    AdjustTrackOffset(i64),
    ResetTrackOffset,
}

#[derive(Clone, Default)]
struct PlaybackProjection {
    current_track: Rc<RefCell<Option<TrackMetadata>>>,
}

impl PlaybackProjection {
    fn set_current_track(&self, track: Option<TrackMetadata>) {
        self.current_track.replace(track);
    }

    fn current_track(&self) -> Option<TrackMetadata> {
        self.current_track.borrow().clone()
    }
}

#[derive(Clone)]
pub(crate) struct ControllerHandle {
    commands: mpsc::Sender<ControllerCommand>,
    playback: PlaybackProjection,
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

/// Decoupled controller: owns playback state and exposes a [`Controller::tick`] method
/// that the caller drives from the GTK main loop (or from tests).
pub(crate) struct Controller {
    receiver: mpsc::Receiver<PlayerWatcherEvent>,
    hint_receiver: mpsc::Receiver<PlayerLyricsHintEvent>,
    cache_receiver: mpsc::Receiver<LyricsCacheEvent>,
    lyrics_receiver: mpsc::Receiver<LyricsFetchEvent>,
    romanization_receiver: mpsc::Receiver<RomanizationEvent>,
    command_receiver: mpsc::Receiver<ControllerCommand>,
    command_sender: mpsc::Sender<ControllerCommand>,
    state: ControllerState,
    playback: PlaybackProjection,
    floating: Rc<dyn LyricsView>,
    cache: CacheService,
    config: LyricsRuntimeConfig,
    runtime: tokio::runtime::Handle,
    lyrics_sender: mpsc::Sender<LyricsFetchEvent>,
    cache_sender: mpsc::Sender<LyricsCacheEvent>,
    romanization_sender: mpsc::Sender<RomanizationEvent>,
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
            romanization_receiver,
            command_receiver,
            command_sender,
            state,
            playback: playback.clone(),
            floating,
            cache,
            config,
            runtime,
            lyrics_sender,
            cache_sender,
            romanization_sender,
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

    /// Replaces the runtime preferences used by subsequent controller ticks.
    pub(crate) fn update_config(&mut self, config: LyricsRuntimeConfig) {
        self.config = config;
    }

    pub(crate) fn reload_lyrics(&mut self) {
        self.state.reload_lyrics();
    }

    pub(crate) fn refresh_lyrics_presentation(&mut self) {
        self.state.refresh_lyrics_presentation();
    }

    /// Process one frame: drain incoming events, check for new lyrics,
    /// and refresh the display. Call from the GTK tick callback.
    pub(crate) fn tick(&mut self) {
        for command in self.command_receiver.try_iter() {
            match command {
                ControllerCommand::ReloadLyrics => self.state.reload_lyrics(),
                ControllerCommand::AdjustTrackOffset(delta_ms) => {
                    let offset_ms = self.state.track_offset_ms.saturating_add(delta_ms);
                    set_track_offset(
                        &mut self.state,
                        &self.cache,
                        self.floating.as_ref(),
                        offset_ms,
                        false,
                    );
                }
                ControllerCommand::ResetTrackOffset => {
                    set_track_offset(
                        &mut self.state,
                        &self.cache,
                        self.floating.as_ref(),
                        0,
                        true,
                    );
                }
            }
        }

        let ctx = ControllerContext {
            floating: self.floating.as_ref(),
            cache: &self.cache,
            config: &self.config,
            runtime: &self.runtime,
            lyrics_sender: &self.lyrics_sender,
            cache_sender: &self.cache_sender,
            romanization_sender: &self.romanization_sender,
        };

        for event in self.receiver.try_iter() {
            handle_player_event(&event, &ctx, &mut self.state, &self.playback);
        }

        for event in self.hint_receiver.try_iter() {
            apply_player_lyrics_hint(event, &mut self.state);
        }

        for event in self.cache_receiver.try_iter() {
            let snapshot = self.state.latest.clone();
            let Some(snapshot) = snapshot else {
                continue;
            };
            let current_generation = self.state.lyrics_generation;
            let applied = apply_lyrics_cache_event(
                event,
                &mut LyricsCacheApplyContext {
                    current_generation,
                    snapshot: &snapshot,
                    state: &mut self.state.lyrics,
                    track_offset_ms: &mut self.state.track_offset_ms,
                    current_offset_generation: self.state.track_offset_generation,
                    config: ctx.config,
                    runtime: ctx.runtime,
                    lyrics_sender: ctx.lyrics_sender,
                    romanization_sender: ctx.romanization_sender,
                },
            );
            if applied {
                self.state.document_dirty = true;
            }
            ctx.floating.set_track_offset(self.state.track_offset_ms);
        }

        for event in self.lyrics_receiver.try_iter() {
            let snapshot = self.state.latest.clone();
            let Some(snapshot) = snapshot else {
                continue;
            };
            let current_generation = self.state.lyrics_generation;
            let applied = apply_lyrics_fetch_event(
                event,
                &mut LyricsFetchApplyContext {
                    current_generation,
                    snapshot: &snapshot,
                    state: &mut self.state.lyrics,
                    cache: ctx.cache,
                    config: ctx.config,
                    cache_sender: ctx.cache_sender,
                },
            );
            if applied {
                self.state.document_dirty = true;
            }
        }

        for event in self.romanization_receiver.try_iter() {
            if apply_romanization_event(
                event,
                &mut self.state.lyrics,
                ctx.config.chinese_romanization,
            ) {
                self.state.document_dirty = true;
            }
        }

        let snapshot = self.state.latest.clone();
        if let Some(snapshot) = snapshot {
            if let Some(track) = snapshot.state.track.as_ref() {
                let lyrics_hint = self.state.lyrics_hint.clone();
                ensure_lyrics_loaded(track, lyrics_hint.as_ref(), &ctx, &mut self.state);
            }
            sync_lyrics_document(&mut self.state, &snapshot, ctx.config, ctx.floating);
            refresh_lyrics_display(
                &snapshot,
                ctx.floating,
                ctx.config,
                &self.state.lyrics,
                self.state.seek_pending,
                self.state.track_offset_ms,
            );
        }
        self.state.seek_pending = false;
    }
}

fn sync_lyrics_document(
    state: &mut ControllerState,
    snapshot: &PlaybackSnapshot,
    config: &LyricsRuntimeConfig,
    floating: &dyn LyricsView,
) {
    if !state.document_dirty {
        return;
    }
    state.document_dirty = false;
    state.document_revision = state.document_revision.wrapping_add(1);
    let revision = state.document_revision;
    let duration_ms = snapshot
        .state
        .track
        .as_ref()
        .and_then(|track| track.duration_ms);
    let document = lyrics_document(&state.lyrics, config, revision, duration_ms);
    floating.set_lyrics_document(document);
}

fn handle_player_event(
    event: &PlayerWatcherEvent,
    ctx: &ControllerContext<'_>,
    controller_state: &mut ControllerState,
    playback: &PlaybackProjection,
) {
    match event {
        PlayerWatcherEvent::Connected(state) | PlayerWatcherEvent::Updated(state) => {
            let previous_fingerprint = controller_state
                .latest
                .as_ref()
                .and_then(|snapshot| snapshot.state.track.as_ref())
                .map(TrackMetadata::fingerprint);
            let next_fingerprint = state.track.as_ref().map(TrackMetadata::fingerprint);
            if previous_fingerprint != next_fingerprint {
                controller_state.lyrics_hint = None;
            }
            let jump_detected =
                playback_jump_detected(controller_state.latest.as_ref(), state.position_ms, state);
            if jump_detected {
                controller_state.seek_pending = true;
            }
            controller_state.latest = Some(PlaybackSnapshot {
                state: state.clone(),
                received_at: Instant::now(),
            });
            playback.set_current_track(state.track.clone());
            update_player_state(state, ctx, controller_state);
        }
        PlayerWatcherEvent::PositionUpdated {
            track_identity,
            position_ms,
            sampled_at,
        } => {
            if let Some(snapshot) = controller_state.latest.as_mut() {
                let predicted = effective_position_ms(snapshot);
                if apply_position_sample(
                    snapshot,
                    track_identity.as_deref(),
                    *position_ms,
                    *sampled_at,
                ) && predicted.is_some_and(|value| value.abs_diff(*position_ms) > 750)
                {
                    controller_state.seek_pending = true;
                }
            }
        }
        PlayerWatcherEvent::Disconnected => {
            controller_state.latest = None;
            controller_state.lyrics = LyricsDisplayState::default();
            controller_state.document_dirty = true;
            playback.set_current_track(None);
            controller_state.track_offset_ms = 0;
            controller_state.track_offset_fingerprint = None;
            controller_state.lyrics_hint = None;
            ctx.floating.set_track_offset(0);
            ctx.floating.set_song_info("FloatLyrics");
            ctx.floating.show_status(Text::OpenPlayer);
        }
        PlayerWatcherEvent::Error(message) => {
            controller_state.latest = None;
            controller_state.lyrics = LyricsDisplayState::default();
            controller_state.document_dirty = true;
            playback.set_current_track(None);
            controller_state.track_offset_ms = 0;
            controller_state.track_offset_fingerprint = None;
            controller_state.lyrics_hint = None;
            ctx.floating.set_track_offset(0);
            tracing::warn!(%message, "MPRIS listener error");
            ctx.floating.set_song_info("FloatLyrics");
            ctx.floating.show_status(Text::PlayerAttention);
        }
    }
}

fn apply_player_lyrics_hint(event: PlayerLyricsHintEvent, state: &mut ControllerState) {
    let Some(snapshot) = state.latest.as_ref() else {
        return;
    };
    if snapshot.state.bus_name != event.bus_name
        || snapshot
            .state
            .track
            .as_ref()
            .map(TrackMetadata::fingerprint)
            != event.track_fingerprint
        || state.lyrics_hint == event.hint
    {
        return;
    }

    state.lyrics_hint = event.hint;
    state.reload_lyrics();
}

fn update_player_state(
    state: &PlayerState,
    ctx: &ControllerContext<'_>,
    controller_state: &mut ControllerState,
) {
    let Some(track) = &state.track else {
        ctx.floating.set_song_info("FloatLyrics");
        ctx.floating.show_status(Text::WaitingForMetadata);
        return;
    };
    ctx.cache.record_track(track.clone());
    update_track_display(
        state,
        ctx.floating,
        ctx.config,
        &controller_state.lyrics,
        state.position_ms,
        controller_state.seek_pending,
        controller_state.track_offset_ms,
    );
}

fn ensure_lyrics_loaded(
    track: &TrackMetadata,
    hint: Option<&floatlyrics_lyrics::lyrics::LyricsLookupHint>,
    ctx: &ControllerContext<'_>,
    state: &mut ControllerState,
) {
    let fingerprint = track.fingerprint();
    if state.lyrics.track_fingerprint.as_deref() == Some(fingerprint.as_str()) {
        return;
    }

    state.lyrics_generation = state.lyrics_generation.wrapping_add(1);
    if state.track_offset_fingerprint.as_deref() != Some(fingerprint.as_str()) {
        state.track_offset_fingerprint = Some(fingerprint.clone());
        state.track_offset_generation = state.track_offset_generation.wrapping_add(1);
        state.track_offset_ms = 0;
        ctx.floating.set_track_offset(0);
    }
    let generation = state.lyrics_generation;
    let load_context = LyricsLoadContext {
        cache: ctx.cache,
        config: ctx.config,
        cache_sender: ctx.cache_sender,
        generation,
        offset_generation: state.track_offset_generation,
        hint: hint.cloned(),
    };
    let lyrics = load_lyrics_for_track(track, fingerprint, &load_context);
    state.lyrics = lyrics;
    state.document_dirty = true;
}

fn set_track_offset(
    state: &mut ControllerState,
    cache: &CacheService,
    floating: &dyn LyricsView,
    offset_ms: i64,
    persist_unchanged: bool,
) {
    let Some(track) = state
        .latest
        .as_ref()
        .and_then(|snapshot| snapshot.state.track.clone())
    else {
        return;
    };
    let offset_ms = offset_ms.clamp(TRACK_OFFSET_MS_MIN, TRACK_OFFSET_MS_MAX);
    if state.track_offset_ms == offset_ms && !persist_unchanged {
        return;
    }

    state.track_offset_ms = offset_ms;
    state.track_offset_fingerprint = Some(track.fingerprint());
    state.track_offset_generation = state.track_offset_generation.wrapping_add(1);
    state.seek_pending = true;
    floating.set_track_offset(offset_ms);
    cache.set_track_offset(track, offset_ms, move |result| {
        if let Err(error) = result {
            tracing::warn!(%error, offset_ms, "failed to save per-track lyrics offset");
        }
    });
}

#[cfg(test)]
#[path = "../test/controller_state_test.rs"]
mod tests;
