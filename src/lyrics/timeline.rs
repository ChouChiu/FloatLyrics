// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

use super::model::TimedLine;

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
mod tests {
    use super::*;

    #[test]
    fn playback_offset_clamps_at_numeric_boundaries() {
        assert_eq!(adjusted_playback_ms(0, i64::MIN), 0);
        assert_eq!(adjusted_playback_ms(u64::MAX, i64::MAX), u64::MAX);
    }
}
