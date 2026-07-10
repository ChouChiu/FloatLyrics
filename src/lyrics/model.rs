use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LyricsProvider {
    QqMusic,
    NetEase,
    LrcLib,
}

impl LyricsProvider {
    pub fn default_order() -> Vec<Self> {
        vec![Self::QqMusic, Self::NetEase]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::QqMusic => "qq-music",
            Self::NetEase => "netease",
            Self::LrcLib => "lrclib",
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
            "qq-music" | "qq" => Ok(Self::QqMusic),
            "netease" | "netease-cloud-music" => Ok(Self::NetEase),
            "lrclib" => Ok(Self::LrcLib),
            _ => Err(LyricsProviderParseError(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unsupported lyrics provider: {0}")]
pub struct LyricsProviderParseError(String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimedLine {
    pub start_ms: u64,
    pub end_ms: Option<u64>,
    pub text: String,
    pub syllables: Vec<TimedSyllable>,
    pub translation: Option<String>,
    pub romanization: Option<String>,
    pub background: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimedSyllable {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FetchedLyrics {
    pub provider: LyricsProvider,
    pub provider_track_id: Option<String>,
    pub title: String,
    pub artists: Vec<String>,
    pub score: f64,
    pub raw_lyrics: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsCandidate {
    pub provider: LyricsProvider,
    pub provider_track_id: String,
    pub numeric_id: Option<i64>,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration_ms: Option<i32>,
    pub match_score: i32,
}
