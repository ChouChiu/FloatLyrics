// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Provider search orchestration and candidate ranking.

use anyhow::Result;
use std::{cmp::Reverse, collections::HashSet};

use floatlyrics_core::track::TrackMetadata;

use super::{
    model::{FetchedLyrics, LyricsCandidate, LyricsProvider},
    parsing::combine_lyrics_with_translation,
};

const MANUAL_SEARCH_LIMIT: usize = 12;

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
    use lyrics_helper::searchers::{
        netease::NeteaseSearcher, qq_music::QQMusicSearcher, search_with_refinement,
    };

    let metadata = lyrics_helper_metadata(track);
    let mut candidates = Vec::new();
    for provider in provider_order {
        let results = match provider {
            LyricsProvider::QqMusic => {
                search_with_refinement(&QQMusicSearcher, &metadata, false).await
            }
            LyricsProvider::NetEase => {
                search_with_refinement(&NeteaseSearcher, &metadata, false).await
            }
        };
        candidates.extend(results.into_iter().map(|result| LyricsCandidate {
            provider: *provider,
            provider_track_id: result.id,
            numeric_id: result.numeric_id,
            title: result.title,
            artists: result.artists,
            album: result.album,
            duration_ms: result.duration_ms,
            match_score: result.match_type.map_or(0, |value| value as i32),
        }));
    }

    Ok(finalize_candidates(candidates))
}

fn finalize_candidates(mut candidates: Vec<LyricsCandidate>) -> Vec<LyricsCandidate> {
    candidates.sort_by_key(|candidate| Reverse(candidate.match_score));
    let mut seen = HashSet::new();
    candidates.retain(|candidate| {
        seen.insert((
            candidate.provider.as_str(),
            candidate.provider_track_id.clone(),
        ))
    });
    candidates.truncate(MANUAL_SEARCH_LIMIT);
    candidates
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

async fn fetch_candidate_raw_lyrics(candidate: &LyricsCandidate) -> Option<String> {
    let artist = candidate.artists.join(", ");
    fetch_raw_lyrics(ProviderTrackRef {
        provider: candidate.provider,
        id: &candidate.provider_track_id,
        numeric_id: candidate.numeric_id,
        title: &candidate.title,
        artist: &artist,
        album: &candidate.album,
        duration_ms: candidate.duration_ms,
    })
    .await
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
        if let Some(fetched) = search_provider(*provider, &metadata).await? {
            return Ok(Some(fetched));
        }
    }

    Ok(None)
}

async fn search_provider(
    provider: LyricsProvider,
    metadata: &lyrics_helper::models::TrackMetadata,
) -> Result<Option<FetchedLyrics>> {
    use lyrics_helper::searchers::{
        compare_helper::MatchType, netease::NeteaseSearcher, qq_music::QQMusicSearcher,
        search_for_best_result_with_match,
    };

    let result = match provider {
        LyricsProvider::QqMusic => {
            search_for_best_result_with_match(&QQMusicSearcher, metadata, MatchType::Medium).await
        }
        LyricsProvider::NetEase => {
            search_for_best_result_with_match(&NeteaseSearcher, metadata, MatchType::Medium).await
        }
    };

    let Some(result) = result else {
        return Ok(None);
    };
    let raw_lyrics = fetch_result_lyrics(provider, &result).await;
    let Some(raw_lyrics) = raw_lyrics.map(|value| value.trim().to_string()) else {
        return Ok(None);
    };
    if raw_lyrics.is_empty() {
        return Ok(None);
    }

    Ok(Some(FetchedLyrics {
        provider,
        provider_track_id: Some(result.id.clone()),
        title: result.title,
        artists: result.artists,
        score: result
            .match_type
            .map_or(0.0, |match_type| match_type as i32 as f64),
        raw_lyrics,
    }))
}

async fn fetch_result_lyrics(
    provider: LyricsProvider,
    result: &lyrics_helper::searchers::search_result::SearchResult,
) -> Option<String> {
    let artist = result.artist();
    fetch_raw_lyrics(ProviderTrackRef {
        provider,
        id: &result.id,
        numeric_id: result.numeric_id,
        title: &result.title,
        artist: &artist,
        album: &result.album,
        duration_ms: result.duration_ms,
    })
    .await
}

struct ProviderTrackRef<'a> {
    provider: LyricsProvider,
    id: &'a str,
    numeric_id: Option<i64>,
    title: &'a str,
    artist: &'a str,
    album: &'a str,
    duration_ms: Option<i32>,
}

async fn fetch_raw_lyrics(track: ProviderTrackRef<'_>) -> Option<String> {
    use lyrics_helper::search::providers::web::{netease, qq_music};

    let response = match track.provider {
        LyricsProvider::QqMusic => {
            qq_music::api::get_lyrics(
                track.id,
                track.numeric_id,
                track.title,
                track.artist,
                track.album,
                track.duration_ms,
            )
            .await
        }
        LyricsProvider::NetEase => {
            let song_id = track.id.parse().ok()?;
            netease::api::get_lyrics(song_id).await
        }
    };

    response.and_then(|(lyrics, translation)| {
        lyrics.map(|lyrics| combine_lyrics_with_translation(&lyrics, translation.as_deref()))
    })
}

pub(super) fn lyrics_helper_metadata(
    track: &TrackMetadata,
) -> lyrics_helper::models::TrackMetadata {
    let mut metadata = lyrics_helper::models::TrackMetadata::new();
    metadata.title = Some(simplify_search_text(&track.title));
    metadata.artist = Some(simplify_search_text(&track.display_artist()));
    metadata.artists = Some(
        track
            .artists
            .iter()
            .map(|artist| simplify_search_text(artist))
            .collect(),
    );
    metadata.album = track.album.as_deref().map(simplify_search_text);
    metadata.duration_ms = track.duration_ms.and_then(|value| value.try_into().ok());
    metadata
}

/// Converts Traditional Chinese characters to Simplified Chinese for provider queries.
#[must_use]
pub fn simplify_search_text(text: &str) -> String {
    lyrics_helper::helpers::chinese_helper::to_simplified(text)
}

/// Validated provider priority used by automatic search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPlan {
    providers: Vec<LyricsProvider>,
}

impl SearchPlan {
    /// Builds a plan, removing repeated providers.
    pub fn new(providers: impl IntoIterator<Item = LyricsProvider>) -> Self {
        let mut providers = providers.into_iter().collect::<Vec<_>>();
        let mut seen = HashSet::new();
        providers.retain(|provider| seen.insert(*provider));
        Self { providers }
    }

    /// Builds the default QQ Music then NetEase plan.
    pub fn default_mvp() -> Self {
        Self::new(LyricsProvider::default_order())
    }

    /// Returns providers in search priority order.
    pub fn providers(&self) -> &[LyricsProvider] {
        &self.providers
    }
}

#[cfg(test)]
#[path = "../test/search_test.rs"]
mod tests;
