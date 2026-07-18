// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Asynchronous manual-search and cache operations exposed to the frontend.

use floatlyrics_core::track::TrackMetadata;
use floatlyrics_lyrics::lyrics::{
    fetch_candidate_lyrics, search_lyrics_candidates, simplify_search_text,
};

use crate::shared::manual_search::{FetchedLyrics, LyricsCandidate, LyricsProvider};

use super::cache::CacheService;

#[derive(Clone)]
pub(crate) struct ManualSearchService {
    runtime: tokio::runtime::Handle,
    cache: CacheService,
}

impl ManualSearchService {
    pub(super) fn new(runtime: tokio::runtime::Handle, cache: CacheService) -> Self {
        Self { runtime, cache }
    }

    pub(crate) fn search(
        &self,
        track: TrackMetadata,
        complete: impl FnOnce(Result<Vec<LyricsCandidate>, String>) + Send + 'static,
    ) {
        self.runtime.spawn(async move {
            let providers = LyricsProvider::default_order();
            complete(
                search_lyrics_candidates(&track, &providers)
                    .await
                    .map_err(|error| error.to_string()),
            );
        });
    }

    pub(crate) fn preview(
        &self,
        candidate: LyricsCandidate,
        complete: impl FnOnce(Result<Option<FetchedLyrics>, String>) + Send + 'static,
    ) {
        self.runtime.spawn(async move {
            complete(
                fetch_candidate_lyrics(&candidate)
                    .await
                    .map_err(|error| error.to_string()),
            );
        });
    }

    pub(crate) fn apply(
        &self,
        track: TrackMetadata,
        lyrics: FetchedLyrics,
        complete: impl FnOnce(Result<(), String>) + Send + 'static,
    ) {
        self.cache.apply_manual(track, lyrics, complete);
    }

    pub(crate) fn search_field_values(track: &TrackMetadata) -> (String, String) {
        (
            simplify_search_text(&track.title),
            simplify_search_text(&track.display_artist()),
        )
    }
}
