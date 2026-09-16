// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Speaker, background, and continuation conventions of the transcriptions
//! themselves.
//!
//! Neither QRC nor plain LRC has a field for a duet part or a background vocal,
//! so both are read from the conventions the transcriber wrote the lyrics with
//! and nothing beyond them: a row that names an artist of the matched track
//! divides the song between performers, a bracketed phrase answers the line it is
//! timed to, and a sentence left open at a comma is continued by the row that
//! follows. The provider's own artist list is what a label is matched against.

use crate::lyrics::model::{BackgroundVocal, TimedLine, TimedSyllable, Voice};

/// How long after the line it answers a bracketed echo may begin.
///
/// QQ Music usually times an echo to start exactly where its parent runs out, but
/// it answers the phrase rather than the line: Saddle Up's backing vocal begins
/// after the rest the line leaves it, one and a half seconds later. A couple of
/// seconds absorbs that without reaching a bracketed line belonging to some later
/// part of the song.
const ECHO_GAP_MS: u64 = 2_000;

/// Performance separators a joint credit is written with.
const LABEL_SEPARATORS: [char; 5] = ['/', '&', ',', '，', '、'];

/// Punctuation a transcription leaves a sentence open with.
const OPEN_MARKS: [char; 7] = [',', '，', '、', ';', '；', ':', '：'];

/// Punctuation that says the sentence before it ended.
const CLOSING_MARKS: [char; 13] = [
    '.', '。', '!', '！', '?', '？', '…', '"', '“', '”', ')', '）', '】',
];

/// Reads the speaker labels of `lines` into the voice of every line.
///
/// A label has to name one of `artists`, which are the ones the provider that
/// supplied these lyrics credits, so an ordinary sung line containing a colon is
/// left alone. A label-only row sets the voice of the rows that follow it and is
/// removed; an inline label is stripped from the display text and its word timing.
pub(super) fn apply_speaker_labels(lines: &mut Vec<TimedLine>, artists: &[String]) {
    let mut current_voice = Voice::Primary;

    lines.retain_mut(|line| {
        if let Some((voice, content_start)) = speaker_label(&line.text, artists) {
            current_voice = voice;
            strip_speaker_label(line, content_start);
        }
        line.voice = current_voice;
        !line.text.trim().is_empty()
    });
}

/// Reads one speaker label, which may credit several performers at once.
///
/// A joint credit is written lead-first, so the first performer the label names
/// that the provider also lists decides the side: a shared section belongs to the
/// performer credited first. One match is enough, which lets a featured performer
/// the provider omits from its artist list ride along on the name beside them.
fn speaker_label(text: &str, artists: &[String]) -> Option<(Voice, usize)> {
    let (colon_start, colon) = text
        .char_indices()
        .find(|(_, character)| matches!(character, ':' | '：'))?;
    let label = text[..colon_start].trim();
    if label.is_empty() {
        return None;
    }

    let leading = label.split(LABEL_SEPARATORS).find_map(|name| {
        let name = normalize_artist(name);
        artists
            .iter()
            .position(|artist| !name.is_empty() && name == normalize_artist(artist))
    })?;

    let after_colon = colon_start + colon.len_utf8();
    let content = text[after_colon..].trim_start();
    let content_start = text.len() - content.len();
    let voice = if leading == 0 {
        Voice::Primary
    } else {
        Voice::Secondary
    };
    Some((voice, content_start))
}

/// Removes a speaker label from a line and from the timing of its words.
fn strip_speaker_label(line: &mut TimedLine, content_start: usize) {
    let characters_to_remove = line.text[..content_start].chars().count();
    line.text = line.text[content_start..].to_string();

    let mut remaining = characters_to_remove;
    line.syllables.retain_mut(|syllable| {
        if remaining == 0 {
            return true;
        }

        let character_count = syllable.text.chars().count();
        if remaining >= character_count {
            remaining -= character_count;
            return false;
        }

        let byte_start = syllable
            .text
            .char_indices()
            .nth(remaining)
            .map_or(syllable.text.len(), |(index, _)| index);
        syllable.text.drain(..byte_start);
        remaining = 0;
        !syllable.text.is_empty()
    });
}

/// Splits a trailing bracketed tail off every line that answers with one.
///
/// QRC and plain LRC have no structured background-vocal field. What they do
/// carry is the convention the lyric was transcribed with: the backing part is
/// written as a bracketed tail on the line it answers, with word timings of its
/// own — NetEase's "Umbrella" opens `Ahuh Ahuh （Yea Rihanna）`, with the brackets
/// as zero-length syllables between the two parts.
///
/// The rule stays narrow, because a bracket is not evidence by itself. Sung text
/// has to remain in front of the tail, the tail has to contain a letter or digit,
/// and the line has to be word-timed, so an aside such as `(instrumental)` or a
/// line-timed source keeps its brackets and stays one ordinary line.
pub(super) fn split_background_vocals(lines: &mut [TimedLine]) {
    for line in lines {
        split_background_vocal(line);
    }
}

fn split_background_vocal(line: &mut TimedLine) {
    if line.background.is_some() || !line.is_word_timed() {
        return;
    }
    let Some(tail) = bracketed_tail(&line.text) else {
        return;
    };
    let head_slice = &line.text[..tail.open];
    let head_text = head_slice.trim_end().to_string();
    if head_text.is_empty() {
        return;
    }

    let mut head = line.syllables.clone();
    let background =
        unwrap_syllable_brackets(split_syllables_at(&mut head, head_slice.chars().count()));
    if head.iter().all(|syllable| syllable.text.trim().is_empty())
        || background
            .iter()
            .all(|syllable| syllable.text.trim().is_empty())
    {
        return;
    }

    line.background = Some(BackgroundVocal {
        text: tail.text,
        // The tail is sung inside the line it belongs to, so the line's own
        // translation covers it.
        translation: None,
        // Its words carry the timing the provider gave them, which is where it
        // sits inside its line.
        start_ms: background
            .first()
            .map_or(line.start_ms, |syllable| syllable.start_ms),
        end_ms: background.last().map(|syllable| syllable.end_ms),
        syllables: background,
    });
    line.text = head_text;
    line.syllables = head;
}

/// Removes the brackets that wrap a background vocal's own words.
///
/// The brackets belong to the phrase, not to the words it is sung with, and the
/// words that held nothing but a bracket are dropped with them.
fn unwrap_syllable_brackets(mut syllables: Vec<TimedSyllable>) -> Vec<TimedSyllable> {
    if let Some(first) = syllables.first_mut() {
        let text = first.text.trim_start();
        first.text = text
            .strip_prefix(['(', '（'])
            .unwrap_or(text)
            .trim_start()
            .to_string();
    }
    if let Some(last) = syllables.last_mut() {
        let text = last.text.trim_end();
        last.text = text
            .strip_suffix([')', '）'])
            .unwrap_or(text)
            .trim_end()
            .to_string();
    }
    syllables.retain(|syllable| !syllable.text.is_empty());
    syllables
}

/// Folds a bracketed phrase into the line it echoes.
///
/// The other half of the convention [`split_background_vocals`] reads: where
/// NetEase writes a backing vocal as a tail, QQ Music writes it as rows of its
/// own, bracketed and timed to the line they answer — "hate that i made you love
/// me" follows `Know that I will find my way from you` at 88377ms with `(My way
/// from you)` at 91796ms. A phrase longer than a row is written as consecutive
/// rows holding the opening and the closing bracket between them, the way Saddle
/// Up repeats its chorus under the outro, and it answers its parent after whatever
/// rest the phrase it answers leaves rather than always at its parent's last word.
///
/// The lyrics the renderer draws carry a background vocal attached to its parent,
/// so the two are joined here rather than left to scroll past as separate lines.
/// An echo has to begin inside the line it joins or just after it: a bracketed
/// line standing on its own elsewhere in the song is not an answer to whatever
/// happens to precede it.
pub(super) fn fold_bracketed_echoes(lines: &mut Vec<TimedLine>) {
    let mut index = 1;
    while index < lines.len() {
        let rows = if lines[index - 1].background.is_none() {
            echo_rows(lines, index)
        } else {
            0
        };
        if rows == 0 {
            index += 1;
            continue;
        }
        // The echo is sung where it stands, so its own translation belongs to it
        // rather than to the line it answers.
        let background = echo_background(&lines[index..index + rows]);
        lines.drain(index..index + rows);
        lines[index - 1].background = Some(background);
    }
}

/// Returns how many rows the echo starting at `lines[index]` is written across.
///
/// Zero when what stands there does not answer the line before it.
fn echo_rows(lines: &[TimedLine], index: usize) -> usize {
    let parent = &lines[index - 1];
    let Some(rows) = bracketed_phrase(lines, index) else {
        return 0;
    };
    let phrase = &lines[index..index + rows];
    // An aside such as `(instrumental)` answers nothing.
    if !phrase_text(phrase).chars().any(char::is_alphanumeric) {
        return 0;
    }
    let answers = phrase[0].start_ms >= parent.start_ms
        && phrase[0].start_ms <= parent.latest_time_ms().saturating_add(ECHO_GAP_MS);
    // The rows of a phrase longer than one are sung back to back, so a bracket
    // left unmatched elsewhere in the song cannot reach across to close it.
    let back_to_back = phrase
        .windows(2)
        .all(|rows| rows[1].start_ms <= rows[0].latest_time_ms().saturating_add(ECHO_GAP_MS));
    if answers && back_to_back { rows } else { 0 }
}

/// Reads the bracketed phrase opening at `lines[index]`.
///
/// A phrase opens on a row whose text begins with a bracket and closes on the row
/// that ends it — a row of its own is wholly bracketed, while a longer phrase holds
/// the opening bracket on its first row and the closing one on its last. Returns
/// how many rows it occupies, or `None` when the text does not spell a phrase that
/// closes.
fn bracketed_phrase(lines: &[TimedLine], index: usize) -> Option<usize> {
    if !lines[index].text.trim_start().starts_with(['(', '（']) {
        return None;
    }

    let mut depth = 0_usize;
    for (offset, line) in lines[index..].iter().enumerate() {
        for character in line.text.chars() {
            match character {
                '(' | '（' => depth += 1,
                ')' | '）' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        if depth == 0 {
            // The row that closes the phrase ends with the bracket that closes it;
            // text standing after it means the brackets are not the phrase's own.
            return line
                .text
                .trim_end()
                .ends_with([')', '）'])
                .then_some(offset + 1);
        }
    }
    None
}

/// The text of a bracketed phrase, without the brackets that mark it.
fn phrase_text(rows: &[TimedLine]) -> String {
    rows.iter()
        .enumerate()
        .map(|(offset, row)| {
            let mut piece = row.text.trim();
            if offset == 0 {
                piece = piece.strip_prefix(['(', '（']).unwrap_or(piece);
            }
            if offset + 1 == rows.len() {
                piece = piece.strip_suffix([')', '）']).unwrap_or(piece);
            }
            piece.trim()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Joins the rows of an echo into the background vocal of the line it answers.
fn echo_background(rows: &[TimedLine]) -> BackgroundVocal {
    let mut syllables = Vec::new();
    for row in rows {
        // The words keep the timing the echo was sung with, without the brackets
        // the phrase itself was written in.
        continue_words(
            &mut syllables,
            unwrap_syllable_brackets(row.syllables.clone()),
        );
    }

    let last = &rows[rows.len() - 1];
    BackgroundVocal {
        text: phrase_text(rows),
        // The bracket the translation is written in is the one the phrase was
        // written in, so it goes with the phrase rather than staying in the text.
        translation: echo_translation(rows),
        start_ms: syllables
            .first()
            .map_or(rows[0].start_ms, |syllable| syllable.start_ms),
        end_ms: syllables
            .last()
            .map(|syllable| syllable.end_ms)
            .or(last.end_ms),
        syllables,
    }
}

/// Returns the translations of a phrase's rows as the one it is sung with.
fn echo_translation(rows: &[TimedLine]) -> Option<String> {
    join_pieces(
        rows.iter()
            .filter_map(|row| row.translation.as_deref())
            .map(unwrap_brackets),
    )
}

/// Appends `words` to a run of words written on an earlier row.
///
/// A row that continues another is written without the space between them, because
/// the break in the transcription is where it belongs, so the last word of the run
/// before it carries the separator. The views draw the word stream rather than the
/// text, so a space that stays out of it would join the two runs' words on screen.
fn continue_words(target: &mut Vec<TimedSyllable>, mut words: Vec<TimedSyllable>) {
    if let Some(last) = target.last_mut()
        && !last.text.ends_with(char::is_whitespace)
    {
        last.text.push(' ');
    }
    target.append(&mut words);
}

/// Joins the pieces of a text written across rows with the space between them.
///
/// The rows say nothing where a piece does not carry one, and a run of nothing but
/// separators is no text at all.
fn join_pieces(pieces: impl IntoIterator<Item = String>) -> Option<String> {
    let text = pieces
        .into_iter()
        .map(|piece| piece.trim().to_string())
        .filter(|piece| !piece.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    (!text.is_empty()).then_some(text)
}

/// Merges the rows a sentence is broken across into the line that begins it.
///
/// A transcriber who runs out of room writes the rest of a sentence on the row
/// after the one that begins it, and the break falls wherever the line did:
/// Saddle Up writes `But I had enough,` then `so I move onto the next thing`, and
/// `Put your money` then `where your mouth is`, both timed to start where the row
/// they continue runs out. The rows are one sentence, so they are joined here into
/// the line the renderer draws.
///
/// What the row after says decides whether it belongs to the one before: a row that
/// opens with a lowercase word carries the sentence on, because a transcriber begins
/// a new one with a capitalized word. The English pronoun is written uppercase while
/// it is the word the rest of the sentence hangs on, and it begins a sentence of its
/// own just as often, so it carries on only a row that left its punctuation open, the
/// way `A tragedy, Ms. RIP,` does. The row before has to be able to say so at all: a
/// row the punctuation closed ends its sentence, and a row ending in a script without
/// letter case keeps its rows, because the transcriber of a language without case
/// breaks where the line ran out rather than where a sentence did.
///
/// This runs after the translations are paired, because a row carries the translation
/// of its own line until then, and after a bracketed echo is folded away, so the
/// phrase a line answers with stays where it was written.
pub(super) fn merge_continued_lines(lines: &mut Vec<TimedLine>) {
    let mut index = 0;
    while index + 1 < lines.len() {
        if !continues(&lines[index], &lines[index + 1]) {
            index += 1;
            continue;
        }
        // A sentence may be broken more than once, so the row that absorbed one is
        // looked at again rather than stepped over.
        let tail = lines.remove(index + 1);
        join_continuation(&mut lines[index], tail);
    }
}

/// Returns whether `tail` is the rest of the sentence `first` begins.
fn continues(first: &TimedLine, tail: &TimedLine) -> bool {
    let ends_with = first.text.trim_end();
    let Some(last) = ends_with.chars().next_back() else {
        return false;
    };
    // A row the punctuation closed says its sentence ended, whatever follows.
    if CLOSING_MARKS.contains(&last) {
        return false;
    }
    // A row says its sentence goes on by ending in the punctuation that leaves it
    // open, or in a letter whose case the transcription chose; a row in a script
    // without letter case breaks where the line did and says nothing.
    let left_open = OPEN_MARKS.contains(&last);
    let sentence_goes_on = left_open || has_case(last);
    first.voice == tail.voice
        // One line carries one background vocal, so joining two phrases would lose
        // one of them.
        && !(first.background.is_some() && tail.background.is_some())
        // The words of the merged line are the words of both rows, so the two are
        // joined only when they are written the same way: a row timed by the word
        // and a row timed by the line each spell their text differently.
        && first.syllables.is_empty() == tail.syllables.is_empty()
        && opens_a_continuation(tail.text.trim_start(), sentence_goes_on, left_open)
}

/// Returns whether `text` opens with a word that carries a sentence on.
///
/// `sentence_goes_on` is whether the row before it said the sentence continues, and
/// `left_open` whether it said so with the punctuation it ended in.
fn opens_a_continuation(text: &str, sentence_goes_on: bool, left_open: bool) -> bool {
    let mut characters = text.chars();
    match characters.next() {
        Some(first) if first.is_lowercase() => sentence_goes_on,
        // The English pronoun begins a sentence of its own as readily as it carries
        // one on, so only punctuation that says the sentence goes on decides it.
        Some('I') => left_open && matches!(characters.next(), None | Some(' ' | '\'' | '’')),
        _ => false,
    }
}

/// Returns whether `character` is a letter a transcription chooses the case of.
fn has_case(character: char) -> bool {
    character.is_uppercase() || character.is_lowercase()
}

/// Joins the rest of a sentence onto the line that begins it.
///
/// The readings of a line are generated after parsing, from the line as it is
/// merged here, so only what the transcription states is carried over.
fn join_continuation(first: &mut TimedLine, tail: TimedLine) {
    let end_ms = tail.latest_time_ms();
    first.text = format!("{} {}", first.text.trim_end(), tail.text.trim_start());
    continue_words(&mut first.syllables, tail.syllables);
    first.translation = join_pieces(first.translation.take().into_iter().chain(tail.translation));
    if let Some(background) = tail.background {
        first.background.get_or_insert(background);
    }
    // The sentence runs to the end of the row that finished it.
    first.end_ms = Some(first.latest_time_ms().max(end_ms));
}

/// Removes the brackets a phrase and its translation are written inside.
///
/// A provider brackets the phrase it repeats and translates the bracket along
/// with it — `(来吧 尽管…)` translates `(Come on, just…)`. The bracket is how the
/// transcription marks the phrase, not what it says, so it is not shown again
/// where the background vocal is already drawn as one.
pub(super) fn unwrap_brackets(text: &str) -> String {
    let trimmed = text.trim();
    let Some(open) = trimmed.chars().next() else {
        return String::new();
    };
    let Some(close) = matching_bracket(open) else {
        return trimmed.to_string();
    };
    if !trimmed.ends_with(close) {
        return trimmed.to_string();
    }
    trimmed[open.len_utf8()..trimmed.len() - close.len_utf8()]
        .trim()
        .to_string()
}

fn matching_bracket(open: char) -> Option<char> {
    match open {
        '(' => Some(')'),
        '（' => Some('）'),
        '[' => Some(']'),
        '【' => Some('】'),
        _ => None,
    }
}

/// Where a line's trailing bracketed segment starts, and what it says.
struct BracketedTail {
    /// Byte index of the opening bracket within the line text.
    open: usize,
    /// The text between the brackets.
    text: String,
}

fn bracketed_tail(text: &str) -> Option<BracketedTail> {
    let trimmed = text.trim_end();
    let close = trimmed
        .chars()
        .next_back()
        .filter(|character| matches!(character, ')' | '）'))?;

    let mut depth = 0_usize;
    let mut open = None;
    for (index, character) in trimmed.char_indices().rev() {
        match character {
            ')' | '）' => depth += 1,
            '(' | '（' => {
                // The scan starts on the closing bracket, so the depth of an
                // opening one is never zero.
                depth -= 1;
                if depth == 0 {
                    open = Some((index, character));
                    break;
                }
            }
            _ => {}
        }
    }

    let (open, bracket) = open?;
    let inner = trimmed[open + bracket.len_utf8()..trimmed.len() - close.len_utf8()].trim();
    if !inner.chars().any(char::is_alphanumeric) {
        return None;
    }
    Some(BracketedTail {
        open,
        text: inner.to_string(),
    })
}

/// Splits `syllables` at a character offset, returning the tail.
///
/// A syllable that straddles the boundary is divided in proportion to its
/// characters, so the two halves stay contiguous in time. An offset past the end
/// yields an empty tail, which the caller reads as "do not split".
fn split_syllables_at(
    syllables: &mut Vec<TimedSyllable>,
    character_index: usize,
) -> Vec<TimedSyllable> {
    let mut seen = 0_usize;
    for index in 0..syllables.len() {
        if seen == character_index {
            return syllables.split_off(index);
        }
        let count = syllables[index].text.chars().count();
        if seen + count > character_index {
            let divided = divide(&mut syllables[index], character_index - seen);
            let mut tail = syllables.split_off(index + 1);
            tail.insert(0, divided);
            return tail;
        }
        seen += count;
    }
    Vec::new()
}

fn divide(syllable: &mut TimedSyllable, character_offset: usize) -> TimedSyllable {
    let characters = syllable.text.chars().count();
    let byte_offset = syllable
        .text
        .char_indices()
        .nth(character_offset)
        .map_or(syllable.text.len(), |(index, _)| index);
    let span = syllable.end_ms.saturating_sub(syllable.start_ms);
    // Only a syllable whose text covers the boundary is divided, so its
    // character count and the offset within it are both at least one.
    let offset = character_offset as u64;
    let total = characters as u64;
    let boundary = syllable.start_ms.saturating_add(span * offset / total);

    let text = syllable.text.split_off(byte_offset);
    let divided = TimedSyllable {
        text,
        start_ms: boundary,
        end_ms: syllable.end_ms,
        romanization: String::new(),
        furigana: String::new(),
    };
    syllable.end_ms = boundary;
    divided
}

/// Normalizes an artist name or a label naming one.
///
/// Casing, spacing, and punctuation differ between a provider's billing and the
/// label it writes in the lyrics, so only letters and digits are compared.
fn normalize_artist(value: &str) -> String {
    let cleaned: String = value
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
#[path = "../../test/conventions_test.rs"]
mod tests;
