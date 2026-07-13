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
    let lyrics_state = Rc::new(RefCell::new(LyricsDisplayState {
        track_fingerprint: Some("track".to_string()),
        lines: vec![line("こんにちは")],
        status_message: None,
    }));
    let mut generated = line("こんにちは");
    generated.romanization = Some("konnichiha".to_string());

    apply_romanization_event(
        RomanizationEvent {
            track_fingerprint: "track".to_string(),
            chinese_mode: ChineseRomanizationMode::Auto,
            lines: vec![generated],
        },
        &lyrics_state,
        ChineseRomanizationMode::Auto,
    );

    assert_eq!(
        lyrics_state.borrow().lines[0].romanization.as_deref(),
        Some("konnichiha")
    );

    lyrics_state.borrow_mut().lines = vec![line("新しい歌詞")];
    apply_romanization_event(
        RomanizationEvent {
            track_fingerprint: "track".to_string(),
            chinese_mode: ChineseRomanizationMode::Auto,
            lines: vec![line("こんにちは")],
        },
        &lyrics_state,
        ChineseRomanizationMode::Auto,
    );

    assert_eq!(lyrics_state.borrow().lines[0].text, "新しい歌詞");
}

#[test]
fn ignores_romanization_generated_for_an_obsolete_chinese_mode() {
    let lyrics_state = Rc::new(RefCell::new(LyricsDisplayState {
        track_fingerprint: Some("track".to_string()),
        lines: vec![line("喜欢你")],
        status_message: None,
    }));
    let mut generated = line("喜欢你");
    generated.romanization = Some("xǐ huān nǐ".to_string());

    apply_romanization_event(
        RomanizationEvent {
            track_fingerprint: "track".to_string(),
            chinese_mode: ChineseRomanizationMode::MandarinPinyin,
            lines: vec![generated],
        },
        &lyrics_state,
        ChineseRomanizationMode::CantoneseJyutping,
    );

    assert_eq!(lyrics_state.borrow().lines[0].romanization, None);
}

#[test]
fn background_fetch_failure_preserves_loaded_lyrics() {
    let mut state = LyricsDisplayState {
        track_fingerprint: Some("track".to_string()),
        lines: vec![line("manual lyrics")],
        status_message: None,
    };

    assert!(!apply_lyrics_fetch_failure(
        &mut state,
        "track".to_string(),
        LyricsFetchFailure::NotFound,
    ));
    assert_eq!(state.lines[0].text, "manual lyrics");
    assert_eq!(state.status_message, None);
}

#[test]
fn initial_fetch_failure_replaces_searching_status() {
    let mut state = LyricsDisplayState {
        track_fingerprint: Some("track".to_string()),
        status_message: Some(Message::Text(Text::SearchingLyrics)),
        ..LyricsDisplayState::default()
    };

    assert!(apply_lyrics_fetch_failure(
        &mut state,
        "track".to_string(),
        LyricsFetchFailure::NotFound,
    ));
    assert!(state.lines.is_empty());
    assert_eq!(
        state.status_message,
        Some(Message::Text(Text::NoLyricsFound))
    );
}

#[test]
fn manual_lyrics_without_translation_do_not_trigger_provider_refresh() {
    let cached = CachedLyrics {
        manually_selected: true,
        id: 1,
        provider: floatlyrics_lyrics::lyrics::LyricsProvider::QqMusic,
        provider_track_id: Some("manual".to_string()),
        title: "Song".to_string(),
        artists: vec!["Artist".to_string()],
        raw_lyrics: "[00:01.00]manual lyrics".to_string(),
    };
    let state = LyricsDisplayState {
        track_fingerprint: Some("track".to_string()),
        lines: vec![line("manual lyrics")],
        status_message: None,
    };

    assert!(!should_refresh_translation(
        &cached,
        &state,
        &AppConfig::default(),
    ));
}

#[test]
fn background_cache_write_failure_preserves_loaded_lyrics() {
    let mut state = LyricsDisplayState {
        track_fingerprint: Some("track".to_string()),
        lines: vec![line("manual lyrics")],
        status_message: None,
    };

    assert!(!apply_lyrics_cache_write_failure(
        &mut state,
        "track".to_string(),
        "database is locked".to_string(),
    ));
    assert_eq!(state.lines[0].text, "manual lyrics");
    assert_eq!(state.status_message, None);
}

#[test]
fn initial_cache_write_failure_replaces_searching_status() {
    let mut state = LyricsDisplayState {
        track_fingerprint: Some("track".to_string()),
        status_message: Some(Message::Text(Text::SearchingLyrics)),
        ..LyricsDisplayState::default()
    };

    assert!(apply_lyrics_cache_write_failure(
        &mut state,
        "track".to_string(),
        "disk is read-only".to_string(),
    ));
    assert_eq!(
        state.status_message,
        Some(Message::Detail(
            Text::LyricsCacheWriteError,
            "disk is read-only".to_string(),
        ))
    );
}
