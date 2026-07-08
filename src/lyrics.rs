use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

pub use lyrics_helper::{LineInfo, LyricsData, LyricsTypes, generate_string, parse_auto};

use crate::track::TrackMetadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LyricsProvider {
    QqMusic,
    NetEase,
    LrcLib,
}

impl LyricsProvider {
    pub fn default_order() -> Vec<Self> {
        vec![Self::QqMusic, Self::NetEase]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::QqMusic => "qq-music",
            Self::NetEase => "netease",
            Self::LrcLib => "lrclib",
        }
    }
}

impl std::fmt::Display for LyricsProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for LyricsProvider {
    type Err = LyricsProviderParseError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "qq-music" | "qq" => Ok(Self::QqMusic),
            "netease" | "netease-cloud-music" => Ok(Self::NetEase),
            "lrclib" => Ok(Self::LrcLib),
            _ => Err(LyricsProviderParseError(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unsupported lyrics provider: {0}")]
pub struct LyricsProviderParseError(String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimedLine {
    pub start_ms: u64,
    pub end_ms: Option<u64>,
    pub text: String,
    pub syllables: Vec<TimedSyllable>,
    pub translation: Option<String>,
    pub romanization: Option<String>,
    pub background: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimedSyllable {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FetchedLyrics {
    pub provider: LyricsProvider,
    pub provider_track_id: Option<String>,
    pub title: String,
    pub artists: Vec<String>,
    pub score: f64,
    pub raw_lyrics: String,
}

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

pub async fn search_best_lyrics(
    track: &TrackMetadata,
    provider_order: &[LyricsProvider],
) -> Result<Option<FetchedLyrics>> {
    let metadata = lyrics_helper_metadata(track);

    for provider in provider_order {
        if let Some(fetched) = search_provider(*provider, &metadata).await? {
            return Ok(Some(fetched));
        }
    }

    Ok(None)
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

pub fn active_line_index(lines: &[TimedLine], playback_ms: u64, offset_ms: i64) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }

    let adjusted = adjusted_playback_ms(playback_ms, offset_ms);
    let index = line_index_at_or_before(lines, playback_ms, offset_ms)?;
    let line = lines.get(index)?;

    match line.end_ms {
        Some(end_ms) if adjusted >= end_ms => None,
        _ => Some(index),
    }
}

pub fn line_index_at_or_before(
    lines: &[TimedLine],
    playback_ms: u64,
    offset_ms: i64,
) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }

    let adjusted = adjusted_playback_ms(playback_ms, offset_ms);
    lines
        .partition_point(|line| line.start_ms <= adjusted)
        .checked_sub(1)
}

fn adjusted_playback_ms(playback_ms: u64, offset_ms: i64) -> u64 {
    (playback_ms as i128 + offset_ms as i128).max(0) as u64
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

async fn search_provider(
    provider: LyricsProvider,
    metadata: &lyrics_helper::models::TrackMetadata,
) -> Result<Option<FetchedLyrics>> {
    use lyrics_helper::searchers::{
        compare_helper::MatchType, netease::NeteaseSearcher, qq_music::QQMusicSearcher,
        search_for_best_result_with_match,
    };

    let result = match provider {
        LyricsProvider::QqMusic => {
            search_for_best_result_with_match(&QQMusicSearcher, metadata, MatchType::Medium).await
        }
        LyricsProvider::NetEase => {
            search_for_best_result_with_match(&NeteaseSearcher, metadata, MatchType::Medium).await
        }
        LyricsProvider::LrcLib => return Ok(None),
    };

    let Some(result) = result else {
        return Ok(None);
    };
    let raw_lyrics = fetch_result_lyrics(provider, &result).await;
    let Some(raw_lyrics) = raw_lyrics.map(|value| value.trim().to_string()) else {
        return Ok(None);
    };
    if raw_lyrics.is_empty() {
        return Ok(None);
    }

    Ok(Some(FetchedLyrics {
        provider,
        provider_track_id: Some(result.id.clone()),
        title: result.title,
        artists: result.artists,
        score: result
            .match_type
            .map_or(0.0, |match_type| match_type as i32 as f64),
        raw_lyrics,
    }))
}

async fn fetch_result_lyrics(
    provider: LyricsProvider,
    result: &lyrics_helper::searchers::search_result::SearchResult,
) -> Option<String> {
    use lyrics_helper::search::providers::web::{netease, qq_music};

    match provider {
        LyricsProvider::QqMusic => qq_music::api::get_lyrics(
            &result.id,
            result.numeric_id,
            &result.title,
            &result.artist(),
            &result.album,
            result.duration_ms,
        )
        .await
        .and_then(|(lyric, translation)| {
            lyric.map(|lyric| combine_lyrics_with_translation(&lyric, translation.as_deref()))
        }),
        LyricsProvider::NetEase => {
            let song_id = result.id.parse().ok()?;
            netease::api::get_lyrics(song_id)
                .await
                .and_then(|(lyric, translation)| {
                    lyric.map(|lyric| {
                        combine_lyrics_with_translation(&lyric, translation.as_deref())
                    })
                })
        }
        LyricsProvider::LrcLib => None,
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

    Some(minutes * 60_000 + seconds * 1_000 + millis)
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

    is_intro_title_line(line, text) || is_credit_line(text) || is_speaker_label_line(text)
}

fn is_intro_title_line(line: &TimedLine, text: &str) -> bool {
    line.start_ms <= 500 && title_artist_separator(text).is_some()
}

fn title_artist_separator(text: &str) -> Option<usize> {
    [" - ", " – ", " — "]
        .iter()
        .find_map(|separator| text.find(separator))
}

fn is_credit_line(text: &str) -> bool {
    let normalized = normalize_line_text(text);
    let prefixes = [
        "lyrics by",
        "lyric by",
        "composed by",
        "compose by",
        "produced by",
        "producer",
        "written by",
        "作词",
        "作詞",
        "作曲",
        "编曲",
        "編曲",
        "制作人",
        "製作人",
        "监制",
        "監製",
        "qq音乐享有",
        "以下歌词翻译由",
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

fn lyrics_helper_metadata(track: &TrackMetadata) -> lyrics_helper::models::TrackMetadata {
    let mut metadata = lyrics_helper::models::TrackMetadata::new();
    metadata.title = Some(track.title.clone());
    metadata.artist = Some(track.display_artist());
    metadata.artists = Some(track.artists.clone());
    metadata.album = track.album.clone();
    metadata.duration_ms = track.duration_ms.and_then(|value| value.try_into().ok());
    metadata
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPlan {
    providers: Vec<LyricsProvider>,
}

impl SearchPlan {
    pub fn new(providers: impl IntoIterator<Item = LyricsProvider>) -> Self {
        let mut providers = providers.into_iter().collect::<Vec<_>>();
        providers.retain(|provider| LyricsProvider::default_order().contains(provider));
        providers.dedup();
        Self { providers }
    }

    pub fn default_mvp() -> Self {
        Self::new(LyricsProvider::default_order())
    }

    pub fn providers(&self) -> &[LyricsProvider] {
        &self.providers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(start_ms: u64, end_ms: Option<u64>, text: &str) -> TimedLine {
        TimedLine {
            start_ms,
            end_ms,
            text: text.to_string(),
            syllables: Vec::new(),
            translation: None,
            romanization: None,
            background: None,
        }
    }

    #[test]
    fn active_line_uses_offset_and_end_time() {
        let lines = vec![
            line(1_000, Some(2_000), "a"),
            line(2_000, Some(3_000), "b"),
            line(4_000, None, "c"),
        ];

        assert_eq!(active_line_index(&lines, 500, 500), Some(0));
        assert_eq!(active_line_index(&lines, 3_500, 0), None);
        assert_eq!(active_line_index(&lines, 4_200, 0), Some(2));
        assert_eq!(active_line_index(&lines, 100, -500), None);
    }

    #[test]
    fn line_index_at_or_before_holds_previous_line_during_gap() {
        let lines = vec![
            line(1_000, Some(2_000), "a"),
            line(2_000, Some(3_000), "b"),
            line(4_000, None, "c"),
        ];

        assert_eq!(active_line_index(&lines, 3_500, 0), None);
        assert_eq!(line_index_at_or_before(&lines, 3_500, 0), Some(1));
        assert_eq!(line_index_at_or_before(&lines, 100, 0), None);
    }

    #[test]
    fn search_plan_keeps_mvp_provider_order() {
        assert_eq!(
            SearchPlan::default_mvp().providers(),
            &[LyricsProvider::QqMusic, LyricsProvider::NetEase]
        );
    }

    #[test]
    fn parse_and_export_lrc_through_lyrics_helper() {
        let parsed = parse_local_lyrics("[00:01.00]Hello World!").unwrap();
        let exported = export_lyrics(&parsed, LyricsTypes::Lrc).unwrap();

        assert!(exported.contains("Hello World"));
    }

    #[test]
    fn maps_track_metadata_for_lyrics_helper_search() {
        let track = crate::track::TrackMetadata {
            title: "Song".to_string(),
            artists: vec!["Alice".to_string(), "Bob".to_string()],
            album: Some("Album".to_string()),
            duration_ms: Some(123_000),
            mpris_track_id: None,
        };

        let metadata = lyrics_helper_metadata(&track);

        assert_eq!(metadata.title.as_deref(), Some("Song"));
        assert_eq!(metadata.artist.as_deref(), Some("Alice, Bob"));
        assert_eq!(
            metadata.artists.as_deref(),
            Some(&["Alice".to_string(), "Bob".to_string()][..])
        );
        assert_eq!(metadata.album.as_deref(), Some("Album"));
        assert_eq!(metadata.duration_ms, Some(123_000));
    }

    #[test]
    fn converts_lyrics_helper_lines_to_timed_lines() {
        let parsed = parse_local_lyrics("[00:01.00]First\n[00:03.00]Second").unwrap();
        let lines = timed_lines_from_data(&parsed);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].start_ms, 1_000);
        assert_eq!(lines[0].text, "First");
        assert_eq!(lines[1].start_ms, 3_000);
        assert_eq!(active_line_index(&lines, 3_200, 0), Some(1));
    }

    #[test]
    fn combines_translation_lrc_into_timed_lines() {
        let raw = combine_lyrics_with_translation(
            "[00:01.00]Hello\n[00:03.00]World",
            Some("[00:01.00]你好\n[00:03.00]世界"),
        );
        let lines = timed_lines_from_raw(&raw).unwrap();

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "Hello");
        assert_eq!(lines[0].translation.as_deref(), Some("你好"));
        assert_eq!(lines[1].text, "World");
        assert_eq!(lines[1].translation.as_deref(), Some("世界"));
    }

    #[test]
    fn ignores_placeholder_translation_lines() {
        let raw = combine_lyrics_with_translation(
            "[00:01.00]Hello\n[00:03.00]World",
            Some("[00:01.00]//\n[00:03.00]世界"),
        );
        let lines = timed_lines_from_raw(&raw).unwrap();

        assert_eq!(lines[0].translation, None);
        assert_eq!(lines[1].translation.as_deref(), Some("世界"));
    }

    #[test]
    fn combines_translation_qrc_into_timed_lines() {
        let raw = combine_lyrics_with_translation(
            "[1000,2000]Hel(1000,500)lo(1500,500)\n[3000,2000]World",
            Some("[1000,2000]你好\n[3000,2000]世界"),
        );
        let lines = timed_lines_from_raw(&raw).unwrap();

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].start_ms, 1_000);
        assert_eq!(lines[0].text, "Hello");
        assert_eq!(
            lines[0].syllables,
            vec![
                TimedSyllable {
                    start_ms: 1_000,
                    end_ms: 1_500,
                    text: "Hel".to_string(),
                },
                TimedSyllable {
                    start_ms: 1_500,
                    end_ms: 2_000,
                    text: "lo".to_string(),
                },
            ]
        );
        assert_eq!(lines[0].translation.as_deref(), Some("你好"));
        assert_eq!(lines[1].start_ms, 3_000);
        assert_eq!(lines[1].text, "World");
        assert_eq!(lines[1].translation.as_deref(), Some("世界"));
    }

    #[test]
    fn filters_intro_title_credit_and_speaker_label_lines() {
        let raw = "\
[0,1800]Señorita(0,600) - (600,200)Shawn(800,400) Mendes(1200,600)
[1800,2000]Lyrics (1800,500)by：(2300,500)Someone(2800,500)
[3800,1200]Both：(3800,1200)
[5000,1600]Camila (5000,500)Cabello：(5500,500)
[6600,2000]Ooh (6600,600)when (7200,400)your (7600,400)lips(8000,600)";
        let lines = timed_lines_from_raw(raw).unwrap();

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].start_ms, 6_600);
        assert_eq!(lines[0].text, "Ooh when your lips");
    }

    #[test]
    fn filters_non_lyric_translation_credit_lines() {
        let raw = combine_lyrics_with_translation(
            "[0,2000]Song(0,1000) - Artist(1000,1000)\n[2000,2000]Hello(2000,1000)",
            Some("[00:00.00]QQ音乐享有本翻译作品的著作权\n[00:02.00]你好"),
        );
        let lines = timed_lines_from_raw(&raw).unwrap();

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Hello");
        assert_eq!(lines[0].translation.as_deref(), Some("你好"));
    }

    #[test]
    fn search_plan_filters_removed_lrclib_provider() {
        let plan = SearchPlan::new([
            LyricsProvider::LrcLib,
            LyricsProvider::QqMusic,
            LyricsProvider::NetEase,
        ]);

        assert_eq!(
            plan.providers(),
            &[LyricsProvider::QqMusic, LyricsProvider::NetEase]
        );
    }
}
