// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Cached lyrics selection, parsing, and refresh policy.

use floatlyrics_core::i18n::{Message, Text};
use floatlyrics_lyrics::{
    cache::CachedLyrics,
    lyrics::{LyricsProvider, SearchPlan, segment_lines_into_words, timed_lines_from_raw},
};

use crate::{backend::model::LyricsDisplayState, shared::runtime::LyricsRuntimeConfig};

pub(super) fn active_provider_order(config: &LyricsRuntimeConfig) -> Vec<LyricsProvider> {
    SearchPlan::new(config.provider_order.clone())
        .providers()
        .to_vec()
}

#[derive(Debug)]
pub(in crate::backend::controller) struct PreparedLyrics {
    pub(super) state: LyricsDisplayState,
    pub(super) provider: LyricsProvider,
    pub(super) provider_track_id: Option<String>,
    pub(super) manually_selected: bool,
}

impl PreparedLyrics {
    pub(super) fn parse(fingerprint: String, cached: CachedLyrics) -> Self {
        let CachedLyrics {
            raw_lyrics,
            artists,
            provider,
            provider_track_id,
            manually_selected,
            ..
        } = cached;
        let mut state = LyricsDisplayState {
            track_fingerprint: Some(fingerprint),
            ..LyricsDisplayState::default()
        };
        match timed_lines_from_raw(&raw_lyrics, &artists) {
            Ok(mut lines) => {
                segment_lines_into_words(&mut lines);
                if lines.is_empty() {
                    state.status_message = Some(Message::Text(Text::CachedLyricsNotSynced));
                }
                state.lines = lines;
                state.credited_artists = artists;
            }
            Err(error) => {
                state.status_message =
                    Some(Message::Detail(Text::LyricsParseError, error.to_string()));
            }
        }
        Self {
            state,
            provider,
            provider_track_id,
            manually_selected,
        }
    }
}

fn has_cached_translation(state: &LyricsDisplayState) -> bool {
    state.lines.iter().any(|line| {
        line.translation
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    })
}

pub(super) fn should_refresh_translation(
    lyrics: &PreparedLyrics,
    config: &LyricsRuntimeConfig,
) -> bool {
    !lyrics.manually_selected && config.show_translation && !has_cached_translation(&lyrics.state)
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
