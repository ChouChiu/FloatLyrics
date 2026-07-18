// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Focused parser for QQ Music QRC timing and syllable tags.

use super::super::model::{TimedLine, TimedSyllable};
use super::{filter_display_lines, merge_translation_marker_lines};

pub(super) fn timed_lines_from_qrc(content: &str) -> Vec<TimedLine> {
    let mut lines = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        let Some((start_ms, end_ms, text, syllables)) = parse_qrc_line(line) else {
            continue;
        };
        if text.is_empty() {
            continue;
        }

        lines.push(TimedLine {
            start_ms,
            end_ms: Some(end_ms),
            text,
            syllables,
            translation: None,
            romanization: None,
            romanization_segments: Vec::new(),
            background: None,
        });
    }

    lines.sort_by_key(|line| line.start_ms);
    filter_display_lines(merge_translation_marker_lines(lines))
}

fn parse_qrc_line(line: &str) -> Option<(u64, u64, String, Vec<TimedSyllable>)> {
    let (tag, rest) = first_bracket_tag(line)?;
    let (start_ms, duration_ms) = parse_qrc_timestamp(tag)?;
    let (text, syllables) = qrc_line_parts(rest);

    Some((
        start_ms,
        start_ms.saturating_add(duration_ms),
        text,
        syllables,
    ))
}

fn first_bracket_tag(line: &str) -> Option<(&str, &str)> {
    let after_open = line.strip_prefix('[')?;
    let end = after_open.find(']')?;
    Some((&after_open[..end], &after_open[end + 1..]))
}

fn parse_qrc_timestamp(tag: &str) -> Option<(u64, u64)> {
    let (start, duration) = tag.split_once(',')?;
    let start = start.trim().parse().ok()?;
    let duration = duration.trim().parse().ok()?;
    Some((start, duration))
}

fn qrc_line_parts(value: &str) -> (String, Vec<TimedSyllable>) {
    let mut output = String::new();
    let mut syllables = Vec::new();
    let mut rest = value;

    while let Some(open) = rest.find('(') {
        let segment_text = &rest[..open];
        output.push_str(segment_text);
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find(')') else {
            output.push_str(&rest[open..]);
            return (output.trim().to_string(), syllables);
        };
        let tag = &after_open[..close];
        if let Some((start_ms, duration_ms)) = parse_qrc_timestamp(tag) {
            if !segment_text.is_empty() {
                syllables.push(TimedSyllable {
                    start_ms,
                    end_ms: start_ms.saturating_add(duration_ms),
                    text: segment_text.to_string(),
                });
            }
        } else {
            output.push('(');
            rest = after_open;
            continue;
        }
        rest = &after_open[close + 1..];
    }

    output.push_str(rest);
    (output.trim().to_string(), syllables)
}
