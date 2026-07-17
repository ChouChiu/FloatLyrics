// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! GTK-independent lyrics presentation contracts.

use serde::Serialize;

use floatlyrics_lyrics::lyrics::TimedSyllable;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct KaraokeRenderState {
    pub(crate) text: String,
    pub(crate) syllables: Vec<TimedSyllable>,
    pub(crate) position_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct LyricSlotText {
    pub(crate) text: String,
    pub(crate) karaoke: Option<KaraokeRenderState>,
    pub(crate) romanization: String,
    pub(crate) translation: String,
}

impl LyricSlotText {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn message(message: &str) -> Self {
        Self {
            text: message.to_string(),
            karaoke: None,
            romanization: String::new(),
            translation: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct PresentedLyricLine {
    pub(crate) start_ms: u64,
    pub(crate) end_ms: Option<u64>,
    pub(crate) text: String,
    pub(crate) syllables: Vec<TimedSyllable>,
    pub(crate) romanization: String,
    pub(crate) translation: String,
    pub(crate) background: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct LyricsDocument {
    pub(crate) revision: u64,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) lines: Vec<PresentedLyricLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct LyricsFrame {
    pub(crate) key: String,
    pub(crate) content: LyricSlotText,
    pub(crate) position_ms: Option<u64>,
    pub(crate) playing: bool,
    pub(crate) seeking: bool,
}
