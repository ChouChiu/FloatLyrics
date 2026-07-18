use super::*;
use std::fs;

#[test]
fn default_provider_order_matches_plan() {
    assert_eq!(
        AppConfig::default().lyrics.provider_order,
        vec![LyricsProvider::QqMusic, LyricsProvider::NetEase]
    );
}

#[test]
fn default_window_uses_compact_width() {
    assert_eq!(AppConfig::default().window.width, 350);
    assert!(AppConfig::default().window.remember_position);
    assert_eq!(AppConfig::default().window.position, None);
}

#[test]
fn default_font_order_uses_generic_sans() {
    assert_eq!(AppConfig::default().lyrics.font_order, ["Sans"]);
}

#[test]
fn apple_music_style_is_opt_in() {
    assert!(!AppConfig::default().lyrics.apple_music_style);
}

#[test]
fn romanization_has_distinct_default_presentation() {
    let lyrics = AppConfig::default().lyrics;

    assert_eq!(lyrics.chinese_romanization, ChineseRomanizationMode::Auto);
    assert_eq!(lyrics.romanization_font_size, 12);
    assert_eq!(lyrics.romanization_color, "#B8D8F0E6");
    assert_ne!(lyrics.romanization_color, lyrics.translation_color);
}

#[test]
fn parses_rgb_and_rgba_hex_colors() {
    assert_eq!(parse_hex_color("#0000FF"), (0.0, 0.0, 1.0, 1.0));
    assert_eq!(parse_hex_color("FF000080"), (1.0, 0.0, 0.0, 128.0 / 255.0));
}

#[test]
fn invalid_hex_color_falls_back_entirely_to_white() {
    for invalid in ["#GG0000", "#1234567", "##123456", "#12345"] {
        assert_eq!(parse_hex_color(invalid), (1.0, 1.0, 1.0, 1.0));
    }
}

#[test]
fn rejects_incomplete_config() {
    assert!(toml::from_str::<AppConfig>("[window]\nwidth = 500").is_err());
}

#[test]
fn rejects_unknown_config_fields() {
    let mut value = toml::Value::try_from(AppConfig::default()).unwrap();
    value
        .as_table_mut()
        .unwrap()
        .insert("obsolete".to_string(), toml::Table::new().into());

    assert!(toml::from_str::<AppConfig>(&value.to_string()).is_err());
}

#[test]
fn save_replaces_config_without_leaving_a_temporary_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let mut config = AppConfig::default();
    config.window.width = 520;

    config.save(&path).unwrap();

    assert_eq!(AppConfig::load_or_default(&path).unwrap(), config);
    let entries = fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(entries, vec!["config.toml"]);
}

#[test]
fn load_rejects_out_of_range_numeric_values() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let mut config = AppConfig::default();
    config.window.width = ConfigLimits::WINDOW_WIDTH_MAX + 1;
    fs::write(&path, toml::to_string(&config).unwrap()).unwrap();

    let error = format!("{:#}", AppConfig::load_or_default(&path).unwrap_err());

    assert!(error.contains("validating config file"));
    assert!(error.contains("config.toml"));
    assert!(error.contains("window.width"));
}

#[test]
fn invalid_save_does_not_replace_existing_config() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let original = AppConfig::default();
    original.save(&path).unwrap();
    let mut invalid = original.clone();
    invalid.window.opacity = ConfigLimits::OPACITY_MAX + 0.1;

    assert!(invalid.save(&path).is_err());
    assert_eq!(AppConfig::load_or_default(&path).unwrap(), original);
}

#[test]
fn save_rejects_invalid_position_font_order_and_colors() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let mut invalid_position = AppConfig::default();
    invalid_position.window.position = Some(WindowPosition {
        horizontal: -0.01,
        vertical: 0.5,
    });
    assert!(invalid_position.save(&path).is_err());

    let mut empty_fonts = AppConfig::default();
    empty_fonts.lyrics.font_order.clear();
    assert!(empty_fonts.save(&path).is_err());

    let mut blank_font = AppConfig::default();
    blank_font.lyrics.font_order = vec!["Sans".to_string(), "  ".to_string()];
    assert!(blank_font.save(&path).is_err());

    let mut invalid_color = AppConfig::default();
    invalid_color.lyrics.played_color = "#GG0000".to_string();
    assert!(invalid_color.save(&path).is_err());
    assert!(!path.exists());
}

#[test]
fn load_rejects_invalid_persisted_color() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let mut config = AppConfig::default();
    config.lyrics.translation_color = "not-a-color".to_string();
    fs::write(&path, toml::to_string(&config).unwrap()).unwrap();

    let error = format!("{:#}", AppConfig::load_or_default(&path).unwrap_err());

    assert!(error.contains("lyrics.translation_color"));
}

#[test]
fn validates_every_numeric_preference_at_its_boundary() {
    fn assert_invalid(mutate: impl FnOnce(&mut AppConfig)) {
        let mut config = AppConfig::default();
        mutate(&mut config);
        assert!(config.validate().is_err());
    }

    assert_invalid(|config| config.window.width = ConfigLimits::WINDOW_WIDTH_MIN - 1);
    assert_invalid(|config| config.window.width = ConfigLimits::WINDOW_WIDTH_MAX + 1);
    assert_invalid(|config| config.window.margin = ConfigLimits::WINDOW_MARGIN_MIN - 1);
    assert_invalid(|config| config.window.margin = ConfigLimits::WINDOW_MARGIN_MAX + 1);
    assert_invalid(|config| {
        config.window.bottom_panel_height = ConfigLimits::BOTTOM_PANEL_HEIGHT_MIN - 1;
    });
    assert_invalid(|config| {
        config.window.bottom_panel_height = ConfigLimits::BOTTOM_PANEL_HEIGHT_MAX + 1;
    });
    assert_invalid(|config| config.window.opacity = ConfigLimits::OPACITY_MIN - 0.01);
    assert_invalid(|config| config.window.opacity = ConfigLimits::OPACITY_MAX + 0.01);
    assert_invalid(|config| config.window.opacity = f64::NAN);
    assert_invalid(|config| config.lyrics.offset_ms = ConfigLimits::OFFSET_MS_MIN - 1);
    assert_invalid(|config| config.lyrics.offset_ms = ConfigLimits::OFFSET_MS_MAX + 1);
    assert_invalid(|config| {
        config.lyrics.lyric_font_size = ConfigLimits::LYRIC_FONT_SIZE_MIN - 1;
    });
    assert_invalid(|config| {
        config.lyrics.lyric_font_size = ConfigLimits::LYRIC_FONT_SIZE_MAX + 1;
    });
    assert_invalid(|config| {
        config.lyrics.translation_font_size = ConfigLimits::SECONDARY_FONT_SIZE_MIN - 1;
    });
    assert_invalid(|config| {
        config.lyrics.romanization_font_size = ConfigLimits::SECONDARY_FONT_SIZE_MAX + 1;
    });
}

#[test]
fn missing_color_fields_fall_back_to_defaults() {
    let old_format = r#"
[general]
language = "en"

[window]
anchor = "bottom-center"
margin = 96
width = 350
opacity = 0.78
bottom_panel_height = 36

[lyrics]
offset_ms = 0
provider_order = ["qq-music", "netease"]
show_translation = true
show_romanization = false
font_order = ["Sans"]
lyric_font_size = 24
translation_font_size = 13

[spotify]
mpris_prefix = "org.mpris.MediaPlayer2.spotify"
"#;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    fs::write(&path, old_format).unwrap();
    let config = AppConfig::load_or_default(&path).unwrap();
    assert_eq!(config.lyrics.played_color, "#FFFFFFFF");
    assert_eq!(config.lyrics.unplayed_color, "#9EA6B3FF");
    assert_eq!(config.lyrics.translation_color, "#FFFFFFC7");
    assert_eq!(config.lyrics.romanization_font_size, 12);
    assert_eq!(config.lyrics.romanization_color, "#B8D8F0E6");
    assert!(!config.lyrics.apple_music_style);
    assert!(config.window.remember_position);
    assert_eq!(config.window.position, None);
    assert_eq!(
        config.lyrics.chinese_romanization,
        ChineseRomanizationMode::Auto
    );
}

#[test]
fn apple_music_style_round_trips_in_config() {
    let mut config = AppConfig::default();
    config.lyrics.apple_music_style = true;

    let serialized = toml::to_string(&config).unwrap();
    let restored: AppConfig = toml::from_str(&serialized).unwrap();

    assert!(serialized.contains("apple_music_style = true"));
    assert!(restored.lyrics.apple_music_style);
}

#[test]
fn remembered_window_position_round_trips_in_config() {
    let mut config = AppConfig::default();
    config.window.position = Some(WindowPosition {
        horizontal: 0.25,
        vertical: 0.75,
    });

    let serialized = toml::to_string(&config).unwrap();
    let restored: AppConfig = toml::from_str(&serialized).unwrap();

    assert_eq!(restored, config);
}

#[test]
fn chinese_romanization_mode_round_trips_in_config() {
    let mut config = AppConfig::default();
    config.lyrics.chinese_romanization = ChineseRomanizationMode::CantoneseJyutpingNoTones;

    let serialized = toml::to_string(&config).unwrap();
    let restored: AppConfig = toml::from_str(&serialized).unwrap();

    assert!(serialized.contains("chinese_romanization = \"cantonese-jyutping-no-tones\""));
    assert_eq!(restored, config);
}
