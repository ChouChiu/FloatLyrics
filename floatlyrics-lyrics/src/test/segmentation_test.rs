use super::super::model::{TimedLine, TimedSyllable, Voice};
use super::segment_lines_into_words;

fn line(start_ms: u64, end_ms: Option<u64>, text: &str) -> TimedLine {
    TimedLine {
        start_ms,
        end_ms,
        text: text.to_string(),
        syllables: Vec::new(),
        translation: None,
        romanization: None,
        romanization_segments: Vec::new(),
        background: None,
        voice: Voice::Primary,
    }
}

fn syllable(start_ms: u64, end_ms: u64, text: &str) -> TimedSyllable {
    TimedSyllable {
        start_ms,
        end_ms,
        text: text.to_string(),
        romanization: String::new(),
        furigana: String::new(),
    }
}

fn words(line: &TimedLine) -> Vec<(u64, u64, &str)> {
    line.syllables
        .iter()
        .map(|syllable| (syllable.start_ms, syllable.end_ms, syllable.text.as_str()))
        .collect()
}

#[test]
fn splits_han_into_characters_sharing_the_line_evenly() {
    let mut lines = vec![line(1_000, Some(2_000), "你好世界")];

    segment_lines_into_words(&mut lines);

    assert_eq!(
        words(&lines[0]),
        vec![
            (1_000, 1_250, "你"),
            (1_250, 1_500, "好"),
            (1_500, 1_750, "世"),
            (1_750, 2_000, "界"),
        ]
    );
}

#[test]
fn keeps_latin_words_with_their_trailing_space() {
    let mut lines = vec![line(0, Some(1_000), "Hello world")];

    segment_lines_into_words(&mut lines);

    assert_eq!(
        words(&lines[0]),
        vec![(0, 500, "Hello "), (500, 1_000, "world")]
    );
}

#[test]
fn weights_latin_words_by_character_count() {
    let mut lines = vec![line(0, Some(700), "Hi there")];

    segment_lines_into_words(&mut lines);

    assert_eq!(words(&lines[0]), vec![(0, 200, "Hi "), (200, 700, "there")]);
}

#[test]
fn attaches_punctuation_to_the_neighbouring_word() {
    let mut lines = vec![line(0, Some(400), "（你好），世界")];

    segment_lines_into_words(&mut lines);

    // Weights: `（你` 1.2, `好），` 1.4, `世` 1, `界` 1 over 400 ms.
    assert_eq!(
        words(&lines[0]),
        vec![
            (0, 104, "（你"),
            (104, 226, "好），"),
            (226, 313, "世"),
            (313, 400, "界"),
        ]
    );
}

#[test]
fn closes_an_ambiguous_quote_on_its_own_word() {
    let mut lines = vec![line(0, Some(400), "\"Hello\" world")];

    segment_lines_into_words(&mut lines);

    let texts = lines[0]
        .syllables
        .iter()
        .map(|syllable| syllable.text.as_str())
        .collect::<Vec<_>>();
    assert_eq!(texts, vec!["\"Hello\" ", "world"]);
    assert_eq!(lines[0].syllables.last().map(|last| last.end_ms), Some(400));
}

#[test]
fn refines_provider_word_timing_without_moving_the_word() {
    let mut lines = vec![TimedLine {
        syllables: vec![
            syllable(3_000, 3_600, "안녕"),
            syllable(3_600, 4_000, " 세계"),
        ],
        ..line(3_000, Some(4_000), "안녕 세계")
    }];

    segment_lines_into_words(&mut lines);

    assert_eq!(
        words(&lines[0]),
        vec![
            (3_000, 3_300, "안"),
            (3_300, 3_600, "녕"),
            (3_600, 3_800, " 세"),
            (3_800, 4_000, "계"),
        ]
    );
}

#[test]
fn keeps_a_word_that_cannot_be_split_unchanged() {
    let mut lines = vec![TimedLine {
        syllables: vec![syllable(1_000, 1_400, "Hey")],
        ..line(1_000, Some(1_400), "Hey")
    }];

    segment_lines_into_words(&mut lines);

    assert_eq!(words(&lines[0]), vec![(1_000, 1_400, "Hey")]);
}

#[test]
fn splits_line_timed_lyrics_up_to_the_next_line() {
    let mut lines = vec![line(1_000, None, "你好"), line(2_000, Some(2_500), "世界")];

    segment_lines_into_words(&mut lines);

    assert_eq!(
        words(&lines[0]),
        vec![(1_000, 1_500, "你"), (1_500, 2_000, "好")]
    );
    assert_eq!(
        words(&lines[1]),
        vec![(2_000, 2_250, "世"), (2_250, 2_500, "界")]
    );
}

#[test]
fn leaves_the_last_line_untouched_without_a_span() {
    let mut lines = vec![line(9_000, None, "你好")];

    segment_lines_into_words(&mut lines);

    assert_eq!(lines[0].syllables, Vec::new());
}

#[test]
fn preserves_the_source_text_and_tiles_the_span_for_every_script() {
    const SOURCES: [&str; 8] = [
        "「안녕, world!」 (2:30) 你好…",
        "君の名前は？",
        "Привет, мир!",
        "Café ☕ déjà vu",
        "Don't stop believin'",
        "一二三、四五六。",
        "Ｈｅｌｌｏ、Ｗｏｒｌｄ",
        "   spaced   out   ",
    ];

    for source in SOURCES {
        let mut lines = vec![line(0, Some(3_000), source)];

        segment_lines_into_words(&mut lines);

        let syllables = &lines[0].syllables;
        let reassembled = syllables
            .iter()
            .map(|syllable| syllable.text.as_str())
            .collect::<String>();
        assert_eq!(reassembled, source, "text changed for {source:?}");

        assert_eq!(syllables.first().map(|first| first.start_ms), Some(0));
        assert_eq!(syllables.last().map(|last| last.end_ms), Some(3_000));
        for pair in syllables.windows(2) {
            assert!(
                pair[0].end_ms <= pair[1].start_ms,
                "spans overlap for {source:?}: {pair:?}"
            );
        }
    }
}

#[test]
fn skips_lines_with_no_text() {
    let mut lines = vec![
        line(0, Some(1_000), "   "),
        line(1_000, Some(2_000), "你好"),
    ];

    segment_lines_into_words(&mut lines);

    assert_eq!(lines[0].syllables, Vec::new());
    assert_eq!(words(&lines[1]).len(), 2);
}
