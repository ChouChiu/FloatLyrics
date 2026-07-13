use super::*;
use floatlyrics_lyrics::lyrics::ChineseRomanizationMode;

#[test]
fn recognizes_settings_command_without_matching_substrings() {
    let commands =
        requested_commands(&["floatlyrics".into(), "--settings".into()]).collect::<Vec<_>>();
    assert_eq!(commands, [AppCommand::OpenSettings]);

    let commands =
        requested_commands(&["floatlyrics".into(), "--settings-file".into()]).collect::<Vec<_>>();
    assert!(commands.is_empty());
}

#[test]
fn recognizes_manual_lyrics_command() {
    let commands =
        requested_commands(&["floatlyrics".into(), "--select-lyrics".into()]).collect::<Vec<_>>();
    assert_eq!(commands, [AppCommand::OpenManualSearch]);
}

#[test]
fn recognizes_multiple_commands_in_argument_order() {
    let commands = requested_commands(&[
        "floatlyrics".into(),
        "--select-lyrics".into(),
        "--settings".into(),
    ])
    .collect::<Vec<_>>();

    assert_eq!(
        commands,
        [AppCommand::OpenManualSearch, AppCommand::OpenSettings]
    );
}

#[test]
fn enabling_romanization_reloads_current_lyrics() {
    let current = AppConfig::default();
    let mut next = current.clone();
    next.lyrics.show_romanization = true;

    assert!(should_reload_lyrics(&current, &next));
    assert!(!should_reload_lyrics(&next, &current));
}

#[test]
fn changing_enabled_chinese_romanization_reloads_current_lyrics() {
    let mut current = AppConfig::default();
    current.lyrics.show_romanization = true;
    let mut next = current.clone();
    next.lyrics.chinese_romanization = ChineseRomanizationMode::CantoneseJyutping;

    assert!(should_reload_lyrics(&current, &next));

    current.lyrics.show_romanization = false;
    next.lyrics.show_romanization = false;
    assert!(!should_reload_lyrics(&current, &next));
}
