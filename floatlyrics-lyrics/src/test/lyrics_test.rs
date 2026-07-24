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
    }
}

fn romanized_lines(raw: &str) -> Vec<TimedLine> {
    let mut lines = timed_lines_from_raw(raw).unwrap();
    generate_local_romanization(&mut lines);
    lines
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
fn generates_japanese_romanization_locally() {
    let lines = romanized_lines("[00:01.00]こんにちは世界\n[00:03.00]音楽");

    assert_eq!(lines[0].romanization.as_deref(), Some("konnichiha sekai"));
    assert_eq!(lines[1].romanization.as_deref(), Some("ongaku"));
}

#[test]
fn generates_chinese_pinyin_without_treating_it_as_japanese() {
    let lines = romanized_lines("[00:01.00]你好世界\n[00:03.00]我喜欢你");

    assert_eq!(lines[0].romanization.as_deref(), Some("nǐ hǎo shì jiè"));
    assert_eq!(lines[1].romanization.as_deref(), Some("wǒ xǐ huān nǐ"));
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
fn replaces_romanization_supplied_by_the_lyrics_source() {
    let mut lines = vec![line(1_000, None, "こんにちは")];
    lines[0].romanization = Some("source romanization".to_string());

    generate_local_romanization(&mut lines);

    assert_eq!(lines[0].romanization.as_deref(), Some("konnichiha"));
}

#[test]
fn distinguishes_chinese_lines_in_mixed_japanese_lyrics() {
    let lines = romanized_lines("[00:01.00]こんにちは\n[00:03.00]我喜欢你");

    assert_eq!(lines[0].romanization.as_deref(), Some("konnichiha"));
    assert_eq!(lines[1].romanization.as_deref(), Some("wǒ xǐ huān nǐ"));
}

#[test]
fn recognizes_japanese_lyrics_written_only_with_kanji() {
    let lines = romanized_lines("[00:01.00]愛\n[00:03.00]音楽");

    assert_eq!(lines[0].romanization.as_deref(), Some("ai"));
    assert_eq!(lines[1].romanization.as_deref(), Some("ongaku"));
}

#[test]
fn parsing_does_not_generate_romanization_until_requested() {
    let lines = timed_lines_from_raw("[00:01.00]¿Cómo estás?").unwrap();

    assert_eq!(lines[0].romanization, None);
    assert!(lines[0].romanization_segments.is_empty());
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
fn filters_chinese_standalone_credit_lines() {
    // 词：XXX and 曲：XXX should be filtered (standalone single-char credits)
    let raw = "\
[0,314]BIZNESS(0,157) - XLOV(158,157)
[315,314]词：(315,157)SCORE(473,157)
[630,158]曲：(630,158)QSTNMRKS(788,0)
[789,1000]Dance (789,300)dance(1089,700)";
    let lines = timed_lines_from_raw(raw).unwrap();

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
    // Composer：XXX and Arranged by：XXX should be filtered
    let raw = "\
[0,1060]Song(0,400) - Artist(400,660)
[1060,1060]Composer：(1060,500)Zacharie Raymond(1560,500)
[2120,1060]Arranged (2120,300)by：(2420,500)Charlie Puth(2920,500)
[3180,1000]Hello (3180,400)World(3580,600)";
    let lines = timed_lines_from_raw(raw).unwrap();

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
    let lines = timed_lines_from_raw(raw).unwrap();

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].start_ms, 17_456);
    assert_eq!(lines[0].text, "滚烫的伤口 会冷成月牙");
}
