// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Lyrics loading pipeline.
//!
//! Cache policy, provider fetching, and CPU-bound romanization are isolated in
//! sibling modules. SQLite work is submitted to the backend cache service and
//! reduced here only after its result returns to the controller.

mod cache;
mod fetch;
mod romanization;

use std::sync::mpsc;

use floatlyrics_core::{
    i18n::{Message, Text},
    track::TrackMetadata,
};
use floatlyrics_lyrics::cache::CachedLyrics;

use crate::{
    backend::{
        cache::{CacheService, ProviderStoreError},
        model::{LyricsDisplayState, PlaybackSnapshot},
    },
    shared::runtime::LyricsRuntimeConfig,
};

use cache::{active_provider_order, lyrics_state_from_cached, should_refresh_translation};
use fetch::spawn_lyrics_fetch;

pub(super) use fetch::{LyricsFetchApplyContext, LyricsFetchEvent, apply_lyrics_fetch_event};
pub(super) use romanization::{RomanizationEvent, apply_romanization_event};

#[derive(Debug)]
pub(super) enum LyricsCacheEvent {
    TrackLoaded {
        track: TrackMetadata,
        track_fingerprint: String,
        generation: u64,
        result: Result<Option<CachedLyrics>, String>,
    },
    ProviderStored {
        track_fingerprint: String,
        generation: u64,
        result: Result<Option<CachedLyrics>, ProviderStoreError>,
    },
}

pub(super) struct LyricsLoadContext<'a> {
    pub(super) cache: &'a CacheService,
    pub(super) config: &'a LyricsRuntimeConfig,
    pub(super) cache_sender: &'a mpsc::Sender<LyricsCacheEvent>,
    pub(super) generation: u64,
}

pub(super) fn load_lyrics_for_track(
    track: &TrackMetadata,
    fingerprint: String,
    ctx: &LyricsLoadContext<'_>,
) -> LyricsDisplayState {
    let provider_order = active_provider_order(ctx.config);
    let sender = ctx.cache_sender.clone();
    let event_track = track.clone();
    let event_fingerprint = fingerprint.clone();
    let generation = ctx.generation;
    ctx.cache
        .load_track(track.clone(), provider_order, move |result| {
            let _ = sender.send(LyricsCacheEvent::TrackLoaded {
                track: event_track,
                track_fingerprint: event_fingerprint,
                generation,
                result,
            });
        });

    LyricsDisplayState {
        track_fingerprint: Some(fingerprint),
        status_message: Some(Message::Text(Text::SearchingLyrics)),
        ..LyricsDisplayState::default()
    }
}

pub(super) struct LyricsCacheApplyContext<'a> {
    pub(super) current_generation: u64,
    pub(super) snapshot: &'a PlaybackSnapshot,
    pub(super) state: &'a mut LyricsDisplayState,
    pub(super) config: &'a LyricsRuntimeConfig,
    pub(super) runtime: &'a tokio::runtime::Handle,
    pub(super) lyrics_sender: &'a mpsc::Sender<LyricsFetchEvent>,
    pub(super) romanization_sender: &'a mpsc::Sender<RomanizationEvent>,
}

pub(super) fn apply_lyrics_cache_event(
    event: LyricsCacheEvent,
    ctx: &mut LyricsCacheApplyContext<'_>,
) -> bool {
    let (track_fingerprint, generation) = match &event {
        LyricsCacheEvent::TrackLoaded {
            track_fingerprint,
            generation,
            ..
        }
        | LyricsCacheEvent::ProviderStored {
            track_fingerprint,
            generation,
            ..
        } => (track_fingerprint, *generation),
    };
    if generation != ctx.current_generation
        || ctx.state.track_fingerprint.as_deref() != Some(track_fingerprint)
        || !ctx
            .snapshot
            .state
            .track
            .as_ref()
            .is_some_and(|track| track.fingerprint() == *track_fingerprint)
    {
        return false;
    }

    match event {
        LyricsCacheEvent::TrackLoaded {
            track,
            track_fingerprint,
            generation,
            result,
        } => apply_track_cache_result(track, track_fingerprint, generation, result, ctx),
        LyricsCacheEvent::ProviderStored {
            track_fingerprint,
            result,
            ..
        } => match result {
            Ok(Some(cached)) => {
                *ctx.state = lyrics_state_from_cached(
                    track_fingerprint,
                    &cached,
                    ctx.config,
                    ctx.runtime,
                    ctx.romanization_sender,
                );
                true
            }
            Ok(None) => {
                *ctx.state = LyricsDisplayState {
                    track_fingerprint: Some(track_fingerprint),
                    status_message: Some(Message::Text(Text::DownloadedLyricsNotStored)),
                    ..LyricsDisplayState::default()
                };
                true
            }
            Err(ProviderStoreError::Store(error)) => {
                if ctx.state.status_message != Some(Message::Text(Text::SearchingLyrics)) {
                    return false;
                }
                *ctx.state = LyricsDisplayState {
                    track_fingerprint: Some(track_fingerprint),
                    status_message: Some(Message::Detail(Text::LyricsCacheWriteError, error)),
                    ..LyricsDisplayState::default()
                };
                true
            }
            Err(ProviderStoreError::Load(error)) => {
                *ctx.state = LyricsDisplayState {
                    track_fingerprint: Some(track_fingerprint),
                    status_message: Some(Message::Detail(Text::LyricsCacheError, error)),
                    ..LyricsDisplayState::default()
                };
                true
            }
        },
    }
}

fn apply_track_cache_result(
    track: TrackMetadata,
    fingerprint: String,
    generation: u64,
    result: Result<Option<CachedLyrics>, String>,
    ctx: &mut LyricsCacheApplyContext<'_>,
) -> bool {
    match result {
        Ok(Some(cached)) => {
            let state = lyrics_state_from_cached(
                fingerprint.clone(),
                &cached,
                ctx.config,
                ctx.runtime,
                ctx.romanization_sender,
            );
            if should_refresh_translation(&cached, &state, ctx.config) {
                spawn_lyrics_fetch(
                    ctx.runtime,
                    ctx.lyrics_sender.clone(),
                    track,
                    active_provider_order(ctx.config),
                    fingerprint,
                    generation,
                );
            }
            *ctx.state = state;
            true
        }
        Ok(None) => {
            spawn_lyrics_fetch(
                ctx.runtime,
                ctx.lyrics_sender.clone(),
                track,
                active_provider_order(ctx.config),
                fingerprint,
                generation,
            );
            false
        }
        Err(error) => {
            *ctx.state = LyricsDisplayState {
                track_fingerprint: Some(fingerprint),
                status_message: Some(Message::Detail(Text::LyricsCacheError, error)),
                ..LyricsDisplayState::default()
            };
            true
        }
    }
}
