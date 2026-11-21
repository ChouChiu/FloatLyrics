use super::*;

use floatlyrics_lyrics::lyrics::{TimedSyllable, Voice};

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
        voice: Voice::Primary,
    }
}

#[test]
fn applies_romanization_only_to_the_matching_lyrics_document() {
    let mut state = display_state(vec![line("こんにちは")]);
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
fn applies_readings_attached_to_syllables() {
    let mut current = line("안녕 세계");
    current.syllables = vec![syllable(0, 400, "안녕"), syllable(400, 800, " 세계")];
    let mut generated = current.clone();
    generated.romanization = Some("annyeong segye".to_string());
    generated.syllables[0].romanization = "annyeong".to_string();
    generated.syllables[1].romanization = "segye".to_string();
    let mut state = display_state(vec![current]);

    assert!(apply_romanization_event(
        RomanizationEvent {
            track_fingerprint: "track".to_string(),
            chinese_mode: ChineseRomanizationMode::Auto,
            lines: vec![generated],
        },
        &mut state,
        ChineseRomanizationMode::Auto,
    ));

    assert_eq!(
        state.lines[0].romanization.as_deref(),
        Some("annyeong segye")
    );
    assert_eq!(state.lines[0].syllables[0].romanization, "annyeong");
    assert_eq!(state.lines[0].syllables[1].romanization, "segye");
}

#[test]
fn ignores_readings_when_the_syllable_timing_changed() {
    let mut current = line("안녕");
    current.syllables = vec![syllable(0, 400, "안녕")];
    let mut generated = line("안녕");
    generated.romanization = Some("annyeong".to_string());
    generated.syllables = vec![syllable(0, 900, "안녕")];
    let mut state = display_state(vec![current]);

    assert!(!apply_romanization_event(
        RomanizationEvent {
            track_fingerprint: "track".to_string(),
            chinese_mode: ChineseRomanizationMode::Auto,
            lines: vec![generated],
        },
        &mut state,
        ChineseRomanizationMode::Auto,
    ));
    assert_eq!(state.lines[0].romanization, None);
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

fn display_state(lines: Vec<TimedLine>) -> LyricsDisplayState {
    LyricsDisplayState {
        track_fingerprint: Some("track".to_string()),
        lines,
        status_message: None,
        credited_artists: Vec::new(),
    }
}

#[test]
fn ignores_romanization_generated_for_an_obsolete_chinese_mode() {
    let mut state = display_state(vec![line("喜欢你")]);
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
    let mut state = display_state(vec![current]);

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
