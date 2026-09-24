// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Conversion between provider payloads and display-ready timed lyrics.

use std::borrow::Cow;
use std::collections::HashMap;

use anyhow::{Context, Result, anyhow};
use lyrics_helper::helpers::conventions::{
    apply_speaker_labels, fold_bracketed_echoes, merge_continued_lines, split_background_vocals,
    unwrap_brackets,
};
use lyrics_helper::helpers::optimization::explicit::fix_explicit;
use lyrics_helper::helpers::word_timing::apply_word_timings;
use lyrics_helper::{
    LineInfo, LyricsAlignment, LyricsData, LyricsTypes, SyllableItem, generate_string,
    parse_auto as parse_helper,
};

use super::model::{BackgroundVocal, TimedLine, TimedSyllable, Voice};

mod filter;

const TRANSLATION_PREFIX: &str = "__FLOATLYRICS_TRANSLATION__:";
const TRANSLATION_SECTION_MARKER: &str = "[floatlyrics:translation]";
const WORD_TIMING_SECTION_MARKER: &str = "[floatlyrics:word-timing]";

/// Parses a local lyrics document using the formats supported by `lyrics-helper`.
///
/// # Errors
/// Returns an error when the format cannot be detected or parsed.
pub fn parse_local_lyrics(content: &str) -> Result<LyricsData> {
    let mut parsed =
        parse_helper(content).context("lyrics-helper could not detect or parse lyrics")?;
    standardize_parsed_payload(content, &mut parsed);
    Ok(parsed)
}

/// Applies `lyrics-helper`'s normalization for a payload format that needs one.
///
/// NetEase times the space between two words as a unit of its own, which upstream
/// folds into the word before it; a renderer would otherwise animate the gap
/// between two words as if it were one.
fn standardize_parsed_payload(content: &str, data: &mut LyricsData) {
    use lyrics_helper::LyricsRawTypes;
    use lyrics_helper::helpers::{optimization::yrc, type_helper::get_lyrics_types};

    if get_lyrics_types(content) != LyricsRawTypes::Yrc {
        return;
    }
    if let Some(lines) = data.lines.as_mut() {
        yrc::standardize_yrc_lyrics(lines);
    }
}

/// Parses raw provider lyrics into sorted, display-ready lines.
///
/// `artists` are the ones the provider that supplied these lyrics credits, and
/// are what a speaker label in the payload is matched against. Words the provider
/// censored are restored before anything reads the text, translation sections are
/// merged, the conventions of the transcription itself are read into the lines,
/// and known metadata or credit lines are removed. A payload that carries a
/// word-timed document beside its transcription takes the times of the words from
/// that document and the text of them from the transcription. Provider
/// pronunciation is intentionally discarded; call [`super::generate_local_romanization`]
/// when local romanization is wanted.
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
    let (word_timing, lyrics) = split_word_timing_section(lyrics);
    let mut lines = parse_lines_block(lyrics, artists)?;
    if let Some(word_timing) = word_timing {
        // The words are read onto the row-timed text, so they are read before
        // anything reads that text. A word-timed document that cannot be read is
        // no reason to lose the transcription beside it: the rows stand on their
        // own, without words.
        if let Ok(word_lines) = parse_lines_block(word_timing, &[]) {
            apply_word_timings(&mut lines, &word_lines);
        }
    }
    if let Some(translation) = translation {
        // A translation document is written for the track rather than by one of
        // its performers, so no label in it names a voice.
        let translation_lines = parse_lines_block(translation, &[])?;
        merge_translation_lines(&mut lines, translation_lines);
    }
    // After the translations are paired: an echo is a line of its own until it is
    // folded away, so it collects the translation timed to it, and each row of a
    // sentence carries its own translation until the rows are joined.
    fold_bracketed_echoes(&mut lines);
    merge_continued_lines(&mut lines);
    require_lines(timed_lines_from_lines(&lines))
}

/// Returns the lines a payload parsed into, or the error for one without any.
fn require_lines<T>(lines: Vec<T>) -> Result<Vec<T>> {
    if lines.is_empty() {
        return Err(anyhow!("lyrics did not include timed lines"));
    }
    Ok(lines)
}

/// Parses one section of a payload into display rows whose conventions are read.
///
/// `lyrics-helper` detects and parses every format, including QQ Music's QRC,
/// whose rows keep the span the tag that opens them states rather than ending
/// at their last word.
fn parse_lines_block(content: &str, artists: &[String]) -> Result<Vec<LineInfo>> {
    let parsed = parse_local_lyrics(content)?;
    require_lines(lines_from_parsed(&parsed, artists))
}

/// Reads a provider's own transcription conventions and drops non-lyric rows.
///
/// A label may name a performer, so it is read before the filter drops the rows
/// that look like one; a bracketed tail is only split off a word-timed line, so
/// this runs before the timed units are split into words.
fn finish(lines: Vec<LineInfo>, artists: &[String]) -> Vec<LineInfo> {
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
    let mut lines = lines_from_parsed(data, artists);
    // A row carries the translation of its own line, so the rows of one sentence are
    // joined once they have both been paired with it.
    merge_continued_lines(&mut lines);
    timed_lines_from_lines(&lines)
}

/// Returns the sorted display rows of already parsed lyrics, leaving the rows a
/// sentence is broken across as they were written.
///
/// Every row carries at most one translation, stored under the key the
/// conventions join translations by. The join waits for the translations to be
/// paired, so a payload that arrived with a translation document of its own is
/// read in this stage first.
fn lines_from_parsed(data: &LyricsData, artists: &[String]) -> Vec<LineInfo> {
    let Some(lines) = data.lines.as_deref() else {
        return Vec::new();
    };

    let mut lines = lines
        .iter()
        .filter(|line| line.start_time().and_then(ms_i32_to_u64).is_some())
        .cloned()
        .map(|mut line| {
            normalize_translation(&mut line);
            if let Some(sub_line) = line.sub_line_mut() {
                normalize_translation(sub_line);
            }
            line
        })
        .filter(|line| {
            !line.text_from_any().trim().is_empty()
                || line.translations().is_some_and(|map| !map.is_empty())
        })
        .collect::<Vec<_>>();
    lines.sort_by_key(LineInfo::start_time_or_zero);
    finish(lines, artists)
}

/// Converts display rows whose conventions are read to timed lines.
fn timed_lines_from_lines(lines: &[LineInfo]) -> Vec<TimedLine> {
    lines.iter().filter_map(timed_line_from_info).collect()
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

/// Converts one display row to a timed line.
///
/// The background vocal is drawn beside the line rather than inside it, so the
/// line keeps its own text and timing, and the side a label gave the row decides
/// its voice.
fn timed_line_from_info(line: &LineInfo) -> Option<TimedLine> {
    let start_ms = ms_i32_to_u64(line.start_time()?)?;
    let end_ms = line.end_time().and_then(ms_i32_to_u64);
    let text = line.text_from_any().trim().to_string();
    let syllables = timed_syllables_from_info(line);
    let translation = preferred_translation(line);
    let background = line.sub_line().map(|sub| BackgroundVocal {
        text: sub.text_from_any().trim().to_string(),
        // A bracketed phrase is translated inside its brackets, which the
        // background vocal is already drawn with.
        translation: preferred_translation(sub).map(|translation| unwrap_brackets(&translation)),
        start_ms: sub.start_time().and_then(ms_i32_to_u64).unwrap_or(start_ms),
        end_ms: sub.end_time().and_then(ms_i32_to_u64),
        syllables: timed_syllables_from_info(sub),
    });

    if text.is_empty() && translation.is_none() {
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
        voice: voice_from_alignment(line.alignment()),
    })
}

/// Returns the voice a speaker label gave a row: the performer credited first
/// sings on the left, everyone else on the right.
fn voice_from_alignment(alignment: LyricsAlignment) -> Voice {
    match alignment {
        LyricsAlignment::Right => Voice::Secondary,
        _ => Voice::Primary,
    }
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

fn merge_translation_marker_lines(lines: Vec<LineInfo>) -> Vec<LineInfo> {
    let mut merged: Vec<LineInfo> = Vec::new();

    for mut line in lines {
        if let Some(translation) = translation_marker_text(&line.text_from_any()) {
            if let Some(target) = merged
                .iter_mut()
                .rev()
                .find(|target| target.start_time() == line.start_time())
            {
                set_translation(target, clean_optional_text(translation));
            }
            continue;
        }

        // A background vocal whose text is a translation marker *is* the
        // translation; any other one stays the background it is.
        let marker = line
            .sub_line()
            .and_then(|background| marker_owned(&background.text_from_any()));
        if marker.is_some() {
            line.set_sub_line(None);
        }
        let translation = preferred_translation(&line).or(marker);
        set_translation(&mut line, translation);
        merged.push(line);
    }

    merged
}

fn merge_translation_lines(lines: &mut [LineInfo], translation_lines: Vec<LineInfo>) {
    for translation in translation_lines {
        let Some(start_ms) = translation.start_time() else {
            continue;
        };
        let target = if let Some(index) = lines
            .iter()
            .position(|line| line.start_time() == Some(start_ms))
        {
            lines.get_mut(index)
        } else {
            nearest_line_mut(lines, start_ms, 800)
        };

        if let Some(target) = target {
            set_translation(target, clean_optional_text(&translation.text_from_any()));
        }
    }
}

fn nearest_line_mut(
    lines: &mut [LineInfo],
    start_ms: i32,
    tolerance_ms: u32,
) -> Option<&mut LineInfo> {
    let index = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let diff = line.start_time()?.abs_diff(start_ms);
            (diff <= tolerance_ms).then_some((index, diff))
        })
        .min_by_key(|(_, diff)| *diff)
        .map(|(index, _)| index)?;

    lines.get_mut(index)
}

/// Combines a word-timed document, the row-timed transcription of the same track,
/// and the translation of that transcription into one parseable payload.
///
/// The word-timed document is a timing table: this application reads it onto the
/// text of the row-timed transcription rather than displaying what it spells the
/// words with, so both travel together.
pub fn combine_word_timing(word_timed: &str, row_timed: &str, translation: Option<&str>) -> String {
    format!(
        "{}\n{}\n{}",
        word_timed.trim_end(),
        WORD_TIMING_SECTION_MARKER,
        combine_lyrics_with_translation(row_timed, translation)
    )
}

fn split_word_timing_section(content: &str) -> (Option<&str>, &str) {
    content
        .split_once(WORD_TIMING_SECTION_MARKER)
        .map_or((None, content), |(word_timing, rows)| {
            (Some(word_timing), rows)
        })
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

/// The key a row's single translation is stored under.
///
/// The conventions join the translations of two rows language by language, so
/// every row keeps its translation under the same key.
const TRANSLATION_KEY: &str = "zh";

/// Keeps only the translation a row is displayed with, under [`TRANSLATION_KEY`].
fn normalize_translation(line: &mut LineInfo) {
    if line.translations().is_some() {
        let translation = preferred_translation(line);
        set_translation(line, translation);
    }
}

/// Replaces the translation of `line`, turning a row without one into a row that
/// can carry it.
fn set_translation(line: &mut LineInfo, translation: Option<String>) {
    let translations: HashMap<String, String> = translation
        .map(|text| (TRANSLATION_KEY.to_string(), text))
        .into_iter()
        .collect();
    match line.translations_mut() {
        Some(existing) => *existing = translations,
        None if translations.is_empty() => {}
        None => {
            let owned = std::mem::replace(line, LineInfo::new_line_simple(String::new()));
            *line = owned.to_full_line(translations, None);
        }
    }
}

fn preferred_translation(line: &LineInfo) -> Option<String> {
    let translations = line.translations()?;
    translations
        .get(TRANSLATION_KEY)
        .or_else(|| translations.get("zh-Hans"))
        .or_else(|| translations.get("zh-CN"))
        .or_else(|| translations.values().next())
        .and_then(|value| clean_optional_text(value))
}

fn filter_display_lines(lines: Vec<LineInfo>) -> Vec<LineInfo> {
    let mut metadata = filter::Metadata::new();
    lines
        .into_iter()
        .filter(|line| {
            let start_ms = line
                .start_time()
                .and_then(ms_i32_to_u64)
                .unwrap_or_default();
            !metadata.drops(start_ms, &line.text_from_any())
        })
        .collect()
}
