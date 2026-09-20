// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! AMLL TTML lyrics serialization.
//!
//! The lyrics player reads a word-timed TTML document, which is where a reading
//! can be written above the characters it belongs to: [`tts:ruby`] spans hold the
//! kana of a word, so a Japanese line reaches the listener as furigana and as
//! readings rather than as readings alone.
//!
//! The shape follows the generator of `@applemusic-like-lyrics/ttml`
//! (<https://github.com/amll-dev/applemusic-like-lyrics>), which is what the
//! player parses: one `<p>` per line, one `<span>` per karaoke word, translations
//! in an `x-translation` span, background vocals in an `x-bg` span, and the
//! readings of every line in the metadata the player matches them to the words
//! of that line from.
//!
//! [`tts:ruby`]: https://www.w3.org/TR/ttml2/#ruby

use std::fmt::Write as _;

use floatlyrics_lyrics::lyrics::Voice;

use crate::shared::presentation::{LyricsDocument, PresentedLyricLine};

/// Duration given to the last line when neither the player nor the next line
/// bounds it.
const LAST_LINE_FALLBACK_MS: u64 = 5_000;

/// Language the readings are written in, as the `x-roman` span is tagged with.
const READING_LANGUAGE: &str = "ja-Latn";

/// Language the connected providers translate into.
const TRANSLATION_LANGUAGE: &str = "zh-CN";

/// Serializes a lyrics document into an AMLL TTML document.
///
/// An empty document stays a valid one, which is how a listener is told that the
/// current track has no lyrics.
pub(super) fn document(document: &LyricsDocument) -> String {
    let lines = document
        .lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let end_ms = resolved_end_ms(line, document.lines.get(index + 1), document.duration_ms);
            // An echo sung after the line it answers runs past its end, and the
            // line is what a listener reads the echo as part of.
            let end_ms = line
                .background_end_ms
                .map_or(end_ms, |background_end| end_ms.max(background_end));
            PreparedLine {
                id: index + 1,
                line,
                end_ms,
                words: words(line, end_ms),
            }
        })
        .collect::<Vec<_>>();
    let end_ms = lines.last().map_or(0, |line| line.end_ms);
    let mut output = String::with_capacity(512 + lines.len() * 512);

    output.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>");
    output.push_str(
        "<tt xmlns=\"http://www.w3.org/ns/ttml\" \
         xmlns:ttm=\"http://www.w3.org/ns/ttml#metadata\" \
         xmlns:tts=\"http://www.w3.org/ns/ttml#styling\" \
         xmlns:itunes=\"http://music.apple.com/lyric-ttml-internal\" \
         itunes:timing=\"Word\">",
    );
    write_head(&mut output, &lines);
    output.push_str("<body dur=\"");
    write_time(&mut output, end_ms);
    output.push_str("\"><div begin=\"");
    write_time(
        &mut output,
        lines.first().map_or(0, |line| line.line.start_ms),
    );
    output.push_str("\" end=\"");
    write_time(&mut output, end_ms);
    output.push_str("\">");

    for line in &lines {
        write_line(&mut output, line);
    }

    output.push_str("</div></body></tt>");
    output
}

/// One line with the words it is written with, ready to be written out.
struct PreparedLine<'a> {
    /// Number the line is addressed by, counting from one.
    id: usize,
    line: &'a PresentedLyricLine,
    end_ms: u64,
    words: Vec<Word>,
}

/// Writes the readings of the lines into the metadata a player reads them from.
///
/// A reading is written with the timing of the word it belongs to, which is how
/// the player knows which word it explains; the readings of a line are written
/// here rather than beside it, because only what is written here is matched to
/// the words.
fn write_head(output: &mut String, lines: &[PreparedLine<'_>]) {
    if !lines.iter().any(has_reading) {
        return;
    }

    output.push_str(
        "<head><metadata><iTunesMetadata xmlns=\"http://music.apple.com/lyric-ttml-internal\">\
         <transliterations><transliteration xml:lang=\"",
    );
    output.push_str(READING_LANGUAGE);
    output.push_str("\">");
    for line in lines.iter().filter(|line| has_reading(line)) {
        let _ = write!(output, "<text for=\"L{}\">", line.id);
        for word in line
            .words
            .iter()
            .filter(|word| !word.romanization.is_empty())
        {
            write_span_open(output, word.start_ms, word.end_ms);
            write_text(output, &word.romanization);
            output.push_str("</span>");
        }
        output.push_str("</text>");
    }
    output.push_str("</transliteration></transliterations></iTunesMetadata></metadata></head>");
}

/// Whether a line has any reading to write.
fn has_reading(line: &PreparedLine<'_>) -> bool {
    line.words.iter().any(|word| !word.romanization.is_empty())
}

/// Writes one line, its words, and the text written around them.
fn write_line(output: &mut String, prepared: &PreparedLine<'_>) {
    let line = prepared.line;
    output.push_str("<p begin=\"");
    write_time(output, line.start_ms);
    output.push_str("\" end=\"");
    write_time(output, prepared.end_ms);
    let _ = write!(
        output,
        "\" ttm:agent=\"{}\" itunes:key=\"L{}\">",
        agent(line),
        prepared.id
    );

    for word in &prepared.words {
        write_word(output, word);
    }

    let translation = line.translation.trim();
    if !translation.is_empty() {
        output.push_str("<span ttm:role=\"x-translation\" xml:lang=\"");
        output.push_str(TRANSLATION_LANGUAGE);
        output.push_str("\">");
        write_text(output, translation);
        output.push_str("</span>");
    }

    let background = line.background.trim();
    if !background.is_empty() {
        output.push_str("<span ttm:role=\"x-bg\">");
        write_background_words(output, line, prepared.end_ms);
        // A background vocal is translated where it is sung rather than where the
        // line it answers is, so its translation is written inside its own span.
        let background_translation = line.background_translation.trim();
        if !background_translation.is_empty() {
            output.push_str("<span ttm:role=\"x-translation\" xml:lang=\"");
            output.push_str(TRANSLATION_LANGUAGE);
            output.push_str("\">");
            write_text(output, background_translation);
            output.push_str("</span>");
        }
        output.push_str("</span>");
    }

    output.push_str("</p>");
}

/// Writes the words of a background vocal, wrapped in the brackets that mark it.
///
/// A background vocal is sung at its own time, which is where it starts and ends
/// rather than where the line it answers does. Its words are written one span
/// each when the provider timed them, so a listener fills the words it hears as
/// it hears them; the brackets that mark the phrase go on the words at its edges,
/// which is the shape a listener strips them from again.
fn write_background_words(output: &mut String, line: &PresentedLyricLine, line_end_ms: u64) {
    let start_ms = line.background_start_ms;
    let end_ms = line.background_end_ms.unwrap_or(line_end_ms).max(start_ms);
    let words = line
        .background_syllables
        .iter()
        .filter(|syllable| !syllable.text.is_empty())
        .collect::<Vec<_>>();
    if words.is_empty() {
        write_span_open(output, start_ms, end_ms);
        output.push('(');
        write_text(output, line.background.trim());
        output.push_str(")</span>");
        return;
    }

    for (index, syllable) in words.iter().enumerate() {
        write_span_open(
            output,
            syllable.start_ms,
            syllable.end_ms.max(syllable.start_ms),
        );
        if index == 0 {
            output.push('(');
        }
        write_text(output, &syllable.text);
        if index + 1 == words.len() {
            output.push(')');
        }
        output.push_str("</span>");
    }
}

/// Identifier written as the agent of a line sung by the main part.
///
/// The listener alternates the duet sides from the agent it reads on each line, so
/// the main part is the value it defaults to and the opposing part is not.
const PRIMARY_AGENT: &str = "v1";

/// Identifier written as the agent of a line sung by the opposing part.
const SECONDARY_AGENT: &str = "v2";

/// The agent a line is written for.
///
/// TTML carries the performer of a line, which is how a listener renders a duet
/// with the two sides distinguished. A line whose performer the payload did not
/// state belongs to the main part.
fn agent(line: &PresentedLyricLine) -> &'static str {
    match line.voice {
        Voice::Primary => PRIMARY_AGENT,
        Voice::Secondary => SECONDARY_AGENT,
    }
}

/// One karaoke word: the text it is written with and the readings above and
/// below it.
struct Word {
    start_ms: u64,
    end_ms: u64,
    text: String,
    /// Kana written above the word.
    furigana: String,
    /// Reading written below the word.
    romanization: String,
}

/// Collects the words of a line, writing the characters that share one reading
/// as one word.
///
/// A word read as a whole — `大人` is "otona" — carries its reading on the
/// character it starts at, so the characters it covers are written together: the
/// kana above them belongs to all of them, and a view can only draw it above the
/// word it was given. A character that carries a reading of its own starts a word
/// of its own, which is what keeps `運命` reading "un" and then "mei".
fn words(line: &PresentedLyricLine, end_ms: u64) -> Vec<Word> {
    let mut words: Vec<Word> = Vec::with_capacity(line.syllables.len());

    for syllable in &line.syllables {
        if syllable.text.is_empty() {
            continue;
        }
        // A fragment that is only a space is not sung: it stays the space of the
        // word it was written after.
        if syllable.text.trim().is_empty() {
            if let Some(word) = words.last_mut() {
                word.text.push_str(&syllable.text);
            }
            continue;
        }
        let continues = words.last().is_some_and(|word| {
            !word.furigana.is_empty()
                && syllable.furigana.is_empty()
                && is_read(syllable.text.trim())
                && !word.text.ends_with(char::is_whitespace)
        });
        if continues {
            let word = words.last_mut().expect("the word was just checked");
            word.text.push_str(&syllable.text);
            word.romanization.push_str(syllable.romanization.trim());
            word.end_ms = word.end_ms.max(syllable.end_ms);
            continue;
        }

        words.push(Word {
            start_ms: syllable.start_ms,
            end_ms: syllable.end_ms.max(syllable.start_ms),
            text: syllable.text.clone(),
            furigana: syllable.furigana.clone(),
            romanization: syllable.romanization.trim().to_string(),
        });
    }

    if words.is_empty() {
        words.push(Word {
            start_ms: line.start_ms,
            end_ms,
            text: line.text.trim().to_string(),
            furigana: String::new(),
            romanization: line.romanization.trim().to_string(),
        });
    }
    words
}

/// Writes one word: its kana above it when it has any, and its timing either way.
fn write_word(output: &mut String, word: &Word) {
    let text = word.text.trim_end_matches(char::is_whitespace);

    if word.furigana.is_empty() {
        write_span_open(output, word.start_ms, word.end_ms);
        write_text(output, text);
        output.push_str("</span>");
    } else {
        output.push_str("<span tts:ruby=\"container\"><span tts:ruby=\"base\">");
        write_text(output, text);
        output.push_str("</span><span tts:ruby=\"textContainer\"><span tts:ruby=\"text\" begin=\"");
        write_time(output, word.start_ms);
        output.push_str("\" end=\"");
        write_time(output, word.end_ms);
        output.push_str("\">");
        write_text(output, &word.furigana);
        output.push_str("</span></span></span>");
    }

    // The space a word was written with stays a space of its own, so a listener
    // renders the spacing of the line rather than of its words.
    if text.len() != word.text.len() {
        output.push(' ');
    }
}

/// Writes the opening of a word span carrying its timing.
fn write_span_open(output: &mut String, start_ms: u64, end_ms: u64) {
    output.push_str("<span begin=\"");
    write_time(output, start_ms);
    output.push_str("\" end=\"");
    write_time(output, end_ms);
    output.push_str("\">");
}

/// Writes `milliseconds` the way TTMl writes a clock time: minutes only when the
/// timestamp has any, and milliseconds with three digits.
fn write_time(output: &mut String, milliseconds: u64) {
    let seconds = milliseconds / 1_000;
    let millis = milliseconds % 1_000;
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    if minutes > 0 {
        let _ = write!(output, "{minutes}:{seconds:02}.{millis:03}");
    } else {
        let _ = write!(output, "{seconds}.{millis:03}");
    }
}

/// Writes `text` with the characters XML reserves escaped.
fn write_text(output: &mut String, text: &str) {
    for character in text.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&apos;"),
            _ => output.push(character),
        }
    }
}

/// Whether a character is read rather than spelled, so that a kana written above
/// it belongs there.
fn is_read(text: &str) -> bool {
    !text.is_empty()
        && text.chars().all(|character| {
            matches!(character as u32, 0x4E00..=0x9FFF | 0x3400..=0x4DBF) || character == '々'
        })
}

/// Resolves the exclusive end of a line the same way the bundled lyrics view
/// does: an explicit end, the next line, the last timed word, the track
/// duration, then a fixed fallback.
fn resolved_end_ms(
    line: &PresentedLyricLine,
    next: Option<&PresentedLyricLine>,
    duration_ms: Option<u64>,
) -> u64 {
    let start_time = line.start_ms;
    let last_word_end = line
        .syllables
        .iter()
        .map(|syllable| syllable.end_ms)
        .max()
        .unwrap_or(start_time);
    [
        line.end_ms,
        next.map(|line| line.start_ms),
        Some(last_word_end),
        duration_ms,
    ]
    .into_iter()
    .flatten()
    .find(|end| *end > start_time)
    .unwrap_or_else(|| start_time.saturating_add(LAST_LINE_FALLBACK_MS))
}

#[cfg(test)]
#[path = "../../test/amll_ttml_test.rs"]
mod tests;
