// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Japanese script detection and segmented Romaji generation.

use super::super::model::RomanizationSegment;
use super::chinese::contains_han;

pub(super) fn romanize(text: &str) -> (String, Vec<RomanizationSegment>) {
    (kakasi::convert(text).romaji, segments(text))
}

pub(super) fn complete_romanization(text: &str) -> Option<String> {
    let romanization = kakasi::convert(text).romaji;
    (!contains_han(&romanization)).then_some(romanization)
}

pub(super) fn segments(text: &str) -> Vec<RomanizationSegment> {
    script_runs(text)
        .into_iter()
        .map(|text| {
            let romanization = if text.chars().any(is_japanese_character) {
                kakasi::convert(&text).romaji.trim().to_string()
            } else {
                String::new()
            };
            RomanizationSegment { text, romanization }
        })
        .collect()
}

fn script_runs(text: &str) -> Vec<String> {
    let mut runs = Vec::new();
    let mut current = String::new();
    let mut current_kind = None;

    for character in text.chars() {
        let kind = japanese_character_kind(character);
        if current_kind.is_some_and(|previous| previous != kind) {
            runs.push(std::mem::take(&mut current));
        }
        current.push(character);
        current_kind = Some(kind);
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs
}

fn japanese_character_kind(character: char) -> u8 {
    match character as u32 {
        0x3400..=0x9fff | 0xf900..=0xfaff => 1,
        0x3040..=0x30ff | 0x31f0..=0x31ff => 2,
        _ if character.is_whitespace() => 3,
        _ => 4,
    }
}

fn is_japanese_character(character: char) -> bool {
    matches!(japanese_character_kind(character), 1 | 2)
}
