// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Pure settings state transitions, independent from GTK widgets.

use floatlyrics_core::i18n::{Language, Text};

use crate::shared::config::{AppConfig, ChineseRomanizationMode, LyricsProvider, WindowPosition};

#[derive(Debug, Clone, PartialEq, Eq)]
enum SaveStatus {
    Automatic,
    Saved,
    Failed(String),
}

impl SaveStatus {
    fn render(&self, language: Language) -> String {
        match self {
            Self::Automatic => language.text(Text::ChangesSavedAutomatically).to_string(),
            Self::Saved => language.text(Text::Saved).to_string(),
            Self::Failed(error) => language.detail(Text::SaveFailed, error),
        }
    }
}

#[derive(Debug)]
pub(super) struct SaveTracker {
    revision: u64,
    status: SaveStatus,
}

impl Default for SaveTracker {
    fn default() -> Self {
        Self {
            revision: 0,
            status: SaveStatus::Automatic,
        }
    }
}

impl SaveTracker {
    pub(super) fn begin_save(&mut self) -> u64 {
        self.revision = self.revision.wrapping_add(1);
        self.status = SaveStatus::Automatic;
        self.revision
    }

    pub(super) fn complete(&mut self, revision: u64, result: Result<(), String>) -> bool {
        if revision != self.revision {
            return false;
        }
        self.status = match result {
            Ok(()) => SaveStatus::Saved,
            Err(error) => SaveStatus::Failed(error),
        };
        true
    }

    pub(super) fn render(&self, language: Language) -> String {
        self.status.render(language)
    }

    pub(super) fn is_error(&self) -> bool {
        matches!(self.status, SaveStatus::Failed(_))
    }
}

#[derive(Debug)]
pub(in crate::frontend) enum ConfigChange {
    Language(Language),
    Offset(i64),
    Translation(bool),
    Romanization(bool),
    ChineseRomanization(ChineseRomanizationMode),
    AppleMusicStyle(bool),
    Width(i32),
    RememberPosition(bool),
    WindowPosition(WindowPosition),
    Margin(i32),
    PanelHeight(i32),
    Opacity(f64),
    Fonts(Vec<String>),
    ProviderOrder(Vec<LyricsProvider>),
    LyricFontSize(i32),
    TranslationFontSize(i32),
    RomanizationFontSize(i32),
    PlayedColor(String),
    UnplayedColor(String),
    TranslationColor(String),
    RomanizationColor(String),
}

impl ConfigChange {
    pub(super) fn apply(self, config: &mut AppConfig) {
        match self {
            Self::Language(value) => config.general.language = value,
            Self::Offset(value) => config.lyrics.offset_ms = value,
            Self::Translation(value) => config.lyrics.show_translation = value,
            Self::Romanization(value) => config.lyrics.show_romanization = value,
            Self::ChineseRomanization(value) => config.lyrics.chinese_romanization = value,
            Self::AppleMusicStyle(value) => config.lyrics.apple_music_style = value,
            Self::Width(value) => config.window.width = value,
            Self::RememberPosition(value) => {
                config.window.remember_position = value;
                if !value {
                    config.window.position = None;
                }
            }
            Self::WindowPosition(value) => {
                if config.window.remember_position {
                    config.window.position = Some(value);
                }
            }
            Self::Margin(value) => config.window.margin = value,
            Self::PanelHeight(value) => config.window.bottom_panel_height = value,
            Self::Opacity(value) => config.window.opacity = value,
            Self::Fonts(value) => config.lyrics.font_order = value,
            Self::ProviderOrder(value) => config.lyrics.provider_order = value,
            Self::LyricFontSize(value) => config.lyrics.lyric_font_size = value,
            Self::TranslationFontSize(value) => config.lyrics.translation_font_size = value,
            Self::RomanizationFontSize(value) => config.lyrics.romanization_font_size = value,
            Self::PlayedColor(value) => config.lyrics.played_color = value,
            Self::UnplayedColor(value) => config.lyrics.unplayed_color = value,
            Self::TranslationColor(value) => config.lyrics.translation_color = value,
            Self::RomanizationColor(value) => config.lyrics.romanization_color = value,
        }
    }
}

#[cfg(test)]
#[path = "../../test/settings_state_test.rs"]
mod tests;
