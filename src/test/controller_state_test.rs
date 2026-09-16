use super::*;

use crate::{
    backend::{
        cache::CacheWorker,
        mpris::{PlaybackStatus, PlayerState, PlayerWatcherEvent},
    },
    shared::{
        config::AppConfig,
        presentation::{LyricsDocument, LyricsFrame},
    },
};
use floatlyrics_lyrics::lyrics::{LyricsLookupHint, LyricsProvider, TimedLine, Voice};
use std::time::Duration;

#[test]
fn reload_state_invalidates_only_the_lyrics_identity() {
    let mut state = ControllerState {
        latest: Some(snapshot("Song")),
        lyrics: LyricsDisplayState {
            track_fingerprint: Some("fingerprint".to_string()),
            lines: vec![line("existing lyrics")],
            status_message: None,
        },
        track_offset_ms: 300,
        track_offset_fingerprint: Some("fingerprint".to_string()),
        ..ControllerState::default()
    };

    state.reload_lyrics();

    assert_eq!(state.lyrics.track_fingerprint, None);
    assert_eq!(state.lyrics.lines[0].text, "existing lyrics");
    assert_eq!(state.track_offset_ms, 300);
    assert_eq!(
        state.track_offset_fingerprint.as_deref(),
        Some("fingerprint")
    );
    assert_eq!(
        state
            .latest
            .as_ref()
            .unwrap()
            .state
            .track
            .as_ref()
            .unwrap()
            .title,
        "Song"
    );
    assert!(state.document_dirty);
}

#[test]
fn presentation_refresh_does_not_invalidate_lyrics() {
    let mut state = ControllerState {
        lyrics: LyricsDisplayState {
            track_fingerprint: Some("fingerprint".to_string()),
            ..LyricsDisplayState::default()
        },
        ..ControllerState::default()
    };

    state.refresh_lyrics_presentation();

    assert_eq!(
        state.lyrics.track_fingerprint.as_deref(),
        Some("fingerprint")
    );
    assert!(state.document_dirty);
}

#[test]
fn a_new_exact_hint_reloads_only_the_same_track() {
    let mut state = ControllerState {
        latest: Some(snapshot("Song")),
        lyrics: LyricsDisplayState {
            track_fingerprint: Some("loaded".to_string()),
            ..LyricsDisplayState::default()
        },
        ..ControllerState::default()
    };
    let track_fingerprint = state
        .latest
        .as_ref()
        .unwrap()
        .state
        .track
        .as_ref()
        .unwrap()
        .fingerprint();
    let hint = LyricsLookupHint {
        provider: LyricsProvider::NetEase,
        provider_track_id: "123".to_string(),
    };

    apply_player_lyrics_hint(
        PlayerLyricsHintEvent {
            bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
            track_fingerprint: Some(track_fingerprint.clone()),
            hint: Some(hint.clone()),
        },
        &mut state,
    );
    assert_eq!(state.lyrics_hint, Some(hint));
    assert_eq!(state.lyrics.track_fingerprint, None);

    state.lyrics.track_fingerprint = Some("still-loaded".to_string());
    apply_player_lyrics_hint(
        PlayerLyricsHintEvent {
            bus_name: "org.mpris.MediaPlayer2.other".to_string(),
            track_fingerprint: Some(track_fingerprint),
            hint: None,
        },
        &mut state,
    );
    assert_eq!(
        state.lyrics.track_fingerprint.as_deref(),
        Some("still-loaded")
    );
}

#[test]
fn handle_sends_reload_command_without_accessing_controller_state() {
    let (commands, receiver) = mpsc::channel();
    let handle = ControllerHandle {
        commands,
        playback: PlaybackProjection::default(),
    };

    handle.reload_lyrics();

    assert_eq!(receiver.try_recv(), Ok(ControllerCommand::ReloadLyrics));
}

#[test]
fn handle_sends_per_track_offset_commands() {
    let (commands, receiver) = mpsc::channel();
    let handle = ControllerHandle {
        commands,
        playback: PlaybackProjection::default(),
    };

    handle.adjust_track_offset(100);
    handle.reset_track_offset();

    assert_eq!(
        receiver.try_recv(),
        Ok(ControllerCommand::AdjustTrackOffset(100))
    );
    assert_eq!(receiver.try_recv(), Ok(ControllerCommand::ResetTrackOffset));
}

#[test]
fn reset_at_zero_invalidates_a_pending_offset_load_and_persists_zero() {
    let directory = tempfile::tempdir().unwrap();
    let cache = CacheWorker::new(&directory.path().join("lyrics.db")).unwrap();
    let service = cache.service();
    let track = snapshot("Song").state.track.unwrap();
    let (sender, receiver) = mpsc::channel();
    service.set_track_offset(track.clone(), 450, move |result| {
        sender.send(result).unwrap();
    });
    assert_eq!(
        receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
        Ok(())
    );
    let mut state = ControllerState {
        latest: Some(snapshot("Song")),
        track_offset_fingerprint: Some(track.fingerprint()),
        track_offset_generation: 7,
        ..ControllerState::default()
    };

    set_track_offset(&mut state, &service, &NoopLyricsView, 0, true);

    assert_eq!(state.track_offset_generation, 8);
    let (sender, receiver) = mpsc::channel();
    service.load_track(track, Vec::new(), move |result| {
        sender.send(result).unwrap();
    });
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap()
            .offset_ms,
        0
    );
}

#[test]
fn adjusting_track_offset_marks_the_next_frame_as_seeking() {
    let directory = tempfile::tempdir().unwrap();
    let cache = CacheWorker::new(&directory.path().join("lyrics.db")).unwrap();
    let mut state = ControllerState {
        latest: Some(snapshot("Song")),
        ..ControllerState::default()
    };

    set_track_offset(&mut state, &cache.service(), &NoopLyricsView, -2_000, false);

    assert_eq!(state.track_offset_ms, -2_000);
    assert!(state.seek_pending);
}

#[test]
fn cloned_handles_read_the_narrow_playback_projection() {
    let (commands, _receiver) = mpsc::channel();
    let playback = PlaybackProjection::default();
    let handle = ControllerHandle {
        commands,
        playback: playback.clone(),
    };
    playback.set_current_track(snapshot("Song").state.track);

    assert_eq!(handle.clone().current_track().unwrap().title, "Song");

    playback.set_current_track(None);
    assert!(handle.current_track().is_none());
}

#[test]
fn playback_projection_follows_connection_lifecycle() {
    let directory = tempfile::tempdir().unwrap();
    let cache = CacheWorker::new(&directory.path().join("lyrics.db")).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let (_hint_sender, hint_receiver) = mpsc::channel();
    let mut config = LyricsRuntimeConfig::from(&AppConfig::default());
    config.provider_order.clear();
    let mut controller = Controller::new(
        receiver,
        hint_receiver,
        runtime.handle().clone(),
        Rc::new(NoopLyricsView),
        cache.service(),
        config,
    );
    let handle = controller.handle();

    sender
        .send(PlayerWatcherEvent::Connected(snapshot("Song").state))
        .unwrap();
    controller.tick();
    assert_eq!(handle.current_track().unwrap().title, "Song");

    sender.send(PlayerWatcherEvent::Disconnected).unwrap();
    controller.tick();
    assert!(handle.current_track().is_none());
}

struct NoopLyricsView;

impl LyricsView for NoopLyricsView {
    fn set_song_info(&self, _value: &str) {}

    fn set_track_offset(&self, _offset_ms: i64) {}

    fn set_lyrics_document(&self, _document: LyricsDocument) {}

    fn show_lyrics(&self, _frame: LyricsFrame) {}

    fn show_status(&self, _key: Text) {}
}

fn snapshot(title: &str) -> PlaybackSnapshot {
    PlaybackSnapshot {
        state: PlayerState {
            bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
            playback_status: PlaybackStatus::Paused,
            position_ms: Some(1_000),
            track: Some(TrackMetadata {
                title: title.to_string(),
                artists: vec!["Artist".to_string()],
                album: None,
                duration_ms: Some(60_000),
                mpris_track_id: None,
                art_url: None,
            }),
        },
        received_at: Instant::now(),
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
        voice: Voice::Primary,
    }
}

/// Records what the listener is told, which is what a listener can observe.
#[derive(Default)]
struct RecordingLyricsView {
    metadata: RefCell<Vec<Vec<String>>>,
    song_info: RefCell<Vec<String>>,
}

impl LyricsView for RecordingLyricsView {
    fn set_song_info(&self, value: &str) {
        self.song_info.borrow_mut().push(value.to_string());
    }

    fn set_track_metadata(&self, track: Option<&TrackMetadata>) {
        if let Some(track) = track {
            self.metadata.borrow_mut().push(track.artists.clone());
        }
    }
}
