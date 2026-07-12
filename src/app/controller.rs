// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Coordinates playback events, lyrics retrieval, caching, and view updates.

use std::{cell::RefCell, rc::Rc, sync::mpsc, time::Instant};

use floatlyrics_core::{
    i18n::{Message, Text},
    track::TrackMetadata,
};
use floatlyrics_lyrics::{
    cache::{CachedLyrics, LyricsCache, ProviderResultInsert},
    lyrics::{FetchedLyrics, SearchPlan, search_best_lyrics, timed_lines_from_raw},
};

use crate::{
    config::AppConfig,
    mpris::{SpotifyPlayerState, SpotifyWatcherEvent},
};

use super::{
    model::{
        LyricsDisplayState, PlaybackSnapshot, apply_position_sample, effective_position_ms,
        lyrics_frame,
    },
    view::{LyricsView, OverlayView},
};

#[derive(Debug)]
struct LyricsFetchEvent {
    track_fingerprint: String,
    result: std::result::Result<FetchedLyrics, LyricsFetchFailure>,
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
    latest: &'a Rc<RefCell<Option<PlaybackSnapshot>>>,
    lyrics_state: &'a Rc<RefCell<LyricsDisplayState>>,
}

#[derive(Clone)]
pub(super) struct ControllerHandle {
    lyrics_state: Rc<RefCell<LyricsDisplayState>>,
    latest: Rc<RefCell<Option<PlaybackSnapshot>>>,
}

impl ControllerHandle {
    pub(super) fn reload_lyrics(&self) {
        self.lyrics_state.borrow_mut().track_fingerprint = None;
    }

    pub(super) fn current_track(&self) -> Option<TrackMetadata> {
        self.latest
            .borrow()
            .as_ref()
            .and_then(|snapshot| snapshot.state.track.clone())
    }
}

/// Decoupled controller: owns playback state and exposes a [`Controller::tick`] method
/// that the caller drives from the GTK main loop (or from tests).
pub(super) struct Controller {
    handle: ControllerHandle,
    receiver: Rc<RefCell<mpsc::Receiver<SpotifyWatcherEvent>>>,
    lyrics_receiver: Rc<RefCell<mpsc::Receiver<LyricsFetchEvent>>>,
    lyrics_state: Rc<RefCell<LyricsDisplayState>>,
    latest: Rc<RefCell<Option<PlaybackSnapshot>>>,
    floating: OverlayView,
    cache: Rc<dyn LyricsCache>,
    config: Rc<RefCell<AppConfig>>,
    runtime: tokio::runtime::Handle,
    lyrics_sender: mpsc::Sender<LyricsFetchEvent>,
}

impl Controller {
    pub(super) fn new(
        receiver: mpsc::Receiver<SpotifyWatcherEvent>,
        runtime: tokio::runtime::Handle,
        floating: OverlayView,
        cache: Rc<dyn LyricsCache>,
        config: Rc<RefCell<AppConfig>>,
    ) -> Self {
        let receiver = Rc::new(RefCell::new(receiver));
        let (lyrics_sender, lyrics_receiver) = mpsc::channel();
        let lyrics_receiver = Rc::new(RefCell::new(lyrics_receiver));
        let latest = Rc::new(RefCell::new(None::<PlaybackSnapshot>));
        let lyrics_state = Rc::new(RefCell::new(LyricsDisplayState::default()));
        let handle = ControllerHandle {
            lyrics_state: Rc::clone(&lyrics_state),
            latest: Rc::clone(&latest),
        };

        Self {
            handle,
            receiver,
            lyrics_receiver,
            lyrics_state,
            latest,
            floating,
            cache,
            config,
            runtime,
            lyrics_sender,
        }
    }

    /// Returns a lightweight handle used by settings and manual-search to
    /// query current track and trigger a lyrics reload.
    pub(super) fn handle(&self) -> ControllerHandle {
        self.handle.clone()
    }

    /// Process one frame: drain incoming events, check for new lyrics,
    /// and refresh the display. Call from the GTK tick callback.
    pub(super) fn tick(&self) {
        let config = self.config.borrow().clone();
        let ctx = SpotifyUiContext {
            floating: &self.floating,
            cache: &*self.cache,
            config: &config,
            runtime: &self.runtime,
            lyrics_sender: &self.lyrics_sender,
            latest: &self.latest,
            lyrics_state: &self.lyrics_state,
        };

        for event in self.receiver.borrow().try_iter() {
            handle_spotify_event(&event, &ctx);
        }

        for event in self.lyrics_receiver.borrow().try_iter() {
            handle_lyrics_fetch_event(
                event,
                ctx.floating,
                ctx.cache,
                ctx.config,
                ctx.latest,
                ctx.lyrics_state,
            );
        }

        if let Some(snapshot) = ctx.latest.borrow().as_ref() {
            if let Some(track) = snapshot.state.track.as_ref() {
                ensure_lyrics_loaded(
                    track,
                    ctx.cache,
                    ctx.config,
                    ctx.runtime,
                    ctx.lyrics_sender,
                    ctx.lyrics_state,
                );
            }
            refresh_progress_from_clock(snapshot, ctx.floating, ctx.config, ctx.lyrics_state);
        }
    }
}

fn handle_spotify_event(event: &SpotifyWatcherEvent, ctx: &SpotifyUiContext<'_>) {
    match event {
        SpotifyWatcherEvent::Connected(state) | SpotifyWatcherEvent::Updated(state) => {
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
                apply_position_sample(
                    snapshot,
                    track_identity.as_deref(),
                    *position_ms,
                    *sampled_at,
                );
            }
        }
        SpotifyWatcherEvent::Disconnected => {
            *ctx.latest.borrow_mut() = None;
            *ctx.lyrics_state.borrow_mut() = LyricsDisplayState::default();
            ctx.floating.set_song_info("FloatLyrics");
            ctx.floating.show_status(Text::OpenSpotify);
            ctx.floating.reset_progress();
        }
        SpotifyWatcherEvent::Error(message) => {
            *ctx.latest.borrow_mut() = None;
            *ctx.lyrics_state.borrow_mut() = LyricsDisplayState::default();
            tracing::warn!(%message, "Spotify listener error");
            ctx.floating.set_song_info("FloatLyrics");
            ctx.floating.show_status(Text::SpotifyAttention);
            ctx.floating.reset_progress();
        }
    }
}

fn update_spotify_state(state: &SpotifyPlayerState, ctx: &SpotifyUiContext<'_>) {
    if let Some(track) = &state.track {
        if let Err(error) = ctx.cache.upsert_track(track) {
            tracing::warn!(%error, "failed to cache Spotify track");
        }
        ensure_lyrics_loaded(
            track,
            ctx.cache,
            ctx.config,
            ctx.runtime,
            ctx.lyrics_sender,
            ctx.lyrics_state,
        );
        update_track_display(
            state,
            ctx.floating,
            ctx.config,
            ctx.lyrics_state,
            state.position_ms,
        );
    } else {
        ctx.floating.set_song_info("FloatLyrics");
        ctx.floating.show_status(Text::WaitingForMetadata);
        ctx.floating.reset_progress();
    }
}

fn refresh_progress_from_clock(
    snapshot: &PlaybackSnapshot,
    floating: &dyn LyricsView,
    config: &AppConfig,
    lyrics_state: &Rc<RefCell<LyricsDisplayState>>,
) {
    if snapshot.state.track.is_some() {
        update_track_display(
            &snapshot.state,
            floating,
            config,
            lyrics_state,
            effective_position_ms(snapshot),
        );
    }
}

fn update_track_display(
    state: &SpotifyPlayerState,
    floating: &dyn LyricsView,
    config: &AppConfig,
    lyrics_state: &Rc<RefCell<LyricsDisplayState>>,
    position_ms: Option<u64>,
) {
    let Some(track) = &state.track else {
        return;
    };

    floating.set_song_info(&format!("{} - {}", track.title, track.display_artist()));
    floating.set_progress(position_ms, track.duration_ms);
    let frame = lyrics_frame(
        &lyrics_state.borrow(),
        config,
        position_ms,
        config.general.language,
    );
    floating.show_lyrics(frame.content, &frame.key);
}

fn ensure_lyrics_loaded(
    track: &TrackMetadata,
    cache: &dyn LyricsCache,
    config: &AppConfig,
    runtime: &tokio::runtime::Handle,
    lyrics_sender: &mpsc::Sender<LyricsFetchEvent>,
    lyrics_state: &Rc<RefCell<LyricsDisplayState>>,
) {
    let fingerprint = track.fingerprint();
    if lyrics_state.borrow().track_fingerprint.as_deref() == Some(fingerprint.as_str()) {
        return;
    }

    *lyrics_state.borrow_mut() =
        load_lyrics_for_track(track, cache, config, runtime, lyrics_sender, fingerprint);
}

fn load_lyrics_for_track(
    track: &TrackMetadata,
    cache: &dyn LyricsCache,
    config: &AppConfig,
    runtime: &tokio::runtime::Handle,
    lyrics_sender: &mpsc::Sender<LyricsFetchEvent>,
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

    let state = lyrics_state_from_cached(fingerprint.clone(), cached);
    if config.lyrics.show_translation && !has_cached_translation(&state) {
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

fn lyrics_state_from_cached(fingerprint: String, cached: CachedLyrics) -> LyricsDisplayState {
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

fn handle_lyrics_fetch_event(
    event: LyricsFetchEvent,
    floating: &dyn LyricsView,
    cache: &dyn LyricsCache,
    config: &AppConfig,
    latest: &Rc<RefCell<Option<PlaybackSnapshot>>>,
    lyrics_state: &Rc<RefCell<LyricsDisplayState>>,
) {
    let Some(snapshot) = latest.borrow().as_ref().cloned() else {
        return;
    };
    let Some(track) = snapshot.state.track.as_ref() else {
        return;
    };
    if track.fingerprint() != event.track_fingerprint {
        return;
    }

    match event.result {
        Ok(fetched) => {
            if let Err(error) = cache.insert_provider_result(ProviderResultInsert {
                track_fingerprint: &event.track_fingerprint,
                provider: fetched.provider,
                provider_track_id: fetched.provider_track_id.as_deref(),
                title: &fetched.title,
                artists: &fetched.artists,
                score: fetched.score,
                raw_lyrics: Some(&fetched.raw_lyrics),
            }) {
                *lyrics_state.borrow_mut() = LyricsDisplayState {
                    track_fingerprint: Some(event.track_fingerprint),
                    status_message: Some(Message::Detail(
                        Text::LyricsCacheWriteError,
                        error.to_string(),
                    )),
                    ..LyricsDisplayState::default()
                };
            } else {
                *lyrics_state.borrow_mut() =
                    load_cached_lyrics_after_fetch(cache, config, event.track_fingerprint);
            }
        }
        Err(failure) => {
            let message = match failure {
                LyricsFetchFailure::NotFound => Message::Text(Text::NoLyricsFound),
                LyricsFetchFailure::Other(detail) => {
                    Message::Detail(Text::LyricsSearchFailed, detail)
                }
            };
            *lyrics_state.borrow_mut() = LyricsDisplayState {
                track_fingerprint: Some(event.track_fingerprint),
                status_message: Some(message),
                ..LyricsDisplayState::default()
            };
        }
    }

    update_track_display(
        &snapshot.state,
        floating,
        config,
        lyrics_state,
        effective_position_ms(&snapshot),
    );
}

fn load_cached_lyrics_after_fetch(
    cache: &dyn LyricsCache,
    config: &AppConfig,
    fingerprint: String,
) -> LyricsDisplayState {
    let provider_order = active_provider_order(config);
    match cache.lyrics_for_track(&fingerprint, &provider_order) {
        Ok(Some(cached)) => lyrics_state_from_cached(fingerprint, cached),
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

fn active_provider_order(config: &AppConfig) -> Vec<floatlyrics_lyrics::lyrics::LyricsProvider> {
    SearchPlan::new(config.lyrics.provider_order.clone())
        .providers()
        .to_vec()
}
