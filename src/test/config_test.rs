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
fn load_recovers_out_of_range_field_and_preserves_valid_preferences() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let mut config = AppConfig::default();
    config.window.width = ConfigLimits::WINDOW_WIDTH_MAX + 1;
    config.window.opacity = 0.42;
    config.lyrics.offset_ms = 275;
    let original = toml::to_string(&config).unwrap();
    fs::write(&path, &original).unwrap();

    let recovered = AppConfig::load_or_default(&path).unwrap();

    assert_eq!(recovered.window.width, AppConfig::default().window.width);
    assert_eq!(recovered.window.opacity, 0.42);
    assert_eq!(recovered.lyrics.offset_ms, 275);
    assert_eq!(
        fs::read_to_string(incompatible_backup(&path)).unwrap(),
        original
    );
    assert_eq!(
        toml::from_str::<AppConfig>(&fs::read_to_string(&path).unwrap()).unwrap(),
        recovered
    );
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
fn load_recovers_invalid_color_without_discarding_other_lyrics_preferences() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let mut config = AppConfig::default();
    config.lyrics.translation_color = "not-a-color".to_string();
    config.lyrics.show_romanization = true;
    fs::write(&path, toml::to_string(&config).unwrap()).unwrap();

    let recovered = AppConfig::load_or_default(&path).unwrap();

    assert_eq!(
        recovered.lyrics.translation_color,
        AppConfig::default().lyrics.translation_color
    );
    assert!(recovered.lyrics.show_romanization);
    assert!(incompatible_backup(&path).exists());
}

#[test]
fn load_ignores_unknown_fields_and_rewrites_the_current_format() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let mut config = AppConfig::default();
    config.window.width = 512;
    let mut value = toml::Value::try_from(&config).unwrap();
    value
        .as_table_mut()
        .unwrap()
        .insert("obsolete".to_string(), toml::Table::new().into());
    let original = toml::to_string(&value).unwrap();
    fs::write(&path, &original).unwrap();

    let recovered = AppConfig::load_or_default(&path).unwrap();

    assert_eq!(recovered.window.width, 512);
    assert_eq!(
        fs::read_to_string(incompatible_backup(&path)).unwrap(),
        original
    );
    assert!(!fs::read_to_string(&path).unwrap().contains("obsolete"));
}

#[test]
fn load_recovers_type_and_enum_changes_field_by_field() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let mut value = toml::Value::try_from(AppConfig::default()).unwrap();
    value["general"]["language"] = toml::Value::String("unsupported".to_string());
    value["window"]["width"] = toml::Value::String("wide".to_string());
    value["lyrics"]["offset_ms"] = toml::Value::Integer(450);
    fs::write(&path, toml::to_string(&value).unwrap()).unwrap();

    let recovered = AppConfig::load_or_default(&path).unwrap();

    assert_eq!(
        recovered.general.language,
        AppConfig::default().general.language
    );
    assert_eq!(recovered.window.width, AppConfig::default().window.width);
    assert_eq!(recovered.lyrics.offset_ms, 450);
}

#[test]
fn malformed_or_non_utf8_config_falls_back_without_losing_original_bytes() {
    for original in [b"[window\nwidth = 500".as_slice(), &[0xff, 0xfe, 0xfd]] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        fs::write(&path, original).unwrap();

        let recovered = AppConfig::load_or_default(&path).unwrap();

        assert_eq!(recovered, AppConfig::default());
        assert_eq!(fs::read(incompatible_backup(&path)).unwrap(), original);
        assert_eq!(AppConfig::load_or_default(&path).unwrap(), recovered);
    }
}

#[test]
fn recovery_never_overwrites_an_existing_incompatible_backup() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let first_backup = incompatible_backup(&path);
    fs::write(&first_backup, "older backup").unwrap();
    fs::write(&path, "not valid toml = [").unwrap();

    AppConfig::load_or_default(&path).unwrap();

    assert_eq!(fs::read_to_string(first_backup).unwrap(), "older backup");
    assert_eq!(
        fs::read_to_string(path.with_file_name("config.toml.incompatible.1")).unwrap(),
        "not valid toml = ["
    );
}

#[test]
fn filesystem_read_errors_remain_fatal() {
    let directory = tempfile::tempdir().unwrap();

    let error = format!(
        "{:#}",
        AppConfig::load_or_default(directory.path()).unwrap_err()
    );

    assert!(error.contains("reading config file"));
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

fn incompatible_backup(path: &std::path::Path) -> std::path::PathBuf {
    path.with_file_name(format!(
        "{}.incompatible",
        path.file_name().unwrap().to_string_lossy()
    ))
}
