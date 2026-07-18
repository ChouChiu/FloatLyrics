// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Lightweight, runtime-switchable user-interface translations.
//!
//! Catalogues are loaded from JSON resources at runtime. Widgets subscribe to
//! [`I18n`](crate::i18n::I18n) and update
//! in place whenever the selected language changes.

use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    env,
    rc::Rc,
};

mod catalogue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Language supported by the compiled translation catalogue.
pub enum Language {
    /// English (`en`).
    #[serde(rename = "en")]
    English,
    /// Simplified Chinese (`zh-CN`).
    #[serde(rename = "zh-CN")]
    SimplifiedChinese,
    /// Traditional Chinese (`zh-TW`).
    #[serde(rename = "zh-TW")]
    TraditionalChinese,
}

impl Default for Language {
    fn default() -> Self {
        Self::detect()
    }
}

impl Language {
    /// Languages in their stable settings-menu order.
    pub const ALL: [Self; 3] = [
        Self::English,
        Self::SimplifiedChinese,
        Self::TraditionalChinese,
    ];

    /// Returns the canonical locale code persisted in configuration.
    pub const fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::SimplifiedChinese => "zh-CN",
            Self::TraditionalChinese => "zh-TW",
        }
    }

    /// Returns the language name written in that language.
    pub fn display_name(self) -> &'static str {
        self.text(Text::LanguageName)
    }

    /// Detects a language from `LC_ALL`, `LC_MESSAGES`, or `LANG`.
    pub fn detect() -> Self {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(|name| env::var(name).ok().filter(|value| !value.is_empty()))
            .map_or(Self::English, |locale| Self::from_locale(&locale))
    }

    /// Maps a POSIX or BCP 47-like locale to the closest supported language.
    pub fn from_locale(locale: &str) -> Self {
        let locale = locale.to_ascii_lowercase().replace('_', "-");
        if !locale.starts_with("zh") {
            return Self::English;
        }
        if ["hant", "tw", "hk", "mo"]
            .iter()
            .any(|marker| locale.split(['-', '.']).any(|part| part == *marker))
        {
            Self::TraditionalChinese
        } else {
            Self::SimplifiedChinese
        }
    }

    /// Looks up a runtime catalogue entry.
    ///
    /// The validated catalogue is loaded once on first use.
    pub fn text(self, key: Text) -> &'static str {
        catalogue::text(self, key)
    }

    /// Renders a catalogue entry followed by diagnostic detail.
    pub fn detail(self, key: Text, detail: &str) -> String {
        format!(
            "{}{}{}",
            self.text(key),
            self.text(Text::DetailSeparator),
            detail
        )
    }

    /// Renders the localized manual-search result count.
    pub fn candidates_found(self, count: usize) -> String {
        let key = if count == 1 {
            Text::CandidatesFoundOne
        } else {
            Text::CandidatesFoundMany
        };
        self.text(key).replace("{count}", &count.to_string())
    }
}

/// Verifies that every supported runtime catalogue can be loaded completely.
///
/// `FLOATLYRICS_LOCALE_DIR` is checked first, followed by the development
/// resource directory and each directory in `XDG_DATA_DIRS`.
///
/// # Errors
///
/// Returns an error naming the first language whose JSON file is missing,
/// malformed, or missing a key declared by `Text`.
pub fn validate_catalogues() -> anyhow::Result<()> {
    catalogue::validate_catalogues()
}

type Listener = Rc<dyn Fn(Language)>;

#[derive(Clone)]
/// Observable runtime language selection for user-interface widgets.
pub struct I18n {
    language: Rc<Cell<Language>>,
    listeners: Rc<RefCell<Vec<Listener>>>,
}

impl I18n {
    /// Creates a catalogue selection using `language`.
    pub fn new(language: Language) -> Self {
        Self {
            language: Rc::new(Cell::new(language)),
            listeners: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Returns the currently selected language.
    pub fn language(&self) -> Language {
        self.language.get()
    }

    /// Looks up `key` in the currently selected language.
    pub fn text(&self, key: Text) -> &'static str {
        self.language().text(key)
    }

    /// Selects a language and notifies subscribers when it changed.
    pub fn set_language(&self, language: Language) {
        if self.language.replace(language) == language {
            return;
        }
        let listeners = self.listeners.borrow().clone();
        for listener in listeners {
            listener(language);
        }
    }

    /// Registers a listener and immediately invokes it with the current value.
    ///
    /// The catalogue retains the listener for its own lifetime.
    pub fn subscribe(&self, listener: impl Fn(Language) + 'static) {
        let listener: Listener = Rc::new(listener);
        listener(self.language());
        self.listeners.borrow_mut().push(listener);
    }
}

macro_rules! define_text_keys {
    ($($variant:ident),+ $(,)?) => {
        /// Key identifying a user-visible entry in the translation catalogue.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[allow(missing_docs)]
        pub enum Text {
            $($variant),+
        }

        impl Text {
            const ALL: &[Self] = &[$(Self::$variant),+];

            const fn key(self) -> &'static str {
                match self {
                    $(Self::$variant => stringify!($variant)),+
                }
            }
        }
    };
}

define_text_keys!(
    SettingsWindowTitle,
    ChangesSavedAutomatically,
    Saved,
    SaveFailed,
    General,
    Display,
    LyricsSources,
    GeneralTitle,
    GeneralDescription,
    Language,
    LanguageDescription,
    GlobalOffset,
    GlobalOffsetDescription,
    ShowTranslation,
    ShowTranslationDescription,
    ShowRomanization,
    ShowRomanizationDescription,
    ChineseRomanization,
    ChineseRomanizationDescription,
    RomanizationAutomatic,
    MandarinPinyin,
    CantoneseJyutping,
    CantoneseJyutpingWithoutTones,
    DisplayTitle,
    DisplayDescription,
    AppleMusicStyle,
    AppleMusicStyleDescription,
    PanelWidth,
    PanelWidthDescription,
    RememberWindowPosition,
    RememberWindowPositionDescription,
    BottomMargin,
    BottomMarginDescription,
    BottomPanelHeight,
    BottomPanelHeightDescription,
    BackgroundOpacity,
    BackgroundOpacityDescription,
    Fonts,
    FontsDescription,
    ChangeFonts,
    FontWindowTitle,
    AvailableFonts,
    FontOrder,
    MoveFontUp,
    MoveFontDown,
    RemoveFont,
    Done,
    LyricFontSize,
    LyricFontSizeDescription,
    TranslationFontSize,
    TranslationFontSizeDescription,
    RomanizationFontSize,
    RomanizationFontSizeDescription,
    PlayedColor,
    PlayedColorDescription,
    UnplayedColor,
    UnplayedColorDescription,
    TranslationColor,
    TranslationColorDescription,
    RomanizationColor,
    RomanizationColorDescription,
    SourcesTitle,
    SourcesDescription,
    SearchPriority,
    SearchPriorityDescription,
    QqThenNetEase,
    NetEaseThenQq,
    OpenAbout,
    AboutWindowTitle,
    About,
    Acknowledgements,
    AppSummary,
    Version,
    Copyright,
    LicenseStatement,
    ProjectWebsite,
    AcknowledgementsTitle,
    InspiredByLyricsX,
    VisitLyricsX,
    ManualSearchTooltip,
    OpenSettingsTooltip,
    OpenSpotify,
    SpotifyAttention,
    WaitingForMetadata,
    WaitingForLyrics,
    WaitingForPosition,
    LyricsCacheError,
    SearchingLyrics,
    LyricsParseError,
    CachedLyricsNotSynced,
    LyricsCacheWriteError,
    LyricsSearchFailed,
    DownloadedLyricsNotStored,
    NoLyricsFound,
    SongTitle,
    Artist,
    Search,
    Title,
    SelectCandidatePreview,
    SearchAfterPlayback,
    ApplySelectedLyrics,
    ManualSearchTitle,
    LoadingPreview,
    NoTrackPlaying,
    EnterSongTitle,
    SearchingCandidates,
    SearchingProviders,
    TrackChanged,
    LyricsApplied,
    ApplyFailed,
    NoCandidates,
    SearchFailed,
    LyricsSearchPreviewFailed,
    PreviewReady,
    CandidateUnavailable,
    PreviewLoadFailed,
    LoadingFailed,
    MatchScore,
    OpenSourceTitle,
    OpenSourceDescription,
    LicenseTexts,
    CloseTooltip,
    LanguageName,
    DetailSeparator,
    CandidatesFoundOne,
    CandidatesFoundMany,
    ProviderNameQqMusic,
    ProviderNameNetEase,
);

#[derive(Debug, Clone, PartialEq, Eq)]
/// Localizable status message with optional runtime detail.
pub enum Message {
    /// A catalogue entry without runtime data.
    Text(Text),
    /// A catalogue entry followed by diagnostic detail.
    Detail(Text, String),
}

impl Message {
    /// Renders the message in `language`.
    pub fn render(&self, language: Language) -> String {
        match self {
            Self::Text(key) => language.text(*key).to_string(),
            Self::Detail(key, detail) => language.detail(*key, detail),
        }
    }

    /// Produces a stable key suitable for UI transition identity.
    pub fn key(&self) -> String {
        match self {
            Self::Text(key) => format!("{key:?}"),
            Self::Detail(key, detail) => format!("{key:?}:{detail}"),
        }
    }
}

#[cfg(test)]
#[path = "test/i18n_test.rs"]
mod tests;
