// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Provider-independent local romanization for lyrics text.

use kakasi::IsJapanese;
use korean_romanize::has_korean;
use serde::{Deserialize, Serialize};

use super::model::TimedLine;

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

        let (romanization, segments) = if has_korean(&line.text) {
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
            continue;
        };

        let romanization = romanization.trim();
        if !romanization.is_empty() && romanization != line.text.trim() {
            line.romanization = Some(romanization.to_string());
            line.romanization_segments = segments;
        }
    }
}

fn push_separator(output: &mut String) {
    if !output.is_empty() && !output.ends_with(char::is_whitespace) {
        output.push(' ');
    }
}
