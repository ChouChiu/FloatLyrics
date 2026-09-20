// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Reading a word-timed document onto the text of the row-timed one.

use super::super::model::{TimedLine, TimedSyllable};

/// How far a word-timed row may sit from the row-timed row it belongs to.
///
/// NetEase times the words of a line from where the singing starts and the rows
/// of the same line from a moment of its own, and the two are a fraction of a
/// second apart on a line whose text is the same; rows are matched in order, so
/// the window only has to hold that drift.
const ROW_TOLERANCE_MS: u64 = 1_000;

/// Attaches the word timings of `word_lines` to the rows of `lines`.
///
/// A word-timed document times every word, but the text it spells them with is a
/// timing table rather than the transcription a listener reads: the provider drops
/// the separators between its words, and reads the brackets of an aside as the
/// brackets of a tag. The row-timed transcription of the same track is the text
/// that was written to be read, so the rows keep it and take only the times of the
/// words, one row at a time. A row whose text the words cannot be read onto — an
/// aside the word-timed document never carried, a word it censored where the row
/// spells it out — keeps its row timing and carries no words.
pub(super) fn apply_word_timings(lines: &mut [TimedLine], word_lines: &[TimedLine]) {
    let mut next_word_line = 0;

    for line in lines.iter_mut() {
        while word_lines
            .get(next_word_line)
            .is_some_and(|word_line| word_line.start_ms + ROW_TOLERANCE_MS < line.start_ms)
        {
            next_word_line += 1;
        }
        let Some(word_line) = word_lines.get(next_word_line) else {
            break;
        };
        if word_line.start_ms.abs_diff(line.start_ms) > ROW_TOLERANCE_MS {
            continue;
        }
        let Some(syllables) = read_words_onto(&line.text, &word_line.syllables) else {
            continue;
        };

        line.syllables = syllables;
        next_word_line += 1;
    }
}

/// Spells the words of `syllables` with the characters of `text`.
///
/// Each word keeps its own timing and takes the characters the transcription
/// wrote for it, which are the characters up to and including its last word
/// character; the separators around it are written where the transcription wrote
/// them, so the words of a row spell that row exactly. Returns `None` when the two
/// do not say the same words, which is the only case in which the word times
/// cannot be trusted to belong to these characters.
fn read_words_onto(text: &str, syllables: &[TimedSyllable]) -> Option<Vec<TimedSyllable>> {
    let mut remaining = text.chars().peekable();
    let mut aligned: Vec<TimedSyllable> = Vec::with_capacity(syllables.len());

    for syllable in syllables {
        let mut word = String::new();
        // A separator stands between this word and the one before it, and a
        // transcription writes it at the end of that word rather than the start of
        // this one.
        while let Some(separator) = remaining.next_if(|character| !character.is_alphanumeric()) {
            match aligned.last_mut() {
                Some(previous) => previous.text.push(separator),
                None => word.push(separator),
            }
        }

        let mut expected = syllable
            .text
            .chars()
            .filter(|character| character.is_alphanumeric());
        let mut word_characters = syllable
            .text
            .chars()
            .filter(|character| character.is_alphanumeric())
            .count();
        // A word is the characters up to its last word character, so a separator
        // inside it — the apostrophe of `you're` — is taken without ending it.
        while word_characters > 0 {
            let character = remaining.next()?;
            if character.is_alphanumeric() {
                let expected = expected.next()?;
                if !character.eq_ignore_ascii_case(&expected) {
                    return None;
                }
                word_characters -= 1;
            }
            word.push(character);
        }

        if !word.is_empty() {
            aligned.push(TimedSyllable {
                start_ms: syllable.start_ms,
                end_ms: syllable.end_ms,
                text: word,
                romanization: String::new(),
                furigana: String::new(),
            });
        }
    }

    // Whatever the transcription wrote after the last word stays with it.
    for character in remaining {
        if character.is_alphanumeric() {
            return None;
        }
        aligned.last_mut()?.text.push(character);
    }

    (!aligned.is_empty()).then_some(aligned)
}

#[cfg(test)]
#[path = "../../test/word_timing_test.rs"]
mod tests;
