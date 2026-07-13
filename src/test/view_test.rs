use super::*;

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

#[test]
fn viewport_reserves_separate_romanization_and_translation_rows() {
    assert_eq!(viewport_height(24, 12, 13, true), 74);
}

#[test]
fn viewport_does_not_reserve_romanization_when_hidden() {
    assert_eq!(viewport_height(24, 12, 13, false), 56);
}
