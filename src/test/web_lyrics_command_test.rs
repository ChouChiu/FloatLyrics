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
