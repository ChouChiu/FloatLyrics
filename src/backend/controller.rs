// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Coordinates backend playback events, lyrics retrieval, and caching.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc,
    time::Instant,
};

use floatlyrics_core::{
    i18n::{Message, Text},
    track::TrackMetadata,
};
use floatlyrics_lyrics::{
    cache::{CachedLyrics, LyricsCache, ProviderResultInsert},
    lyrics::{
        ChineseRomanizationMode, FetchedLyrics, SearchPlan, TimedLine,
        generate_local_romanization_with_mode, search_best_lyrics, timed_lines_from_raw,
    },
};

use crate::shared::{
    config::AppConfig,
    presentation::{LyricsDocument, LyricsFrame},
};

use super::{
    model::{
        LyricsDisplayState, PlaybackSnapshot, apply_position_sample, effective_position_ms,
        lyrics_document, lyrics_frame,
    },
    mpris::{PlaybackStatus, SpotifyPlayerState, SpotifyWatcherEvent},
};

/// Output boundary implemented by the frontend overlay adapter.
pub(crate) trait LyricsView {
    fn set_song_info(&self, value: &str);
    fn set_lyrics_document(&self, document: LyricsDocument);
    fn show_lyrics(&self, frame: LyricsFrame);
    fn show_status(&self, key: Text);
}

#[derive(Debug)]
struct LyricsFetchEvent {
    track_fingerprint: String,
    result: std::result::Result<FetchedLyrics, LyricsFetchFailure>,
}

#[derive(Debug)]
struct RomanizationEvent {
    track_fingerprint: String,
    chinese_mode: ChineseRomanizationMode,
    lines: Vec<TimedLine>,
}

#[derive(Debug)]
enum LyricsFetchFailure {
    NotFound,
    Other(String),
}

struct SpotifyUiContext<'a> {
    floating: &'a dyn LyricsView,
    cache: &'a dyn LyricsCache,
    config: &'a AppConfig,
    runtime: &'a tokio::runtime::Handle,
    lyrics_sender: &'a mpsc::Sender<LyricsFetchEvent>,
    romanization_sender: &'a mpsc::Sender<RomanizationEvent>,
    latest: &'a Rc<RefCell<Option<PlaybackSnapshot>>>,
    lyrics_state: &'a Rc<RefCell<LyricsDisplayState>>,
    document_dirty: &'a Cell<bool>,
    seek_pending: &'a Cell<bool>,
}

#[derive(Clone)]
pub(crate) struct ControllerHandle {
    lyrics_state: Rc<RefCell<LyricsDisplayState>>,
    latest: Rc<RefCell<Option<PlaybackSnapshot>>>,
    document_dirty: Rc<Cell<bool>>,
}

impl ControllerHandle {
    pub(crate) fn reload_lyrics(&self) {
        self.lyrics_state.borrow_mut().track_fingerprint = None;
        self.document_dirty.set(true);
    }

    pub(crate) fn refresh_lyrics_presentation(&self) {
        self.document_dirty.set(true);
    }

    pub(crate) fn current_track(&self) -> Option<TrackMetadata> {
        self.latest
            .borrow()
            .as_ref()
            .and_then(|snapshot| snapshot.state.track.clone())
    }
}

/// Decoupled controller: owns playback state and exposes a [`Controller::tick`] method
/// that the caller drives from the GTK main loop (or from tests).
pub(crate) struct Controller {
    handle: ControllerHandle,
    receiver: Rc<RefCell<mpsc::Receiver<SpotifyWatcherEvent>>>,
    lyrics_receiver: Rc<RefCell<mpsc::Receiver<LyricsFetchEvent>>>,
    romanization_receiver: Rc<RefCell<mpsc::Receiver<RomanizationEvent>>>,
    lyrics_state: Rc<RefCell<LyricsDisplayState>>,
    latest: Rc<RefCell<Option<PlaybackSnapshot>>>,
    floating: Rc<dyn LyricsView>,
    cache: Rc<dyn LyricsCache>,
    config: Rc<RefCell<AppConfig>>,
    runtime: tokio::runtime::Handle,
    lyrics_sender: mpsc::Sender<LyricsFetchEvent>,
    romanization_sender: mpsc::Sender<RomanizationEvent>,
    document_dirty: Rc<Cell<bool>>,
    document_revision: Cell<u64>,
    seek_pending: Rc<Cell<bool>>,
}

impl Controller {
    pub(crate) fn new(
        receiver: mpsc::Receiver<SpotifyWatcherEvent>,
        runtime: tokio::runtime::Handle,
        floating: Rc<dyn LyricsView>,
        cache: Rc<dyn LyricsCache>,
        config: Rc<RefCell<AppConfig>>,
    ) -> Self {
        let receiver = Rc::new(RefCell::new(receiver));
        let (lyrics_sender, lyrics_receiver) = mpsc::channel();
        let lyrics_receiver = Rc::new(RefCell::new(lyrics_receiver));
        let (romanization_sender, romanization_receiver) = mpsc::channel();
        let romanization_receiver = Rc::new(RefCell::new(romanization_receiver));
        let latest = Rc::new(RefCell::new(None::<PlaybackSnapshot>));
        let lyrics_state = Rc::new(RefCell::new(LyricsDisplayState::default()));
        let document_dirty = Rc::new(Cell::new(true));
        let seek_pending = Rc::new(Cell::new(false));
        let handle = ControllerHandle {
            lyrics_state: Rc::clone(&lyrics_state),
            latest: Rc::clone(&latest),
            document_dirty: Rc::clone(&document_dirty),
        };

        Self {
            handle,
            receiver,
            lyrics_receiver,
            romanization_receiver,
            lyrics_state,
            latest,
            floating,
            cache,
            config,
            runtime,
            lyrics_sender,
            romanization_sender,
            document_dirty,
            document_revision: Cell::new(0),
            seek_pending,
        }
    }

    /// Returns a lightweight handle used by settings and manual-search to
    /// query current track and trigger a lyrics reload.
    pub(crate) fn handle(&self) -> ControllerHandle {
        self.handle.clone()
    }

    /// Process one frame: drain incoming events, check for new lyrics,
    /// and refresh the display. Call from the GTK tick callback.
    pub(crate) fn tick(&self) {
        let config = self.config.borrow().clone();
        let ctx = SpotifyUiContext {
            floating: self.floating.as_ref(),
            cache: &*self.cache,
            config: &config,
            runtime: &self.runtime,
            lyrics_sender: &self.lyrics_sender,
            romanization_sender: &self.romanization_sender,
            latest: &self.latest,
            lyrics_state: &self.lyrics_state,
            document_dirty: &self.document_dirty,
            seek_pending: &self.seek_pending,
        };

        for event in self.receiver.borrow().try_iter() {
            handle_spotify_event(&event, &ctx);
        }

        for event in self.lyrics_receiver.borrow().try_iter() {
            handle_lyrics_fetch_event(event, &ctx);
            self.document_dirty.set(true);
        }

        for event in self.romanization_receiver.borrow().try_iter() {
            apply_romanization_event(
                event,
                ctx.lyrics_state,
                ctx.config.lyrics.chinese_romanization,
            );
            self.document_dirty.set(true);
        }

        if let Some(snapshot) = ctx.latest.borrow().as_ref() {
            if let Some(track) = snapshot.state.track.as_ref() {
                ensure_lyrics_loaded(track, &ctx);
            }
            self.sync_lyrics_document(snapshot, &config);
            refresh_lyrics_display(
                snapshot,
                ctx.floating,
                ctx.config,
                ctx.lyrics_state,
                ctx.seek_pending.get(),
            );
        }
        self.seek_pending.set(false);
    }

    fn sync_lyrics_document(&self, snapshot: &PlaybackSnapshot, config: &AppConfig) {
        if !self.document_dirty.replace(false) {
            return;
        }
        let revision = self.document_revision.get().wrapping_add(1);
        self.document_revision.set(revision);
        let duration_ms = snapshot
            .state
            .track
            .as_ref()
            .and_then(|track| track.duration_ms);
        self.floating.set_lyrics_document(lyrics_document(
            &self.lyrics_state.borrow(),
            config,
            revision,
            duration_ms,
        ));
    }
}

fn handle_spotify_event(event: &SpotifyWatcherEvent, ctx: &SpotifyUiContext<'_>) {
    match event {
        SpotifyWatcherEvent::Connected(state) | SpotifyWatcherEvent::Updated(state) => {
            if playback_jump_detected(ctx.latest.borrow().as_ref(), state.position_ms, state) {
                ctx.seek_pending.set(true);
            }
            *ctx.latest.borrow_mut() = Some(PlaybackSnapshot {
                state: state.clone(),
                received_at: Instant::now(),
            });
            update_spotify_state(state, ctx);
        }
        SpotifyWatcherEvent::PositionUpdated {
            track_identity,
            position_ms,
            sampled_at,
        } => {
            if let Some(snapshot) = ctx.latest.borrow_mut().as_mut() {
                let predicted = effective_position_ms(snapshot);
                if apply_position_sample(
                    snapshot,
                    track_identity.as_deref(),
                    *position_ms,
                    *sampled_at,
                ) && predicted.is_some_and(|value| value.abs_diff(*position_ms) > 750)
                {
                    ctx.seek_pending.set(true);
                }
            }
        }
        SpotifyWatcherEvent::Disconnected => {
            *ctx.latest.borrow_mut() = None;
            *ctx.lyrics_state.borrow_mut() = LyricsDisplayState::default();
            ctx.document_dirty.set(true);
            ctx.floating.set_song_info("FloatLyrics");
            ctx.floating.show_status(Text::OpenSpotify);
        }
        SpotifyWatcherEvent::Error(message) => {
            *ctx.latest.borrow_mut() = None;
            *ctx.lyrics_state.borrow_mut() = LyricsDisplayState::default();
            ctx.document_dirty.set(true);
            tracing::warn!(%message, "Spotify listener error");
            ctx.floating.set_song_info("FloatLyrics");
            ctx.floating.show_status(Text::SpotifyAttention);
        }
    }
}

fn playback_jump_detected(
    previous: Option<&PlaybackSnapshot>,
    next_position_ms: Option<u64>,
    next: &SpotifyPlayerState,
) -> bool {
    let Some(previous) = previous else {
        return true;
    };
    let previous_identity = previous
        .state
        .track
        .as_ref()
        .map(TrackMetadata::playback_identity);
    let next_identity = next.track.as_ref().map(TrackMetadata::playback_identity);
    if previous_identity != next_identity {
        return true;
    }
    effective_position_ms(previous)
        .zip(next_position_ms)
        .is_some_and(|(old, new)| old.abs_diff(new) > 750)
}

fn update_spotify_state(state: &SpotifyPlayerState, ctx: &SpotifyUiContext<'_>) {
    if let Some(track) = &state.track {
        if let Err(error) = ctx.cache.upsert_track(track) {
            tracing::warn!(%error, "failed to cache Spotify track");
        }
        ensure_lyrics_loaded(track, ctx);
        update_track_display(
            state,
            ctx.floating,
            ctx.config,
            ctx.lyrics_state,
            state.position_ms,
            ctx.seek_pending.get(),
        );
    } else {
        ctx.floating.set_song_info("FloatLyrics");
        ctx.floating.show_status(Text::WaitingForMetadata);
    }
}

fn refresh_lyrics_display(
    snapshot: &PlaybackSnapshot,
    floating: &dyn LyricsView,
    config: &AppConfig,
    lyrics_state: &Rc<RefCell<LyricsDisplayState>>,
    seeking: bool,
) {
    if snapshot.state.track.is_some() {
        update_track_display(
            &snapshot.state,
            floating,
            config,
            lyrics_state,
            effective_position_ms(snapshot),
            seeking,
        );
    }
}

fn update_track_display(
    state: &SpotifyPlayerState,
    floating: &dyn LyricsView,
    config: &AppConfig,
    lyrics_state: &Rc<RefCell<LyricsDisplayState>>,
    position_ms: Option<u64>,
    seeking: bool,
) {
    let Some(track) = &state.track else {
        return;
    };

    floating.set_song_info(&format!("{} - {}", track.title, track.display_artist()));
    let frame = lyrics_frame(
        &lyrics_state.borrow(),
        config,
        position_ms,
        state.playback_status == PlaybackStatus::Playing,
        seeking,
        config.general.language,
    );
    floating.show_lyrics(frame);
}

fn ensure_lyrics_loaded(track: &TrackMetadata, ctx: &SpotifyUiContext<'_>) {
    let fingerprint = track.fingerprint();
    if ctx.lyrics_state.borrow().track_fingerprint.as_deref() == Some(fingerprint.as_str()) {
        return;
    }

    *ctx.lyrics_state.borrow_mut() = load_lyrics_for_track(
        track,
        ctx.cache,
        ctx.config,
        ctx.runtime,
        ctx.lyrics_sender,
        ctx.romanization_sender,
        fingerprint,
    );
    ctx.document_dirty.set(true);
}

fn load_lyrics_for_track(
    track: &TrackMetadata,
    cache: &dyn LyricsCache,
    config: &AppConfig,
    runtime: &tokio::runtime::Handle,
    lyrics_sender: &mpsc::Sender<LyricsFetchEvent>,
    romanization_sender: &mpsc::Sender<RomanizationEvent>,
    fingerprint: String,
) -> LyricsDisplayState {
    let provider_order = active_provider_order(config);
    let cached = match cache.lyrics_for_track(&fingerprint, &provider_order) {
        Ok(cached) => cached,
        Err(error) => {
            return LyricsDisplayState {
                track_fingerprint: Some(fingerprint),
                status_message: Some(Message::Detail(Text::LyricsCacheError, error.to_string())),
                ..LyricsDisplayState::default()
            };
        }
    };

    let Some(cached) = cached else {
        spawn_lyrics_fetch(
            runtime,
            lyrics_sender.clone(),
            track.clone(),
            provider_order,
            fingerprint.clone(),
        );
        return LyricsDisplayState {
            track_fingerprint: Some(fingerprint),
            status_message: Some(Message::Text(Text::SearchingLyrics)),
            ..LyricsDisplayState::default()
        };
    };

    let state = lyrics_state_from_cached(
        fingerprint.clone(),
        &cached,
        config,
        runtime,
        romanization_sender,
    );
    if should_refresh_translation(&cached, &state, config) {
        spawn_lyrics_fetch(
            runtime,
            lyrics_sender.clone(),
            track.clone(),
            provider_order,
            fingerprint,
        );
    }
    state
}

fn lyrics_state_from_cached(
    fingerprint: String,
    cached: &CachedLyrics,
    config: &AppConfig,
    runtime: &tokio::runtime::Handle,
    romanization_sender: &mpsc::Sender<RomanizationEvent>,
) -> LyricsDisplayState {
    let lines = match timed_lines_from_raw(&cached.raw_lyrics) {
        Ok(lines) => lines,
        Err(error) => {
            return LyricsDisplayState {
                track_fingerprint: Some(fingerprint),
                status_message: Some(Message::Detail(Text::LyricsParseError, error.to_string())),
                ..LyricsDisplayState::default()
            };
        }
    };

    if config.lyrics.show_romanization {
        spawn_local_romanization(
            runtime,
            romanization_sender.clone(),
            fingerprint.clone(),
            lines.clone(),
            config.lyrics.chinese_romanization,
        );
    }

    let status_message = if lines.is_empty() {
        Some(Message::Text(Text::CachedLyricsNotSynced))
    } else {
        None
    };

    LyricsDisplayState {
        track_fingerprint: Some(fingerprint),
        lines,
        status_message,
    }
}

fn handle_lyrics_fetch_event(event: LyricsFetchEvent, ctx: &SpotifyUiContext<'_>) {
    let Some(snapshot) = ctx.latest.borrow().as_ref().cloned() else {
        return;
    };
    let Some(track) = snapshot.state.track.as_ref() else {
        return;
    };
    if track.fingerprint() != event.track_fingerprint {
        return;
    }
    if ctx.lyrics_state.borrow().track_fingerprint.as_deref()
        != Some(event.track_fingerprint.as_str())
    {
        return;
    }

    match event.result {
        Ok(fetched) => {
            if let Err(error) = ctx.cache.insert_provider_result(ProviderResultInsert {
                track_fingerprint: &event.track_fingerprint,
                provider: fetched.provider,
                provider_track_id: fetched.provider_track_id.as_deref(),
                title: &fetched.title,
                artists: &fetched.artists,
                score: fetched.score,
                raw_lyrics: Some(&fetched.raw_lyrics),
            }) {
                tracing::warn!(%error, "failed to cache fetched lyrics");
                if !apply_lyrics_cache_write_failure(
                    &mut ctx.lyrics_state.borrow_mut(),
                    event.track_fingerprint,
                    error.to_string(),
                ) {
                    return;
                }
            } else {
                *ctx.lyrics_state.borrow_mut() = load_cached_lyrics_after_fetch(
                    ctx.cache,
                    ctx.config,
                    ctx.runtime,
                    ctx.romanization_sender,
                    event.track_fingerprint,
                );
            }
        }
        Err(failure) => {
            if !apply_lyrics_fetch_failure(
                &mut ctx.lyrics_state.borrow_mut(),
                event.track_fingerprint,
                failure,
            ) {
                return;
            }
        }
    }

    update_track_display(
        &snapshot.state,
        ctx.floating,
        ctx.config,
        ctx.lyrics_state,
        effective_position_ms(&snapshot),
        ctx.seek_pending.get(),
    );
}

fn apply_lyrics_fetch_failure(
    state: &mut LyricsDisplayState,
    track_fingerprint: String,
    failure: LyricsFetchFailure,
) -> bool {
    if state.status_message != Some(Message::Text(Text::SearchingLyrics)) {
        return false;
    }

    let message = match failure {
        LyricsFetchFailure::NotFound => Message::Text(Text::NoLyricsFound),
        LyricsFetchFailure::Other(detail) => Message::Detail(Text::LyricsSearchFailed, detail),
    };
    *state = LyricsDisplayState {
        track_fingerprint: Some(track_fingerprint),
        status_message: Some(message),
        ..LyricsDisplayState::default()
    };
    true
}

fn apply_lyrics_cache_write_failure(
    state: &mut LyricsDisplayState,
    track_fingerprint: String,
    detail: String,
) -> bool {
    if state.status_message != Some(Message::Text(Text::SearchingLyrics)) {
        return false;
    }

    *state = LyricsDisplayState {
        track_fingerprint: Some(track_fingerprint),
        status_message: Some(Message::Detail(Text::LyricsCacheWriteError, detail)),
        ..LyricsDisplayState::default()
    };
    true
}

fn load_cached_lyrics_after_fetch(
    cache: &dyn LyricsCache,
    config: &AppConfig,
    runtime: &tokio::runtime::Handle,
    romanization_sender: &mpsc::Sender<RomanizationEvent>,
    fingerprint: String,
) -> LyricsDisplayState {
    let provider_order = active_provider_order(config);
    match cache.lyrics_for_track(&fingerprint, &provider_order) {
        Ok(Some(cached)) => {
            lyrics_state_from_cached(fingerprint, &cached, config, runtime, romanization_sender)
        }
        Ok(None) => LyricsDisplayState {
            track_fingerprint: Some(fingerprint),
            status_message: Some(Message::Text(Text::DownloadedLyricsNotStored)),
            ..LyricsDisplayState::default()
        },
        Err(error) => LyricsDisplayState {
            track_fingerprint: Some(fingerprint),
            status_message: Some(Message::Detail(Text::LyricsCacheError, error.to_string())),
            ..LyricsDisplayState::default()
        },
    }
}

fn spawn_local_romanization(
    runtime: &tokio::runtime::Handle,
    sender: mpsc::Sender<RomanizationEvent>,
    track_fingerprint: String,
    mut lines: Vec<TimedLine>,
    chinese_mode: ChineseRomanizationMode,
) {
    runtime.spawn_blocking(move || {
        generate_local_romanization_with_mode(&mut lines, chinese_mode);
        let _ = sender.send(RomanizationEvent {
            track_fingerprint,
            chinese_mode,
            lines,
        });
    });
}

fn apply_romanization_event(
    event: RomanizationEvent,
    lyrics_state: &Rc<RefCell<LyricsDisplayState>>,
    current_chinese_mode: ChineseRomanizationMode,
) {
    let mut state = lyrics_state.borrow_mut();
    if event.chinese_mode == current_chinese_mode
        && state.track_fingerprint.as_deref() == Some(event.track_fingerprint.as_str())
        && same_lyrics_document(&state.lines, &event.lines)
    {
        state.lines = event.lines;
    }
}

fn same_lyrics_document(current: &[TimedLine], generated: &[TimedLine]) -> bool {
    current.len() == generated.len()
        && current.iter().zip(generated).all(|(current, generated)| {
            current.start_ms == generated.start_ms
                && current.end_ms == generated.end_ms
                && current.text == generated.text
                && current.syllables == generated.syllables
                && current.translation == generated.translation
                && current.background == generated.background
        })
}

fn spawn_lyrics_fetch(
    runtime: &tokio::runtime::Handle,
    sender: mpsc::Sender<LyricsFetchEvent>,
    track: TrackMetadata,
    provider_order: Vec<floatlyrics_lyrics::lyrics::LyricsProvider>,
    track_fingerprint: String,
) {
    runtime.spawn(async move {
        let result = match search_best_lyrics(&track, &provider_order).await {
            Ok(Some(fetched)) => Ok(fetched),
            Ok(None) => Err(LyricsFetchFailure::NotFound),
            Err(error) => Err(LyricsFetchFailure::Other(error.to_string())),
        };

        let _ = sender.send(LyricsFetchEvent {
            track_fingerprint,
            result,
        });
    });
}

fn has_cached_translation(state: &LyricsDisplayState) -> bool {
    state.lines.iter().any(|line| {
        line.translation
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    })
}

fn should_refresh_translation(
    cached: &CachedLyrics,
    state: &LyricsDisplayState,
    config: &AppConfig,
) -> bool {
    !cached.manually_selected && config.lyrics.show_translation && !has_cached_translation(state)
}

fn active_provider_order(config: &AppConfig) -> Vec<floatlyrics_lyrics::lyrics::LyricsProvider> {
    SearchPlan::new(config.lyrics.provider_order.clone())
        .providers()
        .to_vec()
}

#[cfg(test)]
#[path = "../test/controller_test.rs"]
mod tests;
