use super::*;

#[test]
fn accepts_settings_entry_point() {
    let cli = Cli::try_parse_from(["floatlyrics", "--settings"]).unwrap();

    assert!(cli.settings);
}

#[test]
fn accepts_manual_lyrics_entry_point() {
    let cli = Cli::try_parse_from(["floatlyrics", "--select-lyrics"]).unwrap();

    assert!(cli.select_lyrics);
}
