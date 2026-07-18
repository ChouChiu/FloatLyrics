// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Mandarin Pinyin and Cantonese Jyutping generation.

use pinyin::ToPinyin;
use serde::Deserialize;

use super::super::model::{RomanizationSegment, TimedLine};
use super::{ChineseRomanizationMode, push_separator};

pub(super) fn looks_like_cantonese(lines: &[TimedLine]) -> bool {
    const CANTONESE_MARKERS: &str = "佢唔嘅咗冇喺啲咁哋嚟噉咩啱攞搵嘢噃囉喎啫喐冧";

    lines.iter().any(|line| {
        line.text
            .chars()
            .any(|character| CANTONESE_MARKERS.contains(character))
    })
}

pub(super) fn contains_han(text: &str) -> bool {
    text.chars()
        .any(|character| character.to_pinyin().is_some())
}

pub(super) fn romanize(
    text: &str,
    mode: ChineseRomanizationMode,
) -> (String, Vec<RomanizationSegment>) {
    match mode {
        ChineseRomanizationMode::CantoneseJyutping => cantonese_jyutping(text, true),
        ChineseRomanizationMode::CantoneseJyutpingNoTones => cantonese_jyutping(text, false),
        ChineseRomanizationMode::Auto | ChineseRomanizationMode::MandarinPinyin => {
            (chinese_pinyin(text), chinese_segments(text))
        }
    }
}

fn chinese_pinyin(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut previous_was_pinyin = false;

    for character in text.chars() {
        if let Some(pinyin) = character.to_pinyin() {
            push_separator(&mut output);
            output.push_str(pinyin.with_tone());
            previous_was_pinyin = true;
        } else {
            if previous_was_pinyin && character.is_alphanumeric() {
                push_separator(&mut output);
            }
            output.push(character);
            previous_was_pinyin = false;
        }
    }

    output
}

fn chinese_segments(text: &str) -> Vec<RomanizationSegment> {
    text.chars()
        .map(|character| RomanizationSegment {
            text: character.to_string(),
            romanization: character
                .to_pinyin()
                .map_or_else(String::new, |pinyin| pinyin.with_tone().to_string()),
        })
        .collect()
}

#[derive(Deserialize)]
struct CantoneseAnnotation {
    word: String,
    jyutping: Option<String>,
}

fn cantonese_jyutping(text: &str, include_tones: bool) -> (String, Vec<RomanizationSegment>) {
    let traditional = lyrics_helper::helpers::chinese_helper::to_traditional(text);
    let annotations = serde_json::from_slice::<Vec<CantoneseAnnotation>>(&rust_canto::annotate(
        traditional.as_bytes(),
    ))
    .unwrap_or_default();
    let mut original = text.chars();
    let mut output = String::with_capacity(text.len());
    let mut segments = Vec::with_capacity(annotations.len());
    let mut previous_was_jyutping = false;

    for annotation in annotations {
        let source = original
            .by_ref()
            .take(annotation.word.chars().count())
            .collect::<String>();
        let mut jyutping = annotation
            .jyutping
            .filter(|_| {
                source
                    .chars()
                    .any(|character| character.to_pinyin().is_some())
            })
            .unwrap_or_default();
        if !include_tones {
            jyutping = jyutping_without_tones(&jyutping);
        }
        if jyutping.is_empty() {
            if previous_was_jyutping && source.starts_with(char::is_alphanumeric) {
                push_separator(&mut output);
            }
            output.push_str(&source);
            previous_was_jyutping = false;
        } else {
            push_separator(&mut output);
            output.push_str(&jyutping);
            previous_was_jyutping = true;
        }
        segments.push(RomanizationSegment {
            text: source,
            romanization: jyutping,
        });
    }

    let remainder = original.collect::<String>();
    if !remainder.is_empty() {
        output.push_str(&remainder);
        segments.push(RomanizationSegment {
            text: remainder,
            romanization: String::new(),
        });
    }

    (output, segments)
}

fn jyutping_without_tones(value: &str) -> String {
    value
        .split_whitespace()
        .map(|syllable| {
            syllable.trim_end_matches(|character: char| ('1'..='6').contains(&character))
        })
        .collect::<Vec<_>>()
        .join(" ")
}
