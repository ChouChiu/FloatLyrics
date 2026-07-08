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
    pub translation: Option<String>,
    pub romanization: Option<String>,
    pub background: Option<String>,
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

pub fn parse_local_lyrics(content: &str) -> Result<LyricsData> {
    parse_auto(content).context("lyrics-helper could not detect or parse lyrics")
}

pub fn timed_lines_from_raw(content: &str) -> Result<Vec<TimedLine>> {
    let lrc_lines = timed_lines_from_lrc(content);
    if !lrc_lines.is_empty() {
        return Ok(lrc_lines);
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
    merge_translation_marker_lines(timed_lines)
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
    let tagged_translation = tag_translation_lrc(translation);
    if tagged_translation.is_empty() {
        lyrics.to_string()
    } else {
        format!("{}\n{}\n", lyrics.trim_end(), tagged_translation)
    }
}

pub fn active_line_index(lines: &[TimedLine], playback_ms: u64, offset_ms: i64) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }

    let adjusted = (playback_ms as i128 + offset_ms as i128).max(0) as u64;
    let insertion = lines.partition_point(|line| line.start_ms <= adjusted);
    let index = insertion.checked_sub(1)?;
    let line = lines.get(index)?;

    match line.end_ms {
        Some(end_ms) if adjusted >= end_ms => None,
        _ => Some(index),
    }
}

fn timed_line_from_info(line: &LineInfo) -> Option<TimedLine> {
    let start_ms = ms_i32_to_u64(line.start_time_with_sub_line()?)?;
    let end_ms = line.end_time_with_sub_line().and_then(ms_i32_to_u64);
    let text = line.full_text();
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
        translation,
        romanization,
        background,
    })
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
                target.translation = Some(translation.to_string());
            }
            continue;
        }

        line.translation = line.translation.take().or_else(|| {
            line.background
                .take()
                .and_then(|value| marker_owned(&value))
        });
        merged.push(line);
    }

    merged
}

fn translation_marker_text(value: &str) -> Option<&str> {
    value
        .trim()
        .strip_prefix(TRANSLATION_PREFIX)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn marker_owned(value: &str) -> Option<String> {
    translation_marker_text(value).map(str::to_string)
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
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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

fn tag_translation_lrc(translation: &str) -> String {
    translation
        .lines()
        .filter_map(tag_translation_lrc_line)
        .collect::<Vec<_>>()
        .join("\n")
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
                    translation: None,
                    romanization: None,
                    background: None,
                });
            }
        }
    }

    lines.sort_by_key(|line| line.start_ms);
    merge_translation_marker_lines(lines)
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

fn tag_translation_lrc_line(line: &str) -> Option<String> {
    let line = line.trim();
    let (timestamps, text) = split_lrc_timestamps(line)?;
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    Some(format!("{timestamps}{TRANSLATION_PREFIX}{text}"))
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
