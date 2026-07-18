// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! CPU-bound romanization task and pure result reducer.

use std::sync::mpsc;

use floatlyrics_lyrics::lyrics::{
    ChineseRomanizationMode, TimedLine, generate_local_romanization_with_mode,
};

use crate::backend::model::LyricsDisplayState;

#[derive(Debug)]
pub(in crate::backend::controller) struct RomanizationEvent {
    pub(in crate::backend::controller) track_fingerprint: String,
    pub(in crate::backend::controller) chinese_mode: ChineseRomanizationMode,
    pub(in crate::backend::controller) lines: Vec<TimedLine>,
}

pub(super) fn spawn_local_romanization(
    runtime: &tokio::runtime::Handle,
    sender: mpsc::Sender<RomanizationEvent>,
    track_fingerprint: String,
    mut lines: Vec<TimedLine>,
    chinese_mode: ChineseRomanizationMode,
) {
    runtime.spawn_blocking(move || {
        generate_local_romanization_with_mode(&mut lines, chinese_mode);
        let _ = sender.send(RomanizationEvent {
            track_fingerprint,
            chinese_mode,
            lines,
        });
    });
}

pub(in crate::backend::controller) fn apply_romanization_event(
    event: RomanizationEvent,
    state: &mut LyricsDisplayState,
    current_chinese_mode: ChineseRomanizationMode,
) -> bool {
    if event.chinese_mode == current_chinese_mode
        && state.track_fingerprint.as_deref() == Some(event.track_fingerprint.as_str())
        && same_lyrics_document(&state.lines, &event.lines)
    {
        state.lines = event.lines;
        true
    } else {
        false
    }
}

fn same_lyrics_document(current: &[TimedLine], generated: &[TimedLine]) -> bool {
    current.len() == generated.len()
        && current.iter().zip(generated).all(|(current, generated)| {
            current.start_ms == generated.start_ms
                && current.end_ms == generated.end_ms
                && current.text == generated.text
                && current.syllables == generated.syllables
                && current.translation == generated.translation
                && current.background == generated.background
        })
}

#[cfg(test)]
#[path = "../../../test/romanization_loading_test.rs"]
mod tests;
