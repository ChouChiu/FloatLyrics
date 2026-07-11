// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{Context, Result, anyhow};
use lyrics_helper::{LineInfo, LyricsData, LyricsTypes, generate_string, parse_auto};

use super::model::{TimedLine, TimedSyllable};

const TRANSLATION_PREFIX: &str = "__FLOATLYRICS_TRANSLATION__:";
const TRANSLATION_SECTION_MARKER: &str = "[floatlyrics:translation]";

pub fn parse_local_lyrics(content: &str) -> Result<LyricsData> {
    parse_auto(content).context("lyrics-helper could not detect or parse lyrics")
}

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

pub fn export_lyrics(data: &LyricsData, ty: LyricsTypes) -> Result<String> {
    generate_string(data, ty).context("lyrics-helper could not generate lyrics in requested format")
}

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
    let romanization = pronunciation(line);
    let background = line.sub_line().map(LineInfo::text_from_any);

    if text.trim().is_empty() && translation.is_none() && romanization.is_none() {
        return None;
    }

    Some(TimedLine {
        start_ms,
        end_ms,
        text,
        syllables,
        translation,
        romanization,
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

fn pronunciation(line: &LineInfo) -> Option<String> {
    match line {
        LineInfo::FullLine { pronunciation, .. } | LineInfo::FullSyllable { pronunciation, .. } => {
            pronunciation
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        }
        _ => None,
    }
}

fn timed_lines_from_lrc(content: &str) -> Vec<TimedLine> {
    let mut lines = Vec::new();

    for line in content.lines() {
        let Some((timestamps, text)) = split_lrc_timestamps(line.trim()) else {
            continue;
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }

        for timestamp in timestamps_in(&timestamps) {
            if let Some(start_ms) = parse_lrc_timestamp(timestamp) {
                lines.push(TimedLine {
                    start_ms,
                    end_ms: None,
                    text: text.to_string(),
                    syllables: Vec::new(),
                    translation: None,
                    romanization: None,
                    background: None,
                });
            }
        }
    }

    lines.sort_by_key(|line| line.start_ms);
    filter_display_lines(merge_translation_marker_lines(lines))
}

fn timed_lines_from_qrc(content: &str) -> Vec<TimedLine> {
    let mut lines = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        let Some((start_ms, end_ms, text, syllables)) = parse_qrc_line(line) else {
            continue;
        };
        if text.is_empty() {
            continue;
        }

        lines.push(TimedLine {
            start_ms,
            end_ms: Some(end_ms),
            text,
            syllables,
            translation: None,
            romanization: None,
            background: None,
        });
    }

    lines.sort_by_key(|line| line.start_ms);
    filter_display_lines(merge_translation_marker_lines(lines))
}

fn timestamps_in(tags: &str) -> impl Iterator<Item = &str> {
    tags.split('[')
        .filter_map(|part| part.strip_suffix(']'))
        .filter(|tag| is_lrc_timestamp(tag))
}

fn parse_lrc_timestamp(timestamp: &str) -> Option<u64> {
    let (minutes, seconds) = timestamp.split_once(':')?;
    let minutes = minutes.parse::<u64>().ok()?;
    let (seconds, millis) = seconds
        .split_once('.')
        .map_or((seconds, "0"), |(seconds, millis)| (seconds, millis));
    let seconds = seconds.parse::<u64>().ok()?;
    let millis = parse_lrc_millis(millis)?;

    minutes
        .checked_mul(60_000)?
        .checked_add(seconds.checked_mul(1_000)?)?
        .checked_add(millis)
}

fn parse_lrc_millis(value: &str) -> Option<u64> {
    let digits = value
        .bytes()
        .take_while(u8::is_ascii_digit)
        .take(3)
        .collect::<Vec<_>>();
    if digits.is_empty() {
        return Some(0);
    }

    let parsed = std::str::from_utf8(&digits).ok()?.parse::<u64>().ok()?;
    Some(match digits.len() {
        1 => parsed * 100,
        2 => parsed * 10,
        _ => parsed,
    })
}

fn parse_qrc_line(line: &str) -> Option<(u64, u64, String, Vec<TimedSyllable>)> {
    let (tag, rest) = first_bracket_tag(line)?;
    let (start_ms, duration_ms) = parse_qrc_timestamp(tag)?;
    let (text, syllables) = qrc_line_parts(rest);

    Some((
        start_ms,
        start_ms.saturating_add(duration_ms),
        text,
        syllables,
    ))
}

fn first_bracket_tag(line: &str) -> Option<(&str, &str)> {
    let after_open = line.strip_prefix('[')?;
    let end = after_open.find(']')?;
    Some((&after_open[..end], &after_open[end + 1..]))
}

fn parse_qrc_timestamp(tag: &str) -> Option<(u64, u64)> {
    let (start, duration) = tag.split_once(',')?;
    let start = start.trim().parse().ok()?;
    let duration = duration.trim().parse().ok()?;
    Some((start, duration))
}

fn qrc_line_parts(value: &str) -> (String, Vec<TimedSyllable>) {
    let mut output = String::new();
    let mut syllables = Vec::new();
    let mut rest = value;

    while let Some(open) = rest.find('(') {
        let segment_text = &rest[..open];
        output.push_str(segment_text);
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find(')') else {
            output.push_str(&rest[open..]);
            return (output.trim().to_string(), syllables);
        };
        let tag = &after_open[..close];
        if let Some((start_ms, duration_ms)) = parse_qrc_timestamp(tag) {
            if !segment_text.is_empty() {
                syllables.push(TimedSyllable {
                    start_ms,
                    end_ms: start_ms.saturating_add(duration_ms),
                    text: segment_text.to_string(),
                });
            }
        } else {
            output.push('(');
            rest = after_open;
            continue;
        }
        rest = &after_open[close + 1..];
    }

    output.push_str(rest);
    (output.trim().to_string(), syllables)
}

fn filter_display_lines(lines: Vec<TimedLine>) -> Vec<TimedLine> {
    lines
        .into_iter()
        .filter(|line| !is_non_lyric_display_line(line))
        .collect()
}

fn is_non_lyric_display_line(line: &TimedLine) -> bool {
    let text = line.text.trim();
    if text.is_empty() {
        return true;
    }

    is_intro_title_line(line, text) || is_credit_line(line, text) || is_speaker_label_line(text)
}

fn is_intro_title_line(line: &TimedLine, text: &str) -> bool {
    if line.start_ms > 5_000 {
        return false;
    }

    // Title – Artist or Title - Artist patterns with various separators.
    title_artist_separator(text).is_some()
        // QRC-style title line: "LEMONADE (Feat. Becky G) - aespa (에스파)/Becky G"
        // where multi-artist listing uses "/" and the main separator is " - ".
        || looks_like_title_and_artist_list(text)
}

fn title_artist_separator(text: &str) -> Option<usize> {
    [" - ", " – ", " — ", " ~ ", "～", " | ", " · "]
        .iter()
        .find_map(|separator| text.find(separator))
}

/// Detects lines that combine a title and artist list via "/" in the first seconds.
fn looks_like_title_and_artist_list(text: &str) -> bool {
    // e.g. "Title (feat. Artist A)/Artist B" or "歌名 - 歌手A/歌手B"
    let slash_count = text.matches('/').count();
    slash_count > 0
        && slash_count <= 4
        && text.chars().count() > 8
        && !text.contains('\n')
        && text.matches(|c: char| c.is_whitespace()).count() >= 2
}

fn is_credit_line(line: &TimedLine, text: &str) -> bool {
    let normalized = normalize_line_text(text);

    // Generic key-value metadata detector: only within the first 10 seconds,
    // matches lines like "演唱：Leana Mask", "出品：XX", "Mixing: EE".
    if line.start_ms < 10_000
        && let Some((key, _value)) = normalized.split_once(':')
    {
        let key = key.trim();
        if (2..=18).contains(&key.chars().count())
            && key
                .chars()
                .all(|ch| ch.is_alphanumeric() || ch == ' ' || ch == '&' || ch == '/')
            && !key.starts_with("http")
        {
            return true;
        }
    }

    // Common credit line prefixes in both English and Chinese.
    let prefixes = [
        // English credits
        "lyrics by",
        "lyric by",
        "written by",
        "words by",
        "composed by",
        "composer",
        "compose by",
        "composition",
        "arranged by",
        "arranger",
        "arrangement",
        "produced by",
        "producer",
        "music:",
        "melody:",
        "song:",
        "title:",
        "track:",
        "artist:",
        "singer:",
        "performer:",
        "vocals:",
        "vocal:",
        "feat:",
        "album:",
        "mixing:",
        "mix:",
        "mastering:",
        "master:",
        "mastered by",
        "recording:",
        "recorded by",
        "guitar:",
        "guitars:",
        "piano:",
        "keyboard:",
        "keyboards:",
        "bass:",
        "drums:",
        "strings:",
        "backing vocals:",
        "background vocals:",
        "chorus:",
        "orchestration:",
        "orchestrated by",
        "release:",
        "label:",
        "publisher:",
        "copyright:",
        "upload:",
        "uploader:",
        "uploaded by",
        "synced by",
        "synchronized by",
        "edited by",
        "created by",
        "programming:",
        "programmed by",
        "directed by",
        "op:",
        "sp:",
        // NetEase / Chinese credits
        "作词",
        "作詞",
        "作曲",
        "编曲",
        "編曲",
        "制作人",
        "製作人",
        "监制",
        "監製",
        "词:",
        "詞:",
        "曲:",
        "演唱",
        "歌手",
        "专辑",
        "專輯",
        "歌名",
        "歌曲",
        "标题",
        "標題",
        "歌:",
        "唱:",
        "原唱",
        "翻唱",
        "和声",
        "和聲",
        "和声编写",
        "和聲編寫",
        "混音",
        "母带",
        "母帶",
        "录音",
        "錄音",
        "吉他",
        "钢琴",
        "鋼琴",
        "贝斯",
        "貝斯",
        "鼓:",
        "弦乐",
        "弦樂",
        "发行",
        "發行",
        "厂牌",
        "廠牌",
        "上传",
        "上傳",
        "歌词制作",
        "歌詞製作",
        "歌词编辑",
        "歌詞編輯",
        "配唱",
        "出品",
        "版权",
        "版權",
        "词曲",
        "詞曲",
        "qq音乐享有",
        "以下歌词翻译由",
        "翻译:",
        "翻譯:",
        // URLs / metadata
        "http://",
        "https://",
        "www.",
        // Typographic marks indicating extra-lyric content
        "℗",
    ];

    prefixes.iter().any(|prefix| normalized.starts_with(prefix))
}

fn is_speaker_label_line(text: &str) -> bool {
    let label = text.trim().trim_end_matches([':', '：']).trim();

    if label == text.trim() || label.is_empty() || label.chars().count() > 42 {
        return false;
    }

    label.eq_ignore_ascii_case("both")
        || label.contains('/')
        || label.contains('&')
        || label.contains(" and ")
        || looks_like_artist_label(label)
}

fn looks_like_artist_label(label: &str) -> bool {
    let words = label.split_whitespace().collect::<Vec<_>>();
    (1..=4).contains(&words.len())
        && words.iter().all(|word| {
            word.chars()
                .next()
                .is_some_and(|character| character.is_uppercase())
        })
        // CJK names: short label (≤ 15 chars) with ideographs and no Latin lowercase.
        || (label.chars().count() <= 15
            && !label.is_empty()
            && label.chars().any(|ch| ch >= '\u{2E80}')
            && !label.chars().any(|ch| ch.is_lowercase()))
}

fn normalize_line_text(text: &str) -> String {
    text.trim()
        .trim_start_matches(['(', '[', '【'])
        .trim_end_matches([')', ']', '】'])
        .replace('：', ":")
        .to_lowercase()
}

fn split_lrc_timestamps(line: &str) -> Option<(String, &str)> {
    let mut rest = line;
    let mut timestamps = String::new();

    while let Some(after_open) = rest.strip_prefix('[') {
        let Some(end) = after_open.find(']') else {
            break;
        };
        let tag = &after_open[..end];
        if !is_lrc_timestamp(tag) {
            break;
        }

        timestamps.push('[');
        timestamps.push_str(tag);
        timestamps.push(']');
        rest = &after_open[end + 1..];
    }

    if timestamps.is_empty() {
        None
    } else {
        Some((timestamps, rest))
    }
}

fn is_lrc_timestamp(tag: &str) -> bool {
    let Some(first) = tag.as_bytes().first() else {
        return false;
    };
    first.is_ascii_digit()
        && tag.contains(':')
        && tag
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b':' || byte == b'.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_at(text: &str, start_ms: u64) -> TimedLine {
        TimedLine {
            start_ms,
            end_ms: None,
            text: text.to_string(),
            syllables: vec![],
            translation: None,
            romanization: None,
            background: None,
        }
    }

    #[test]
    fn overflowing_lrc_timestamp_is_ignored() {
        assert_eq!(parse_lrc_timestamp("18446744073709551615:00.00"), None);
        assert!(timed_lines_from_lrc("[18446744073709551615:00.00]Never").is_empty());
    }

    #[test]
    fn filters_netease_credit_lines() {
        assert!(is_credit_line(
            &line_at(" 演唱 Leana Mask", 6273),
            " 演唱 Leana Mask"
        ));
        assert!(is_credit_line(
            &line_at("作曲 : Marina Diamandis/Rick Nowels", 0),
            "作曲 : Marina Diamandis/Rick Nowels"
        ));
        assert!(is_credit_line(
            &line_at("作词 Marina Diamandis/Rick Nowells", 0),
            "作词 Marina Diamandis/Rick Nowells"
        ));
    }

    #[test]
    fn filters_english_credit_lines() {
        assert!(is_credit_line(
            &line_at("Lyrics by John Doe", 0),
            "Lyrics by John Doe"
        ));
        assert!(is_credit_line(
            &line_at("Composed by Jane", 0),
            "Composed by Jane"
        ));
        assert!(is_credit_line(
            &line_at("Mixing: Engineer", 2000),
            "Mixing: Engineer"
        ));
    }

    #[test]
    fn filters_generic_key_value_metadata_in_intro() {
        assert!(is_credit_line(
            &line_at("出品：某唱片公司", 1500),
            "出品：某唱片公司"
        ));
        assert!(is_credit_line(&line_at("配唱：某人", 300), "配唱：某人"));
    }

    #[test]
    fn does_not_filter_real_lyrics_with_colon() {
        // Past 10s window — real lyrics with colon are safe.
        let line = line_at("love: it's real", 30000);
        assert!(!is_credit_line(&line, "love: it's real"));
        // Key too short (1 char) — no generic match.
        let line = line_at("I: am here", 5000);
        assert!(!is_credit_line(&line, "I: am here"));
    }

    #[test]
    fn filters_intro_title_line() {
        let line = line_at("Hello World - Adele", 100);
        assert!(is_intro_title_line(&line, "Hello World - Adele"));
        let line = line_at("Hello World - Adele", 6000);
        assert!(!is_intro_title_line(&line, "Hello World - Adele"));
    }

    #[test]
    fn filters_cjk_speaker_label() {
        assert!(is_speaker_label_line("周杰伦："));
        assert!(is_speaker_label_line("阿信："));
        assert!(!is_speaker_label_line("我们一起学猫叫"));
    }

    #[test]
    fn filters_urls_and_typographic_marks() {
        assert!(is_credit_line(
            &line_at("http://example.com", 0),
            "http://example.com"
        ));
        assert!(is_credit_line(
            &line_at("https://example.com", 0),
            "https://example.com"
        ));
        assert!(is_credit_line(
            &line_at("www.example.com", 0),
            "www.example.com"
        ));
        assert!(is_credit_line(&line_at("℗ 2024 Label", 0), "℗ 2024 Label"));
    }
}
