//! Coordinates playback events, lyrics retrieval, caching, and view updates.

use std::{cell::RefCell, rc::Rc, sync::mpsc, time::Instant};

use gtk::prelude::*;

use crate::{
    cache::{Cache, CachedLyrics, ProviderResultInsert},
    config::AppConfig,
    lyrics::{FetchedLyrics, SearchPlan, search_best_lyrics, timed_lines_from_raw},
    mpris::{SpotifyPlayerState, SpotifyWatcherEvent},
    track::TrackMetadata,
};

use super::{
    model::{
        LyricsDisplayState, PlaybackSnapshot, apply_position_sample, effective_position_ms,
        lyrics_frame,
    },
    view::OverlayView,
};

#[derive(Debug)]
struct LyricsFetchEvent {
    track_fingerprint: String,
    result: std::result::Result<FetchedLyrics, String>,
}

struct SpotifyUiContext<'a> {
    floating: &'a OverlayView,
    cache: &'a Cache,
    config: &'a AppConfig,
    runtime: &'a tokio::runtime::Handle,
    lyrics_sender: &'a mpsc::Sender<LyricsFetchEvent>,
    latest: &'a Rc<RefCell<Option<PlaybackSnapshot>>>,
    lyrics_state: &'a Rc<RefCell<LyricsDisplayState>>,
}

pub(super) fn attach(
    receiver: mpsc::Receiver<SpotifyWatcherEvent>,
    runtime: tokio::runtime::Handle,
    floating: OverlayView,
    cache: Rc<Cache>,
    config: Rc<AppConfig>,
) {
    let receiver = Rc::new(RefCell::new(receiver));
    let (lyrics_sender, lyrics_receiver) = mpsc::channel();
    let lyrics_receiver = Rc::new(RefCell::new(lyrics_receiver));
    let latest = Rc::new(RefCell::new(None::<PlaybackSnapshot>));
    let lyrics_state = Rc::new(RefCell::new(LyricsDisplayState::default()));

    let tick_widget = floating.tick_widget();
    tick_widget.add_tick_callback(move |_, _| {
        let ctx = SpotifyUiContext {
            floating: &floating,
            cache: &cache,
            config: &config,
            runtime: &runtime,
            lyrics_sender: &lyrics_sender,
            latest: &latest,
            lyrics_state: &lyrics_state,
        };

        for event in receiver.borrow().try_iter() {
            handle_spotify_event(&event, &ctx);
        }

        for event in lyrics_receiver.borrow().try_iter() {
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
            refresh_progress_from_clock(snapshot, ctx.floating, ctx.config, ctx.lyrics_state);
        }

        gtk::glib::ControlFlow::Continue
    });
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
            ctx.floating.show_status("Open Spotify to start tracking");
            ctx.floating.reset_progress();
        }
        SpotifyWatcherEvent::Error(message) => {
            *ctx.latest.borrow_mut() = None;
            *ctx.lyrics_state.borrow_mut() = LyricsDisplayState::default();
            tracing::warn!(%message, "Spotify listener error");
            ctx.floating.set_song_info("FloatLyrics");
            ctx.floating.show_status("Spotify listener needs attention");
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
        ctx.floating.show_status("Waiting for Spotify metadata");
        ctx.floating.reset_progress();
    }
}

fn refresh_progress_from_clock(
    snapshot: &PlaybackSnapshot,
    floating: &OverlayView,
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
    floating: &OverlayView,
    config: &AppConfig,
    lyrics_state: &Rc<RefCell<LyricsDisplayState>>,
    position_ms: Option<u64>,
) {
    let Some(track) = &state.track else {
        return;
    };

    floating.set_song_info(&format!("{} - {}", track.title, track.display_artist()));
    floating.set_progress(position_ms, track.duration_ms);
    let frame = lyrics_frame(&lyrics_state.borrow(), config, position_ms);
    floating.show_lyrics(frame.content, &frame.key);
}

fn ensure_lyrics_loaded(
    track: &TrackMetadata,
    cache: &Cache,
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
    cache: &Cache,
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
                status_message: Some(format!("Lyrics cache error: {error}")),
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
            status_message: Some("Searching lyrics...".to_string()),
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
                status_message: Some(format!("Lyrics parse error: {error}")),
                ..LyricsDisplayState::default()
            };
        }
    };

    let status_message = if lines.is_empty() {
        Some("Cached lyrics are not time-synced".to_string())
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
    floating: &OverlayView,
    cache: &Cache,
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
                    status_message: Some(format!("Lyrics cache write error: {error}")),
                    ..LyricsDisplayState::default()
                };
            } else {
                *lyrics_state.borrow_mut() =
                    load_cached_lyrics_after_fetch(cache, config, event.track_fingerprint);
            }
        }
        Err(message) => {
            *lyrics_state.borrow_mut() = LyricsDisplayState {
                track_fingerprint: Some(event.track_fingerprint),
                status_message: Some(format!("Lyrics search failed: {message}")),
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
    cache: &Cache,
    config: &AppConfig,
    fingerprint: String,
) -> LyricsDisplayState {
    let provider_order = active_provider_order(config);
    match cache.lyrics_for_track(&fingerprint, &provider_order) {
        Ok(Some(cached)) => lyrics_state_from_cached(fingerprint, cached),
        Ok(None) => LyricsDisplayState {
            track_fingerprint: Some(fingerprint),
            status_message: Some("Downloaded lyrics were not stored".to_string()),
            ..LyricsDisplayState::default()
        },
        Err(error) => LyricsDisplayState {
            track_fingerprint: Some(fingerprint),
            status_message: Some(format!("Lyrics cache error: {error}")),
            ..LyricsDisplayState::default()
        },
    }
}

fn spawn_lyrics_fetch(
    runtime: &tokio::runtime::Handle,
    sender: mpsc::Sender<LyricsFetchEvent>,
    track: TrackMetadata,
    provider_order: Vec<crate::lyrics::LyricsProvider>,
    track_fingerprint: String,
) {
    runtime.spawn(async move {
        let result = match search_best_lyrics(&track, &provider_order).await {
            Ok(Some(fetched)) => Ok(fetched),
            Ok(None) => Err("No lyrics found from configured providers".to_string()),
            Err(error) => Err(error.to_string()),
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

fn active_provider_order(config: &AppConfig) -> Vec<crate::lyrics::LyricsProvider> {
    SearchPlan::new(config.lyrics.provider_order.clone())
        .providers()
        .to_vec()
}
