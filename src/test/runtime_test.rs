// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

use super::*;

#[test]
fn runtime_config_contains_only_playback_preferences() {
    let mut config = AppConfig::default();
    config.window.width = 720;
    config.lyrics.offset_ms = -250;
    config.lyrics.show_translation = false;
    config.lyrics.show_romanization = true;

    let runtime = LyricsRuntimeConfig::from(&config);

    assert_eq!(runtime.language, config.general.language);
    assert_eq!(runtime.offset_ms, -250);
    assert_eq!(runtime.provider_order, config.lyrics.provider_order);
    assert!(!runtime.show_translation);
    assert!(runtime.show_romanization);
    assert_eq!(
        runtime.chinese_romanization,
        config.lyrics.chinese_romanization
    );
}
