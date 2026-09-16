// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Provider-independent local romanization for lyrics text.

use serde::{Deserialize, Serialize};

use super::model::{RomanizationSegment, TimedLine, TimedSyllable};

mod chinese;
mod japanese;
mod korean;

/// Preferred pronunciation system for Chinese lyrics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChineseRomanizationMode {
    /// Detect explicit Cantonese wording and otherwise use Mandarin Pinyin.
    #[default]
    Auto,
    /// Always generate Mandarin Hanyu Pinyin.
    MandarinPinyin,
    /// Always generate Cantonese Jyutping.
    CantoneseJyutping,
    /// Always generate Cantonese Jyutping without tone numbers.
    CantoneseJyutpingNoTones,
}

impl ChineseRomanizationMode {
    /// Modes in their stable settings-menu order.
    pub const ALL: [Self; 4] = [
        Self::Auto,
        Self::MandarinPinyin,
        Self::CantoneseJyutping,
        Self::CantoneseJyutpingNoTones,
    ];
}

/// Replaces any existing pronunciation with locally generated romanization.
///
/// Japanese and Chinese are distinguished using document-level evidence, with
/// per-line fallback for multilingual lyrics. Only Chinese, Japanese, and
/// Korean text is romanized.
pub fn generate_local_romanization(lines: &mut [TimedLine]) {
    generate_local_romanization_with_mode(lines, ChineseRomanizationMode::Auto);
}

/// Replaces pronunciation using `chinese_mode` for Chinese lyrics.
pub fn generate_local_romanization_with_mode(
    lines: &mut [TimedLine],
    chinese_mode: ChineseRomanizationMode,
) {
    let has_japanese_kana = lines.iter().any(|line| japanese::contains_kana(&line.text));
    let has_chinese_evidence = lines.iter().any(|line| {
        !japanese::contains_kana(&line.text)
            && chinese::contains_han(&line.text)
            && !japanese::reads_han_text(&line.text)
    });
    let prefer_japanese_han = has_japanese_kana || !has_chinese_evidence;
    let chinese_mode = match chinese_mode {
        ChineseRomanizationMode::Auto if chinese::looks_like_cantonese(lines) => {
            ChineseRomanizationMode::CantoneseJyutping
        }
        ChineseRomanizationMode::Auto => ChineseRomanizationMode::MandarinPinyin,
        mode => mode,
    };

    for line in lines {
        line.romanization = None;
        line.romanization_segments.clear();
        for syllable in &mut line.syllables {
            syllable.romanization.clear();
            syllable.furigana.clear();
        }

        let analyzed = if korean::has_hangul(&line.text) {
            Some(korean::romanize(&line.text))
        } else if japanese::contains_kana(&line.text) {
            japanese::romanize(&line.text)
        } else if chinese::contains_han(&line.text) {
            // A line written only with Han is Japanese when the Japanese
            // dictionary reads all of it, and Chinese when it does not.
            if prefer_japanese_han && japanese::reads_han_text(&line.text) {
                japanese::romanize(&line.text)
            } else {
                Some(chinese::romanize(&line.text, chinese_mode))
            }
        } else {
            continue;
        };
        let Some((romanization, segments)) = analyzed else {
            continue;
        };

        let romanization = romanization.trim();
        if !romanization.is_empty() && romanization != line.text.trim() {
            assign_syllable_readings(&mut line.syllables, &segments);
            line.romanization = Some(romanization.to_string());
            line.romanization_segments = segments;
        }
    }
}

/// Copies the readings of `segments` onto the syllables covering the same text.
///
/// Segments partition the line text in reading order and the syllables are
/// ordered, non-overlapping spans of that same text, so both lists are walked
/// together: each syllable is located with [`str::find`] at or after a running
/// byte cursor, and it receives the readings and the kana of the segments
/// overlapping its span. Alignment stops at the first syllable that cannot be
/// found, so a reading is never attached to the wrong syllable.
fn assign_syllable_readings(syllables: &mut [TimedSyllable], segments: &[RomanizationSegment]) {
    let source = segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<String>();
    let mut boundaries = Vec::with_capacity(segments.len() + 1);
    let mut boundary = 0;
    boundaries.push(boundary);
    for segment in segments {
        boundary += segment.text.len();
        boundaries.push(boundary);
    }

    let mut search_from = 0;
    let mut first_segment = 0;

    for syllable in syllables {
        let text = syllable.text.trim();
        if text.is_empty() {
            continue;
        }
        let Some(offset) = source.get(search_from..).and_then(|rest| rest.find(text)) else {
            return;
        };
        let start = search_from + offset;
        let end = start + text.len();

        let mut romanization = String::new();
        let mut furigana = String::new();
        let mut reading_end = start;
        while first_segment < segments.len() && boundaries[first_segment] < end {
            let segment = &segments[first_segment];
            let segment_start = boundaries[first_segment];
            let segment_end = boundaries[first_segment + 1];
            if segment_end > start {
                if !segment.romanization.is_empty() {
                    if !romanization.is_empty()
                        && source[reading_end..segment_start].contains(char::is_whitespace)
                    {
                        romanization.push(' ');
                    }
                    romanization.push_str(&segment.romanization);
                    reading_end = segment_end;
                }
                // Kana sits above the characters it reads, which is what the
                // segment it came from covers.
                furigana.push_str(&segment.furigana);
            }
            first_segment += 1;
        }

        syllable.romanization = romanization;
        syllable.furigana = furigana;
        search_from = end;
    }
}

fn push_separator(output: &mut String) {
    if !output.is_empty() && !output.ends_with(char::is_whitespace) {
        output.push(' ');
    }
}
