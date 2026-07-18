use super::*;

fn line(text: &str) -> TimedLine {
    TimedLine {
        start_ms: 1_000,
        end_ms: None,
        text: text.to_string(),
        syllables: Vec::new(),
        translation: None,
        romanization: None,
        romanization_segments: Vec::new(),
        background: None,
    }
}

#[test]
fn applies_romanization_only_to_the_matching_lyrics_document() {
    let mut state = LyricsDisplayState {
        track_fingerprint: Some("track".to_string()),
        lines: vec![line("こんにちは")],
        status_message: None,
    };
    let mut generated = line("こんにちは");
    generated.romanization = Some("konnichiha".to_string());

    assert!(apply_romanization_event(
        RomanizationEvent {
            track_fingerprint: "track".to_string(),
            chinese_mode: ChineseRomanizationMode::Auto,
            lines: vec![generated],
        },
        &mut state,
        ChineseRomanizationMode::Auto,
    ));

    assert_eq!(state.lines[0].romanization.as_deref(), Some("konnichiha"));

    state.lines = vec![line("新しい歌詞")];
    assert!(!apply_romanization_event(
        RomanizationEvent {
            track_fingerprint: "track".to_string(),
            chinese_mode: ChineseRomanizationMode::Auto,
            lines: vec![line("こんにちは")],
        },
        &mut state,
        ChineseRomanizationMode::Auto,
    ));

    assert_eq!(state.lines[0].text, "新しい歌詞");
}

#[test]
fn ignores_romanization_generated_for_an_obsolete_chinese_mode() {
    let mut state = LyricsDisplayState {
        track_fingerprint: Some("track".to_string()),
        lines: vec![line("喜欢你")],
        status_message: None,
    };
    let mut generated = line("喜欢你");
    generated.romanization = Some("xǐ huān nǐ".to_string());

    assert!(!apply_romanization_event(
        RomanizationEvent {
            track_fingerprint: "track".to_string(),
            chinese_mode: ChineseRomanizationMode::MandarinPinyin,
            lines: vec![generated],
        },
        &mut state,
        ChineseRomanizationMode::CantoneseJyutping,
    ));

    assert_eq!(state.lines[0].romanization, None);
}

#[test]
fn ignores_romanization_when_the_translation_document_has_changed() {
    let mut current = line("同一行");
    current.translation = Some("new translation".to_string());
    let mut generated = line("同一行");
    generated.translation = Some("old translation".to_string());
    generated.romanization = Some("tóng yī háng".to_string());
    let mut state = LyricsDisplayState {
        track_fingerprint: Some("track".to_string()),
        lines: vec![current],
        status_message: None,
    };

    assert!(!apply_romanization_event(
        RomanizationEvent {
            track_fingerprint: "track".to_string(),
            chinese_mode: ChineseRomanizationMode::Auto,
            lines: vec![generated],
        },
        &mut state,
        ChineseRomanizationMode::Auto,
    ));
    assert_eq!(
        state.lines[0].translation.as_deref(),
        Some("new translation")
    );
    assert_eq!(state.lines[0].romanization, None);
}
