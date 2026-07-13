use super::*;

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
}

#[test]
fn default_font_order_uses_generic_sans() {
    assert_eq!(AppConfig::default().lyrics.font_order, ["Sans"]);
}

#[test]
fn romanization_has_distinct_default_presentation() {
    let lyrics = AppConfig::default().lyrics;

    assert_eq!(lyrics.romanization_font_size, 12);
    assert_eq!(lyrics.romanization_color, "#B8D8F0E6");
    assert_ne!(lyrics.romanization_color, lyrics.translation_color);
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
    config.window.width = 720;

    config.save(&path).unwrap();

    assert_eq!(AppConfig::load_or_default(&path).unwrap(), config);
    let entries = fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(entries, vec!["config.toml"]);
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
    let config: AppConfig = toml::from_str(old_format).unwrap();
    assert_eq!(config.lyrics.played_color, "#FFFFFFFF");
    assert_eq!(config.lyrics.unplayed_color, "#9EA6B3FF");
    assert_eq!(config.lyrics.translation_color, "#FFFFFFC7");
    assert_eq!(config.lyrics.romanization_font_size, 12);
    assert_eq!(config.lyrics.romanization_color, "#B8D8F0E6");
}
