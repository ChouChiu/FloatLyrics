use super::*;

#[test]
fn lyric_slot_only_switches_when_key_changes() {
    let mut state = LyricsTransitionState::default();

    assert_eq!(select_lyric_slot(&mut state, "line:0"), (0, false));
    assert_eq!(select_lyric_slot(&mut state, "line:0"), (0, false));
    assert_eq!(select_lyric_slot(&mut state, "line:1"), (1, true));
    assert_eq!(select_lyric_slot(&mut state, "line:2"), (0, true));
}

#[test]
fn panel_width_is_kept_in_compact_bounds() {
    assert_eq!(compact_panel_width(200), MIN_PANEL_WIDTH);
    assert_eq!(compact_panel_width(520), 520);
    assert_eq!(compact_panel_width(960), MAX_PANEL_WIDTH);
}

#[test]
fn long_lyrics_expand_until_available_width() {
    assert_eq!(expanded_panel_width(520, 400, 900), 520);
    assert_eq!(expanded_panel_width(520, 740, 900), 740);
    assert_eq!(expanded_panel_width(520, 1_100, 900), 900);
}

#[test]
fn bottom_panel_reservation_is_never_undercut() {
    let mut config = AppConfig::default();
    config.window.margin = 12;
    config.window.bottom_panel_height = 36;
    assert_eq!(effective_bottom_margin(&config), 36);

    config.window.margin = 96;
    assert_eq!(effective_bottom_margin(&config), 96);
}
