use super::*;
use crate::shared::presentation::LyricSlotText;

#[test]
fn render_script_serializes_lyrics_as_data() {
    let frame = LyricsFrame {
        key: "line:1".to_string(),
        content: LyricSlotText::message("'quoted' </script> 歌词"),
        position_ms: Some(1_000),
        playing: true,
        seeking: false,
    };

    let script = frame_script(&frame).unwrap();

    assert!(script.starts_with("((command) => {"));
    assert!(script.contains("window.floatLyrics.dispatch(command)"));
    assert!(script.contains("window.floatLyricsPendingCommands"));
    assert!(script.contains("\"type\":\"frame\""));
    assert!(script.contains("\"key\":\"line:1\""));
    assert!(script.contains("'quoted' </script> 歌词"));
}

#[test]
fn invalid_config_color_uses_opaque_white() {
    assert_eq!(css_color("invalid"), "rgba(255,255,255,1.0000)");
}

#[test]
fn overlay_placement_serializes_edge_classes_for_react() {
    let script = overlay_placement_script(&["snapped-left", "snapped-bottom"]).unwrap();

    assert!(script.contains("\"type\":\"overlay-placement\""));
    assert!(script.contains("\"classes\":[\"snapped-left\",\"snapped-bottom\"]"));
}

#[test]
fn overlay_appearance_serializes_live_opacity_for_react() {
    let script = overlay_appearance_script(0.42).unwrap();

    assert!(script.contains("\"type\":\"overlay-appearance\""));
    assert!(script.contains("\"opacity\":0.42"));
}

#[test]
fn manual_search_bootstraps_as_an_independent_surface() {
    let script = bootstrap_script(
        UiSurface::ManualSearch,
        &AppConfig::default(),
        std::collections::BTreeMap::new(),
        &serde_json::json!({ "dependencies": [], "licenses": [] }),
        &[],
    )
    .unwrap();

    assert!(script.contains("\"surface\":\"manual-search\""));
}

#[test]
fn font_picker_bootstrap_includes_discovered_font_families() {
    let script = bootstrap_script(
        UiSurface::FontPicker,
        &AppConfig::default(),
        std::collections::BTreeMap::new(),
        &serde_json::json!({ "dependencies": [], "licenses": [] }),
        &["Noto Sans".to_string(), "Source Han Sans".to_string()],
    )
    .unwrap();

    assert!(script.contains("\"surface\":\"font-picker\""));
    assert!(script.contains("\"available_fonts\":[\"Noto Sans\",\"Source Han Sans\"]"));
}

#[test]
fn settings_navigation_serializes_sidebar_pages() {
    let script = navigate_script(ControlPage::Display).unwrap();

    assert!(script.contains("\"type\":\"navigate\""));
    assert!(script.contains("\"page\":\"display\""));
}
