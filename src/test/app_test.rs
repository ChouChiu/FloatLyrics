use super::*;

#[test]
fn recognizes_settings_command_without_matching_substrings() {
    assert!(command_requests_settings(&[
        "floatlyrics".into(),
        "--settings".into(),
    ]));
    assert!(!command_requests_settings(&[
        "floatlyrics".into(),
        "--settings-file".into(),
    ]));
}

#[test]
fn recognizes_manual_lyrics_command() {
    assert!(command_requests_manual_search(&[
        "floatlyrics".into(),
        "--select-lyrics".into(),
    ]));
}
