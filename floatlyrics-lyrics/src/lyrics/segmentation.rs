// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Karaoke word segmentation.
//!
//! Provider timing is unevenly granular: QRC groups several characters under one
//! timed unit and LRC carries no word timing at all. Splitting every timed unit
//! into the tokens a karaoke renderer animates — one CJK character or one Latin
//! word, with punctuation attached to the neighbouring word — gives per-word
//! effects and per-word readings, and distributes the unit's own duration over
//! its tokens by character weight.
//!
//! The tokenizer, the punctuation merging, and the weighted time distribution
//! follow the AMLL TTML Tool (`amll-dev/amll-ttml-tool`,
//! `src/modules/segmentation/utils/segmentation.ts`). Two deliberate
//! differences: whitespace never becomes its own zero-length token but stays
//! with the token it follows, and Latin words are never split into syllables
//! because that would need per-language hyphenation dictionaries.

use super::model::{TimedLine, TimedSyllable};

/// Share of a character's duration that one punctuation mark consumes.
const PUNCTUATION_WEIGHT: f64 = 0.2;

/// Punctuation that attaches to the word after it.
const OPENING_PUNCTUATION: &str = "([{<「『（【《〈〔｢“‘«‹¿¡";

/// Quote without a fixed direction: it opens when closed and closes when open.
const AMBIGUOUS_QUOTE: char = '"';

/// Splits every line into karaoke word tokens with distributed timing.
///
/// A line that already carries word timing is refined inside each timed unit, so
/// provider timing survives at word granularity. A line with only line timing is
/// split across the span a renderer would highlight: the explicit end, or the
/// start of the next line. A line without a resolvable span keeps the timing it
/// had, and the texts of the resulting tokens always concatenate to the text
/// they were made from.
pub fn segment_lines_into_words(lines: &mut [TimedLine]) {
    for index in 0..lines.len() {
        let next_start_ms = lines.get(index + 1).map(|line| line.start_ms);
        let line = &mut lines[index];
        if line.text.trim().is_empty() {
            continue;
        }

        if line.syllables.is_empty() {
            let Some(end_ms) = line_end_ms(line, next_start_ms) else {
                continue;
            };
            line.syllables = split_unit(&line.text, line.start_ms, end_ms);
            continue;
        }

        let mut refined: Option<Vec<TimedSyllable>> = None;
        for (index, syllable) in line.syllables.iter().enumerate() {
            let split = (syllable.end_ms > syllable.start_ms)
                .then(|| split_unit(&syllable.text, syllable.start_ms, syllable.end_ms))
                .filter(|split| split.len() > 1);
            let Some(split) = split else {
                // A unit that keeps the timing it stores is copied only once a
                // later unit actually splits.
                if let Some(refined) = &mut refined {
                    refined.push(syllable.clone());
                }
                continue;
            };
            let refined = refined.get_or_insert_with(|| line.syllables[..index].to_vec());
            refined.extend(split);
        }
        if let Some(refined) = refined {
            line.syllables = refined;
        }
    }
}

/// Resolves the span to distribute over, matching the renderers' line end.
///
/// Only reached for a line that has no timed unit yet, so the units cannot bound
/// the end.
fn line_end_ms(line: &TimedLine, next_start_ms: Option<u64>) -> Option<u64> {
    [line.end_ms, next_start_ms]
        .into_iter()
        .flatten()
        .find(|end_ms| *end_ms > line.start_ms)
}

/// Splits one timed unit into tokens whose spans tile `start_ms..end_ms`.
fn split_unit(text: &str, start_ms: u64, end_ms: u64) -> Vec<TimedSyllable> {
    let tokens = merge_punctuation(fold_whitespace(tokenize(text)));
    if tokens.is_empty() {
        return Vec::new();
    }
    if tokens.len() == 1 {
        return vec![TimedSyllable {
            start_ms,
            end_ms,
            text: text.to_string(),
            romanization: String::new(),
            furigana: String::new(),
        }];
    }

    debug_assert!(
        tokens
            .iter()
            .flat_map(|token| token.text.chars())
            .eq(text.chars()),
        "segmentation must not change the text it splits"
    );

    distribute(start_ms, end_ms, &tokens)
}

/// Distributes `start_ms..end_ms` over `tokens` in proportion to their weights.
///
/// A unit whose tokens all carry zero weight is spread evenly.
fn distribute(start_ms: u64, end_ms: u64, tokens: &[Token]) -> Vec<TimedSyllable> {
    // Callers only distribute over a span with time in it; a zero-length one
    // would still put every token at `start_ms`.
    let end_ms = end_ms.max(start_ms);
    let total_weight = tokens.iter().map(|token| token.weight).sum::<f64>();
    let weighted = total_weight > 0.0;
    let divisor = if weighted {
        total_weight
    } else {
        tokens.len() as f64
    };

    let duration_per_weight = (end_ms - start_ms) as f64 / divisor;
    let mut syllables = Vec::with_capacity(tokens.len());
    let mut cursor = start_ms;
    for (index, token) in tokens.iter().enumerate() {
        // The last token takes the remainder so the spans always tile the unit.
        let token_end_ms = if index + 1 == tokens.len() {
            end_ms
        } else {
            let weight = if weighted { token.weight } else { 1.0 };
            cursor
                .saturating_add((weight * duration_per_weight).round() as u64)
                .min(end_ms)
        };
        syllables.push(TimedSyllable {
            start_ms: cursor,
            end_ms: token_end_ms,
            text: token.text.clone(),
            romanization: String::new(),
            furigana: String::new(),
        });
        cursor = token_end_ms.max(cursor);
    }
    syllables
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CharKind {
    /// Han, Kana, and Hangul: one animated unit per character.
    Cjk,
    /// Letters of space-separated scripts, including apostrophes and marks.
    Latin,
    /// ASCII digits.
    Numeric,
    /// Any whitespace.
    Whitespace,
    /// Punctuation, symbols, and everything else.
    Other,
}

struct Token {
    text: String,
    kind: CharKind,
    weight: f64,
}

impl Token {
    fn empty() -> Self {
        Self {
            text: String::new(),
            kind: CharKind::Other,
            weight: 0.0,
        }
    }

    fn push(&mut self, other: Self) {
        self.text.push_str(&other.text);
        self.weight += other.weight;
    }
}

/// Breaks `text` into raw tokens without reordering or dropping anything.
fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut current_kind: Option<CharKind> = None;

    for character in text.chars() {
        let kind = char_kind(character);
        if let Some(previous) = current_kind
            && !merges_with(previous, kind)
        {
            push_token(&mut tokens, &mut current, previous);
        }
        current.push(character);
        current_kind = Some(kind);
    }
    if let Some(kind) = current_kind {
        push_token(&mut tokens, &mut current, kind);
    }
    tokens
}

/// Whether two adjacent characters belong to the same token.
fn merges_with(previous: CharKind, current: CharKind) -> bool {
    previous == current && !matches!(previous, CharKind::Cjk | CharKind::Other)
}

fn push_token(tokens: &mut Vec<Token>, text: &mut String, kind: CharKind) {
    if text.is_empty() {
        return;
    }
    let weight = match kind {
        CharKind::Whitespace => 0.0,
        CharKind::Other => PUNCTUATION_WEIGHT,
        CharKind::Cjk | CharKind::Latin | CharKind::Numeric => text.chars().count() as f64,
    };
    tokens.push(Token {
        text: std::mem::take(text),
        kind,
        weight,
    });
}

/// Keeps whitespace with the token it follows, so no token has an empty text.
///
/// Whitespace still separates tokens: the tokenizer already broke the run there.
fn fold_whitespace(tokens: Vec<Token>) -> Vec<Token> {
    let mut folded: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut leading = String::new();

    for token in tokens {
        if token.kind != CharKind::Whitespace {
            folded.push(token);
            continue;
        }
        match folded.last_mut() {
            Some(previous) => previous.text.push_str(&token.text),
            None => leading.push_str(&token.text),
        }
    }

    if !leading.is_empty() {
        match folded.first_mut() {
            Some(first) => first.text.insert_str(0, &leading),
            None => folded.push(Token {
                text: leading,
                kind: CharKind::Whitespace,
                weight: 0.0,
            }),
        }
    }
    folded
}

/// Attaches punctuation to its neighbouring word.
///
/// Opening brackets, quotes, and inverted marks start the word after them, and
/// every other mark ends the word before it. Punctuation at a line edge joins the
/// only word it can reach.
fn merge_punctuation(tokens: Vec<Token>) -> Vec<Token> {
    let mut merged: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut pending: Option<Token> = None;
    let mut quote_open = false;

    for token in tokens {
        if token.kind != CharKind::Other {
            let mut word = pending.take().unwrap_or_else(Token::empty);
            word.push(token);
            merged.push(word);
            continue;
        }

        let first = token.text.chars().next().unwrap_or_default();
        let attaches_right = if first == AMBIGUOUS_QUOTE {
            let attaches = !quote_open;
            quote_open = !quote_open;
            attaches
        } else {
            OPENING_PUNCTUATION.contains(first)
        };

        if attaches_right {
            match &mut pending {
                Some(pending) => pending.push(token),
                None => pending = Some(token),
            }
            continue;
        }

        match merged.last_mut() {
            Some(previous) => previous.push(token),
            None => match &mut pending {
                Some(pending) => pending.push(token),
                None => pending = Some(token),
            },
        }
    }

    if let Some(pending) = pending {
        match merged.last_mut() {
            Some(previous) => previous.push(pending),
            None => merged.push(pending),
        }
    }
    merged
}

fn char_kind(character: char) -> CharKind {
    if character.is_whitespace() {
        return CharKind::Whitespace;
    }
    if is_cjk(character) {
        return CharKind::Cjk;
    }
    if character.is_ascii_digit() {
        return CharKind::Numeric;
    }
    if character.is_alphabetic() || character == '\'' || is_combining_mark(character) {
        return CharKind::Latin;
    }
    CharKind::Other
}

/// Covers the character ranges the reference tool splits and their neighbours,
/// so halfwidth Katakana and the CJK extensions also segment per character.
fn is_cjk(character: char) -> bool {
    matches!(character as u32,
        0x1100..=0x11FF        // Hangul Jamo
        | 0x2E80..=0x2FDF      // CJK radicals and Kangxi radical supplements
        | 0x3040..=0x30FF      // Hiragana and Katakana
        | 0x3130..=0x318F      // Hangul compatibility Jamo
        | 0x3400..=0x4DBF      // CJK unified ideographs extension A
        | 0x4E00..=0x9FFF      // CJK unified ideographs
        | 0xA960..=0xA97F      // Hangul Jamo extended-A
        | 0xAC00..=0xD7AF      // Hangul syllables
        | 0xD7B0..=0xD7FF      // Hangul Jamo extended-B
        | 0xF900..=0xFAFF      // CJK compatibility ideographs
        | 0xFF66..=0xFF9F      // Halfwidth Katakana
        | 0x20000..=0x3FFFF    // CJK unified ideographs extensions B and later
    )
}

fn is_combining_mark(character: char) -> bool {
    matches!(character as u32,
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F
    )
}

#[cfg(test)]
#[path = "../test/segmentation_test.rs"]
mod tests;
