use super::*;
use crate::shared::config::ChineseRomanizationMode;

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

#[test]
fn disabling_position_memory_clears_and_rejects_saved_positions() {
    let mut config = AppConfig::default();
    config.window.position = Some(crate::shared::config::WindowPosition {
        horizontal: 0.25,
        vertical: 0.75,
    });

    ConfigChange::RememberPosition(false).apply(&mut config);
    ConfigChange::WindowPosition(crate::shared::config::WindowPosition {
        horizontal: 0.5,
        vertical: 0.5,
    })
    .apply(&mut config);

    assert!(!config.window.remember_position);
    assert_eq!(config.window.position, None);
}

#[test]
fn config_change_updates_only_its_owned_preference() {
    let mut config = AppConfig::default();
    let original = config.clone();

    ConfigChange::Offset(-375).apply(&mut config);

    assert_eq!(config.lyrics.offset_ms, -375);
    assert_eq!(config.general, original.general);
    assert_eq!(config.window, original.window);
    assert_eq!(config.spotify, original.spotify);
}
