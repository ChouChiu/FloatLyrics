// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Asynchronous manual-search and cache operations exposed to the frontend.

use std::rc::Rc;

use floatlyrics_core::track::TrackMetadata;
use floatlyrics_lyrics::{
    cache::{LyricsCache, LyricsInsert},
    lyrics::{fetch_candidate_lyrics, search_lyrics_candidates, simplify_search_text},
};

use crate::shared::manual_search::{FetchedLyrics, LyricsCandidate, LyricsProvider};

#[derive(Clone)]
pub(crate) struct ManualSearchService {
    runtime: tokio::runtime::Handle,
    cache: Rc<dyn LyricsCache>,
}

impl ManualSearchService {
    pub(super) fn new(runtime: tokio::runtime::Handle, cache: Rc<dyn LyricsCache>) -> Self {
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
        track: &TrackMetadata,
        lyrics: &FetchedLyrics,
    ) -> Result<(), String> {
        self.cache
            .insert_lyrics(LyricsInsert {
                provider: lyrics.provider,
                provider_track_id: lyrics.provider_track_id.as_deref(),
                title: &lyrics.title,
                artists: &lyrics.artists,
                raw_lyrics: &lyrics.raw_lyrics,
            })
            .and_then(|lyrics_id| {
                self.cache
                    .bind_manual_match(&track.fingerprint(), lyrics_id)
            })
            .map_err(|error| error.to_string())
    }

    pub(crate) fn search_field_values(track: &TrackMetadata) -> (String, String) {
        (
            simplify_search_text(&track.title),
            simplify_search_text(&track.display_artist()),
        )
    }
}
