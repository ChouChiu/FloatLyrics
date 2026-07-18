// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Cached lyrics selection, parsing, and refresh policy.

use std::sync::mpsc;

use floatlyrics_core::i18n::{Message, Text};
use floatlyrics_lyrics::{
    cache::CachedLyrics,
    lyrics::{LyricsProvider, SearchPlan, timed_lines_from_raw},
};

use crate::{backend::model::LyricsDisplayState, shared::runtime::LyricsRuntimeConfig};

use super::romanization::{RomanizationEvent, spawn_local_romanization};

pub(super) fn active_provider_order(config: &LyricsRuntimeConfig) -> Vec<LyricsProvider> {
    SearchPlan::new(config.provider_order.clone())
        .providers()
        .to_vec()
}

pub(super) fn lyrics_state_from_cached(
    fingerprint: String,
    cached: &CachedLyrics,
    config: &LyricsRuntimeConfig,
    runtime: &tokio::runtime::Handle,
    romanization_sender: &mpsc::Sender<RomanizationEvent>,
) -> LyricsDisplayState {
    let lines = match timed_lines_from_raw(&cached.raw_lyrics) {
        Ok(lines) => lines,
        Err(error) => {
            return LyricsDisplayState {
                track_fingerprint: Some(fingerprint),
                status_message: Some(Message::Detail(Text::LyricsParseError, error.to_string())),
                ..LyricsDisplayState::default()
            };
        }
    };

    if config.show_romanization {
        spawn_local_romanization(
            runtime,
            romanization_sender.clone(),
            fingerprint.clone(),
            lines.clone(),
            config.chinese_romanization,
        );
    }

    let status_message = if lines.is_empty() {
        Some(Message::Text(Text::CachedLyricsNotSynced))
    } else {
        None
    };

    LyricsDisplayState {
        track_fingerprint: Some(fingerprint),
        lines,
        status_message,
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
    cached: &CachedLyrics,
    state: &LyricsDisplayState,
    config: &LyricsRuntimeConfig,
) -> bool {
    !cached.manually_selected && config.show_translation && !has_cached_translation(state)
}

#[cfg(test)]
#[path = "../../../test/lyrics_cache_loading_test.rs"]
mod tests;
