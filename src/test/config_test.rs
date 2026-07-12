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
fn loads_legacy_config_without_general_section() {
    let config: AppConfig = toml::from_str(
        r#"
            [window]
            width = 500
            "#,
    )
    .unwrap();

    assert_eq!(config.window.width, 500);
    assert!(Language::ALL.contains(&config.general.language));
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
