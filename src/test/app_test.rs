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
fn parses_sidebar_settings_navigation() {
    let action =
        serde_json::from_str::<UiAction>(r#"{"type":"open-settings-page","page":"display"}"#)
            .unwrap();

    assert!(matches!(
        action,
        UiAction::OpenSettingsPage {
            page: view::web_lyrics::ControlPage::Display
        }
    ));
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

#[test]
fn switching_run_mode_changes_the_output() {
    let current = AppConfig::default();
    let mut next = current.clone();
    next.general.mode = AppMode::Amll;

    assert_eq!(output_change(&current, &next), OutputChange::Mode);
    assert_eq!(output_change(&next, &current), OutputChange::Mode);
    assert_eq!(output_change(&current, &current), OutputChange::None);
}

#[test]
fn a_new_amll_address_reconnects_only_the_sender() {
    let mut floating = AppConfig::default();
    floating.general.mode = AppMode::Floating;
    let mut moved = floating.clone();
    moved.amll.address = "127.0.0.1:12000".to_string();
    assert_eq!(output_change(&floating, &moved), OutputChange::None);

    let mut sender = floating.clone();
    sender.general.mode = AppMode::Amll;
    let mut moved_sender = sender.clone();
    moved_sender.amll.address = "127.0.0.1:12000".to_string();
    assert_eq!(
        output_change(&sender, &moved_sender),
        OutputChange::AmllAddress
    );
}
