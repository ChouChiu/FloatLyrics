use super::*;

use crate::lyrics::model::Voice;

fn line_at(text: &str, start_ms: u64) -> TimedLine {
    TimedLine {
        start_ms,
        end_ms: None,
        text: text.to_string(),
        syllables: vec![],
        translation: None,
        romanization: None,
        romanization_segments: Vec::new(),
        background: None,
        voice: Voice::Primary,
    }
}

/// The rows the lyrics view draws, out of the rows of a payload in the order the
/// transcription wrote them.
fn drawn(rows: &[(&str, u64)]) -> Vec<String> {
    let mut metadata = Metadata::new();
    rows.iter()
        .filter(|(text, start)| !metadata.drops(&line_at(text, *start)))
        .map(|(text, _)| (*text).to_string())
        .collect()
}

#[test]
fn filters_the_credit_block_a_provider_writes_before_its_lyrics() {
    // QQ Music writes its credits up to fifteen seconds into the song, and the roles
    // of the last ones are not the ones written the same way elsewhere.
    let rows = drawn(&[
        ("Perfect Night - LE SSERAFIM", 0),
        ("Lyrics by：SCORE(13)/Megatone(13)/Sofia Quinn", 466),
        ("Produced by：13/\"hitman\" bang", 6061),
        ("Keyboard：SCORE(13)", 9231),
        (
            "Vocals Arrangement：SCORE(13)/Megatone(13)/Young Chance",
            11002,
        ),
        ("Digital Editing：SCORE(13)/Megatone(13)/김준혁", 11748),
        ("Recording Engineer：황민희/이평욱 @ HYBE Studio", 12401),
        ("Mix Engineer：Josh Gudwin @ Henson Studios", 12960),
        (
            "Mastering Engineer：Chris Gehringer @ Sterling Sound",
            14173,
        ),
        ("Me and my girlies", 14733),
        ("We gon party 'til its early", 15803),
    ]);

    assert_eq!(
        rows,
        vec!["Me and my girlies", "We gon party 'til its early"]
    );
}

#[test]
fn filters_the_credit_written_with_a_space_before_a_single_ideograph() {
    // NetEase writes the role of its first two credits as `词 : 卡西恩Cacien`, which
    // is one ideograph and names the same role wherever the credit falls.
    let rows = drawn(&[
        (" 词 : 卡西恩Cacien", 0),
        (" 曲 : 卡西恩Cacien", 1000),
        (" 编曲 : John Ho/卡西恩Cacien", 2000),
        (" 制作 : 卡西恩Cacien/John Ho", 3000),
        ("Baby", 8290),
    ]);

    assert_eq!(rows, vec!["Baby"]);
}

#[test]
fn filters_a_credit_split_across_a_row_of_its_own() {
    // QQ Music repeats the names of a credit on a bracketed row that follows it,
    // which is the rest of the credit rather than a line sung with brackets.
    let rows = drawn(&[
        ("Produced by：13/\"hitman\" bang", 6061),
        (
            "(SCORE(13)/Megatone(13)/Sofia Quinn/\"hitman\" bang/Amanda \"Kiddo A.I. Ibanez\")",
            6527,
        ),
        ("Me and my girlies", 14733),
    ]);

    assert_eq!(rows, vec!["Me and my girlies"]);
}

#[test]
fn filters_english_credit_lines() {
    // A known role is read wherever its credit falls, beyond the block as well.
    let rows = drawn(&[
        ("We gon party 'til its early", 15803),
        ("Mixing: Engineer", 40000),
    ]);

    assert_eq!(rows, vec!["We gon party 'til its early"]);
}

#[test]
fn keeps_a_sung_row_that_contains_a_colon() {
    // The block closes at the first row the view draws, so a row of the lyrics that
    // is shaped like a credit is read as one wherever it falls.
    let rows = drawn(&[
        ("We gon party 'til its early", 15803),
        ("love: it's real", 30000),
        ("Baby: I love you", 60000),
    ]);

    assert_eq!(
        rows,
        vec![
            "We gon party 'til its early",
            "love: it's real",
            "Baby: I love you"
        ]
    );
}

#[test]
fn filters_generic_key_value_metadata_in_intro() {
    let rows = drawn(&[
        ("出品：某唱片公司", 1500),
        ("配唱：某人", 300),
        ("Baby", 8290),
    ]);

    assert_eq!(rows, vec!["Baby"]);
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
    let rows = drawn(&[
        ("http://example.com", 0),
        ("https://example.com", 100),
        ("www.example.com", 200),
        ("℗ 2024 Label", 300),
        ("Baby", 8290),
    ]);

    assert_eq!(rows, vec!["Baby"]);
}

#[test]
fn filters_a_role_upstreams_vocabulary_knows_and_the_shape_does_not() {
    // A role is not always written as a heading: QQ Music writes the place a part
    // was recorded into the role itself, and a provider that writes its roles in
    // lower case leaves nothing for the shape to read. Upstream's vocabulary knows
    // the words either one is built from.
    let rows = drawn(&[
        ("Recorded @ HYBE Studio：황민희", 500),
        ("mixing engineer：Josh Gudwin", 1200),
        ("Me and my girlies", 14733),
    ]);

    assert_eq!(rows, vec!["Me and my girlies"]);
}

#[test]
fn filters_a_traditional_credit_beyond_the_block() {
    // A role is read wherever its credit falls, and the two scripts a provider may
    // write it in name the one role: the row is folded to simplified Chinese, so
    // the vocabulary spells each role once rather than twice.
    let rows = drawn(&[
        ("We gon party 'til its early", 15803),
        ("編曲：John Ho", 40000),
        ("混音：Josh Gudwin", 41000),
    ]);

    assert_eq!(rows, vec!["We gon party 'til its early"]);
}

#[test]
fn filters_the_notice_a_provider_signs_its_lyrics_off_with() {
    // A copyright notice is written after the last line rather than before the
    // first, so it falls outside the block. It names no role and carries no colon:
    // what reads it is the notice's own wording, which no sung row spells out.
    let rows = drawn(&[
        ("We gon party 'til its early", 15803),
        ("未经著作权人许可不得翻唱翻录或使用", 200_000),
        ("腾讯音乐娱乐集团享有本翻译作品的著作权", 201_000),
    ]);

    assert_eq!(rows, vec!["We gon party 'til its early"]);
}

#[test]
fn keeps_a_sung_row_naming_something_the_vocabulary_lists() {
    // Upstream's Chinese entries are single characters — `声`, `曲`, `鼓` — and a
    // row carrying a colon and any one of them reads as a credit. A row the view
    // has begun drawing is past the block, where that reading is not trusted.
    let rows = drawn(&[
        ("We gon party 'til its early", 15803),
        ("他说：你的声音很好听", 30000),
        ("鼓起勇气说：我喜欢你", 31000),
    ]);

    assert_eq!(
        rows,
        vec![
            "We gon party 'til its early",
            "他说：你的声音很好听",
            "鼓起勇气说：我喜欢你"
        ]
    );
}
