// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Backend playback state and deterministic clock calculations.

use std::time::Instant;

use floatlyrics_core::{
    i18n::{Language, Message, Text},
    track::TrackMetadata,
};
use floatlyrics_lyrics::lyrics::{TimedLine, active_line_index, line_index_at_or_before};

use crate::shared::{
    config::AppConfig,
    presentation::{
        KaraokeRenderState, LyricSlotText, LyricsDocument, LyricsFrame, PresentedLyricLine,
    },
};

use super::mpris::{PlaybackStatus, SpotifyPlayerState};

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

pub(super) fn lyrics_frame(
    state: &LyricsDisplayState,
    config: &AppConfig,
    position_ms: Option<u64>,
    playing: bool,
    seeking: bool,
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
            position_ms: Some(adjusted_position_ms(position_ms, config.lyrics.offset_ms)),
            playing,
            seeking,
        },
        None => LyricsFrame {
            key: "before-first-line".to_string(),
            content: LyricSlotText::message("…"),
            position_ms: Some(adjusted_position_ms(position_ms, config.lyrics.offset_ms)),
            playing,
            seeking,
        },
    }
}

pub(super) fn lyrics_document(
    state: &LyricsDisplayState,
    config: &AppConfig,
    revision: u64,
    duration_ms: Option<u64>,
) -> LyricsDocument {
    LyricsDocument {
        revision,
        duration_ms,
        lines: state
            .lines
            .iter()
            .map(|line| {
                let visible = line_text(Some(line), config);
                PresentedLyricLine {
                    start_ms: line.start_ms,
                    end_ms: line.end_ms,
                    text: line.text.trim().to_string(),
                    syllables: line.syllables.clone(),
                    romanization: visible.romanization,
                    translation: visible.translation,
                    background: line
                        .background
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .unwrap_or_default()
                        .to_string(),
                }
            })
            .collect(),
    }
}

fn status_frame(message: &Message, language: Language) -> LyricsFrame {
    LyricsFrame {
        key: format!("status:{}", message.key()),
        content: LyricSlotText::message(&message.render(language)),
        position_ms: None,
        playing: false,
        seeking: false,
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
