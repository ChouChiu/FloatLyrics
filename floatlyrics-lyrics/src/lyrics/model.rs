// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Provider-neutral lyrics domain models.

use serde::{Deserialize, Serialize};

/// Lyrics source supported by search and persistence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LyricsProvider {
    /// QQ Music.
    QqMusic,
    /// NetEase Cloud Music.
    #[serde(rename = "netease")]
    NetEase,
}

/// Provider-specific track identifier suggested by a playback source.
///
/// Applications can use this hint to bypass fuzzy metadata search while still
/// falling back to normal provider search when the identifier is stale or the
/// provider is disabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsLookupHint {
    /// Lyrics provider that owns the identifier.
    pub provider: LyricsProvider,
    /// Provider-specific song identifier.
    pub provider_track_id: String,
}

impl LyricsProvider {
    /// Returns the default automatic search priority.
    pub fn default_order() -> Vec<Self> {
        vec![Self::QqMusic, Self::NetEase]
    }

    /// Returns the stable identifier used in configuration and storage.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::QqMusic => "qq-music",
            Self::NetEase => "netease",
        }
    }
}

impl std::fmt::Display for LyricsProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for LyricsProvider {
    type Err = LyricsProviderParseError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "qq-music" => Ok(Self::QqMusic),
            "netease" => Ok(Self::NetEase),
            _ => Err(LyricsProviderParseError(value.to_string())),
        }
    }
}

/// Error returned when a persisted provider name is unsupported.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unsupported lyrics provider: {0}")]
pub struct LyricsProviderParseError(String);

/// Which vocal part a line belongs to.
///
/// A provider that structures its transcription states the performer outright; a
/// provider that does not divides the lyrics with labelled rows instead, and such
/// a label is only read when it names an artist of the matched track. Everything
/// else resolves to [`Voice::Primary`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Voice {
    /// The main vocal part.
    #[default]
    Primary,
    /// The opposing part of a duet.
    Secondary,
}

/// One display line with optional word timing and secondary text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimedLine {
    /// Start time relative to the track, in milliseconds.
    pub start_ms: u64,
    /// Exclusive end time, when known.
    pub end_ms: Option<u64>,
    /// Primary lyrics text.
    pub text: String,
    /// Word or syllable timing used for karaoke highlighting.
    pub syllables: Vec<TimedSyllable>,
    /// Translated text, when available.
    pub translation: Option<String>,
    /// Romanized text, when available.
    pub romanization: Option<String>,
    /// Source fragments paired with locally generated readings for interlinear display.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub romanization_segments: Vec<RomanizationSegment>,
    /// Background-vocal text, when available.
    pub background: Option<BackgroundVocal>,
    /// Which vocal part sings this line.
    #[serde(default)]
    pub voice: Voice,
}

/// A part sung behind the line it belongs to.
///
/// A provider that structures its transcription states it outright; a provider
/// that does not writes it as a bracketed phrase, which the parsers read out of
/// the text. It carries its own timing, translation, and words because it is sung
/// at its own time inside the line rather than with it: a listener fills the
/// words it hears as it hears them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundVocal {
    /// Text that is sung behind the line.
    pub text: String,
    /// Translation of the background vocal, when the provider supplied one.
    pub translation: Option<String>,
    /// Start of the background vocal relative to the track, in milliseconds.
    pub start_ms: u64,
    /// Exclusive end of the background vocal, when the provider timed one.
    pub end_ms: Option<u64>,
    /// Words of the background vocal, when the provider timed them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub syllables: Vec<TimedSyllable>,
}

impl TimedLine {
    /// Returns whether the line carries usable word-level timing.
    #[must_use]
    pub fn is_word_timed(&self) -> bool {
        self.syllables
            .iter()
            .any(|syllable| !syllable.text.trim().is_empty())
    }

    /// Returns the latest time the line refers to.
    ///
    /// A line without an explicit end is bounded by its last timed word, so an
    /// echo timed to start where its parent runs out can be recognized.
    #[must_use]
    pub fn latest_time_ms(&self) -> u64 {
        let from_syllables = self
            .syllables
            .iter()
            .map(|syllable| syllable.end_ms)
            .max()
            .unwrap_or(self.start_ms);
        self.end_ms
            .unwrap_or(self.start_ms)
            .max(from_syllables)
            .max(self.start_ms)
    }
}

/// A source-text fragment and the reading displayed directly below it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomanizationSegment {
    /// Fragment from the original lyrics.
    pub text: String,
    /// Locally generated Latin-script reading, or an empty string for punctuation.
    pub romanization: String,
    /// Locally generated kana reading, for fragments written with characters
    /// that are read rather than spelled — empty for everything else.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub furigana: String,
}

/// Timed fragment within a [`TimedLine`].
///
/// The optional reading in [`TimedSyllable::romanization`] and the kana in
/// [`TimedSyllable::furigana`] are generated locally by
/// [`super::generate_local_romanization`]; fragments of the source text that have
/// no reading of their own keep them empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimedSyllable {
    /// Fragment start time relative to the track, in milliseconds.
    pub start_ms: u64,
    /// Exclusive fragment end time, in milliseconds.
    pub end_ms: u64,
    /// Fragment text.
    pub text: String,
    /// Locally generated reading for this fragment, or an empty string when the fragment has none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub romanization: String,
    /// Kana this fragment is read with, for fragments written with characters
    /// that are read rather than spelled — empty for everything else.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub furigana: String,
}

/// Successfully downloaded lyrics and their provider metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct FetchedLyrics {
    /// Provider that supplied the result.
    pub provider: LyricsProvider,
    /// Provider-specific track identifier, when available.
    pub provider_track_id: Option<String>,
    /// Provider-reported title.
    pub title: String,
    /// Provider-reported artists.
    pub artists: Vec<String>,
    /// Match quality assigned during search.
    pub score: f64,
    /// Original lyrics payload.
    pub raw_lyrics: String,
}

/// Search result that can be previewed or manually selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsCandidate {
    /// Provider that returned the candidate.
    pub provider: LyricsProvider,
    /// Provider-specific track identifier.
    pub provider_track_id: String,
    /// Numeric provider identifier, when required by its API.
    pub numeric_id: Option<i64>,
    /// Provider-reported title.
    pub title: String,
    /// Provider-reported artists.
    pub artists: Vec<String>,
    /// Provider-reported album, or an empty string.
    pub album: String,
    /// Provider-reported duration in milliseconds.
    pub duration_ms: Option<i32>,
    /// Integer match quality used to rank candidates.
    pub match_score: i32,
}
