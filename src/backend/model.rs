// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Backend playback state and deterministic clock calculations.

use std::time::Instant;

use floatlyrics_core::{
    i18n::{Language, Message, Text},
    track::TrackMetadata,
};
use floatlyrics_lyrics::lyrics::{
    TimedLine, TimedSyllable, active_line_index, line_index_at_or_before,
};

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

/// A difference between the local clock and a player sample larger than this
/// is a seek rather than drift.
pub(super) const PLAYBACK_JUMP_THRESHOLD_MS: u64 = 750;

#[derive(Clone)]
pub(super) struct PlaybackSnapshot {
    pub(super) state: PlayerState,
    pub(super) received_at: Instant,
    /// Position the local clock had already reached when the player sampled a
    /// slightly earlier one.
    ///
    /// Players sample `Position` coarsely and late, so a sample can trail the
    /// local clock by a few frames. Rewinding to it would draw the line before a
    /// boundary again for a moment, so the clock holds here until the sample
    /// catches up. Only set while playing and for drift below
    /// [`PLAYBACK_JUMP_THRESHOLD_MS`].
    pub(super) position_floor_ms: Option<u64>,
}

impl PlaybackSnapshot {
    /// Anchors the clock of `state` at `received_at`, continuing from
    /// `previous` without rewinding when the new position only trails it by
    /// sampling drift.
    pub(super) fn following(
        previous: Option<&PlaybackSnapshot>,
        state: PlayerState,
        received_at: Instant,
    ) -> Self {
        let position_floor_ms = previous.and_then(|previous| {
            let same_track = previous
                .state
                .track
                .as_ref()
                .map(TrackMetadata::playback_identity)
                == state.track.as_ref().map(TrackMetadata::playback_identity);
            same_track
                .then(|| {
                    drift_floor_ms(
                        previous,
                        state.playback_status,
                        state.position_ms,
                        received_at,
                    )
                })
                .flatten()
        });
        Self {
            state,
            received_at,
            position_floor_ms,
        }
    }
}

/// Returns the position `previous` predicts at `sampled_at` when the sampled
/// position trails it by sampling drift during uninterrupted playback.
fn drift_floor_ms(
    previous: &PlaybackSnapshot,
    status: PlaybackStatus,
    sampled_ms: Option<u64>,
    sampled_at: Instant,
) -> Option<u64> {
    if previous.state.playback_status != PlaybackStatus::Playing
        || status != PlaybackStatus::Playing
    {
        return None;
    }
    let predicted = position_at_ms(previous, sampled_at)?;
    let sampled = sampled_ms?;
    (sampled < predicted && predicted - sampled <= PLAYBACK_JUMP_THRESHOLD_MS).then_some(predicted)
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
                    syllables: presented_syllables(&line.syllables, config),
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
                        .map(|sung| presented_syllables(&sung.syllables, config))
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
            syllables: presented_syllables(&line.syllables, config),
            position_ms: adjusted_position_ms(position_ms, offset_ms),
        });
    }
    value
}

/// The words of a line as the views may show them.
///
/// The per-word romanization and furigana are generated only while
/// romanization is shown, and stay on the loaded lines once it is hidden, so
/// they are dropped here rather than left for each view to ignore.
fn presented_syllables(
    syllables: &[TimedSyllable],
    config: &LyricsRuntimeConfig,
) -> Vec<TimedSyllable> {
    let mut syllables = syllables.to_vec();
    if !config.show_romanization {
        for syllable in &mut syllables {
            syllable.romanization.clear();
            syllable.furigana.clear();
        }
    }
    syllables
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
    position_at_ms(snapshot, Instant::now())
}

/// Returns the position `snapshot` predicts for the instant `now`.
pub(super) fn position_at_ms(snapshot: &PlaybackSnapshot, now: Instant) -> Option<u64> {
    let base = snapshot.state.position_ms?;
    let position = match snapshot.state.playback_status {
        PlaybackStatus::Playing => {
            let elapsed = now.saturating_duration_since(snapshot.received_at);
            base.saturating_add(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        }
        _ => base,
    };
    let position = snapshot
        .position_floor_ms
        .map_or(position, |floor| position.max(floor));
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
        .is_some_and(|(old, new)| old.abs_diff(new) > PLAYBACK_JUMP_THRESHOLD_MS)
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
    snapshot.position_floor_ms = drift_floor_ms(
        snapshot,
        snapshot.state.playback_status,
        Some(position_ms),
        sampled_at,
    );
    snapshot.state.position_ms = Some(position_ms);
    snapshot.received_at = sampled_at;
    true
}

#[cfg(test)]
#[path = "../test/model_test.rs"]
mod tests;
