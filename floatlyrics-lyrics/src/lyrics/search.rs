// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Provider-neutral search orchestration.
//!
//! Ranking and provider-order policy live in [`ranking`], query normalization
//! lives in [`query`], and concrete `lyrics-helper` adapters live in
//! [`provider`]. This facade keeps those details behind the public lyrics API.

mod provider;
mod query;
mod ranking;

use anyhow::Result;
use floatlyrics_core::track::TrackMetadata;

use super::model::{FetchedLyrics, LyricsCandidate, LyricsProvider};
use provider::{fetch_candidate_raw_lyrics, search_provider_best, search_provider_candidates};
pub(super) use query::lyrics_helper_metadata;
use ranking::finalize_candidates;

pub use query::simplify_search_text;
pub use ranking::SearchPlan;

/// Searches configured providers and returns ranked, deduplicated candidates.
///
/// At most twelve candidates are returned.
///
/// # Errors
/// Returns an error when a provider search reports a recoverable failure.
pub async fn search_lyrics_candidates(
    track: &TrackMetadata,
    provider_order: &[LyricsProvider],
) -> Result<Vec<LyricsCandidate>> {
    let metadata = lyrics_helper_metadata(track);
    let mut candidates = Vec::new();
    for provider in provider_order {
        candidates.extend(search_provider_candidates(*provider, &metadata).await);
    }

    Ok(finalize_candidates(candidates))
}

/// Downloads the lyrics represented by a manually selected candidate.
///
/// Empty provider responses are returned as `Ok(None)`.
///
/// # Errors
/// Returns an error when a provider reports a recoverable download failure.
pub async fn fetch_candidate_lyrics(candidate: &LyricsCandidate) -> Result<Option<FetchedLyrics>> {
    let raw_lyrics = fetch_candidate_raw_lyrics(candidate).await;
    let Some(raw_lyrics) = raw_lyrics.map(|value| value.trim().to_string()) else {
        return Ok(None);
    };
    if raw_lyrics.is_empty() {
        return Ok(None);
    }

    Ok(Some(FetchedLyrics {
        provider: candidate.provider,
        provider_track_id: Some(candidate.provider_track_id.clone()),
        title: candidate.title.clone(),
        artists: candidate.artists.clone(),
        score: candidate.match_score as f64,
        raw_lyrics,
    }))
}

/// Searches providers in priority order and returns the first acceptable result.
///
/// # Errors
/// Returns an error when a provider reports a recoverable search failure.
pub async fn search_best_lyrics(
    track: &TrackMetadata,
    provider_order: &[LyricsProvider],
) -> Result<Option<FetchedLyrics>> {
    let metadata = lyrics_helper_metadata(track);

    for provider in provider_order {
        if let Some(fetched) = search_provider_best(*provider, &metadata).await? {
            return Ok(Some(fetched));
        }
    }

    Ok(None)
}

#[cfg(test)]
#[path = "../test/search_test.rs"]
mod tests;
