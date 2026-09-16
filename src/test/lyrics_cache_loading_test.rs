use super::*;

use crate::shared::config::AppConfig;
use floatlyrics_lyrics::{
    cache::CachedLyrics,
    lyrics::{LyricsProvider, TimedLine, Voice},
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
        voice: Voice::Primary,
    }
}

#[test]
fn loading_cached_lyrics_reads_the_billing_and_labels_of_their_provider() {
    let cached = CachedLyrics {
        manually_selected: false,
        id: 2,
        provider: LyricsProvider::QqMusic,
        provider_track_id: Some("42".to_string()),
        title: "Problem".to_string(),
        artists: vec!["Ariana Grande".to_string(), "Iggy Azalea".to_string()],
        raw_lyrics: "\
[0,500]Iggy Azalea/Ariana Grande：(0,500)
[1000,1000]Uh-huh, it's Iggy(1000,1000)"
            .to_string(),
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let (sender, _receiver) = mpsc::channel();
    let mut config = LyricsRuntimeConfig::from(&AppConfig::default());
    config.show_romanization = false;

    let state = lyrics_state_from_cached(
        "track".to_string(),
        &cached,
        &config,
        runtime.handle(),
        &sender,
    );

    assert_eq!(state.credited_artists, cached.artists);
    assert_eq!(state.lines.len(), 1);
    assert_eq!(state.lines[0].text, "Uh-huh, it's Iggy");
    assert_eq!(state.lines[0].voice, Voice::Secondary);
}
