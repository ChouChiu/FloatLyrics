// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Filters non-lyric display lines (credits, speaker labels, intro titles).

use crate::lyrics::model::TimedLine;

/// The longest a credit role is written with.
///
/// QQ Music writes `Mixed in Dolby Atmos by` and `Computer programming by`, which
/// are the longest roles in the material read here.
const CREDIT_ROLE_MAX_CHARS: usize = 24;

/// What a credit role is written with besides letters, digits, and ideographs.
const ROLE_PUNCTUATION: [char; 3] = ['-', '&', '/'];

/// The fewest names a bracketed row has to list to be part of a credit.
const CREDIT_CONTINUATION_NAMES: usize = 3;

/// The rows a provider writes around its lyrics, read as the block they stand in.
///
/// The block opens at the first row of the payload and closes at the first row the
/// lyrics view draws. Reading a credit role by its shape is what recognizes the
/// roles the vocabulary does not know — QQ Music writes `Vocals Arrangement`,
/// `Recording Engineer`, and `Mixed in Dolby Atmos by`, and NetEase writes `词` and
/// `曲` — but a sung row that contains a colon is shaped like one too, so the shape
/// is read only inside the block: the rows a provider wrote around its lyrics are
/// the ones before the first row it sang, whichever second they fall on. QQ Music
/// times its credit block up to fifteen seconds into the song, which a fixed window
/// over the intro cut off in the middle.
pub(super) struct Metadata {
    /// Whether every row read so far was written around the lyrics.
    open: bool,
    /// Whether the row before this one was a credit.
    after_credit: bool,
}

impl Metadata {
    pub(super) fn new() -> Self {
        Self {
            open: true,
            after_credit: false,
        }
    }

    /// Reads `line` as part of the block and returns whether the view draws it.
    ///
    /// The credits it reads are the ones a row of their own may continue, so the
    /// rows are read in the order the transcription wrote them.
    pub(super) fn drops(&mut self, line: &TimedLine) -> bool {
        let text = line.text.trim();
        let credit = is_credit_line(text, self.open);
        let row = credit
            || (self.after_credit && is_bracketed_name_list(text))
            || text.is_empty()
            || is_intro_title_line(line, text)
            || is_speaker_label_line(text);

        self.after_credit = credit;
        if !row {
            self.open = false;
        }
        row
    }
}

/// Returns whether `text` is the names of a credit written on a row of their own.
///
/// QQ Music repeats the names a credit lists on a bracketed row that follows it —
/// `Produced by：13/"hitman" bang` and then `(SCORE(13)/Megatone(13)/Sofia Quinn/…)`
/// — which is the rest of that credit rather than a line the view draws with its
/// brackets. The names are read by their shape, because the row repeats names a
/// credit spelled in another script in between: the names of one are the names of
/// the other, written differently.
fn is_bracketed_name_list(text: &str) -> bool {
    let trimmed = text.trim();
    let Some(inner) = trimmed
        .strip_prefix(['(', '（'])
        .and_then(|rest| rest.strip_suffix([')', '）']))
    else {
        return false;
    };

    inner.trim().split('/').count() >= CREDIT_CONTINUATION_NAMES
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

fn is_credit_line(text: &str, in_block: bool) -> bool {
    let normalized = normalize_line_text(text);

    // Key-value credits can extend well beyond the first ten seconds in live
    // releases, so a role the vocabulary knows is read wherever its credit falls. An
    // unknown one is read by its shape, which is only trusted inside the block the
    // provider wrote around its lyrics.
    if let Some((key, _value)) = split_credit_key(text)
        && (is_known_credit_key(&key.to_lowercase()) || (in_block && is_credit_role(key)))
    {
        return true;
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
        "和音",
        "合音",
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

/// Splits a row into the role it names and the names it credits.
///
/// The role is read before the text is folded to lower case, because a role is
/// written as a heading and it is the case that tells it from the head of a sung
/// line. The spaces a provider puts around the colon are not part of the role.
fn split_credit_key(text: &str) -> Option<(&str, &str)> {
    let (key, value) = text.split_once([':', '：'])?;
    let key = key.trim();
    (!key.is_empty()).then_some((key, value))
}

/// Returns whether `key` reads as a credit role rather than as the head of a line
/// that was sung.
///
/// A role is written as a heading — `Lyrics by`, `Vocals Arrangement`, `Recording
/// Engineer`, `Sub-publisher`, `词` — and a sung line that contains a colon is not:
/// it opens with a lowercase word (`love: it's real`) as readily as with a capital,
/// and it carries the punctuation a sentence is written with, which a role is not.
fn is_credit_role(key: &str) -> bool {
    let Some(first) = key.chars().next() else {
        return false;
    };
    // An ideograph is written in one case, so a role spelled with one is read by the
    // ideograph itself; the Latin roles are written as headings.
    if !(first.is_uppercase() || first >= '\u{2e80}') || key.starts_with("http") {
        return false;
    }

    key.chars().count() <= CREDIT_ROLE_MAX_CHARS
        && key.chars().all(|character| {
            character.is_alphanumeric() || character == ' ' || ROLE_PUNCTUATION.contains(&character)
        })
}

fn is_known_credit_key(key: &str) -> bool {
    let mut components = key.split('/').map(str::trim);
    let Some(first) = components.next() else {
        return false;
    };

    is_known_credit_key_component(first) && components.all(is_known_credit_key_component)
}

fn is_known_credit_key_component(key: &str) -> bool {
    matches!(
        key,
        // NetEase writes `词 : 卡西恩Cacien` and `曲 : 卡西恩Cacien`: the role is one
        // ideograph, and the role is the same wherever the credit falls.
        "词" | "詞"
            | "曲"
            | "pgm"
            | "音乐总监"
            | "音樂總監"
            | "音响总监"
            | "音響總監"
            | "音乐设计"
            | "音樂設計"
            | "乐队队长"
            | "樂隊隊長"
            | "键盘"
            | "鍵盤"
            | "管弦配器"
            | "和音"
            | "合音"
            | "竹笛"
            | "长笛"
            | "長笛"
            | "柳琴"
            | "打击乐"
            | "打擊樂"
    )
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
        // NetEase writes its credits as `词 : 卡西恩Cacien`, which names the role the
        // vocabulary is written without the space for.
        .replace(" :", ":")
        .replace(": ", ":")
        .to_lowercase()
}

#[cfg(test)]
#[path = "../../test/filter_test.rs"]
mod tests;
