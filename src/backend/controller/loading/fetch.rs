// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Provider fetch tasks and cache write-back reducer.

use std::sync::mpsc;

use floatlyrics_core::{
    i18n::{Message, Text},
    track::TrackMetadata,
};
use floatlyrics_lyrics::lyrics::{FetchedLyrics, LyricsProvider, search_best_lyrics};

use crate::{
    backend::{
        cache::CacheService,
        model::{LyricsDisplayState, PlaybackSnapshot},
    },
    shared::runtime::LyricsRuntimeConfig,
};

use super::{LyricsCacheEvent, cache::active_provider_order};

#[derive(Debug)]
pub(in crate::backend::controller) struct LyricsFetchEvent {
    pub(in crate::backend::controller) track_fingerprint: String,
    pub(in crate::backend::controller) generation: u64,
    pub(in crate::backend::controller) result:
        std::result::Result<FetchedLyrics, LyricsFetchFailure>,
}

#[derive(Debug)]
pub(in crate::backend::controller) enum LyricsFetchFailure {
    NotFound,
    Other(String),
}

pub(in crate::backend::controller) struct LyricsFetchApplyContext<'a> {
    pub(in crate::backend::controller) current_generation: u64,
    pub(in crate::backend::controller) snapshot: &'a PlaybackSnapshot,
    pub(in crate::backend::controller) state: &'a mut LyricsDisplayState,
    pub(in crate::backend::controller) cache: &'a CacheService,
    pub(in crate::backend::controller) config: &'a LyricsRuntimeConfig,
    pub(in crate::backend::controller) cache_sender: &'a mpsc::Sender<LyricsCacheEvent>,
}

fn lyrics_fetch_matches_current(
    event: &LyricsFetchEvent,
    current_generation: u64,
    snapshot: &PlaybackSnapshot,
    state: &LyricsDisplayState,
) -> bool {
    event.generation == current_generation
        && snapshot
            .state
            .track
            .as_ref()
            .is_some_and(|track| track.fingerprint() == event.track_fingerprint)
        && state.track_fingerprint.as_deref() == Some(event.track_fingerprint.as_str())
}

pub(in crate::backend::controller) fn apply_lyrics_fetch_event(
    event: LyricsFetchEvent,
    ctx: &mut LyricsFetchApplyContext<'_>,
) -> bool {
    if !lyrics_fetch_matches_current(&event, ctx.current_generation, ctx.snapshot, ctx.state) {
        return false;
    }

    match event.result {
        Ok(fetched) => {
            let sender = ctx.cache_sender.clone();
            let track_fingerprint = event.track_fingerprint;
            let event_fingerprint = track_fingerprint.clone();
            let generation = event.generation;
            ctx.cache.store_provider_and_load(
                track_fingerprint,
                fetched,
                active_provider_order(ctx.config),
                move |result| {
                    let _ = sender.send(LyricsCacheEvent::ProviderStored {
                        track_fingerprint: event_fingerprint,
                        generation,
                        result,
                    });
                },
            );
            false
        }
        Err(failure) => apply_lyrics_fetch_failure(ctx.state, event.track_fingerprint, failure),
    }
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

pub(super) fn spawn_lyrics_fetch(
    runtime: &tokio::runtime::Handle,
    sender: mpsc::Sender<LyricsFetchEvent>,
    track: TrackMetadata,
    provider_order: Vec<LyricsProvider>,
    track_fingerprint: String,
    generation: u64,
) {
    runtime.spawn(async move {
        let result = match search_best_lyrics(&track, &provider_order).await {
            Ok(Some(fetched)) => Ok(fetched),
            Ok(None) => Err(LyricsFetchFailure::NotFound),
            Err(error) => Err(LyricsFetchFailure::Other(error.to_string())),
        };

        let _ = sender.send(LyricsFetchEvent {
            track_fingerprint,
            generation,
            result,
        });
    });
}

#[cfg(test)]
#[path = "../../../test/lyrics_fetch_test.rs"]
mod tests;
