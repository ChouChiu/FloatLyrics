// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Provider-independent local romanization for lyrics text.

use kakasi::IsJapanese;
use korean_romanize::{convert as romanize_korean, has_korean};
use pinyin::ToPinyin;
use serde::{Deserialize, Serialize};
use uroman::{Uroman, rom_format};
use whatlang::{Lang, detect};

use super::model::{RomanizationSegment, TimedLine};

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
            && complete_japanese_romanization(&line.text).is_none()
    });
    let prefer_japanese_han = has_japanese_kana || !has_chinese_evidence;
    let chinese_mode = match chinese_mode {
        ChineseRomanizationMode::Auto if looks_like_cantonese(lines) => {
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
            (romanize_korean(&line.text), korean_segments(&line.text))
        } else if kakasi::is_japanese(&line.text) == IsJapanese::True {
            japanese_romanization(&line.text)
        } else if contains_han(&line.text)
            && prefer_japanese_han
            && let Some(romanization) = complete_japanese_romanization(&line.text)
        {
            (romanization, japanese_segments(&line.text))
        } else if contains_han(&line.text) {
            match chinese_mode {
                ChineseRomanizationMode::CantoneseJyutping => cantonese_jyutping(&line.text, true),
                ChineseRomanizationMode::CantoneseJyutpingNoTones => {
                    cantonese_jyutping(&line.text, false)
                }
                ChineseRomanizationMode::Auto | ChineseRomanizationMode::MandarinPinyin => {
                    (chinese_pinyin(&line.text), chinese_segments(&line.text))
                }
            }
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

fn looks_like_cantonese(lines: &[TimedLine]) -> bool {
    const CANTONESE_MARKERS: &str = "佢唔嘅咗冇喺啲咁哋嚟噉咩啱攞搵嘢噃囉喎啫喐冧";

    lines.iter().any(|line| {
        line.text
            .chars()
            .any(|character| CANTONESE_MARKERS.contains(character))
    })
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

fn japanese_romanization(text: &str) -> (String, Vec<RomanizationSegment>) {
    (kakasi::convert(text).romaji, japanese_segments(text))
}

fn complete_japanese_romanization(text: &str) -> Option<String> {
    let romanization = kakasi::convert(text).romaji;
    (!contains_han(&romanization)).then_some(romanization)
}

fn contains_han(text: &str) -> bool {
    text.chars()
        .any(|character| character.to_pinyin().is_some())
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

fn japanese_segments(text: &str) -> Vec<RomanizationSegment> {
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

fn korean_segments(text: &str) -> Vec<RomanizationSegment> {
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
