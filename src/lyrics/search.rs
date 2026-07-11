// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use std::{cmp::Reverse, collections::HashSet};

use crate::track::TrackMetadata;

use super::{
    model::{FetchedLyrics, LyricsCandidate, LyricsProvider},
    parsing::combine_lyrics_with_translation,
};

const MANUAL_SEARCH_LIMIT: usize = 12;

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
            LyricsProvider::LrcLib => Vec::new(),
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
    use lyrics_helper::search::providers::web::{netease, qq_music};

    match candidate.provider {
        LyricsProvider::QqMusic => qq_music::api::get_lyrics(
            &candidate.provider_track_id,
            candidate.numeric_id,
            &candidate.title,
            &candidate.artists.join(", "),
            &candidate.album,
            candidate.duration_ms,
        )
        .await
        .and_then(|(lyric, translation)| {
            lyric.map(|lyric| combine_lyrics_with_translation(&lyric, translation.as_deref()))
        }),
        LyricsProvider::NetEase => {
            let song_id = candidate.provider_track_id.parse().ok()?;
            netease::api::get_lyrics(song_id)
                .await
                .and_then(|(lyric, translation)| {
                    lyric.map(|lyric| {
                        combine_lyrics_with_translation(&lyric, translation.as_deref())
                    })
                })
        }
        LyricsProvider::LrcLib => None,
    }
}

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
        LyricsProvider::LrcLib => return Ok(None),
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
    use lyrics_helper::search::providers::web::{netease, qq_music};

    match provider {
        LyricsProvider::QqMusic => qq_music::api::get_lyrics(
            &result.id,
            result.numeric_id,
            &result.title,
            &result.artist(),
            &result.album,
            result.duration_ms,
        )
        .await
        .and_then(|(lyric, translation)| {
            lyric.map(|lyric| combine_lyrics_with_translation(&lyric, translation.as_deref()))
        }),
        LyricsProvider::NetEase => {
            let song_id = result.id.parse().ok()?;
            netease::api::get_lyrics(song_id)
                .await
                .and_then(|(lyric, translation)| {
                    lyric.map(|lyric| {
                        combine_lyrics_with_translation(&lyric, translation.as_deref())
                    })
                })
        }
        LyricsProvider::LrcLib => None,
    }
}

pub(super) fn lyrics_helper_metadata(
    track: &TrackMetadata,
) -> lyrics_helper::models::TrackMetadata {
    let mut metadata = lyrics_helper::models::TrackMetadata::new();
    metadata.title = Some(track.title.clone());
    metadata.artist = Some(track.display_artist());
    metadata.artists = Some(track.artists.clone());
    metadata.album = track.album.clone();
    metadata.duration_ms = track.duration_ms.and_then(|value| value.try_into().ok());
    metadata
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPlan {
    providers: Vec<LyricsProvider>,
}

impl SearchPlan {
    pub fn new(providers: impl IntoIterator<Item = LyricsProvider>) -> Self {
        let mut providers = providers.into_iter().collect::<Vec<_>>();
        providers.retain(|provider| LyricsProvider::default_order().contains(provider));
        providers.dedup();
        Self { providers }
    }

    pub fn default_mvp() -> Self {
        Self::new(LyricsProvider::default_order())
    }

    pub fn providers(&self) -> &[LyricsProvider] {
        &self.providers
    }
}

#[cfg(test)]
#[path = "../test/search_test.rs"]
mod tests;
