// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Focused parser for QQ Music QRC timing and syllable tags.

use super::super::model::{TimedLine, TimedSyllable, Voice};

/// Parses QQ Music word timing and syllable tags without interpreting what the
/// lines say.
///
/// The rows are returned in start order as the payload wrote them; the
/// conventions a transcription uses are read once for every provider format, in
/// [`super::finish`].
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
            voice: Voice::Primary,
        });
    }

    lines.sort_by_key(|line| line.start_ms);
    lines
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

/// Splits the tagged body of one QRC line into its text and timed syllables.
///
/// A parenthesis pair whose content is not a timestamp is provider text: QQ Music
/// repeats a phrase as `((109473,249)Come on, just…)(110123,7118)`, where only the
/// inner pairs are word tags. Such a bracket belongs to the word that the next
/// timestamp times, so the syllables always spell the line a renderer shows.
fn qrc_line_parts(value: &str) -> (String, Vec<TimedSyllable>) {
    let mut text = String::new();
    let mut syllables = Vec::new();
    let mut word = String::new();
    let mut rest = value;

    while let Some(open) = rest.find('(') {
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find(')') else {
            break;
        };
        let Some((start_ms, duration_ms)) = parse_qrc_timestamp(&after_open[..close]) else {
            word.push_str(&rest[..=open]);
            rest = after_open;
            continue;
        };

        word.push_str(&rest[..open]);
        text.push_str(&word);
        if !word.is_empty() {
            syllables.push(TimedSyllable {
                start_ms,
                end_ms: start_ms.saturating_add(duration_ms),
                text: std::mem::take(&mut word),
                romanization: String::new(),
                furigana: String::new(),
            });
        }
        rest = &after_open[close + 1..];
    }

    word.push_str(rest);
    text.push_str(&word);
    if let Some(last) = syllables.last_mut() {
        // Text after the last timestamp carries no timing of its own, so the final
        // word covers it instead of dropping it from the syllables.
        last.text.push_str(&word);
    }

    debug_assert!(
        syllables.is_empty()
            || syllables
                .iter()
                .map(|syllable| syllable.text.as_str())
                .collect::<String>()
                .trim()
                == text.trim(),
        "qrc syllables must spell the line text"
    );

    (text.trim().to_string(), syllables)
}

#[cfg(test)]
#[path = "../../test/qrc_test.rs"]
mod tests;
