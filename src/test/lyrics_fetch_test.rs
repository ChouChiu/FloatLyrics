use super::*;
use std::{sync::mpsc, time::Duration, time::Instant};

use crate::{
    backend::{
        cache::{CacheWorker, CachedTrack, ProviderStoreError},
        controller::loading::{
            LyricsCacheApplyContext, LyricsCacheEvent, apply_lyrics_cache_event,
        },
        mpris::{PlaybackStatus, SpotifyPlayerState},
    },
    shared::config::AppConfig,
};
use floatlyrics_core::{i18n::Message, track::TrackMetadata};
use floatlyrics_lyrics::{
    cache::CachedLyrics,
    lyrics::{FetchedLyrics, LyricsLookupHint, LyricsProvider, TimedLine},
};

#[test]
fn background_fetch_failure_preserves_loaded_lyrics() {
    let mut state = loaded_state();

    assert!(!apply_lyrics_fetch_failure(
        &mut state,
        "track".to_string(),
        LyricsFetchFailure::NotFound,
    ));
    assert_eq!(state.lines[0].text, "manual lyrics");
    assert_eq!(state.status_message, None);
}

#[test]
fn initial_fetch_failure_replaces_searching_status() {
    let mut state = searching_state("track");

    assert!(apply_lyrics_fetch_failure(
        &mut state,
        "track".to_string(),
        LyricsFetchFailure::NotFound,
    ));
    assert!(state.lines.is_empty());
    assert_eq!(
        state.status_message,
        Some(Message::Text(Text::NoLyricsFound))
    );
}

#[test]
fn background_cache_write_failure_preserves_loaded_lyrics() {
    let snapshot = PlaybackSnapshot {
        state: player_state("Song", 10_000),
        received_at: Instant::now(),
    };
    let fingerprint = snapshot.state.track.as_ref().unwrap().fingerprint();
    let mut state = loaded_state();
    state.track_fingerprint = Some(fingerprint.clone());

    assert!(!apply_cache_event(
        LyricsCacheEvent::ProviderStored {
            track_fingerprint: fingerprint,
            generation: 4,
            result: Err(ProviderStoreError::Store("database is locked".to_string())),
        },
        4,
        &snapshot,
        &mut state,
    ));
    assert_eq!(state.lines[0].text, "manual lyrics");
    assert_eq!(state.status_message, None);
}

#[test]
fn initial_cache_write_failure_replaces_searching_status() {
    let snapshot = PlaybackSnapshot {
        state: player_state("Song", 10_000),
        received_at: Instant::now(),
    };
    let fingerprint = snapshot.state.track.as_ref().unwrap().fingerprint();
    let mut state = searching_state(&fingerprint);

    assert!(apply_cache_event(
        LyricsCacheEvent::ProviderStored {
            track_fingerprint: fingerprint,
            generation: 4,
            result: Err(ProviderStoreError::Store("disk is read-only".to_string())),
        },
        4,
        &snapshot,
        &mut state,
    ));
    assert_eq!(
        state.status_message,
        Some(Message::Detail(
            Text::LyricsCacheWriteError,
            "disk is read-only".to_string(),
        ))
    );
}

#[test]
fn obsolete_same_track_fetch_generation_is_ignored() {
    let snapshot = PlaybackSnapshot {
        state: player_state("Song", 10_000),
        received_at: Instant::now(),
    };
    let fingerprint = snapshot.state.track.as_ref().unwrap().fingerprint();
    let state = searching_state(&fingerprint);
    let event = LyricsFetchEvent {
        track_fingerprint: fingerprint,
        generation: 4,
        result: Err(LyricsFetchFailure::NotFound),
    };

    assert!(lyrics_fetch_matches_current(&event, 4, &snapshot, &state));
    assert!(!lyrics_fetch_matches_current(&event, 5, &snapshot, &state));
}

#[test]
fn obsolete_same_track_cache_generation_is_ignored() {
    let snapshot = PlaybackSnapshot {
        state: player_state("Song", 10_000),
        received_at: Instant::now(),
    };
    let fingerprint = snapshot.state.track.as_ref().unwrap().fingerprint();
    let mut state = searching_state(&fingerprint);
    let event = LyricsCacheEvent::TrackLoaded {
        track: snapshot.state.track.clone().unwrap(),
        hint: None,
        track_fingerprint: fingerprint,
        generation: 4,
        offset_generation: 2,
        result: Err("stale cache failure".to_string()),
    };

    assert!(!apply_cache_event(event, 5, &snapshot, &mut state));
    assert_eq!(
        state.status_message,
        Some(Message::Text(Text::SearchingLyrics))
    );
}

#[test]
fn accepted_fetch_is_persisted_and_reloaded_as_display_state() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let cache_worker = CacheWorker::new(&directory.path().join("lyrics.db")).unwrap();
    let cache = cache_worker.service();
    let player = player_state("Song", 10_000);
    let track = player.track.as_ref().unwrap().clone();
    let fingerprint = track.fingerprint();
    let (load_sender, load_receiver) = mpsc::channel();
    cache.load_track(
        track.clone(),
        vec![LyricsProvider::QqMusic],
        move |result| {
            load_sender.send(result).unwrap();
        },
    );
    assert_eq!(
        load_receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
        Ok(CachedTrack {
            lyrics: None,
            offset_ms: 0,
        })
    );
    let snapshot = PlaybackSnapshot {
        state: player,
        received_at: Instant::now(),
    };
    let mut state = searching_state(&fingerprint);
    let config = LyricsRuntimeConfig::from(&AppConfig::default());
    let (romanization_sender, _romanization_receiver) = mpsc::channel();
    let mut track_offset_ms = 0;
    let (cache_sender, cache_receiver) = mpsc::channel();
    let event = LyricsFetchEvent {
        track_fingerprint: fingerprint.clone(),
        generation: 7,
        result: Ok(FetchedLyrics {
            provider: LyricsProvider::QqMusic,
            provider_track_id: Some("provider-song".to_string()),
            title: "Song".to_string(),
            artists: vec!["Artist".to_string()],
            score: 100.0,
            raw_lyrics: "[00:01.00]Hello".to_string(),
        }),
    };

    assert!(!apply_lyrics_fetch_event(
        event,
        &mut LyricsFetchApplyContext {
            current_generation: 7,
            snapshot: &snapshot,
            state: &mut state,
            cache: &cache,
            config: &config,
            cache_sender: &cache_sender,
        },
    ));

    let cache_event = cache_receiver.recv_timeout(Duration::from_secs(3)).unwrap();
    let (lyrics_sender, _lyrics_receiver) = mpsc::channel();
    assert!(apply_lyrics_cache_event(
        cache_event,
        &mut LyricsCacheApplyContext {
            current_generation: 7,
            snapshot: &snapshot,
            state: &mut state,
            track_offset_ms: &mut track_offset_ms,
            current_offset_generation: 0,
            config: &config,
            runtime: runtime.handle(),
            lyrics_sender: &lyrics_sender,
            romanization_sender: &romanization_sender,
        },
    ));
    assert_eq!(state.lines.len(), 1);
    assert_eq!(state.lines[0].text, "Hello");
    assert_eq!(state.status_message, None);
    let (load_sender, load_receiver) = mpsc::channel();
    cache.load_track(track, config.provider_order.clone(), move |result| {
        load_sender.send(result).unwrap();
    });
    let cached = load_receiver
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
        .unwrap()
        .lyrics
        .unwrap();
    assert_eq!(cached.provider_track_id.as_deref(), Some("provider-song"));
}

#[test]
fn track_cache_load_applies_remembered_offset_without_cached_lyrics() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let snapshot = PlaybackSnapshot {
        state: player_state("Song", 10_000),
        received_at: Instant::now(),
    };
    let fingerprint = snapshot.state.track.as_ref().unwrap().fingerprint();
    let mut state = searching_state(&fingerprint);
    let mut config = LyricsRuntimeConfig::from(&AppConfig::default());
    config.provider_order.clear();
    let (lyrics_sender, _lyrics_receiver) = mpsc::channel();
    let (romanization_sender, _romanization_receiver) = mpsc::channel();
    let mut track_offset_ms = 0;

    let applied = apply_lyrics_cache_event(
        LyricsCacheEvent::TrackLoaded {
            track: snapshot.state.track.as_ref().unwrap().clone(),
            hint: None,
            track_fingerprint: fingerprint,
            generation: 4,
            offset_generation: 6,
            result: Ok(CachedTrack {
                lyrics: None,
                offset_ms: -450,
            }),
        },
        &mut LyricsCacheApplyContext {
            current_generation: 4,
            snapshot: &snapshot,
            state: &mut state,
            track_offset_ms: &mut track_offset_ms,
            current_offset_generation: 6,
            config: &config,
            runtime: runtime.handle(),
            lyrics_sender: &lyrics_sender,
            romanization_sender: &romanization_sender,
        },
    );

    assert!(!applied);
    assert_eq!(track_offset_ms, -450);
}

#[test]
fn stale_offset_load_does_not_overwrite_a_new_user_adjustment() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let snapshot = PlaybackSnapshot {
        state: player_state("Song", 10_000),
        received_at: Instant::now(),
    };
    let fingerprint = snapshot.state.track.as_ref().unwrap().fingerprint();
    let mut state = searching_state(&fingerprint);
    let mut config = LyricsRuntimeConfig::from(&AppConfig::default());
    config.provider_order.clear();
    let (lyrics_sender, _lyrics_receiver) = mpsc::channel();
    let (romanization_sender, _romanization_receiver) = mpsc::channel();
    let mut track_offset_ms = 100;

    apply_lyrics_cache_event(
        LyricsCacheEvent::TrackLoaded {
            track: snapshot.state.track.as_ref().unwrap().clone(),
            hint: None,
            track_fingerprint: fingerprint,
            generation: 4,
            offset_generation: 6,
            result: Ok(CachedTrack {
                lyrics: None,
                offset_ms: -450,
            }),
        },
        &mut LyricsCacheApplyContext {
            current_generation: 4,
            snapshot: &snapshot,
            state: &mut state,
            track_offset_ms: &mut track_offset_ms,
            current_offset_generation: 7,
            config: &config,
            runtime: runtime.handle(),
            lyrics_sender: &lyrics_sender,
            romanization_sender: &romanization_sender,
        },
    );

    assert_eq!(track_offset_ms, 100);
}

#[test]
fn exact_player_hint_refreshes_only_automatic_cache_from_an_enabled_provider() {
    let config = LyricsRuntimeConfig::from(&AppConfig::default());
    let hint = LyricsLookupHint {
        provider: LyricsProvider::NetEase,
        provider_track_id: "123".to_string(),
    };
    let mut cached = CachedLyrics {
        manually_selected: false,
        id: 1,
        provider: LyricsProvider::NetEase,
        provider_track_id: Some("old".to_string()),
        title: "Song".to_string(),
        artists: vec!["Artist".to_string()],
        raw_lyrics: "[00:01.00]Hello".to_string(),
    };

    assert!(super::super::hint_requires_refresh(
        &cached,
        Some(&hint),
        &config
    ));

    cached.manually_selected = true;
    assert!(!super::super::hint_requires_refresh(
        &cached,
        Some(&hint),
        &config
    ));

    cached.manually_selected = false;
    let mut disabled = config.clone();
    disabled.provider_order = vec![LyricsProvider::QqMusic];
    assert!(!super::super::hint_requires_refresh(
        &cached,
        Some(&hint),
        &disabled
    ));

    let earlier_provider_cache = CachedLyrics {
        provider: LyricsProvider::QqMusic,
        ..cached
    };
    assert!(!super::super::hint_requires_refresh(
        &earlier_provider_cache,
        Some(&hint),
        &config
    ));
}

fn apply_cache_event(
    event: LyricsCacheEvent,
    generation: u64,
    snapshot: &PlaybackSnapshot,
    state: &mut LyricsDisplayState,
) -> bool {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let config = LyricsRuntimeConfig::from(&AppConfig::default());
    let (lyrics_sender, _lyrics_receiver) = mpsc::channel();
    let (romanization_sender, _romanization_receiver) = mpsc::channel();
    let mut track_offset_ms = 0;
    apply_lyrics_cache_event(
        event,
        &mut LyricsCacheApplyContext {
            current_generation: generation,
            snapshot,
            state,
            track_offset_ms: &mut track_offset_ms,
            current_offset_generation: 0,
            config: &config,
            runtime: runtime.handle(),
            lyrics_sender: &lyrics_sender,
            romanization_sender: &romanization_sender,
        },
    )
}

fn loaded_state() -> LyricsDisplayState {
    LyricsDisplayState {
        track_fingerprint: Some("track".to_string()),
        lines: vec![line("manual lyrics")],
        status_message: None,
    }
}

fn searching_state(fingerprint: &str) -> LyricsDisplayState {
    LyricsDisplayState {
        track_fingerprint: Some(fingerprint.to_string()),
        status_message: Some(Message::Text(Text::SearchingLyrics)),
        ..LyricsDisplayState::default()
    }
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

fn player_state(title: &str, position_ms: u64) -> SpotifyPlayerState {
    SpotifyPlayerState {
        bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
        playback_status: PlaybackStatus::Paused,
        position_ms: Some(position_ms),
        track: Some(TrackMetadata {
            title: title.to_string(),
            artists: vec!["Artist".to_string()],
            album: None,
            duration_ms: Some(60_000),
            mpris_track_id: None,
        }),
    }
}
