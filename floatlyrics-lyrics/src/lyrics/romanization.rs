// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Provider-independent local romanization for lyrics text.

use kakasi::IsJapanese;
use korean_romanize::has_korean;
use serde::{Deserialize, Serialize};
use uroman::{Uroman, rom_format};
use whatlang::{Lang, detect};

use super::model::{RomanizationSegment, TimedLine};

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
/// per-line fallback for multilingual lyrics. Other supported scripts use
/// `uroman` as a general transliterator. ASCII-only Spanish is detected so its
/// unchanged Latin spelling can still be exposed as romanization.
pub fn generate_local_romanization(lines: &mut [TimedLine]) {
    generate_local_romanization_with_mode(lines, ChineseRomanizationMode::Auto);
}

/// Replaces pronunciation using `chinese_mode` for Chinese lyrics.
pub fn generate_local_romanization_with_mode(
    lines: &mut [TimedLine],
    chinese_mode: ChineseRomanizationMode,
) {
    let mut uroman = None;
    let has_japanese_kana = lines
        .iter()
        .any(|line| kakasi::is_japanese(&line.text) == IsJapanese::True);
    let has_chinese_evidence = lines.iter().any(|line| {
        kakasi::is_japanese(&line.text) == IsJapanese::Maybe
            && japanese::complete_romanization(&line.text).is_none()
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
        let is_ascii_spanish = is_unaccented_spanish(&line.text);
        if line.text.is_ascii() && !is_ascii_spanish {
            continue;
        }

        let (romanization, segments) = if is_ascii_spanish {
            latin_script_romanization(&line.text)
        } else if has_korean(&line.text) {
            korean::romanize(&line.text)
        } else if kakasi::is_japanese(&line.text) == IsJapanese::True {
            japanese::romanize(&line.text)
        } else if chinese::contains_han(&line.text)
            && prefer_japanese_han
            && let Some(romanization) = japanese::complete_romanization(&line.text)
        {
            (romanization, japanese::segments(&line.text))
        } else if chinese::contains_han(&line.text) {
            chinese::romanize(&line.text, chinese_mode)
        } else {
            let uroman = uroman.get_or_insert_with(Uroman::new);
            (
                uroman
                    .romanize_string::<rom_format::Str>(&line.text, None)
                    .to_string(),
                uroman_segments(uroman, &line.text),
            )
        };

        let romanization = romanization.trim();
        if !romanization.is_empty() && (romanization != line.text.trim() || is_ascii_spanish) {
            line.romanization = Some(romanization.to_string());
            line.romanization_segments = segments;
        }
    }
}

fn is_unaccented_spanish(text: &str) -> bool {
    // Short lyric lines do not reach whatlang's prose-oriented reliability
    // threshold, so use a conservative floor covered by cache-derived English
    // regression cases.
    const MINIMUM_SHORT_LINE_CONFIDENCE: f64 = 0.05;

    text.is_ascii()
        && text
            .chars()
            .any(|character| character.is_ascii_alphabetic())
        && detect(text).is_some_and(|info| {
            info.lang() == Lang::Spa && info.confidence() >= MINIMUM_SHORT_LINE_CONFIDENCE
        })
}

fn latin_script_romanization(text: &str) -> (String, Vec<RomanizationSegment>) {
    let text = text.trim().to_string();
    let segment = RomanizationSegment {
        romanization: text.clone(),
        text: text.clone(),
    };
    (text, vec![segment])
}

fn uroman_segments(uroman: &Uroman, text: &str) -> Vec<RomanizationSegment> {
    let characters = text.chars().collect::<Vec<_>>();
    uroman
        .romanize_string::<rom_format::Edges>(text, None)
        .to_edges()
        .into_iter()
        .filter_map(|edge| {
            let data = edge.get_data();
            let source = characters
                .get(data.start..data.end)?
                .iter()
                .collect::<String>();
            Some(RomanizationSegment {
                romanization: if data.txt != source {
                    data.txt.clone()
                } else {
                    String::new()
                },
                text: source,
            })
        })
        .collect()
}

fn push_separator(output: &mut String) {
    if !output.is_empty() && !output.ends_with(char::is_whitespace) {
        output.push(' ');
    }
}
