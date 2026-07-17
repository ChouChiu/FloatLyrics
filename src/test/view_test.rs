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
fn apple_music_lyrics_can_expand_to_the_available_width() {
    assert_eq!(maximum_lyrics_width(1_400, false), MAX_EXPANDED_PANEL_WIDTH);
    assert_eq!(maximum_lyrics_width(1_400, true), 1_400);
}

#[test]
fn apple_music_width_reserves_both_amll_line_insets() {
    assert_eq!(
        lyrics_horizontal_padding(false, 56),
        TEXT_HORIZONTAL_PADDING
    );
    assert_eq!(lyrics_horizontal_padding(true, 12), 40);
    assert_eq!(lyrics_horizontal_padding(true, 24), 48);
    assert_eq!(lyrics_horizontal_padding(true, 56), 112);
}

#[test]
fn apple_music_line_changes_animate_to_the_current_line_width() {
    assert_eq!(lyrics_resize_animation(true, true), Some(true));
    assert_eq!(lyrics_resize_animation(false, true), Some(false));
    assert_eq!(lyrics_resize_animation(true, false), None);
}

#[test]
fn panel_width_animation_eases_between_both_endpoints() {
    assert_eq!(animated_panel_width(520, 900, 0), 520);

    let midpoint = animated_panel_width(520, 900, PANEL_RESIZE_DURATION_US / 2);
    assert!((711..900).contains(&midpoint));

    let shrinking_midpoint = animated_panel_width(900, 520, PANEL_RESIZE_DURATION_US / 2);
    assert!((711..900).contains(&shrinking_midpoint));

    assert_eq!(
        animated_panel_width(520, 900, PANEL_RESIZE_DURATION_US),
        900
    );
    assert_eq!(
        animated_panel_width(900, 520, PANEL_RESIZE_DURATION_US),
        520
    );
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
    assert_eq!(viewport_height(24, 12, 13, true, false), 74);
}

#[test]
fn viewport_does_not_reserve_romanization_when_hidden() {
    assert_eq!(viewport_height(24, 12, 13, false, false), 56);
}

#[test]
fn apple_music_style_reserves_only_the_current_lyric_group() {
    assert_eq!(viewport_height(24, 12, 13, false, true), 75);
    assert_eq!(viewport_height(24, 12, 13, true, true), 100);
}

#[test]
fn apple_music_single_line_height_uses_configured_secondary_fonts() {
    assert_eq!(viewport_height(24, 12, 8, false, true), 68);
    assert_eq!(viewport_height(24, 12, 36, false, true), 110);
    assert_eq!(viewport_height(56, 36, 36, true, true), 254);
}
