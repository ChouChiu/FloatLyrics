// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! GTK-independent presentation state and playback clock calculations.

use std::time::Instant;

use crate::{
    config::AppConfig,
    i18n::{Language, Message, Text},
    lyrics::{TimedLine, TimedSyllable, active_line_index, line_index_at_or_before},
    mpris::{PlaybackStatus, SpotifyPlayerState},
    track::TrackMetadata,
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
    let mut text = line.text.trim().to_string();
    if config.lyrics.show_translation
        && let Some(translation) = line.translation.as_deref().map(str::trim)
        && !translation.is_empty()
        && !is_placeholder_text(translation)
    {
        return LyricSlotText {
            text,
            karaoke: None,
            translation: translation.to_string(),
        };
    }
    if config.lyrics.show_romanization
        && let Some(romanization) = line.romanization.as_deref().map(str::trim)
        && !romanization.is_empty()
    {
        text = format!("{text}  /  {romanization}");
    }
    LyricSlotText {
        text,
        karaoke: None,
        translation: String::new(),
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

pub(super) fn progress_fraction(position_ms: Option<u64>, duration_ms: Option<u64>) -> Option<f64> {
    let position_ms = position_ms?;
    let duration_ms = duration_ms?;
    if duration_ms == 0 {
        return None;
    }
    Some((position_ms as f64 / duration_ms as f64).clamp(0.0, 1.0))
}

pub(super) fn progress_text(position_ms: Option<u64>, duration_ms: Option<u64>) -> Option<String> {
    let position = position_ms?;
    Some(match duration_ms {
        Some(duration) if duration > 0 => format!(
            "{} / {}",
            format_duration(position),
            format_duration(duration)
        ),
        _ => format_duration(position),
    })
}

fn format_duration(ms: u64) -> String {
    let total_seconds = ms / 1_000;
    format!("{}:{:02}", total_seconds / 60, total_seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn formats_progress_text() {
        assert_eq!(
            progress_text(Some(65_000), Some(185_000)).as_deref(),
            Some("1:05 / 3:05")
        );
        assert_eq!(progress_text(Some(5_000), None).as_deref(), Some("0:05"));
        assert_eq!(progress_text(None, Some(10_000)), None);
    }

    #[test]
    fn clamps_progress_fraction() {
        assert_eq!(progress_fraction(Some(50), Some(100)), Some(0.5));
        assert_eq!(progress_fraction(Some(150), Some(100)), Some(1.0));
        assert_eq!(progress_fraction(Some(50), Some(0)), None);
    }

    #[test]
    fn advances_local_clock_only_while_playing() {
        let playing = snapshot(PlaybackStatus::Playing, Duration::from_millis(1_500));
        let paused = snapshot(PlaybackStatus::Paused, Duration::from_millis(1_500));

        assert!(effective_position_ms(&playing).unwrap() >= 11_000);
        assert_eq!(effective_position_ms(&paused), Some(10_000));
    }

    #[test]
    fn authoritative_sample_reanchors_matching_track() {
        let mut snapshot = snapshot(PlaybackStatus::Playing, Duration::from_secs(2));
        let identity = snapshot.state.track.as_ref().unwrap().playback_identity();
        let sampled_at = Instant::now();

        assert!(apply_position_sample(
            &mut snapshot,
            Some(&identity),
            10_500,
            sampled_at,
        ));
        assert!(effective_position_ms(&snapshot).unwrap() < 10_600);
        assert_eq!(snapshot.received_at, sampled_at);
    }

    #[test]
    fn sample_from_another_track_is_ignored() {
        let mut snapshot = snapshot(PlaybackStatus::Playing, Duration::ZERO);
        let received_at = snapshot.received_at;

        assert!(!apply_position_sample(
            &mut snapshot,
            Some("another-track"),
            500,
            Instant::now(),
        ));
        assert_eq!(snapshot.state.position_ms, Some(10_000));
        assert_eq!(snapshot.received_at, received_at);
    }

    #[test]
    fn syllable_progress_is_clamped_to_its_time_range() {
        let syllable = TimedSyllable {
            start_ms: 1_000,
            end_ms: 1_500,
            text: "hello".to_string(),
        };

        assert_eq!(syllable_progress(&syllable, 900), 0.0);
        assert_eq!(syllable_progress(&syllable, 1_250), 0.5);
        assert_eq!(syllable_progress(&syllable, 1_700), 1.0);
    }

    #[test]
    fn placeholder_translation_is_hidden() {
        let mut line = test_line();
        line.translation = Some("//".to_string());
        let text = line_text(Some(&line), &AppConfig::default());
        assert_eq!(text.text, "Hello");
        assert!(text.translation.is_empty());

        line.translation = Some("你好".to_string());
        let text = line_text(Some(&line), &AppConfig::default());
        assert_eq!(text.translation, "你好");
    }

    #[test]
    fn lyric_frame_uses_stable_key_for_active_line() {
        let state = LyricsDisplayState {
            lines: vec![test_line()],
            ..LyricsDisplayState::default()
        };

        let frame = lyrics_frame(
            &state,
            &AppConfig::default(),
            Some(1_500),
            Language::English,
        );
        assert_eq!(frame.key, "line:0");
        assert_eq!(frame.content.text, "Hello");
    }

    #[test]
    fn adjusted_position_is_saturated_at_both_bounds() {
        assert_eq!(adjusted_position_ms(0, -1), 0);
        assert_eq!(adjusted_position_ms(u64::MAX, i64::MAX), u64::MAX);
    }

    fn snapshot(status: PlaybackStatus, elapsed: Duration) -> PlaybackSnapshot {
        PlaybackSnapshot {
            state: SpotifyPlayerState {
                bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
                playback_status: status,
                position_ms: Some(10_000),
                track: Some(TrackMetadata {
                    title: "Song".to_string(),
                    artists: vec!["Artist".to_string()],
                    album: None,
                    duration_ms: Some(20_000),
                    mpris_track_id: None,
                }),
            },
            received_at: Instant::now() - elapsed,
        }
    }

    fn test_line() -> TimedLine {
        TimedLine {
            start_ms: 1_000,
            end_ms: Some(2_000),
            text: "Hello".to_string(),
            syllables: Vec::new(),
            translation: None,
            romanization: None,
            background: None,
        }
    }
}
