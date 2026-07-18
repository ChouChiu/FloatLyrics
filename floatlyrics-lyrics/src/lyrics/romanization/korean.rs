// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Korean Revised Romanization with syllable-aligned segments.

use korean_romanize::convert as romanize_korean;

use super::super::model::RomanizationSegment;

pub(super) fn romanize(text: &str) -> (String, Vec<RomanizationSegment>) {
    (romanize_korean(text), segments(text))
}

fn segments(text: &str) -> Vec<RomanizationSegment> {
    script_runs_by(text, is_hangul_syllable)
        .into_iter()
        .flat_map(|run| {
            if run.chars().all(is_hangul_syllable) {
                split_korean_run(&run)
            } else {
                vec![RomanizationSegment {
                    text: run,
                    romanization: String::new(),
                }]
            }
        })
        .collect()
}

fn script_runs_by(text: &str, predicate: impl Fn(char) -> bool) -> Vec<String> {
    let mut runs = Vec::new();
    let mut current = String::new();
    let mut current_matches = None;
    for character in text.chars() {
        let matches = predicate(character);
        if current_matches.is_some_and(|previous| previous != matches) {
            runs.push(std::mem::take(&mut current));
        }
        current.push(character);
        current_matches = Some(matches);
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs
}

fn split_korean_run(text: &str) -> Vec<RomanizationSegment> {
    let characters = text.chars().collect::<Vec<_>>();
    let romanization = romanize_korean(text);
    let vowels = characters
        .iter()
        .filter_map(|character| korean_vowel(*character))
        .collect::<Vec<_>>();
    let Some(ranges) = split_romanized_syllables(&romanization, &vowels) else {
        return characters
            .into_iter()
            .map(|character| RomanizationSegment {
                text: character.to_string(),
                romanization: romanize_korean(character.to_string()),
            })
            .collect();
    };

    characters
        .into_iter()
        .zip(ranges)
        .map(|(character, range)| RomanizationSegment {
            text: character.to_string(),
            romanization: romanization[range].to_string(),
        })
        .collect()
}

fn is_hangul_syllable(character: char) -> bool {
    matches!(character as u32, 0xac00..=0xd7a3)
}

fn korean_vowel(character: char) -> Option<&'static str> {
    const VOWELS: [&str; 21] = [
        "a", "ae", "ya", "yae", "eo", "e", "yeo", "ye", "o", "wa", "wae", "oe", "yo", "u", "wo",
        "we", "wi", "yu", "eu", "ui", "i",
    ];
    let code = character as u32;
    is_hangul_syllable(character).then(|| VOWELS[((code - 0xac00) % 588 / 28) as usize])
}

fn split_romanized_syllables(
    romanization: &str,
    vowels: &[&str],
) -> Option<Vec<std::ops::Range<usize>>> {
    let mut vowel_ranges = Vec::with_capacity(vowels.len());
    let mut search_from = 0;
    for vowel in vowels {
        let start = romanization.get(search_from..)?.find(vowel)? + search_from;
        let end = start + vowel.len();
        vowel_ranges.push(start..end);
        search_from = end;
    }

    let mut boundaries = vec![0];
    for pair in vowel_ranges.windows(2) {
        let between = romanization.get(pair[0].end..pair[1].start)?;
        let onset_len = next_onset_len(between);
        boundaries.push(pair[1].start.saturating_sub(onset_len));
    }
    boundaries.push(romanization.len());

    let ranges = boundaries
        .windows(2)
        .map(|pair| pair[0]..pair[1])
        .collect::<Vec<_>>();
    (ranges.len() == vowels.len() && ranges.iter().all(|range| !range.is_empty())).then_some(ranges)
}

fn next_onset_len(consonants: &str) -> usize {
    const DOUBLE_ONSETS: [&str; 6] = ["kk", "tt", "pp", "ss", "jj", "ch"];
    if DOUBLE_ONSETS
        .iter()
        .any(|onset| consonants.ends_with(onset))
    {
        2
    } else if consonants
        .chars()
        .last()
        .is_some_and(|character| "gndrm bsjktph".contains(character))
    {
        1
    } else {
        0
    }
}
