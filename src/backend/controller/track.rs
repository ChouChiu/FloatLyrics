// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Per-track lyrics loading, the published lyrics document, and the per-track offset.

use floatlyrics_lyrics::cache::{TRACK_OFFSET_MS_MAX, TRACK_OFFSET_MS_MIN};

use crate::{
    backend::{
        cache::CacheService,
        model::{PlaybackSnapshot, lyrics_document},
    },
    shared::runtime::LyricsRuntimeConfig,
};

use super::{
    ControllerContext, ControllerState, LyricsView,
    loading::{LyricsLoadContext, load_lyrics_for_track},
};

pub(super) fn sync_lyrics_document(
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

/// Starts loading the lyrics of the current track when they are not loaded yet.
///
/// Runs on every tick, so it compares the fingerprint stored by the player
/// events instead of hashing the track metadata again.
pub(super) fn ensure_lyrics_loaded(ctx: &ControllerContext<'_>, state: &mut ControllerState) {
    let Some(fingerprint) = state.current_fingerprint.as_deref() else {
        return;
    };
    if state.lyrics.track_fingerprint.as_deref() == Some(fingerprint) {
        return;
    }
    let Some(track) = state
        .latest
        .as_ref()
        .and_then(|snapshot| snapshot.state.track.as_ref())
    else {
        return;
    };

    state.lyrics_generation = state.lyrics_generation.wrapping_add(1);
    if state.track_offset_fingerprint.as_deref() != Some(fingerprint) {
        state.track_offset_fingerprint = Some(fingerprint.to_string());
        state.track_offset_generation = state.track_offset_generation.wrapping_add(1);
        state.track_offset_ms = 0;
        ctx.floating.set_track_offset(0);
    }
    let load_context = LyricsLoadContext {
        cache: ctx.cache,
        config: ctx.config,
        cache_sender: ctx.cache_sender,
        generation: state.lyrics_generation,
        offset_generation: state.track_offset_generation,
        hint: state.lyrics_hint.clone(),
    };
    state.lyrics = load_lyrics_for_track(track, fingerprint.to_string(), &load_context);
    state.document_dirty = true;
}

pub(super) fn set_track_offset(
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
