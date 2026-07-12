// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Playback-time lookup over sorted lyrics lines.

use super::model::TimedLine;

/// Returns the line active at the offset-adjusted playback position.
///
/// `lines` must be sorted by ascending `start_ms`. A line with a known end is
/// inactive at and after that end, even when the following line has not begun.
pub fn active_line_index(lines: &[TimedLine], playback_ms: u64, offset_ms: i64) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }

    let adjusted = adjusted_playback_ms(playback_ms, offset_ms);
    let index = line_index_at_or_before(lines, playback_ms, offset_ms)?;
    let line = lines.get(index)?;

    match line.end_ms {
        Some(end_ms) if adjusted >= end_ms => None,
        _ => Some(index),
    }
}

/// Returns the last line beginning at or before the adjusted playback position.
///
/// `lines` must be sorted by ascending `start_ms`. Unlike
/// [`active_line_index`], this function intentionally holds the previous line
/// across timing gaps.
pub fn line_index_at_or_before(
    lines: &[TimedLine],
    playback_ms: u64,
    offset_ms: i64,
) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }

    let adjusted = adjusted_playback_ms(playback_ms, offset_ms);
    lines
        .partition_point(|line| line.start_ms <= adjusted)
        .checked_sub(1)
}

fn adjusted_playback_ms(playback_ms: u64, offset_ms: i64) -> u64 {
    (playback_ms as i128 + offset_ms as i128).clamp(0, u64::MAX as i128) as u64
}

#[cfg(test)]
#[path = "../test/timeline_test.rs"]
mod tests;
