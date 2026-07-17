use super::*;

#[test]
fn language_indices_follow_the_public_language_order() {
    for (index, language) in Language::ALL.into_iter().enumerate() {
        assert_eq!(language_index(language), index as u32);
    }
}

#[test]
fn chinese_romanization_indices_follow_the_public_mode_order() {
    for (index, mode) in ChineseRomanizationMode::ALL.into_iter().enumerate() {
        assert_eq!(chinese_romanization_index(mode), index as u32);
    }
}

#[test]
fn apple_music_style_disables_the_unplayed_color_setting() {
    assert!(unplayed_color_sensitive(false));
    assert!(!unplayed_color_sensitive(true));
}
