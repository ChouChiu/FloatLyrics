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
use futures::{StreamExt, stream};
use lyrics_helper::SearchError;

use super::model::{FetchedLyrics, LyricsCandidate, LyricsLookupHint, LyricsProvider};
use provider::{
    fetch_candidate_raw_lyrics, fetch_hint_lyrics, lookup_qq_candidate, search_provider_best,
    search_provider_candidates,
};
pub(super) use query::lyrics_helper_metadata;
use ranking::finalize_candidates;

pub use query::simplify_search_text;
pub use ranking::SearchPlan;

/// Searches configured providers and returns ranked, deduplicated candidates.
///
/// At most twelve candidates are returned. A provider that reports a failure is
/// skipped so the remaining ones still answer. Requests overlap, and results
/// are collected in provider order before ranking.
///
/// # Errors
/// Returns an error when no candidate was found and at least one provider failed.
pub async fn search_lyrics_candidates(
    track: &TrackMetadata,
    provider_order: &[LyricsProvider],
) -> Result<Vec<LyricsCandidate>> {
    let metadata = lyrics_helper_metadata(track);
    let mut candidates = Vec::new();
    let mut failure: Option<SearchError> = None;
    let mut searches = provider_requests(provider_order, |provider| {
        search_provider_candidates(provider, &metadata)
    });
    while let Some((provider, result)) = searches.next().await {
        match result {
            Ok(results) => candidates.extend(results),
            Err(error) => {
                tracing::warn!(provider = provider.as_str(), %error, "provider search failed");
                failure = Some(error);
            }
        }
    }

    if candidates.is_empty()
        && let Some(failure) = failure
    {
        return Err(failure.into());
    }

    Ok(finalize_candidates(candidates))
}

/// Resolves a QQ Music song URL, MID, or numeric ID into an exact candidate.
///
/// # Errors
/// Returns an error if the reference is invalid or the provider lookup fails.
pub async fn lookup_qq_lyrics_candidate(reference: &str) -> Result<Option<LyricsCandidate>> {
    Ok(lookup_qq_candidate(reference).await?)
}

/// Downloads the lyrics represented by a manually selected candidate.
///
/// Empty provider responses are returned as `Ok(None)`.
///
/// # Errors
/// Returns an error when the provider reports a download failure.
pub async fn fetch_candidate_lyrics(candidate: &LyricsCandidate) -> Result<Option<FetchedLyrics>> {
    let Some(raw_lyrics) = fetch_candidate_raw_lyrics(candidate).await? else {
        return Ok(None);
    };
    let raw_lyrics = raw_lyrics.trim().to_string();
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

/// Returns the first acceptable result in provider order. Nearby requests
/// overlap so a miss can fall through without waiting for a full second lookup.
///
/// # Errors
/// Returns an error when no provider returned lyrics and at least one failed.
pub async fn search_best_lyrics(
    track: &TrackMetadata,
    provider_order: &[LyricsProvider],
) -> Result<Option<FetchedLyrics>> {
    search_best_lyrics_with_hint(track, provider_order, None).await
}

/// Uses an exact playback-source hint before falling back to provider search.
///
/// Hints for providers absent from `provider_order` are ignored. A missing or
/// stale identifier is recoverable and falls back to the same metadata search
/// used by [`search_best_lyrics`], and so is a provider that reports a failure:
/// the next provider in the order is asked instead.
///
/// # Errors
/// Returns an error when no provider returned lyrics and at least one failed.
pub async fn search_best_lyrics_with_hint(
    track: &TrackMetadata,
    provider_order: &[LyricsProvider],
    hint: Option<&LyricsLookupHint>,
) -> Result<Option<FetchedLyrics>> {
    let metadata = lyrics_helper_metadata(track);
    let mut failure: Option<SearchError> = None;

    let mut searches = provider_requests(provider_order, |provider| {
        let metadata = &metadata;
        async move {
            if let Some(hint) = hint.filter(|hint| hint.provider == provider)
                && let Some(fetched) = fetch_hint_lyrics(track, hint).await
            {
                return Ok(Some(fetched));
            }
            search_provider_best(provider, metadata).await
        }
    });

    while let Some((provider, result)) = searches.next().await {
        match result {
            Ok(Some(fetched)) => return Ok(Some(fetched)),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(provider = provider.as_str(), %error, "provider search failed");
                failure = Some(error);
            }
        }
    }

    match failure {
        Some(failure) => Err(failure.into()),
        None => Ok(None),
    }
}

// Limit speculative requests so a source that usually has lyrics only overlaps
// with its immediate fallback. Ordered buffering preserves configured priority.
const MAX_PROVIDER_REQUESTS: usize = 2;

fn provider_requests<'a, F, Fut, T>(
    order: &'a [LyricsProvider],
    mut request: F,
) -> impl futures::Stream<Item = (LyricsProvider, T)> + Unpin + 'a
where
    F: FnMut(LyricsProvider) -> Fut + Unpin + 'a,
    Fut: Future<Output = T> + 'a,
    T: 'a,
{
    stream::iter(order.iter().copied())
        .map(move |provider| {
            let result = request(provider);
            async move { (provider, result.await) }
        })
        .buffered(MAX_PROVIDER_REQUESTS)
}

#[cfg(test)]
#[path = "search/tests.rs"]
mod tests;
