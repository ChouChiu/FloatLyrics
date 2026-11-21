// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Concrete `lyrics-helper` provider adapters.

use crate::lyrics::{
    model::{FetchedLyrics, LyricsCandidate, LyricsLookupHint, LyricsProvider},
    parsing::{combine_lyrics_with_translation, combine_word_timing},
};
use floatlyrics_core::track::TrackMetadata;
use lyrics_helper::{
    FetchedLyrics as ProviderLyrics, LyricsRawTypes, SearchError, fetch_lyrics, resolve_qq_track,
    searchers::{Searchers, compare_helper::MatchType, search_result::SearchResult},
};

pub(super) async fn search_provider_candidates(
    provider: LyricsProvider,
    metadata: &lyrics_helper::models::TrackMetadata,
) -> Result<Vec<LyricsCandidate>, SearchError> {
    use lyrics_helper::searchers::{
        amll_ttml_db::AmllTtmlDbSearcher, kugou::KugouSearcher, lrclib::LRCLIBSearcher,
        netease::NeteaseSearcher, qq_music::QQMusicSearcher, search_with_refinement,
        soda_music::SodaMusicSearcher,
    };

    let results = match provider {
        LyricsProvider::QqMusic => {
            search_with_refinement(&QQMusicSearcher, metadata, false).await?
        }
        LyricsProvider::NetEase => {
            search_with_refinement(&NeteaseSearcher, metadata, false).await?
        }
        LyricsProvider::Kugou => search_with_refinement(&KugouSearcher, metadata, false).await?,
        LyricsProvider::Lrclib => search_with_refinement(&LRCLIBSearcher, metadata, false).await?,
        LyricsProvider::SodaMusic => {
            search_with_refinement(&SodaMusicSearcher, metadata, false).await?
        }
        LyricsProvider::AmllTtmlDb => {
            search_with_refinement(&AmllTtmlDbSearcher, metadata, false).await?
        }
    };

    Ok(results
        .into_iter()
        .map(|result| to_candidate(provider, result))
        .collect())
}

pub(super) async fn lookup_qq_candidate(
    reference: &str,
) -> Result<Option<LyricsCandidate>, SearchError> {
    let Some(mut result) = resolve_qq_track(reference).await? else {
        return Ok(None);
    };
    result.match_type = Some(MatchType::Perfect);
    Ok(Some(to_candidate(LyricsProvider::QqMusic, result)))
}

fn to_candidate(provider: LyricsProvider, result: SearchResult) -> LyricsCandidate {
    LyricsCandidate {
        provider,
        provider_track_id: result.id,
        numeric_id: result.numeric_id,
        title: result.title,
        artists: result.artists,
        album: result.album,
        duration_ms: result.duration_ms,
        match_score: result.match_type.map_or(0, |value| value as i32),
    }
}

pub(super) async fn fetch_candidate_raw_lyrics(
    candidate: &LyricsCandidate,
) -> Result<Option<String>, SearchError> {
    let source = match candidate.provider {
        LyricsProvider::QqMusic => Searchers::QQMusic,
        LyricsProvider::NetEase => Searchers::Netease,
        LyricsProvider::Kugou => Searchers::Kugou,
        LyricsProvider::Lrclib => Searchers::LRCLIB,
        LyricsProvider::SodaMusic => Searchers::SodaMusic,
        LyricsProvider::AmllTtmlDb => Searchers::AmllTtmlDb,
    };
    let mut track = SearchResult::new(
        source,
        candidate.title.clone(),
        candidate.artists.clone(),
        candidate.album.clone(),
        candidate.duration_ms,
        candidate.provider_track_id.clone(),
    );
    track.numeric_id = candidate.numeric_id;
    fetch_result_lyrics(&track).await
}

pub(super) async fn fetch_hint_lyrics(
    track: &TrackMetadata,
    hint: &LyricsLookupHint,
) -> Option<FetchedLyrics> {
    let source = match hint.provider {
        LyricsProvider::QqMusic => Searchers::QQMusic,
        LyricsProvider::NetEase => Searchers::Netease,
        // Only these two playback sources provide track IDs for direct lookup.
        LyricsProvider::Kugou
        | LyricsProvider::Lrclib
        | LyricsProvider::SodaMusic
        | LyricsProvider::AmllTtmlDb => return None,
    };
    let mut result = SearchResult::new(
        source,
        track.title.clone(),
        track.artists.clone(),
        track.album.clone().unwrap_or_default(),
        track.duration_ms.and_then(|value| value.try_into().ok()),
        hint.provider_track_id.clone(),
    );
    if source == Searchers::QQMusic {
        result.numeric_id = hint.provider_track_id.parse().ok();
    }
    // Failed or stale hints fall back to metadata search.
    let raw_lyrics = fetch_result_lyrics(&result)
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
        amll_ttml_db::AmllTtmlDbSearcher, kugou::KugouSearcher, lrclib::LRCLIBSearcher,
        netease::NeteaseSearcher, qq_music::QQMusicSearcher, search_for_best_result_with_match,
        soda_music::SodaMusicSearcher,
    };

    let result = match provider {
        LyricsProvider::QqMusic => {
            search_for_best_result_with_match(&QQMusicSearcher, metadata, MatchType::Medium).await?
        }
        LyricsProvider::NetEase => {
            search_for_best_result_with_match(&NeteaseSearcher, metadata, MatchType::Medium).await?
        }
        LyricsProvider::Kugou => {
            search_for_best_result_with_match(&KugouSearcher, metadata, MatchType::Medium).await?
        }
        LyricsProvider::Lrclib => {
            search_for_best_result_with_match(&LRCLIBSearcher, metadata, MatchType::Medium).await?
        }
        LyricsProvider::SodaMusic => {
            search_for_best_result_with_match(&SodaMusicSearcher, metadata, MatchType::Medium)
                .await?
        }
        LyricsProvider::AmllTtmlDb => {
            search_for_best_result_with_match(&AmllTtmlDbSearcher, metadata, MatchType::Medium)
                .await?
        }
    };

    let Some(result) = result else {
        return Ok(None);
    };
    if is_another_song(metadata, &result) {
        // The provider does not hold this track; the next source in the order is
        // asked instead.
        return Ok(None);
    }
    let Some(raw_lyrics) = fetch_result_lyrics(&result).await? else {
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

/// Returns whether a provider's best result is another song of the same artist.
///
/// `compare_track` rescales its score by the fields the comparison lacked, so a
/// result with a different title still scores `MatchType::Medium` when the album
/// and the duration are unknown. That is how Kugou answers a title it does not
/// hold — with another song of the same artist, which would be displayed as this
/// track's lyrics — so the name and the artist are read on their own, before the
/// score is trusted.
pub(super) fn is_another_song(
    track: &lyrics_helper::models::TrackMetadata,
    result: &SearchResult,
) -> bool {
    use lyrics_helper::searchers::compare_helper::{
        ArtistMatchType, NameMatchType, compare_artist, compare_name,
    };

    let name = compare_name(track.title.as_deref(), Some(result.title.as_str()));
    let artist = compare_artist(
        track.artists.as_deref().unwrap_or_default(),
        &result.artists,
    );

    matches!(name, Some(NameMatchType::NoMatch)) || matches!(artist, Some(ArtistMatchType::NoMatch))
}

async fn fetch_result_lyrics(track: &SearchResult) -> Result<Option<String>, SearchError> {
    Ok(fetch_lyrics(track)
        .await?
        .as_ref()
        .and_then(display_payload))
}

pub(super) fn display_payload(lyrics: &ProviderLyrics) -> Option<String> {
    if lyrics.source == Searchers::Netease {
        let rows = lyrics
            .originals
            .iter()
            .find(|lyric| lyric.raw_type == LyricsRawTypes::Lrc)?;
        // NetEase's LRC spells the displayed text; YRC supplies its word timing.
        // The row translation is appended after the word-timed translation.
        let translation = lyrics
            .translations
            .iter()
            .rev()
            .find(|lyric| lyric.raw_type == LyricsRawTypes::Lrc)
            .or_else(|| lyrics.translations.first())
            .map(|lyric| lyric.text.as_str());
        if let Some(words) = lyrics
            .originals
            .iter()
            .find(|lyric| lyric.raw_type == LyricsRawTypes::Yrc)
        {
            return Some(combine_word_timing(&words.text, &rows.text, translation));
        }
        return Some(combine_lyrics_with_translation(&rows.text, translation));
    }

    let original = lyrics
        .originals
        .iter()
        .find(|lyric| lyric.raw_type != LyricsRawTypes::Unknown)?;
    let translation = lyrics.translations.first().map(|lyric| lyric.text.as_str());
    Some(combine_lyrics_with_translation(&original.text, translation))
}
