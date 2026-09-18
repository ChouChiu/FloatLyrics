// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Concrete `lyrics-helper` provider adapters.

use crate::lyrics::{
    model::{FetchedLyrics, LyricsCandidate, LyricsLookupHint, LyricsProvider},
    parsing::combine_lyrics_with_translation,
};
use floatlyrics_core::track::TrackMetadata;
use lyrics_helper::SearchError;

pub(super) async fn search_provider_candidates(
    provider: LyricsProvider,
    metadata: &lyrics_helper::models::TrackMetadata,
) -> Result<Vec<LyricsCandidate>, SearchError> {
    use lyrics_helper::searchers::{
        netease::NeteaseSearcher, qq_music::QQMusicSearcher, search_with_refinement,
    };

    let results = match provider {
        LyricsProvider::QqMusic => {
            search_with_refinement(&QQMusicSearcher, metadata, false).await?
        }
        LyricsProvider::NetEase => {
            search_with_refinement(&NeteaseSearcher, metadata, false).await?
        }
    };

    Ok(results
        .into_iter()
        .map(|result| LyricsCandidate {
            provider,
            provider_track_id: result.id,
            numeric_id: result.numeric_id,
            title: result.title,
            artists: result.artists,
            album: result.album,
            duration_ms: result.duration_ms,
            match_score: result.match_type.map_or(0, |value| value as i32),
        })
        .collect())
}

pub(super) async fn fetch_candidate_raw_lyrics(
    candidate: &LyricsCandidate,
) -> Result<Option<String>, SearchError> {
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

pub(super) async fn fetch_hint_lyrics(
    track: &TrackMetadata,
    hint: &LyricsLookupHint,
) -> Option<FetchedLyrics> {
    let artist = track.display_artist();
    let (id, numeric_id) = match hint.provider {
        LyricsProvider::QqMusic => (
            hint.provider_track_id.as_str(),
            hint.provider_track_id.parse::<i64>().ok(),
        ),
        LyricsProvider::NetEase => (hint.provider_track_id.as_str(), None),
    };
    // The hint only shortcuts the search below: an identifier the provider can no
    // longer serve — and a failed request — falls back to it, so the outcome here
    // stays an `Option`.
    let raw_lyrics = fetch_raw_lyrics(ProviderTrackRef {
        provider: hint.provider,
        id,
        numeric_id,
        title: &track.title,
        artist: &artist,
        album: track.album.as_deref().unwrap_or_default(),
        duration_ms: track.duration_ms.and_then(|value| value.try_into().ok()),
    })
    .await
    .ok()
    .flatten()?
    .trim()
    .to_string();
    if raw_lyrics.is_empty() {
        return None;
    }

    Some(FetchedLyrics {
        provider: hint.provider,
        provider_track_id: Some(hint.provider_track_id.clone()),
        title: track.title.clone(),
        artists: track.artists.clone(),
        score: 100.0,
        raw_lyrics,
    })
}

pub(super) async fn search_provider_best(
    provider: LyricsProvider,
    metadata: &lyrics_helper::models::TrackMetadata,
) -> Result<Option<FetchedLyrics>, SearchError> {
    use lyrics_helper::searchers::{
        compare_helper::MatchType, netease::NeteaseSearcher, qq_music::QQMusicSearcher,
        search_for_best_result_with_match,
    };

    let result = match provider {
        LyricsProvider::QqMusic => {
            search_for_best_result_with_match(&QQMusicSearcher, metadata, MatchType::Medium).await?
        }
        LyricsProvider::NetEase => {
            search_for_best_result_with_match(&NeteaseSearcher, metadata, MatchType::Medium).await?
        }
    };

    let Some(result) = result else {
        return Ok(None);
    };
    let Some(raw_lyrics) = fetch_result_lyrics(provider, &result).await? else {
        return Ok(None);
    };
    let raw_lyrics = raw_lyrics.trim().to_string();
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
) -> Result<Option<String>, SearchError> {
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

async fn fetch_raw_lyrics(track: ProviderTrackRef<'_>) -> Result<Option<String>, SearchError> {
    use lyrics_helper::search::providers::web::{netease, qq_music};

    let (lyrics, translation) = match track.provider {
        LyricsProvider::QqMusic => {
            qq_music::api::get_lyrics(
                track.id,
                track.numeric_id,
                track.title,
                track.artist,
                track.album,
                track.duration_ms,
            )
            .await?
        }
        LyricsProvider::NetEase => {
            let Ok(song_id) = track.id.parse() else {
                // An identifier NetEase cannot address names no song, which is the
                // same outcome as a song without lyrics.
                return Ok(None);
            };
            netease::api::get_lyrics(song_id).await?
        }
    };

    Ok(lyrics.map(|lyrics| combine_lyrics_with_translation(&lyrics, translation.as_deref())))
}
