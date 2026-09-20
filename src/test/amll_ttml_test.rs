use super::*;

use crate::shared::presentation::{LyricsDocument, PresentedLyricLine};
use floatlyrics_lyrics::lyrics::{TimedSyllable, Voice};

fn word(
    start_ms: u64,
    end_ms: u64,
    text: &str,
    romanization: &str,
    furigana: &str,
) -> TimedSyllable {
    TimedSyllable {
        start_ms,
        end_ms,
        text: text.to_string(),
        romanization: romanization.to_string(),
        furigana: furigana.to_string(),
    }
}

fn line(start_ms: u64, end_ms: Option<u64>, text: &str) -> PresentedLyricLine {
    PresentedLyricLine {
        start_ms,
        end_ms,
        text: text.to_string(),
        syllables: Vec::new(),
        romanization: String::new(),
        translation: String::new(),
        background: String::new(),
        background_translation: String::new(),
        background_start_ms: 0,
        background_end_ms: None,
        background_syllables: Vec::new(),
        voice: Voice::Primary,
    }
}

fn lyrics(lines: Vec<PresentedLyricLine>) -> LyricsDocument {
    LyricsDocument {
        revision: 1,
        duration_ms: Some(120_000),
        lines,
    }
}

/// A listener draws its own karaoke, so a line reaches it as words with their
/// timing, and the readings of those words follow them in a span of their own.
#[test]
fn writes_a_word_timed_line_with_its_readings_and_translation() {
    let mut first = line(1_000, Some(3_000), "안녕 세계");
    first.syllables = vec![
        word(1_000, 1_300, "안녕", "annyeong", ""),
        word(1_300, 1_600, " ", "", ""),
        word(1_600, 2_400, "세계", "segye", ""),
    ];
    first.translation = "你好世界".to_string();

    let xml = document(&lyrics(vec![first]));

    assert!(
        xml.contains(r#"<p begin="1.000" end="3.000" ttm:agent="v1" itunes:key="L1">"#),
        "{xml}"
    );
    assert!(
        xml.contains(r#"<span begin="1.000" end="1.300">안녕</span> "#),
        "a space stays with the word it follows: {xml}"
    );
    assert!(
        xml.contains(r#"<span begin="1.600" end="2.400">세계</span>"#),
        "{xml}"
    );
    assert!(
        xml.contains(
            r#"<transliteration xml:lang="ja-Latn"><text for="L1"><span begin="1.000" end="1.300">annyeong</span><span begin="1.600" end="2.400">segye</span></text>"#
        ),
        "readings are written per line and placed by time: {xml}"
    );
    assert!(
        xml.contains(r#"<span ttm:role="x-translation" xml:lang="zh-CN">你好世界</span>"#),
        "{xml}"
    );
}

/// An empty word list leaves the line as it was written, which is what a line
/// without word timing is.
#[test]
fn writes_a_line_without_word_timing_as_text() {
    let mut plain = line(0, Some(1_000), "こんにちは");
    plain.romanization = "konnichiwa".to_string();

    let xml = document(&lyrics(vec![plain]));

    assert!(
        xml.contains(r#"<span begin="0.000" end="1.000">こんにちは</span>"#),
        "{xml}"
    );
    assert!(
        xml.contains(r#"<text for="L1"><span begin="0.000" end="1.000">konnichiwa</span></text>"#),
        "{xml}"
    );
}

/// The kana of a word is written above the character it reads, which is what a
/// listener needs to draw furigana at all.
#[test]
fn writes_the_kana_of_a_word_above_it() {
    let mut japanese = line(1_000, Some(3_000), "世界");
    japanese.syllables = vec![
        word(1_000, 2_000, "世", "se", "せ"),
        word(2_000, 3_000, "界", "kai", "かい"),
    ];

    let xml = document(&lyrics(vec![japanese]));

    assert!(
        xml.contains(
            r#"<span tts:ruby="container"><span tts:ruby="base">世</span><span tts:ruby="textContainer"><span tts:ruby="text" begin="1.000" end="2.000">せ</span></span></span>"#
        ),
        "{xml}"
    );
    assert!(
        xml.contains(r#"<span tts:ruby="text" begin="2.000" end="3.000">かい</span>"#),
        "{xml}"
    );
}

/// A word read as a whole keeps one reading for all of its characters, so they
/// are written as one word and the kana is drawn across them.
#[test]
fn writes_a_word_read_as_a_whole_as_one_word() {
    let mut japanese = line(0, Some(1_000), "今日");
    japanese.syllables = vec![
        word(0, 500, "今", "kyou", "きょう"),
        word(500, 1_000, "日", "", ""),
    ];

    let xml = document(&lyrics(vec![japanese]));

    assert!(
        xml.contains(r#"<span tts:ruby="base">今日</span>"#),
        "{xml}"
    );
    assert!(
        xml.contains(r#"<span tts:ruby="text" begin="0.000" end="1.000">きょう</span>"#),
        "{xml}"
    );
}

/// Background vocals are written beside the line they belong to rather than as a
/// line of their own, and carry the translation of the phrase they repeat.
#[test]
fn writes_background_vocals_beside_the_line() {
    // A background vocal the provider timed nothing for is drawn over the line
    // it belongs to.
    let mut lead = line(0, Some(1_000), "lead");
    lead.background = "echo".to_string();

    let xml = document(&lyrics(vec![lead]));

    assert!(
        xml.contains(
            r#"<span ttm:role="x-bg"><span begin="0.000" end="1.000">(echo)</span></span>"#
        ),
        "{xml}"
    );

    // An echo is sung where it stands, which is where it is drawn.
    let mut answered = line(4_000, Some(5_500), "answered");
    answered.background = "My way".to_string();
    answered.background_translation = "我走过的路".to_string();
    answered.background_start_ms = 4_800;
    answered.background_end_ms = Some(5_400);

    let xml = document(&lyrics(vec![answered]));

    assert!(
        xml.contains(
            r#"<span ttm:role="x-bg"><span begin="4.800" end="5.400">(My way)</span><span ttm:role="x-translation" xml:lang="zh-CN">我走过的路</span></span>"#
        ),
        "the echo keeps its own timing and its own translation: {xml}"
    );
}

/// The characters XML reserves cannot appear in text as they are.
#[test]
fn escapes_the_characters_xml_reserves() {
    let mut awkward = line(0, Some(1_000), "a < b & c");
    awkward.translation = "\"quoted\"".to_string();

    let xml = document(&lyrics(vec![awkward]));

    assert!(xml.contains("a &lt; b &amp; c"), "{xml}");
    assert!(xml.contains("&quot;quoted&quot;"), "{xml}");
}

/// A line reaches the end the listener stops drawing it at: its own, the next
/// line, or the track duration, and a fixed span when nothing bounds it.
#[test]
fn resolves_line_ends_like_the_overlay_does() {
    let first = line(0, None, "first");
    let second = line(9_000, None, "second");
    let xml = document(&lyrics(vec![first, second]));
    assert!(
        xml.contains(r#"<p begin="0.000" end="9.000" ttm:agent="v1" itunes:key="L1">"#),
        "{xml}"
    );
    assert!(
        xml.contains(r#"<p begin="9.000" end="2:00.000" ttm:agent="v1" itunes:key="L2">"#),
        "a minute of playback carries its minutes: {xml}"
    );

    let unbounded = LyricsDocument {
        revision: 1,
        duration_ms: None,
        lines: vec![line(7_000, None, "only")],
    };
    let xml = document(&unbounded);
    assert!(
        xml.contains(r#"<p begin="7.000" end="12.000" ttm:agent="v1" itunes:key="L1">"#),
        "{xml}"
    );
}

/// A duet reaches the listener as the performer of each line: it alternates the
/// two sides from the agent it reads on every line, so the opposing part is
/// written with an agent of its own while the main part keeps the default one.
#[test]
fn writes_the_performer_of_each_line_as_its_agent() {
    let mut answer = line(3_000, Some(4_000), "I can't feel my face");
    answer.voice = Voice::Secondary;

    let xml = document(&lyrics(vec![
        line(1_000, Some(2_000), "Take my hand"),
        answer,
    ]));

    assert!(
        xml.contains(r#"<p begin="1.000" end="2.000" ttm:agent="v1" itunes:key="L1">"#),
        "{xml}"
    );
    assert!(
        xml.contains(r#"<p begin="3.000" end="4.000" ttm:agent="v2" itunes:key="L2">"#),
        "{xml}"
    );
}

/// A track without lyrics still leaves the listener a document it can read.
#[test]
fn keeps_an_empty_document_valid() {
    let xml = document(&LyricsDocument {
        revision: 0,
        duration_ms: None,
        lines: Vec::new(),
    });

    assert!(
        xml.contains("<body dur=\"0.000\"><div begin=\"0.000\" end=\"0.000\"></div></body>"),
        "{xml}"
    );
}
