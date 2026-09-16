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
    presentation::{
        KaraokeRenderState, LyricSlotText, LyricsDocument, LyricsFrame, PresentedLyricLine,
    },
    runtime::LyricsRuntimeConfig,
};

use super::mpris::{PlaybackStatus, PlayerState};

#[derive(Debug, Clone, Default)]
pub(super) struct LyricsDisplayState {
    pub(super) track_fingerprint: Option<String>,
    pub(super) lines: Vec<TimedLine>,
    pub(super) status_message: Option<Message>,
    /// Artists as the provider that supplied these lyrics credits them.
    ///
    /// A provider matches a release rather than a playback session, so its
    /// billing can name featured performers the playback source omits. Empty for
    /// lyrics resolved from a provider identifier the player itself suggested.
    pub(super) credited_artists: Vec<String>,
}

#[derive(Clone)]
pub(super) struct PlaybackSnapshot {
    pub(super) state: PlayerState,
    pub(super) received_at: Instant,
}

pub(super) fn lyrics_frame(
    state: &LyricsDisplayState,
    config: &LyricsRuntimeConfig,
    position_ms: Option<u64>,
    playing: bool,
    seeking: bool,
    language: Language,
    track_offset_ms: i64,
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

    let offset_ms = config.offset_ms.saturating_add(track_offset_ms);
    let index = active_line_index(&state.lines, position_ms, offset_ms)
        .or_else(|| line_index_at_or_before(&state.lines, position_ms, offset_ms));
    match index {
        Some(index) => LyricsFrame {
            key: format!("line:{index}"),
            content: current_line_text(state.lines.get(index), config, position_ms, offset_ms),
            position_ms: Some(adjusted_position_ms(position_ms, offset_ms)),
            playing,
            seeking,
        },
        None => LyricsFrame {
            key: "before-first-line".to_string(),
            content: LyricSlotText::message("…"),
            position_ms: Some(adjusted_position_ms(position_ms, offset_ms)),
            playing,
            seeking,
        },
    }
}

pub(super) fn lyrics_document(
    state: &LyricsDisplayState,
    config: &LyricsRuntimeConfig,
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
                let background = line.background.as_ref();
                PresentedLyricLine {
                    start_ms: line.start_ms,
                    end_ms: line.end_ms,
                    text: line.text.trim().to_string(),
                    syllables: line.syllables.clone(),
                    romanization: visible.romanization,
                    translation: visible.translation,
                    background: background
                        .map(|sung| sung.text.trim())
                        .filter(|value| !value.is_empty())
                        .unwrap_or_default()
                        .to_string(),
                    background_translation: background
                        .and_then(|sung| sung.translation.as_deref())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .unwrap_or_default()
                        .to_string(),
                    background_start_ms: background.map_or(line.start_ms, |sung| sung.start_ms),
                    background_end_ms: background.and_then(|sung| sung.end_ms),
                    background_syllables: background
                        .map(|sung| sung.syllables.clone())
                        .unwrap_or_default(),
                    voice: line.voice,
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

fn line_text(line: Option<&TimedLine>, config: &LyricsRuntimeConfig) -> LyricSlotText {
    let Some(line) = line else {
        return LyricSlotText::empty();
    };
    let text = line.text.trim().to_string();
    let translation = if config.show_translation
        && let Some(translation) = line.translation.as_deref().map(str::trim)
        && !translation.is_empty()
        && !is_placeholder_text(translation)
    {
        translation.to_string()
    } else {
        String::new()
    };
    let romanization = if config.show_romanization
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
    config: &LyricsRuntimeConfig,
    position_ms: u64,
    offset_ms: i64,
) -> LyricSlotText {
    let mut value = line_text(line, config);
    let Some(line) = line else {
        return value;
    };
    if !line.syllables.is_empty() {
        value.karaoke = Some(KaraokeRenderState {
            text: line.text.clone(),
            syllables: line.syllables.clone(),
            position_ms: adjusted_position_ms(position_ms, offset_ms),
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

pub(super) fn playback_jump_detected(
    previous: Option<&PlaybackSnapshot>,
    next_position_ms: Option<u64>,
    next: &PlayerState,
) -> bool {
    let Some(previous) = previous else {
        return true;
    };
    let previous_identity = previous
        .state
        .track
        .as_ref()
        .map(TrackMetadata::playback_identity);
    let next_identity = next.track.as_ref().map(TrackMetadata::playback_identity);
    if previous_identity != next_identity {
        return true;
    }
    effective_position_ms(previous)
        .zip(next_position_ms)
        .is_some_and(|(old, new)| old.abs_diff(new) > 750)
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
