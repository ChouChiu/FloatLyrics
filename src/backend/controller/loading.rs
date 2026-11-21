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
use floatlyrics_lyrics::lyrics::LyricsLookupHint;

use crate::{
    backend::{
        cache::{CacheService, ProviderStoreError},
        model::{LyricsDisplayState, PlaybackSnapshot},
    },
    shared::runtime::LyricsRuntimeConfig,
};

use cache::{PreparedLyrics, active_provider_order, should_refresh_translation};
use fetch::spawn_lyrics_fetch;
use romanization::spawn_local_romanization;

pub(super) use fetch::{LyricsFetchApplyContext, LyricsFetchEvent, apply_lyrics_fetch_event};
pub(super) use romanization::{RomanizationEvent, apply_romanization_event};

#[derive(Debug)]
pub(super) enum LyricsCacheEvent {
    TrackLoaded {
        track: TrackMetadata,
        hint: Option<LyricsLookupHint>,
        track_fingerprint: String,
        generation: u64,
        offset_generation: u64,
        result: Result<(Option<PreparedLyrics>, i64), String>,
    },
    ProviderStored {
        track_fingerprint: String,
        generation: u64,
        result: Result<Option<PreparedLyrics>, ProviderStoreError>,
    },
}

pub(super) struct LyricsLoadContext<'a> {
    pub(super) runtime: &'a tokio::runtime::Handle,
    pub(super) cache: &'a CacheService,
    pub(super) config: &'a LyricsRuntimeConfig,
    pub(super) cache_sender: &'a mpsc::Sender<LyricsCacheEvent>,
    pub(super) generation: u64,
    pub(super) offset_generation: u64,
    pub(super) hint: Option<LyricsLookupHint>,
}

pub(super) fn load_lyrics_for_track(
    track: &TrackMetadata,
    fingerprint: String,
    ctx: &LyricsLoadContext<'_>,
) -> LyricsDisplayState {
    let provider_order = active_provider_order(ctx.config);
    let sender = ctx.cache_sender.clone();
    let event_track = track.clone();
    let event_hint = ctx.hint.clone();
    let event_fingerprint = fingerprint.clone();
    let lyrics_fingerprint = fingerprint.clone();
    let generation = ctx.generation;
    let offset_generation = ctx.offset_generation;
    let runtime = ctx.runtime.clone();
    ctx.cache
        .load_track(track.clone(), provider_order, move |result| {
            let complete = move |result| {
                let _ = sender.send(LyricsCacheEvent::TrackLoaded {
                    track: event_track,
                    hint: event_hint,
                    track_fingerprint: event_fingerprint,
                    generation,
                    offset_generation,
                    result,
                });
            };
            match result {
                Ok(cached) => match cached.lyrics {
                    Some(lyrics) => {
                        let fingerprint = lyrics_fingerprint;
                        runtime.spawn_blocking(move || {
                            let lyrics = PreparedLyrics::parse(fingerprint, lyrics);
                            complete(Ok((Some(lyrics), cached.offset_ms)));
                        });
                    }
                    None => complete(Ok((None, cached.offset_ms))),
                },
                Err(error) => complete(Err(error)),
            }
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
    pub(super) track_offset_ms: &'a mut i64,
    pub(super) current_offset_generation: u64,
    pub(super) config: &'a LyricsRuntimeConfig,
    pub(super) runtime: &'a tokio::runtime::Handle,
    pub(super) lyrics_sender: &'a mpsc::Sender<LyricsFetchEvent>,
    pub(super) pending_fetch: &'a mut Option<tokio::task::JoinHandle<()>>,
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
        || ctx
            .snapshot
            .state
            .track
            .as_ref()
            .is_none_or(|track| track.fingerprint() != *track_fingerprint)
    {
        return false;
    }

    match event {
        LyricsCacheEvent::TrackLoaded {
            track,
            hint,
            track_fingerprint,
            generation,
            offset_generation,
            result,
        } => apply_track_cache_result(
            track,
            hint,
            track_fingerprint,
            generation,
            offset_generation,
            result,
            ctx,
        ),
        LyricsCacheEvent::ProviderStored {
            track_fingerprint,
            result,
            ..
        } => match result {
            Ok(Some(lyrics)) => {
                publish_prepared_lyrics(lyrics, ctx);
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
    hint: Option<LyricsLookupHint>,
    fingerprint: String,
    generation: u64,
    offset_generation: u64,
    result: Result<(Option<PreparedLyrics>, i64), String>,
    ctx: &mut LyricsCacheApplyContext<'_>,
) -> bool {
    match result {
        Ok((lyrics, offset_ms)) => {
            if offset_generation == ctx.current_offset_generation {
                *ctx.track_offset_ms = offset_ms;
            }
            let should_fetch = lyrics.as_ref().is_none_or(|lyrics| {
                should_refresh_translation(lyrics, ctx.config)
                    || hint_requires_refresh(lyrics, hint.as_ref(), ctx.config)
            });
            if should_fetch {
                *ctx.pending_fetch = Some(spawn_lyrics_fetch(
                    ctx.runtime,
                    ctx.lyrics_sender.clone(),
                    track,
                    hint,
                    active_provider_order(ctx.config),
                    fingerprint,
                    generation,
                ));
            }
            if let Some(lyrics) = lyrics {
                publish_prepared_lyrics(lyrics, ctx);
                return true;
            }
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

fn publish_prepared_lyrics(lyrics: PreparedLyrics, ctx: &mut LyricsCacheApplyContext<'_>) {
    let state = lyrics.state;
    if ctx.config.show_romanization
        && !state.lines.is_empty()
        && let Some(fingerprint) = &state.track_fingerprint
    {
        spawn_local_romanization(
            ctx.runtime,
            ctx.romanization_sender.clone(),
            fingerprint.clone(),
            state.lines.clone(),
            ctx.config.chinese_romanization,
        );
    }
    *ctx.state = state;
}

fn hint_requires_refresh(
    cached: &PreparedLyrics,
    hint: Option<&LyricsLookupHint>,
    config: &LyricsRuntimeConfig,
) -> bool {
    let Some(hint) = hint else {
        return false;
    };
    let provider_order = active_provider_order(config);
    let Some(hint_index) = provider_order
        .iter()
        .position(|provider| *provider == hint.provider)
    else {
        return false;
    };
    let cached_index = provider_order
        .iter()
        .position(|provider| *provider == cached.provider);
    !cached.manually_selected
        && cached_index.is_none_or(|cached_index| hint_index <= cached_index)
        && (cached.provider != hint.provider
            || cached.provider_track_id.as_deref() != Some(hint.provider_track_id.as_str()))
}
