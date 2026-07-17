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
    pub background: Option<String>,
}

/// A source-text fragment and the reading displayed directly below it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomanizationSegment {
    /// Fragment from the original lyrics.
    pub text: String,
    /// Locally generated Latin-script reading, or an empty string for punctuation.
    pub romanization: String,
}

/// Timed fragment within a [`TimedLine`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimedSyllable {
    /// Fragment start time relative to the track, in milliseconds.
    pub start_ms: u64,
    /// Exclusive fragment end time, in milliseconds.
    pub end_ms: u64,
    /// Fragment text.
    pub text: String,
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
