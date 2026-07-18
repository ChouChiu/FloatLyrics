// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Focused parser for timestamped LRC lines.

use super::super::model::TimedLine;
use super::{filter_display_lines, merge_translation_marker_lines};

pub(super) fn timed_lines_from_lrc(content: &str) -> Vec<TimedLine> {
    let mut lines = Vec::new();

    for line in content.lines() {
        let Some((timestamps, text)) = split_lrc_timestamps(line.trim()) else {
            continue;
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }

        for timestamp in timestamps_in(&timestamps) {
            if let Some(start_ms) = parse_lrc_timestamp(timestamp) {
                lines.push(TimedLine {
                    start_ms,
                    end_ms: None,
                    text: text.to_string(),
                    syllables: Vec::new(),
                    translation: None,
                    romanization: None,
                    romanization_segments: Vec::new(),
                    background: None,
                });
            }
        }
    }

    lines.sort_by_key(|line| line.start_ms);
    filter_display_lines(merge_translation_marker_lines(lines))
}

fn timestamps_in(tags: &str) -> impl Iterator<Item = &str> {
    tags.split('[')
        .filter_map(|part| part.strip_suffix(']'))
        .filter(|tag| is_lrc_timestamp(tag))
}

fn parse_lrc_timestamp(timestamp: &str) -> Option<u64> {
    let (minutes, seconds) = timestamp.split_once(':')?;
    let minutes = minutes.parse::<u64>().ok()?;
    let (seconds, millis) = seconds
        .split_once('.')
        .map_or((seconds, "0"), |(seconds, millis)| (seconds, millis));
    let seconds = seconds.parse::<u64>().ok()?;
    let millis = parse_lrc_millis(millis)?;

    minutes
        .checked_mul(60_000)?
        .checked_add(seconds.checked_mul(1_000)?)?
        .checked_add(millis)
}

fn parse_lrc_millis(value: &str) -> Option<u64> {
    let digits = value
        .bytes()
        .take_while(u8::is_ascii_digit)
        .take(3)
        .collect::<Vec<_>>();
    if digits.is_empty() {
        return Some(0);
    }

    let parsed = std::str::from_utf8(&digits).ok()?.parse::<u64>().ok()?;
    Some(match digits.len() {
        1 => parsed * 100,
        2 => parsed * 10,
        _ => parsed,
    })
}

fn split_lrc_timestamps(line: &str) -> Option<(String, &str)> {
    let mut rest = line;
    let mut timestamps = String::new();

    while let Some(after_open) = rest.strip_prefix('[') {
        let Some(end) = after_open.find(']') else {
            break;
        };
        let tag = &after_open[..end];
        if !is_lrc_timestamp(tag) {
            break;
        }

        timestamps.push('[');
        timestamps.push_str(tag);
        timestamps.push(']');
        rest = &after_open[end + 1..];
    }

    if timestamps.is_empty() {
        None
    } else {
        Some((timestamps, rest))
    }
}

fn is_lrc_timestamp(tag: &str) -> bool {
    let Some(first) = tag.as_bytes().first() else {
        return false;
    };
    first.is_ascii_digit()
        && tag.contains(':')
        && tag
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b':' || byte == b'.')
}

#[cfg(test)]
#[path = "../../test/parsing_test.rs"]
mod tests;
