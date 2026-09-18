use super::search::lyrics_helper_metadata;
use super::*;

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

fn romanized_lines(raw: &str) -> Vec<TimedLine> {
    let mut lines = timed_lines_from_raw(raw, &[]).unwrap();
    generate_local_romanization(&mut lines);
    lines
}

/// The text and the generated reading of every syllable of a line.
fn syllable_readings(line: &TimedLine) -> Vec<(&str, &str)> {
    line.syllables
        .iter()
        .map(|syllable| (syllable.text.as_str(), syllable.romanization.as_str()))
        .collect()
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
fn rejects_xml_lyrics_before_the_dependency_parser() {
    let error = parse_local_lyrics("\u{feff}  <tt><body /></tt>").unwrap_err();

    assert!(error.to_string().contains("XML lyrics"));
    assert!(parse_auto("<tt><body /></tt>").is_none());
}

#[test]
fn maps_track_metadata_for_lyrics_helper_search() {
    let track = floatlyrics_core::track::TrackMetadata {
        title: "Song".to_string(),
        artists: vec!["Alice".to_string(), "Bob".to_string()],
        album: Some("Album".to_string()),
        duration_ms: Some(123_000),
        mpris_track_id: None,
        art_url: None,
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
    let lines = timed_lines_from_data(&parsed, &[]);

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].start_ms, 1_000);
    assert_eq!(lines[0].text, "First");
    assert_eq!(lines[1].start_ms, 3_000);
    assert_eq!(active_line_index(&lines, 3_200, 0), Some(1));
}

/// Upstream merges the syllables of a word into one item and keeps the timing of
/// the items it was merged from; those are the times a renderer animates.
#[test]
fn merged_syllable_items_keep_the_timing_of_their_parts() {
    use lyrics_helper::{FullSyllableInfo, SyllableInfo, SyllableItem};

    let merged = SyllableItem::from(FullSyllableInfo::new(vec![
        SyllableInfo::new("合".to_string(), 1_000, 1_250),
        SyllableInfo::new("声".to_string(), 1_250, 1_500),
    ]));
    let data = LyricsData {
        lines: Some(vec![LineInfo::new_syllable(vec![
            merged,
            SyllableItem::from(SyllableInfo::new("是你".to_string(), 1_500, 2_000)),
        ])]),
        ..LyricsData::default()
    };

    let lines = timed_lines_from_data(&data, &[]);

    assert_eq!(lines[0].text, "合声是你");
    assert_eq!(
        lines[0]
            .syllables
            .iter()
            .map(|syllable| (syllable.start_ms, syllable.end_ms, syllable.text.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (1_000, 1_250, "合"),
            (1_250, 1_500, "声"),
            (1_500, 2_000, "是你")
        ]
    );
}

#[test]
fn generates_japanese_romanization_locally() {
    let lines = romanized_lines("[00:01.00]こんにちは世界\n[00:03.00]音楽");

    assert_eq!(lines[0].romanization.as_deref(), Some("konnichiwa sekai"));
    assert_eq!(lines[1].romanization.as_deref(), Some("ongaku"));
}

#[test]
fn generates_chinese_pinyin_without_treating_it_as_japanese() {
    let lines = romanized_lines("[00:01.00]你好世界\n[00:03.00]我喜欢你");

    assert_eq!(lines[0].romanization.as_deref(), Some("nǐ hǎo shì jiè"));
    assert_eq!(lines[1].romanization.as_deref(), Some("wǒ xǐ huan nǐ"));
}

#[test]
fn generates_cantonese_jyutping_when_requested() {
    let mut lines = vec![line(0, Some(1_000), "喜歡你"), line(1_000, None, "喜欢你")];

    generate_local_romanization_with_mode(&mut lines, ChineseRomanizationMode::CantoneseJyutping);

    assert_eq!(lines[0].romanization.as_deref(), Some("hei2 fun1 nei5"));
    assert_eq!(lines[1].romanization.as_deref(), Some("hei2 fun1 nei5"));
}

#[test]
fn generates_cantonese_jyutping_without_tones_when_requested() {
    let mut lines = vec![line(0, None, "喜歡你")];

    generate_local_romanization_with_mode(
        &mut lines,
        ChineseRomanizationMode::CantoneseJyutpingNoTones,
    );

    assert_eq!(lines[0].romanization.as_deref(), Some("hei fun nei"));
}

#[test]
fn automatic_chinese_mode_uses_explicit_cantonese_markers() {
    let mut lines = vec![line(0, None, "佢喜歡你")];

    generate_local_romanization(&mut lines);

    assert_eq!(
        lines[0].romanization.as_deref(),
        Some("keoi5 hei2 fun1 nei5")
    );
}

#[test]
fn does_not_generate_romanization_for_non_cjk_text() {
    let lines = romanized_lines(
        "[00:01.00]Привет мир\n\
         [00:02.00]¿Cómo estás?\n\
         [00:03.00]Muchas gracias mi amor\n\
         [00:04.00]Hello world",
    );

    assert!(lines.iter().all(|line| line.romanization.is_none()));
}

#[test]
fn applies_korean_pronunciation_rules() {
    let lines = romanized_lines("[00:01.00]안녕하세요 세계\n[00:03.00]왕십리 같이");

    assert_eq!(
        lines[0].romanization.as_deref(),
        Some("annyeonghaseyo segye")
    );
    assert_eq!(lines[1].romanization.as_deref(), Some("wangsimni gachi"));
}

#[test]
fn keeps_only_korean_readings_in_mixed_lines() {
    let lines = romanized_lines("[00:01.00]안녕 Hello 세계\n[00:03.00]Hello world");

    assert_eq!(lines[0].romanization.as_deref(), Some("annyeong segye"));
    assert_eq!(lines[1].romanization, None);
    assert_eq!(
        lines[0]
            .romanization_segments
            .iter()
            .find(|segment| segment.text.contains("Hello"))
            .map(|segment| segment.romanization.as_str()),
        Some("")
    );
}

#[test]
fn keeps_only_han_readings_in_mixed_chinese_lines() {
    let lines = romanized_lines("[00:01.00]Hello 世界\n[00:03.00]我喜欢你 Baby");

    assert_eq!(lines[0].romanization.as_deref(), Some("shì jiè"));
    assert_eq!(lines[1].romanization.as_deref(), Some("wǒ xǐ huan nǐ"));
}

#[test]
fn keeps_only_han_readings_in_mixed_cantonese_lines() {
    let mut lines = vec![line(0, Some(1_000), "Hello 喜歡你")];

    generate_local_romanization_with_mode(&mut lines, ChineseRomanizationMode::CantoneseJyutping);

    assert_eq!(lines[0].romanization.as_deref(), Some("hei2 fun1 nei5"));
}

#[test]
fn keeps_japanese_whole_line_readings_in_mixed_lines() {
    let lines = romanized_lines("[00:01.00]こんにちは World");

    assert_eq!(lines[0].romanization.as_deref(), Some("konnichiwa World"));
}

#[test]
fn attaches_korean_readings_to_timed_syllables() {
    let mut lines = timed_lines_from_raw("[1000,2000]안녕(0,500)하세요(500,500)", &[]).unwrap();

    generate_local_romanization(&mut lines);

    assert_eq!(lines[0].romanization.as_deref(), Some("annyeonghaseyo"));
    assert_eq!(
        syllable_readings(&lines[0]),
        vec![("안녕", "annyeong"), ("하세요", "haseyo")]
    );
}

#[test]
fn attaches_chinese_readings_to_timed_syllables() {
    let mut lines = timed_lines_from_raw("[1000,2000]你好(0,500)世界(500,500)", &[]).unwrap();

    generate_local_romanization(&mut lines);

    assert_eq!(lines[0].romanization.as_deref(), Some("nǐ hǎo shì jiè"));
    assert_eq!(
        syllable_readings(&lines[0]),
        vec![("你好", "nǐhǎo"), ("世界", "shìjiè")]
    );
}

/// Every kana character carries its own reading, and a word the dictionary reads
/// as a unit keeps its reading on the character it starts at instead of repeating
/// it under all of them.
#[test]
fn attaches_japanese_readings_to_timed_syllables() {
    let mut lines = timed_lines_from_raw("[1000,4000]こんにちは世界", &[]).unwrap();
    segment_lines_into_words(&mut lines);

    generate_local_romanization(&mut lines);

    assert_eq!(lines[0].romanization.as_deref(), Some("konnichiwa sekai"));
    assert_eq!(
        syllable_readings(&lines[0]),
        vec![
            ("こ", "ko"),
            ("ん", "n"),
            ("に", "ni"),
            ("ち", "chi"),
            ("は", "wa"),
            ("世", "se"),
            ("界", "kai"),
        ]
    );
}

/// A digraph or a doubled consonant is one mora, so the character after it is
/// read with it rather than on its own.
#[test]
fn splits_japanese_readings_at_digraphs() {
    let mut lines = timed_lines_from_raw("[1000,4000]ちょっとキョウ", &[]).unwrap();
    segment_lines_into_words(&mut lines);

    generate_local_romanization(&mut lines);

    assert_eq!(
        syllable_readings(&lines[0]),
        vec![
            ("ち", "cho"),
            ("ょ", ""),
            ("っ", "t"),
            ("と", "to"),
            ("キ", "kyo"),
            ("ョ", ""),
            ("ウ", "u"),
        ]
    );
}

/// A prolonged sound mark holds the vowel it lengthens, so every character of
/// the word carries a reading of its own.
#[test]
fn reads_prolonged_sound_marks_as_the_lengthened_vowel() {
    let mut lines = timed_lines_from_raw("[1000,4000]ノート", &[]).unwrap();
    segment_lines_into_words(&mut lines);

    generate_local_romanization(&mut lines);

    assert_eq!(lines[0].romanization.as_deref(), Some("nooto"));
    assert_eq!(
        syllable_readings(&lines[0]),
        vec![("ノ", "no"), ("ー", "o"), ("ト", "to")]
    );
}

/// A word is read from the line it sits in: the dictionary reads `好` with the
/// word it belongs to and `君` as the pronoun it is here, and the word readings
/// are a split of the line's own reading, so they add up to it instead of
/// contradicting it.
#[test]
fn reads_japanese_words_from_the_reading_of_the_line() {
    let mut lines = timed_lines_from_raw("[1000,4000]好きだよ(0,1000)君(1000,1000)", &[]).unwrap();
    segment_lines_into_words(&mut lines);

    generate_local_romanization(&mut lines);

    assert_eq!(lines[0].romanization.as_deref(), Some("suki da yo kimi"));
    assert_eq!(
        syllable_readings(&lines[0]),
        vec![
            ("好", "su"),
            ("き", "ki"),
            ("だ", "da"),
            ("よ", "yo"),
            ("君", "kimi")
        ]
    );
}

/// Whatever a view shows under a word is a split of the pronunciation shown for
/// the line: the two can never disagree.
#[test]
fn japanese_word_readings_add_up_to_the_line_reading() {
    for text in [
        "こんにちは世界",
        "好きだよ",
        "今日はいい天気ですね",
        "ちょっと",
        "ノート",
        "だって、そうだよ",
        "がっこうへ行く",
        "しんいち",
        "スーパーマーケット",
        "食べる",
    ] {
        let mut lines = vec![line(0, Some(9_000), text)];
        segment_lines_into_words(&mut lines);

        generate_local_romanization(&mut lines);

        let line = &lines[0];
        // Separators are what the two sides differ in: the line reading keeps
        // the space between words, the word readings do not.
        let words = line
            .syllables
            .iter()
            .map(|syllable| syllable.romanization.as_str())
            .collect::<String>()
            .chars()
            .filter(|character| character.is_ascii_alphanumeric() || *character == '\'')
            .collect::<String>();
        let expected = line
            .romanization
            .as_deref()
            .unwrap_or_default()
            .chars()
            .filter(|character| character.is_ascii_alphanumeric() || *character == '\'')
            .collect::<String>();
        assert!(!expected.is_empty(), "{text}");
        assert_eq!(words, expected, "{text}");
    }
}

/// A kanji is read with the word it sits in rather than on its own, and a word
/// whose reading spans several kanji keeps it on the one it starts at.
#[test]
fn reads_japanese_kanji_with_the_word_it_belongs_to() {
    for (text, reading) in [
        ("君の名前を呼んでいる", "kimi no namae wo yon de iru"),
        ("今日は雨が降る", "kyou wa ame ga furu"),
        ("運命の人", "unmei no hito"),
        ("一昨日の話", "ototoi no hanashi"),
        ("世界中の誰より", "sekaijuu no dare yori"),
    ] {
        let lines = romanized_lines(&format!("[00:01.00]{text}"));

        assert_eq!(lines[0].romanization.as_deref(), Some(reading), "{text}");
    }
}

/// `は` and `へ` are read as the sounds they are heard as when they stand for the
/// particles of those names, while an orthographic long vowel keeps the spelling
/// the word is written with.
#[test]
fn reads_japanese_particles_as_they_are_pronounced() {
    for (text, reading) in [
        ("こんにちは", "konnichiwa"),
        ("学校へ行く", "gakkou e iku"),
        ("本を読む", "hon wo yomu"),
    ] {
        let lines = romanized_lines(&format!("[00:01.00]{text}"));

        assert_eq!(lines[0].romanization.as_deref(), Some(reading), "{text}");
    }
}

/// A small tsu is heard as the consonant of the mora after it even when the
/// analyzer ends its word there, so `がっ` and `こう` together read "gakkou".
#[test]
fn reads_a_double_consonant_across_the_words_of_a_line() {
    let mut lines = timed_lines_from_raw("[1000,4000]がっこう", &[]).unwrap();
    segment_lines_into_words(&mut lines);

    generate_local_romanization(&mut lines);

    assert_eq!(lines[0].romanization.as_deref(), Some("gakkou"));
    assert_eq!(
        syllable_readings(&lines[0]),
        vec![("が", "ga"), ("っ", "k"), ("こ", "ko"), ("う", "u")]
    );
}

/// Kana the dictionary does not list are read as they are written, an ん the next
/// word starts with a vowel after is written with an apostrophe, and a word that
/// starts with a y stays a word of its own.
#[test]
fn reads_kana_the_dictionary_does_not_list() {
    let lines = romanized_lines("[00:01.00]ラララ\n[00:03.00]しんいち\n[00:05.00]少年よ");

    assert_eq!(lines[0].romanization.as_deref(), Some("rarara"));
    assert_eq!(lines[1].romanization.as_deref(), Some("shin'ichi"));
    assert_eq!(lines[2].romanization.as_deref(), Some("shounen yo"));
}

/// The kana of a word that the dictionary reads as a unit is handed to the
/// characters that spell it, which is what a view renders as furigana.
#[test]
fn aligns_japanese_furigana_with_the_kana_of_the_word() {
    let mut lines = timed_lines_from_raw("[1000,4000]食べる", &[]).unwrap();
    segment_lines_into_words(&mut lines);

    generate_local_romanization(&mut lines);

    assert_eq!(lines[0].romanization.as_deref(), Some("taberu"));
    assert_eq!(
        syllable_readings(&lines[0]),
        vec![("食", "ta"), ("べ", "be"), ("る", "ru")]
    );
}

/// Final jamo are read as the sounds they stand for: a cluster keeps one of its
/// two jamo, and the seven sounds a syllable can end with decide the rest.
#[test]
fn reads_korean_final_jamo_as_they_are_pronounced() {
    for (text, reading) in [
        ("닭", "dak"),
        ("값", "gap"),
        ("읽다", "ikda"),
        ("없어", "eopseo"),
        ("꽃", "kkot"),
        ("삶", "sam"),
        ("여덟", "yeodeol"),
    ] {
        let lines = romanized_lines(&format!("[00:01.00]{text}"));

        assert_eq!(lines[0].romanization.as_deref(), Some(reading), "{text}");
    }
}

/// A syllable is read with its neighbours, and the readings are handed out one
/// syllable at a time in the order the syllables are written.
#[test]
fn reads_korean_sound_changes_between_syllables() {
    for (text, reading) in [
        ("신라", "silla"),
        ("종로", "jongno"),
        ("백마", "baengma"),
        ("한국말", "hangungmal"),
        ("학여울", "hangnyeoul"),
        ("십육", "simnyuk"),
        ("알약", "allyak"),
        ("같이", "gachi"),
        ("좋은", "joeun"),
        ("넓다", "neolda"),
        ("감사합니다", "gamsahamnida"),
    ] {
        let lines = romanized_lines(&format!("[00:01.00]{text}"));

        assert_eq!(lines[0].romanization.as_deref(), Some(reading), "{text}");
    }

    let mut lines = timed_lines_from_raw("[1000,4000]신라", &[]).unwrap();
    segment_lines_into_words(&mut lines);

    generate_local_romanization(&mut lines);

    assert_eq!(
        syllable_readings(&lines[0]),
        vec![("신", "sil"), ("라", "la")]
    );
}

/// A character is read the way its word is read, so a polyphone is not read with
/// the reading it has on its own.
#[test]
fn reads_chinese_polyphones_from_the_word_they_are_in() {
    for (text, reading) in [
        ("音乐", "yīn yuè"),
        ("银行", "yín háng"),
        ("长大", "zhǎng dà"),
        ("还是", "hái shi"),
        ("还是喜欢你", "hái shi xǐ huan nǐ"),
    ] {
        let lines = romanized_lines(&format!("[00:01.00]{text}"));

        assert_eq!(lines[0].romanization.as_deref(), Some(reading), "{text}");
    }
}

/// Provider word timing keeps its syllables, and one spanning several morae
/// receives the readings covering it, joined the way the source is written.
#[test]
fn joins_japanese_readings_across_a_syllable() {
    let mut lines =
        timed_lines_from_raw("[1000,4000]こんにちは(0,1000)世界(1000,1000)", &[]).unwrap();

    generate_local_romanization(&mut lines);

    assert_eq!(
        syllable_readings(&lines[0]),
        vec![("こんにちは", "konnichiwa"), ("世界", "sekai")]
    );
}

/// Cantonese readings are annotated per word, so a word is handed out one
/// character at a time.
#[test]
fn attaches_cantonese_readings_to_timed_syllables() {
    let mut lines = timed_lines_from_raw("[1000,4000]我喜歡你", &[]).unwrap();
    segment_lines_into_words(&mut lines);

    generate_local_romanization_with_mode(&mut lines, ChineseRomanizationMode::CantoneseJyutping);

    assert_eq!(
        lines[0].romanization.as_deref(),
        Some("ngo5 hei2 fun1 nei5")
    );
    assert_eq!(
        syllable_readings(&lines[0]),
        vec![
            ("我", "ngo5"),
            ("喜", "hei2"),
            ("歡", "fun1"),
            ("你", "nei5")
        ]
    );
}

#[test]
fn leaves_latin_syllable_readings_empty() {
    let mut lines = timed_lines_from_raw("[1000,2000]안녕(0,500) Hello(500,500)", &[]).unwrap();

    generate_local_romanization(&mut lines);

    assert_eq!(lines[0].romanization.as_deref(), Some("annyeong"));
    assert_eq!(lines[0].syllables[0].romanization, "annyeong");
    assert!(lines[0].syllables[1].romanization.is_empty());
}

#[test]
fn stops_attaching_readings_at_the_first_unmatched_syllable() {
    let mut lines = vec![line(0, Some(1_000), "안녕하세요")];
    lines[0].syllables = vec![syllable(0, 500, "다른"), syllable(500, 1_000, "안녕하세요")];

    generate_local_romanization(&mut lines);

    assert!(
        lines[0]
            .syllables
            .iter()
            .all(|syllable| syllable.romanization.is_empty()),
        "a stale reading must never be attached to the wrong syllable"
    );
}

#[test]
fn clears_stale_syllable_readings_when_no_romanization_is_generated() {
    let mut lines = vec![line(0, Some(1_000), "Hello world")];
    lines[0].syllables = vec![syllable(0, 500, "Hello")];
    lines[0].syllables[0].romanization = "stale".to_string();

    generate_local_romanization(&mut lines);

    assert_eq!(lines[0].romanization, None);
    assert!(lines[0].syllables[0].romanization.is_empty());
}

#[test]
fn joins_readings_inside_one_syllable_with_a_space_when_the_source_has_one() {
    let mut lines = timed_lines_from_raw("[1000,2000]안녕 세계(0,1000)", &[]).unwrap();

    generate_local_romanization(&mut lines);

    assert_eq!(lines[0].romanization.as_deref(), Some("annyeong segye"));
    assert_eq!(lines[0].syllables[0].romanization, "annyeong segye");
}

#[test]
fn replaces_romanization_supplied_by_the_lyrics_source() {
    let mut lines = vec![line(1_000, None, "こんにちは")];
    lines[0].romanization = Some("source romanization".to_string());

    generate_local_romanization(&mut lines);

    assert_eq!(lines[0].romanization.as_deref(), Some("konnichiwa"));
}

#[test]
fn distinguishes_chinese_lines_in_mixed_japanese_lyrics() {
    let lines = romanized_lines("[00:01.00]こんにちは\n[00:03.00]我喜欢你");

    assert_eq!(lines[0].romanization.as_deref(), Some("konnichiwa"));
    assert_eq!(lines[1].romanization.as_deref(), Some("wǒ xǐ huan nǐ"));
}

#[test]
fn recognizes_japanese_lyrics_written_only_with_kanji() {
    let lines = romanized_lines("[00:01.00]愛\n[00:03.00]音楽");

    assert_eq!(lines[0].romanization.as_deref(), Some("ai"));
    assert_eq!(lines[1].romanization.as_deref(), Some("ongaku"));
}

#[test]
fn parsing_does_not_generate_romanization_until_requested() {
    let lines = timed_lines_from_raw("[00:01.00]¿Cómo estás?", &[]).unwrap();

    assert_eq!(lines[0].romanization, None);
    assert!(lines[0].romanization_segments.is_empty());
}

#[test]
fn combines_translation_lrc_into_timed_lines() {
    let raw = combine_lyrics_with_translation(
        "[00:01.00]Hello\n[00:03.00]World",
        Some("[00:01.00]你好\n[00:03.00]世界"),
    );
    let lines = timed_lines_from_raw(&raw, &[]).unwrap();

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
    let lines = timed_lines_from_raw(&raw, &[]).unwrap();

    assert_eq!(lines[0].translation, None);
    assert_eq!(lines[1].translation.as_deref(), Some("世界"));
}

#[test]
fn combines_translation_qrc_into_timed_lines() {
    let raw = combine_lyrics_with_translation(
        "[1000,2000]Hel(1000,500)lo(1500,500)\n[3000,2000]World",
        Some("[1000,2000]你好\n[3000,2000]世界"),
    );
    let lines = timed_lines_from_raw(&raw, &[]).unwrap();

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
                romanization: String::new(),
                furigana: String::new(),
            },
            TimedSyllable {
                start_ms: 1_500,
                end_ms: 2_000,
                text: "lo".to_string(),
                romanization: String::new(),
                furigana: String::new(),
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
    let lines = timed_lines_from_raw(raw, &[]).unwrap();

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
    let lines = timed_lines_from_raw(&raw, &[]).unwrap();

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "Hello");
    assert_eq!(lines[0].translation.as_deref(), Some("你好"));
}

#[test]
fn filters_chinese_standalone_credit_lines() {
    let raw = "\
[0,314]BIZNESS(0,157) - XLOV(158,157)
[315,314]词：(315,157)SCORE(473,157)
[630,158]曲：(630,158)QSTNMRKS(788,0)
[789,1000]Dance (789,300)dance(1089,700)";
    let lines = timed_lines_from_raw(raw, &[]).unwrap();

    assert_eq!(
        lines.len(),
        1,
        "词：and 曲：credit lines should be filtered"
    );
    assert_eq!(lines[0].start_ms, 789);
    assert_eq!(lines[0].text, "Dance dance");
}

#[test]
fn filters_english_composer_and_arranged_by_lines() {
    let raw = "\
[0,1060]Song(0,400) - Artist(400,660)
[1060,1060]Composer：(1060,500)Zacharie Raymond(1560,500)
[2120,1060]Arranged (2120,300)by：(2420,500)Charlie Puth(2920,500)
[3180,1000]Hello (3180,400)World(3580,600)";
    let lines = timed_lines_from_raw(raw, &[]).unwrap();

    assert_eq!(
        lines.len(),
        1,
        "Composer and Arranged by lines should be filtered"
    );
    assert_eq!(lines[0].text, "Hello World");
}

#[test]
fn filters_extended_live_performance_header_lines() {
    let raw = "\
[0,1142]珠(0,103)玉(104,103) ((208,103)Live(312,103)|(416,103)典(519,103)藏(623,103)) - (727,103)单(831,103)依(935,103)纯(1039,103)
[1143,415]作(1143,103)词：(1247,103)李(1351,103)聪(1455,103)
[10909,1246]和(10909,103)音(11013,103)：(11117,103)刘(11221,103)芳(11325,103)/(11429,103)胡(11533,103)维(11637,103)纳(11741,103)/(11844,103)叶(11948,103)俊(12052,103)
[12156,726]PGM：(12156,103)杨(12260,103)阳(12364,103)/(12468,103)施(12572,103)森(12676,103)铭(12780,103)
[13819,726]管(13819,103)弦(13922,103)配(14026,103)器(14130,103)：(14234,103)李(14338,103)超(14442,103)
[14546,1038]竹(14546,103)笛(14650,103)：(14754,103)鱼(14858,103)椒(14961,103)盐(15065,103)/(15169,103)王(15273,103)思(15377,103)远(15481,103)
[15585,622]长(15585,103)笛(15689,103)：(15793,103)吴(15897,103)梦(16000,103)圆(16104,103)
[16208,622]柳(16208,103)琴(16312,103)：(16416,103)李(16520,103)雨(16624,103)涵(16728,103)
[16832,623]打(16832,103)击(16936,103)乐(17039,103)：(17143,103)郑(17247,103)瑀(17351,103)
[17456,3563]滚(17456,186)烫(17642,216)的(17858,284)伤(18142,372)口(18514,233) (18747,233)会(18980,215)冷(19195,291)成(19486,348)月(19834,336)牙(20170,849)";
    let lines = timed_lines_from_raw(raw, &[]).unwrap();

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].start_ms, 17_456);
    assert_eq!(lines[0].text, "滚烫的伤口 会冷成月牙");
}

#[test]
fn splits_a_bracketed_tail_into_a_background_vocal() {
    let raw = "[1000,2000]Hold (1000,500)on (oh(1500,1000)yeah)(2500,500)";
    let lines = timed_lines_from_raw(raw, &[]).unwrap();

    assert_eq!(lines[0].text, "Hold on");
    assert_eq!(
        lines[0]
            .background
            .as_ref()
            .map(|background| background.text.as_str()),
        Some("ohyeah")
    );
    assert_eq!(
        lines[0]
            .syllables
            .iter()
            .map(|syllable| syllable.text.as_str())
            .collect::<String>()
            .trim(),
        lines[0].text
    );
}

#[test]
fn folds_a_bracketed_echo_into_the_line_it_answers() {
    let raw = "\
[1000,2000]Know (1000,1000)the way(2000,1000)
[3000,1000]((3000,100)My (3100,400)way)(3500,500)";
    let lines = timed_lines_from_raw(raw, &[]).unwrap();

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "Know the way");
    assert_eq!(
        lines[0]
            .background
            .as_ref()
            .map(|background| background.text.as_str()),
        Some("My way")
    );
}

#[test]
fn a_folded_echo_keeps_its_own_translation_beside_its_parent() {
    let raw = combine_lyrics_with_translation(
        "\
[1000,2000]Know (1000,1000)the way(2000,1000)
[3000,1000]((3000,100)My (3100,400)way)(3500,500)",
        Some("[00:01.00]要知道方法\n[00:03.00]（从你身边离开的方法）"),
    );
    let lines = timed_lines_from_raw(&raw, &[]).unwrap();

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].translation.as_deref(), Some("要知道方法"));
    let background = lines[0].background.as_ref().expect("the echo is folded in");
    assert_eq!(background.text, "My way");
    assert_eq!(
        (background.start_ms, background.end_ms),
        (3_100, Some(4_000)),
        "the echo keeps the timing its own words were sung with"
    );
    assert_eq!(
        background
            .syllables
            .iter()
            .map(|syllable| (syllable.text.as_str(), syllable.start_ms, syllable.end_ms))
            .collect::<Vec<_>>(),
        vec![("My ", 3_100, 3_500), ("way", 3_500, 4_000)],
        "every word of the echo keeps its own timing, so a listener fills them as it hears them"
    );
    assert_eq!(
        background.translation.as_deref(),
        Some("从你身边离开的方法"),
        "the bracket that marked the phrase goes with the phrase"
    );
}

#[test]
fn a_bracketed_line_elsewhere_in_the_song_stays_its_own_line() {
    let raw = "\
[1000,1000]Know (1000,500)the way(1500,500)
[9000,1000]((9000,100)Instrumental)(9100,900)";
    let lines = timed_lines_from_raw(raw, &[]).unwrap();

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1].text, "(Instrumental)");
    assert!(lines[0].background.is_none());
}

#[test]
fn a_line_timed_source_keeps_a_bracketed_tail_as_sung_text() {
    let raw = "[00:01.00]Know the way (My way)";
    let lines = timed_lines_from_raw(raw, &[]).unwrap();

    assert_eq!(lines[0].text, "Know the way (My way)");
    assert!(lines[0].background.is_none());
}

#[test]
fn reads_speaker_labels_against_the_providers_artists() {
    let artists = vec!["Ariana Grande".to_string(), "Iggy Azalea".to_string()];
    let raw = "\
[0,500]Iggy Azalea/Ariana Grande：(0,500)
[1000,1000]Uh-huh (1000,500)it's Iggy(1500,500)
[2000,500]Big Sean/Ariana Grande：(2000,500)
[3000,1000]One (3000,500)less problem(3500,500)
[12000,1000]Love: it hurts(12000,1000)";
    let lines = timed_lines_from_raw(raw, &artists).unwrap();

    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].text, "Uh-huh it's Iggy");
    assert_eq!(lines[0].voice, Voice::Secondary);
    assert_eq!(lines[1].text, "One less problem");
    assert_eq!(lines[1].voice, Voice::Primary);
    // A sung line containing a colon names no artist, so it keeps its text.
    assert_eq!(lines[2].text, "Love: it hurts");
    assert_eq!(lines[2].voice, Voice::Primary);
}

#[test]
fn a_speaker_label_without_provider_artists_is_not_read() {
    let raw = "[0,500]The Weeknd：(0,500)\n[1000,1000]I can't feel my face(1000,1000)";
    let lines = timed_lines_from_raw(raw, &[]).unwrap();

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "I can't feel my face");
    assert_eq!(lines[0].voice, Voice::Primary);
}
