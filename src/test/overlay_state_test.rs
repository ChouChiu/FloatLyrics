use super::*;

use crate::shared::presentation::LyricSlotText;

#[test]
fn content_and_status_updates_control_runtime_relocalization() {
    let config = AppConfig::default();
    let mut state = OverlayState::new(&config, 400);
    assert_eq!(state.static_status(), Some(Text::OpenSpotify));

    state.show_content();
    assert_eq!(state.static_status(), None);

    state.show_status(Text::WaitingForMetadata);
    assert_eq!(state.static_status(), Some(Text::WaitingForMetadata));
}

#[test]
fn layout_identity_includes_secondary_text_and_uses_current_animation_mode() {
    let mut config = AppConfig::default();
    let mut state = OverlayState::new(&config, 400);
    let initial_frame = frame("line:1", "romanization", "translation");

    assert_eq!(state.register_frame(&initial_frame), Some(false));
    assert_eq!(state.register_frame(&initial_frame), None);
    assert_eq!(
        state.register_frame(&frame("line:1", "romanization", "new translation")),
        Some(false)
    );

    config.lyrics.apple_music_style = true;
    state.apply_config(&config, 420);
    assert_eq!(state.register_frame(&initial_frame), Some(true));
}

#[test]
fn applying_config_updates_metrics_invalidates_layout_and_cancels_animation() {
    let mut config = AppConfig::default();
    let mut state = OverlayState::new(&config, 400);
    assert_eq!(state.register_frame(&frame("line:1", "", "")), Some(false));
    assert_eq!(state.animation_generation(), 0);

    config.lyrics.lyric_font_size = 42;
    config.lyrics.romanization_font_size = 19;
    config.lyrics.translation_font_size = 21;
    config.lyrics.apple_music_style = true;
    state.apply_config(&config, 520);

    assert_eq!(
        state.metrics(),
        OverlayMetrics {
            compact_width: 520,
            lyric_font_size: 42,
            romanization_font_size: 19,
            translation_font_size: 21,
            apple_music_style: true,
        }
    );
    assert_eq!(state.animation_generation(), 1);
    assert_eq!(state.register_frame(&frame("line:1", "", "")), Some(true));
}

fn frame(key: &str, romanization: &str, translation: &str) -> LyricsFrame {
    LyricsFrame {
        key: key.to_string(),
        content: LyricSlotText {
            text: "lyrics".to_string(),
            karaoke: None,
            romanization: romanization.to_string(),
            translation: translation.to_string(),
        },
        position_ms: Some(1_000),
        playing: true,
        seeking: false,
    }
}
