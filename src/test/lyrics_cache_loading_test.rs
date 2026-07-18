use super::*;

use crate::shared::config::AppConfig;
use floatlyrics_lyrics::{
    cache::CachedLyrics,
    lyrics::{LyricsProvider, TimedLine},
};

#[test]
fn manual_lyrics_without_translation_do_not_trigger_provider_refresh() {
    let cached = CachedLyrics {
        manually_selected: true,
        id: 1,
        provider: LyricsProvider::QqMusic,
        provider_track_id: Some("manual".to_string()),
        title: "Song".to_string(),
        artists: vec!["Artist".to_string()],
        raw_lyrics: "[00:01.00]manual lyrics".to_string(),
    };
    let state = LyricsDisplayState {
        track_fingerprint: Some("track".to_string()),
        lines: vec![line("manual lyrics")],
        status_message: None,
    };

    assert!(!should_refresh_translation(
        &cached,
        &state,
        &LyricsRuntimeConfig::from(&AppConfig::default()),
    ));
}

fn line(text: &str) -> TimedLine {
    TimedLine {
        start_ms: 1_000,
        end_ms: None,
        text: text.to_string(),
        syllables: Vec::new(),
        translation: None,
        romanization: None,
        romanization_segments: Vec::new(),
        background: None,
    }
}
