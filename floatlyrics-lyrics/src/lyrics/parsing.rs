// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Conversion between provider payloads and display-ready timed lyrics.

use anyhow::{Context, Result, anyhow};
use lyrics_helper::{
    LineInfo, LyricsData, LyricsTypes, generate_string, parse_auto as parse_helper,
};

use super::model::{TimedLine, TimedSyllable};

mod filter;
mod lrc;
mod qrc;

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
    // `lyrics-parsers` 0.1.3 uses quick-xml's checked attribute iterator for
    // TTML. Reject XML before reaching the quadratic paths described by
    // RUSTSEC-2026-0194 and RUSTSEC-2026-0195. QQ Music and NetEase payloads
    // use the LRC/QRC paths above this fallback.
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
/// Translation sections are merged and known metadata or credit lines are
/// removed. Provider pronunciation is intentionally discarded; call
/// [`super::generate_local_romanization`] when local romanization is wanted.
///
/// # Errors
/// Returns an error when no supported timed format can be parsed.
pub fn timed_lines_from_raw(content: &str) -> Result<Vec<TimedLine>> {
    let (lyrics, translation) = split_translation_section(content);
    let mut lines = parse_timed_lines_block(lyrics)?;
    if let Some(translation) = translation {
        let translation_lines = parse_timed_lines_block(translation)?;
        merge_translation_lines(&mut lines, translation_lines);
    }
    if lines.is_empty() {
        Err(anyhow!("lyrics did not include timed lines"))
    } else {
        Ok(lines)
    }
}

fn parse_timed_lines_block(content: &str) -> Result<Vec<TimedLine>> {
    let mut lines = timed_lines_from_lrc(content);
    if lines.is_empty() {
        lines = timed_lines_from_qrc(content);
    }
    if !lines.is_empty() {
        return Ok(lines);
    }

    let parsed = parse_local_lyrics(content)?;
    let lines = timed_lines_from_data(&parsed);
    if lines.is_empty() {
        Err(anyhow!("lyrics did not include timed lines"))
    } else {
        Ok(lines)
    }
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
/// Provider pronunciation is intentionally discarded; call
/// [`super::generate_local_romanization`] when local romanization is wanted.
pub fn timed_lines_from_data(data: &LyricsData) -> Vec<TimedLine> {
    let Some(lines) = data.lines.as_deref() else {
        return Vec::new();
    };

    let mut timed_lines = lines
        .iter()
        .filter_map(timed_line_from_info)
        .collect::<Vec<_>>();
    timed_lines.sort_by_key(|line| line.start_ms);
    filter_display_lines(merge_translation_marker_lines(timed_lines))
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
    let background = line.sub_line().map(LineInfo::text_from_any);

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
    })
}

fn timed_syllables_from_info(line: &LineInfo) -> Vec<TimedSyllable> {
    match line {
        LineInfo::Syllable { syllables, .. } | LineInfo::FullSyllable { syllables, .. } => {
            syllables
                .iter()
                .filter_map(|syllable| {
                    Some(TimedSyllable {
                        start_ms: ms_i32_to_u64(syllable.start_time)?,
                        end_ms: ms_i32_to_u64(syllable.end_time)?,
                        text: syllable.text.clone(),
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

        line.translation = line
            .translation
            .take()
            .and_then(|value| clean_optional_text(&value))
            .or_else(|| {
                line.background
                    .take()
                    .and_then(|value| marker_owned(&value))
            });
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
