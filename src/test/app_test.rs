use super::*;

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
