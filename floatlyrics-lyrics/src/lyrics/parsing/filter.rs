// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Filters non-lyric display lines (credits, speaker labels, intro titles).

use crate::lyrics::model::TimedLine;

/// Returns `true` when the line should be hidden from lyric display
/// (credit, speaker label, or intro title lines).
pub(super) fn is_non_lyric_display_line(line: &TimedLine) -> bool {
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
        "\u{2117}",
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

#[cfg(test)]
#[path = "../../test/filter_test.rs"]
mod tests;
