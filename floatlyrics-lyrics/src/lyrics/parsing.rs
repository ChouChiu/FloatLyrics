// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Conversion between provider payloads and display-ready timed lyrics.

use std::borrow::Cow;

use anyhow::{Context, Result, anyhow};
use lyrics_helper::helpers::optimization::explicit::fix_explicit;
use lyrics_helper::{
    LineInfo, LyricsData, LyricsTypes, SyllableItem, generate_string, parse_auto as parse_helper,
};

use super::model::{BackgroundVocal, TimedLine, TimedSyllable, Voice};

mod conventions;
mod filter;
mod lrc;
mod qrc;

use conventions::{
    apply_speaker_labels, fold_bracketed_echoes, merge_continued_lines, split_background_vocals,
};
use lrc::timed_lines_from_lrc;
use qrc::timed_lines_from_qrc;

const TRANSLATION_PREFIX: &str = "__FLOATLYRICS_TRANSLATION__:";
const TRANSLATION_SECTION_MARKER: &str = "[floatlyrics:translation]";

/// Parses a local lyrics document using the formats supported by `lyrics-helper`.
/// XML-based formats are rejected until the transitive XML parser can be
/// upgraded to a release that safely handles untrusted attributes.
///
/// # Errors
/// Returns an error when the input is XML or the format cannot be detected or parsed.
pub fn parse_local_lyrics(content: &str) -> Result<LyricsData> {
    // `lyrics-parsers` reads TTML through quick-xml 0.36's checked attribute
    // iterator and namespace resolver, which hold the quadratic run time and the
    // unbounded namespace allocation of RUSTSEC-2026-0194 and RUSTSEC-2026-0195.
    // Reject XML before reaching that parser; QQ Music and NetEase payloads use
    // the LRC/QRC paths above this fallback.
    if content
        .trim_start_matches(|character: char| character.is_whitespace() || character == '\u{feff}')
        .starts_with('<')
    {
        return Err(anyhow!("XML lyrics are temporarily unsupported"));
    }
    parse_helper(content).context("lyrics-helper could not detect or parse lyrics")
}

/// Parses raw provider lyrics into sorted, display-ready lines.
///
/// `artists` are the ones the provider that supplied these lyrics credits, and
/// are what a speaker label in the payload is matched against. Words the provider
/// censored are restored before anything reads the text, translation sections are
/// merged, the conventions of the transcription itself are read into the lines,
/// and known metadata or credit lines are removed. Provider pronunciation is
/// intentionally discarded; call [`super::generate_local_romanization`] when local
/// romanization is wanted.
///
/// # Errors
/// Returns an error when no supported timed format can be parsed.
pub fn timed_lines_from_raw(content: &str, artists: &[String]) -> Result<Vec<TimedLine>> {
    // A provider censors the explicit words it serves by leaving the shape of the
    // word behind (`s**t`, `b***h`, `mother****in'`), and upstream's word list
    // restores the ones that shape still spells; a mask that kept no letter at all
    // (`*******`) stays a mask, because nothing left in the payload says the word.
    // Restoring before the payload is read keeps the line text and the words it is
    // timed by spelling the same.
    let content = if content.contains('*') {
        Cow::Owned(fix_explicit(content))
    } else {
        Cow::Borrowed(content)
    };
    let (lyrics, translation) = split_translation_section(&content);
    let mut lines = parse_timed_lines_block(lyrics, artists)?;
    if let Some(translation) = translation {
        // A translation document is written for the track rather than by one of
        // its performers, so no label in it names a voice.
        let translation_lines = parse_timed_lines_block(translation, &[])?;
        merge_translation_lines(&mut lines, translation_lines);
    }
    // After the translations are paired: an echo is a line of its own until it is
    // folded away, so it collects the translation timed to it, and each row of a
    // sentence carries its own translation until the rows are joined.
    fold_bracketed_echoes(&mut lines);
    merge_continued_lines(&mut lines);
    require_lines(lines)
}

/// Returns the lines a payload parsed into, or the error for one without any.
fn require_lines(lines: Vec<TimedLine>) -> Result<Vec<TimedLine>> {
    if lines.is_empty() {
        return Err(anyhow!("lyrics did not include timed lines"));
    }
    Ok(lines)
}

fn parse_timed_lines_block(content: &str, artists: &[String]) -> Result<Vec<TimedLine>> {
    let mut lines = timed_lines_from_lrc(content);
    if lines.is_empty() {
        lines = timed_lines_from_qrc(content);
    }
    if !lines.is_empty() {
        return Ok(finish(lines, artists));
    }

    let parsed = parse_local_lyrics(content)?;
    require_lines(timed_lines_from_data(&parsed, artists))
}

/// Reads a provider's own transcription conventions and drops non-lyric rows.
///
/// A label may name a performer, so it is read before the filter drops the rows
/// that look like one; a bracketed tail is only split off a word-timed line, so
/// this runs before the timed units are split into words.
fn finish(lines: Vec<TimedLine>, artists: &[String]) -> Vec<TimedLine> {
    let mut lines = merge_translation_marker_lines(lines);
    apply_speaker_labels(&mut lines, artists);
    split_background_vocals(&mut lines);
    filter_display_lines(lines)
}

/// Serializes parsed lyrics as `ty`.
///
/// # Errors
/// Returns an error when `lyrics-helper` cannot generate the requested format.
pub fn export_lyrics(data: &LyricsData, ty: LyricsTypes) -> Result<String> {
    generate_string(data, ty).context("lyrics-helper could not generate lyrics in requested format")
}

/// Converts already parsed lyrics to sorted, display-ready lines.
///
/// `artists` are matched against speaker labels exactly as in
/// [`timed_lines_from_raw`]. Provider pronunciation is intentionally discarded;
/// call [`super::generate_local_romanization`] when local romanization is wanted.
pub fn timed_lines_from_data(data: &LyricsData, artists: &[String]) -> Vec<TimedLine> {
    let Some(lines) = data.lines.as_deref() else {
        return Vec::new();
    };

    let mut timed_lines = lines
        .iter()
        .filter_map(timed_line_from_info)
        .collect::<Vec<_>>();
    timed_lines.sort_by_key(|line| line.start_ms);
    let mut timed_lines = finish(timed_lines, artists);
    // A row carries the translation of its own line, so the rows of one sentence are
    // joined once they have both been paired with it.
    merge_continued_lines(&mut timed_lines);
    timed_lines
}

/// Combines a primary document and optional translation into one parseable payload.
pub fn combine_lyrics_with_translation(lyrics: &str, translation: Option<&str>) -> String {
    let Some(translation) = translation else {
        return lyrics.to_string();
    };
    if translation.trim().is_empty() {
        lyrics.to_string()
    } else {
        format!(
            "{}\n{}\n{}\n",
            lyrics.trim_end(),
            TRANSLATION_SECTION_MARKER,
            translation.trim()
        )
    }
}

fn timed_line_from_info(line: &LineInfo) -> Option<TimedLine> {
    let start_ms = ms_i32_to_u64(line.start_time_with_sub_line()?)?;
    let end_ms = line.end_time_with_sub_line().and_then(ms_i32_to_u64);
    let text = line.full_text();
    let syllables = timed_syllables_from_info(line);
    let translation = preferred_translation(line);
    let background = line.sub_line().map(|sub| BackgroundVocal {
        text: sub.text_from_any(),
        // A bracketed phrase is translated inside its brackets, which the
        // background vocal is already drawn with.
        translation: preferred_translation(sub)
            .map(|translation| conventions::unwrap_brackets(&translation)),
        start_ms: sub.start_time().and_then(ms_i32_to_u64).unwrap_or(start_ms),
        end_ms: sub.end_time().and_then(ms_i32_to_u64),
        syllables: timed_syllables_from_info(sub),
    });

    if text.trim().is_empty() && translation.is_none() {
        return None;
    }

    Some(TimedLine {
        start_ms,
        end_ms,
        text,
        syllables,
        translation,
        romanization: None,
        romanization_segments: Vec::new(),
        background,
        voice: Voice::Primary,
    })
}

fn timed_syllables_from_info(line: &LineInfo) -> Vec<TimedSyllable> {
    match line {
        LineInfo::Syllable { syllables, .. } | LineInfo::FullSyllable { syllables, .. } => {
            syllables
                .iter()
                // A merged word keeps the items it was merged from, whose times are
                // the ones a renderer animates; the aggregate would only approximate
                // them.
                .flat_map(SyllableItem::parts)
                .filter_map(|syllable| {
                    Some(TimedSyllable {
                        start_ms: ms_i32_to_u64(syllable.start_time)?,
                        end_ms: ms_i32_to_u64(syllable.end_time)?,
                        text: syllable.text.clone(),
                        romanization: String::new(),
                        furigana: String::new(),
                    })
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn merge_translation_marker_lines(lines: Vec<TimedLine>) -> Vec<TimedLine> {
    let mut merged: Vec<TimedLine> = Vec::new();

    for mut line in lines {
        if let Some(translation) = translation_marker_text(&line.text) {
            if let Some(target) = merged
                .iter_mut()
                .rev()
                .find(|target| target.start_ms == line.start_ms)
            {
                target.translation = clean_optional_text(translation);
            }
            continue;
        }

        // A background vocal whose text is a translation marker *is* the
        // translation; any other one stays the background it is.
        let marker = line
            .background
            .as_ref()
            .and_then(|background| marker_owned(&background.text));
        if marker.is_some() {
            line.background = None;
        }
        line.translation = line
            .translation
            .as_deref()
            .and_then(clean_optional_text)
            .or(marker);
        merged.push(line);
    }

    merged
}

fn merge_translation_lines(lines: &mut [TimedLine], translation_lines: Vec<TimedLine>) {
    for translation in translation_lines {
        let target = if let Some(index) = lines
            .iter()
            .position(|line| line.start_ms == translation.start_ms)
        {
            lines.get_mut(index)
        } else {
            nearest_line_mut(lines, translation.start_ms, 800)
        };

        if let Some(target) = target {
            target.translation = clean_optional_text(&translation.text);
        }
    }
}

fn nearest_line_mut(
    lines: &mut [TimedLine],
    start_ms: u64,
    tolerance_ms: u64,
) -> Option<&mut TimedLine> {
    let index = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let diff = line.start_ms.abs_diff(start_ms);
            (diff <= tolerance_ms).then_some((index, diff))
        })
        .min_by_key(|(_, diff)| *diff)
        .map(|(index, _)| index)?;

    lines.get_mut(index)
}

fn split_translation_section(content: &str) -> (&str, Option<&str>) {
    content
        .split_once(TRANSLATION_SECTION_MARKER)
        .map_or((content, None), |(lyrics, translation)| {
            (lyrics, Some(translation))
        })
}

fn translation_marker_text(value: &str) -> Option<&str> {
    value
        .trim()
        .strip_prefix(TRANSLATION_PREFIX)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn marker_owned(value: &str) -> Option<String> {
    translation_marker_text(value).and_then(clean_optional_text)
}

fn clean_optional_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || is_placeholder_text(trimmed) {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn is_placeholder_text(value: &str) -> bool {
    let normalized = value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();

    matches!(normalized.as_str(), "//" | "/" | "／" | "／／")
}

fn ms_i32_to_u64(value: i32) -> Option<u64> {
    value.try_into().ok()
}

fn preferred_translation(line: &LineInfo) -> Option<String> {
    let translations = line.translations()?;
    translations
        .get("zh")
        .or_else(|| translations.get("zh-Hans"))
        .or_else(|| translations.get("zh-CN"))
        .or_else(|| translations.values().next())
        .and_then(|value| clean_optional_text(value))
}

fn filter_display_lines(lines: Vec<TimedLine>) -> Vec<TimedLine> {
    lines
        .into_iter()
        .filter(|line| !filter::is_non_lyric_display_line(line))
        .collect()
}
