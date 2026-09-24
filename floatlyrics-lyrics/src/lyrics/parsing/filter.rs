// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Filters non-lyric display lines (credits, speaker labels, intro titles).

use lyrics_helper::helpers::chinese_helper::to_simplified;
use lyrics_helper::helpers::optimization::info_lines::is_info_line;

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
/// lyrics view draws. Inside it a role is read by upstream's credit vocabulary and
/// by its shape, which is what recognizes the roles neither list knows — QQ Music
/// writes `Vocals Arrangement`, `Recording Engineer`, and `Mixed in Dolby Atmos
/// by`, and NetEase writes `词` and `曲`. A sung row that contains a colon is
/// shaped like a credit too, and upstream's vocabulary is written to be read
/// against a whole document rather than a row at a time — its Chinese entries are
/// single characters, so `他说：你的声音很好听` reads as a credit on `声` — so
/// neither is trusted outside the block: the rows a provider wrote around its
/// lyrics are the ones before the first row it sang, whichever second they fall
/// on. QQ Music times its credit block up to fifteen seconds into the song, which
/// a fixed window over the intro cut off in the middle.
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

    /// Reads the row `text` that starts at `start_ms` as part of the block and
    /// returns whether the view draws it.
    ///
    /// The credits it reads are the ones a row of their own may continue, so the
    /// rows are read in the order the transcription wrote them.
    pub(super) fn drops(&mut self, start_ms: u64, text: &str) -> bool {
        let text = text.trim();
        let credit = is_credit_line(text, self.open);
        let row = credit
            || (self.after_credit && is_bracketed_name_list(text))
            || text.is_empty()
            || is_intro_title_line(start_ms, text)
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

fn is_intro_title_line(start_ms: u64, text: &str) -> bool {
    if start_ms > 5_000 {
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
        && (is_known_credit_key(&to_simplified(&key.to_lowercase()))
            || (in_block && is_credit_role(key)))
    {
        return true;
    }

    // Upstream keeps the roles a provider writes its credits with, along with the
    // copyright and distribution claims they are signed off with, and reads them
    // against the row folded to simplified Chinese — which is the vocabulary below
    // plus every role it never listed, and the notices no role names at all.
    //
    // Its vocabulary is written to be read against a whole document rather than a
    // row at a time: a row is a credit once it carries a colon and any one entry,
    // and the Chinese entries are single characters, so `他说：你的声音很好听` reads
    // as a credit on `声`. A row written without a colon cannot be read that way —
    // upstream then answers for the copyright and distribution claims alone, which
    // are spelled out far too fully to be mistaken for a row that was sung. So the
    // vocabulary is read inside the block, and the claims wherever they fall: a
    // provider signs its lyrics off after the last line as readily as before the
    // first.
    if (in_block || !carries_colon(text)) && is_info_line(text, None) {
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
        "作曲",
        "编曲",
        "制作人",
        "监制",
        "词:",
        "曲:",
        "演唱",
        "歌手",
        "专辑",
        "歌名",
        "歌曲",
        "标题",
        "歌:",
        "唱:",
        "原唱",
        "翻唱",
        "和声",
        "和音",
        "合音",
        "和声编写",
        "混音",
        "母带",
        "录音",
        "吉他",
        "钢琴",
        "贝斯",
        "鼓:",
        "弦乐",
        "发行",
        "厂牌",
        "上传",
        "歌词制作",
        "歌词编辑",
        "配唱",
        "出品",
        "版权",
        "词曲",
        "qq音乐享有",
        "以下歌词翻译由",
        "翻译:",
        // URLs / metadata
        "http://",
        "https://",
        "www.",
        // Typographic marks indicating extra-lyric content
        "\u{2117}",
    ];

    prefixes.iter().any(|prefix| normalized.starts_with(prefix))
}

/// Returns whether `text` is written with a colon in either width.
///
/// Upstream folds a full-width colon to a half-width one before it reads a row, so
/// both are the colon its vocabulary is keyed on.
fn carries_colon(text: &str) -> bool {
    text.contains(':') || text.contains('：')
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
        // ideograph, and the role is the same wherever the credit falls. The role
        // arrives folded to simplified Chinese, so each is written once.
        "词" | "曲"
            | "pgm"
            | "音乐总监"
            | "音响总监"
            | "音乐设计"
            | "乐队队长"
            | "键盘"
            | "管弦配器"
            | "和音"
            | "合音"
            | "竹笛"
            | "长笛"
            | "柳琴"
            | "打击乐"
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

/// Folds a row to the one spelling the credit vocabulary is written in.
///
/// A provider writes the same role in either script — `编曲` and `編曲` name the
/// one role — so the row is folded to simplified Chinese and the vocabulary lists
/// each role once.
fn normalize_line_text(text: &str) -> String {
    let normalized = text
        .trim()
        .trim_start_matches(['(', '[', '【'])
        .trim_end_matches([')', ']', '】'])
        .replace('：', ":")
        // NetEase writes its credits as `词 : 卡西恩Cacien`, which names the role the
        // vocabulary is written without the space for.
        .replace(" :", ":")
        .replace(": ", ":")
        .to_lowercase();

    to_simplified(&normalized)
}

#[cfg(test)]
#[path = "../../test/filter_test.rs"]
mod tests;
