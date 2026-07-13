// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! GTK-independent presentation state and playback clock calculations.

use std::time::Instant;

use floatlyrics_core::{
    i18n::{Language, Message, Text},
    track::TrackMetadata,
};
use floatlyrics_lyrics::lyrics::{
    TimedLine, TimedSyllable, active_line_index, line_index_at_or_before,
};

use crate::{
    config::AppConfig,
    mpris::{PlaybackStatus, SpotifyPlayerState},
};

#[derive(Debug, Clone, Default)]
pub(super) struct LyricsDisplayState {
    pub(super) track_fingerprint: Option<String>,
    pub(super) lines: Vec<TimedLine>,
    pub(super) status_message: Option<Message>,
}

#[derive(Clone)]
pub(super) struct PlaybackSnapshot {
    pub(super) state: SpotifyPlayerState,
    pub(super) received_at: Instant,
}

#[derive(Debug, Clone, Default)]
pub(super) struct KaraokeRenderState {
    pub(super) text: String,
    pub(super) syllables: Vec<TimedSyllable>,
    pub(super) position_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub(super) struct LyricSlotText {
    pub(super) text: String,
    pub(super) karaoke: Option<KaraokeRenderState>,
    pub(super) romanization: String,
    pub(super) translation: String,
}

impl LyricSlotText {
    fn empty() -> Self {
        Self::default()
    }

    pub(super) fn message(message: &str) -> Self {
        Self {
            text: message.to_string(),
            karaoke: None,
            romanization: String::new(),
            translation: String::new(),
        }
    }
}

pub(super) struct LyricsFrame {
    pub(super) key: String,
    pub(super) content: LyricSlotText,
}

pub(super) fn lyrics_frame(
    state: &LyricsDisplayState,
    config: &AppConfig,
    position_ms: Option<u64>,
    language: Language,
) -> LyricsFrame {
    if let Some(message) = &state.status_message {
        return status_frame(message, language);
    }
    if state.lines.is_empty() {
        return status_frame(&Message::Text(Text::WaitingForLyrics), language);
    }
    let Some(position_ms) = position_ms else {
        return status_frame(&Message::Text(Text::WaitingForPosition), language);
    };

    let index = active_line_index(&state.lines, position_ms, config.lyrics.offset_ms)
        .or_else(|| line_index_at_or_before(&state.lines, position_ms, config.lyrics.offset_ms));
    match index {
        Some(index) => LyricsFrame {
            key: format!("line:{index}"),
            content: current_line_text(state.lines.get(index), config, position_ms),
        },
        None => LyricsFrame {
            key: "before-first-line".to_string(),
            content: LyricSlotText::message("…"),
        },
    }
}

fn status_frame(message: &Message, language: Language) -> LyricsFrame {
    LyricsFrame {
        key: format!("status:{}", message.key()),
        content: LyricSlotText::message(&message.render(language)),
    }
}

fn line_text(line: Option<&TimedLine>, config: &AppConfig) -> LyricSlotText {
    let Some(line) = line else {
        return LyricSlotText::empty();
    };
    let text = line.text.trim().to_string();
    let translation = if config.lyrics.show_translation
        && let Some(translation) = line.translation.as_deref().map(str::trim)
        && !translation.is_empty()
        && !is_placeholder_text(translation)
    {
        translation.to_string()
    } else {
        String::new()
    };
    let romanization = if config.lyrics.show_romanization
        && let Some(romanization) = line.romanization.as_deref().map(str::trim)
        && !romanization.is_empty()
    {
        romanization.to_string()
    } else {
        String::new()
    };
    LyricSlotText {
        text,
        karaoke: None,
        romanization,
        translation,
    }
}

fn current_line_text(
    line: Option<&TimedLine>,
    config: &AppConfig,
    position_ms: u64,
) -> LyricSlotText {
    let mut value = line_text(line, config);
    let Some(line) = line else {
        return value;
    };
    if !line.syllables.is_empty() {
        value.karaoke = Some(KaraokeRenderState {
            text: line.text.clone(),
            syllables: line.syllables.clone(),
            position_ms: adjusted_position_ms(position_ms, config.lyrics.offset_ms),
        });
    }
    value
}

fn adjusted_position_ms(position_ms: u64, offset_ms: i64) -> u64 {
    (position_ms as i128 + offset_ms as i128).clamp(0, u64::MAX as i128) as u64
}

pub(super) fn syllable_progress(syllable: &TimedSyllable, position_ms: u64) -> f64 {
    let duration = syllable.end_ms.saturating_sub(syllable.start_ms);
    if duration == 0 {
        return 1.0;
    }
    position_ms.saturating_sub(syllable.start_ms).min(duration) as f64 / duration as f64
}

fn is_placeholder_text(value: &str) -> bool {
    let normalized = value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    matches!(normalized.as_str(), "//" | "/" | "／" | "／／")
}

pub(super) fn effective_position_ms(snapshot: &PlaybackSnapshot) -> Option<u64> {
    let base = snapshot.state.position_ms?;
    let position = match snapshot.state.playback_status {
        PlaybackStatus::Playing => {
            base.saturating_add(snapshot.received_at.elapsed().as_millis() as u64)
        }
        _ => base,
    };
    Some(
        snapshot
            .state
            .track
            .as_ref()
            .and_then(|track| track.duration_ms)
            .map_or(position, |duration| position.min(duration)),
    )
}

pub(super) fn apply_position_sample(
    snapshot: &mut PlaybackSnapshot,
    track_identity: Option<&str>,
    position_ms: u64,
    sampled_at: Instant,
) -> bool {
    let current_identity = snapshot
        .state
        .track
        .as_ref()
        .map(TrackMetadata::playback_identity);
    if current_identity.as_deref() != track_identity {
        return false;
    }
    snapshot.state.position_ms = Some(position_ms);
    snapshot.received_at = sampled_at;
    true
}

#[cfg(test)]
#[path = "../test/model_test.rs"]
mod tests;
