// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Narrow runtime preferences shared with the playback backend.

use floatlyrics_core::i18n::Language;
use floatlyrics_lyrics::lyrics::{ChineseRomanizationMode, LyricsProvider};

use super::config::AppConfig;

/// Preferences required to load and present lyrics during playback.
///
/// Keeping this contract separate from [`AppConfig`] prevents the backend from
/// depending on unrelated persisted window, styling, and MPRIS settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LyricsRuntimeConfig {
    pub(crate) language: Language,
    pub(crate) offset_ms: i64,
    pub(crate) provider_order: Vec<LyricsProvider>,
    pub(crate) show_translation: bool,
    pub(crate) show_romanization: bool,
    pub(crate) chinese_romanization: ChineseRomanizationMode,
}

impl From<&AppConfig> for LyricsRuntimeConfig {
    fn from(config: &AppConfig) -> Self {
        Self {
            language: config.general.language,
            offset_ms: config.lyrics.offset_ms,
            provider_order: config.lyrics.provider_order.clone(),
            show_translation: config.lyrics.show_translation,
            show_romanization: config.lyrics.show_romanization,
            chinese_romanization: config.lyrics.chinese_romanization,
        }
    }
}

#[cfg(test)]
#[path = "../test/runtime_test.rs"]
mod tests;
