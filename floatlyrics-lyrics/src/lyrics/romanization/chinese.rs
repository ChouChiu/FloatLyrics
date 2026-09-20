// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Mandarin Pinyin and Cantonese Jyutping generation.

use std::borrow::Cow;
use std::sync::LazyLock;

use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::token::Token;
use pinyin::ToPinyin;
use serde::Deserialize;

use super::super::model::{RomanizationSegment, TimedLine};
use super::{ChineseRomanizationMode, push_separator};

/// Field of the CC-CEDICT details that holds the reading of a word.
const PINYIN: usize = 4;

/// The shared CC-CEDICT analyzer, or `None` when it could not be read.
///
/// The dictionary is compiled into the binary, so a failure here means the crate
/// was built without it; Chinese lines then keep the reading of every character
/// on its own instead of the one its word gives it.
static ANALYZER: LazyLock<Option<Segmenter>> =
    LazyLock::new(|| match load_dictionary("embedded://cc-cedict") {
        Ok(dictionary) => Some(Segmenter::new(Mode::Normal, dictionary, None)),
        Err(error) => {
            tracing::warn!(%error, "the embedded CC-CEDICT dictionary could not be loaded");
            None
        }
    });

/// The tone marks of the six pinyin vowels, one per tone.
const TONE_MARKS: [(char, [char; 4]); 6] = [
    ('a', ['ā', 'á', 'ǎ', 'à']),
    ('e', ['ē', 'é', 'ě', 'è']),
    ('i', ['ī', 'í', 'ǐ', 'ì']),
    ('o', ['ō', 'ó', 'ǒ', 'ò']),
    ('u', ['ū', 'ú', 'ǔ', 'ù']),
    ('ü', ['ǖ', 'ǘ', 'ǚ', 'ǜ']),
];

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
            let segments = chinese_segments(text);
            (pinyin_line(text, &segments), segments)
        }
    }
}

/// Joins the pinyin readings of `text`, dropping alphabetic text that has no
/// reading so a mixed line keeps only its Han readings. Whitespace, punctuation,
/// and digits separate adjacent readings.
///
/// `segments` holds the reading of each character of the same `text`, so the line
/// is derived from them instead of looking every character up again.
fn pinyin_line(text: &str, segments: &[RomanizationSegment]) -> String {
    let mut output = String::with_capacity(text.len());
    let mut previous_was_pinyin = false;

    debug_assert_eq!(
        text.chars().count(),
        segments.len(),
        "chinese_segments keeps one segment per character"
    );
    for (character, segment) in text.chars().zip(segments) {
        if !segment.romanization.is_empty() {
            push_separator(&mut output);
            output.push_str(&segment.romanization);
            previous_was_pinyin = true;
        } else if !character.is_alphabetic() {
            if previous_was_pinyin && character.is_alphanumeric() {
                push_separator(&mut output);
            }
            output.push(character);
            previous_was_pinyin = false;
        }
    }

    output
}

/// The reading of each character of `text`, one segment per character.
///
/// Readings come from the embedded CC-CEDICT dictionary, so a character is read
/// the way the word it belongs to is read: `音乐` is "yīn yuè" where `乐` alone is
/// "lè", and `银行` is "yín háng" where `行` alone is "xíng". A single character
/// and a word whose reading does not line up one syllable per character keep the
/// character's own reading, which is the one the pinyin table marks as most
/// common.
fn chinese_segments(text: &str) -> Vec<RomanizationSegment> {
    let characters = text.chars().collect::<Vec<_>>();
    let mut readings = characters
        .iter()
        .copied()
        .map(character_reading)
        .collect::<Vec<_>>();

    if let Some(analyzer) = ANALYZER.as_ref()
        && let Ok(mut tokens) = analyzer.segment(Cow::Borrowed(text))
    {
        let mut byte_cursor = 0;
        let mut char_cursor = 0;
        for token in &mut tokens {
            let surface = token.surface.as_ref();
            let Some(start) = text
                .get(byte_cursor..)
                .and_then(|rest| rest.find(surface))
                .map(|offset| byte_cursor + offset)
            else {
                continue;
            };
            let length = surface.chars().count();
            let index = char_cursor + text[byte_cursor..start].chars().count();
            byte_cursor = start + surface.len();
            char_cursor = index + length;

            let Some(word) = word_readings(token, length) else {
                continue;
            };
            readings[index..char_cursor].clone_from_slice(&word);
        }
    }

    characters
        .into_iter()
        .zip(readings)
        .map(|(character, romanization)| RomanizationSegment {
            text: character.to_string(),
            romanization,
            furigana: String::new(),
        })
        .collect()
}

/// The most common reading of one character, or an empty string without one.
fn character_reading(character: char) -> String {
    character
        .to_pinyin()
        .map_or_else(String::new, |pinyin| pinyin.with_tone().to_string())
}

/// The reading of every character of a word, or `None` when the word has none to
/// hand out one character at a time.
///
/// A word is only read as a whole when its reading carries exactly one syllable
/// per character; otherwise the characters keep their own readings, which keeps a
/// reading from being attached to the wrong character.
fn word_readings(token: &mut Token<'_>, characters: usize) -> Option<Vec<String>> {
    if characters < 2 {
        return None;
    }
    let pinyin = token.details_iter().nth(PINYIN).unwrap_or("");
    let syllables = pinyin.split_whitespace().collect::<Vec<_>>();
    (syllables.len() == characters).then(|| syllables.into_iter().map(tone_marked).collect())
}

/// Writes a tone-numbered syllable as Hanyu Pinyin with a tone mark.
///
/// CC-CEDICT writes every reading with the tone as a trailing digit — "yin1" is
/// "yīn" and "lu:4" is "lǜ" — and a tone mark goes on the vowel the syllable is
/// named after: a, o, and e take it over i, u, and ü, and the latter two take it
/// when the syllable has none of the first three, which puts the mark on the u of
/// "iu" and on the i of "ui".
fn tone_marked(syllable: &str) -> String {
    let syllable = syllable.replace("u:", "ü").replace('v', "ü");
    let tone = syllable
        .chars()
        .next_back()
        .and_then(|character| character.to_digit(10))
        .unwrap_or_default();
    let letters = syllable
        .trim_end_matches(|character: char| character.is_ascii_digit())
        .to_string();
    let tone = tone as usize;
    if !(1..=4).contains(&tone) {
        return letters;
    }

    let vowels = letters
        .char_indices()
        .filter(|(_, character)| matches!(character, 'a' | 'e' | 'i' | 'o' | 'u' | 'ü'))
        .collect::<Vec<_>>();
    let Some(&(index, vowel)) = vowels
        .iter()
        .find(|(_, character)| matches!(character, 'a' | 'o' | 'e'))
        .or_else(|| vowels.last())
    else {
        return letters;
    };
    let Some((_, marks)) = TONE_MARKS.iter().find(|(marked, _)| *marked == vowel) else {
        return letters;
    };

    let mut marked = letters;
    marked.replace_range(
        index..index + vowel.len_utf8(),
        &marks[tone - 1].to_string(),
    );
    marked
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
            // Readings only: alphabetic text without a jyutping reading is
            // dropped, leaving whitespace, punctuation, and digits as
            // separators between adjacent readings.
            for character in source.chars() {
                if character.is_alphabetic() {
                    continue;
                }
                if previous_was_jyutping && character.is_alphanumeric() {
                    push_separator(&mut output);
                }
                output.push(character);
                previous_was_jyutping = false;
            }
        } else {
            push_separator(&mut output);
            output.push_str(&jyutping);
            previous_was_jyutping = true;
        }
        segments.extend(jyutping_segments(&source, &jyutping));
    }

    let remainder = original.collect::<String>();
    if !remainder.is_empty() {
        output.push_str(&remainder);
        segments.push(RomanizationSegment {
            text: remainder,
            romanization: String::new(),
            furigana: String::new(),
        });
    }

    (output, segments)
}

/// Splits a word's Jyutping across the characters it annotates.
///
/// `rust_canto` annotates whole words while a karaoke renderer highlights one
/// character at a time, so a word whose annotation carries one syllable per
/// character hands them out one by one — `喜歡` reads "hei2" and then "fun1". A
/// word that does not line up that way keeps its reading on its first character
/// rather than repeating the whole of it under every character.
fn jyutping_segments(source: &str, jyutping: &str) -> Vec<RomanizationSegment> {
    let characters = source.chars().collect::<Vec<_>>();
    let syllables = jyutping.split_whitespace().collect::<Vec<_>>();
    if syllables.len() == characters.len() {
        return characters
            .into_iter()
            .zip(syllables)
            .map(|(character, syllable)| RomanizationSegment {
                text: character.to_string(),
                romanization: syllable.to_string(),
                furigana: String::new(),
            })
            .collect();
    }
    characters
        .into_iter()
        .enumerate()
        .map(|(index, character)| RomanizationSegment {
            text: character.to_string(),
            romanization: if index == 0 {
                jyutping.to_string()
            } else {
                String::new()
            },
            furigana: String::new(),
        })
        .collect()
}

fn jyutping_without_tones(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for syllable in value.split_whitespace() {
        if !output.is_empty() {
            output.push(' ');
        }
        output.push_str(
            syllable.trim_end_matches(|character: char| ('1'..='6').contains(&character)),
        );
    }
    output
}
