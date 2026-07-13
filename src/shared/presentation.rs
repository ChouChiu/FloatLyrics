// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! GTK-independent lyrics presentation contracts.

use serde::Serialize;

use floatlyrics_lyrics::lyrics::TimedSyllable;

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct KaraokeRenderState {
    pub(crate) text: String,
    pub(crate) syllables: Vec<TimedSyllable>,
    pub(crate) position_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
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

pub(crate) struct LyricsFrame {
    pub(crate) key: String,
    pub(crate) content: LyricSlotText,
}
