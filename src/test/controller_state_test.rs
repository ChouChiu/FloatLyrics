use super::*;

use crate::{
    backend::{
        cache::CacheWorker,
        mpris::{PlaybackStatus, SpotifyPlayerState},
    },
    shared::{
        config::AppConfig,
        presentation::{LyricsDocument, LyricsFrame},
    },
};
use floatlyrics_lyrics::lyrics::TimedLine;

#[test]
fn reload_state_invalidates_only_the_lyrics_identity() {
    let mut state = ControllerState {
        latest: Some(snapshot("Song")),
        lyrics: LyricsDisplayState {
            track_fingerprint: Some("fingerprint".to_string()),
            lines: vec![line("existing lyrics")],
            status_message: None,
        },
        ..ControllerState::default()
    };

    state.reload_lyrics();

    assert_eq!(state.lyrics.track_fingerprint, None);
    assert_eq!(state.lyrics.lines[0].text, "existing lyrics");
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
    let mut config = LyricsRuntimeConfig::from(&AppConfig::default());
    config.provider_order.clear();
    let mut controller = Controller::new(
        receiver,
        runtime.handle().clone(),
        Rc::new(NoopLyricsView),
        cache.service(),
        config,
    );
    let handle = controller.handle();

    sender
        .send(SpotifyWatcherEvent::Connected(snapshot("Song").state))
        .unwrap();
    controller.tick();
    assert_eq!(handle.current_track().unwrap().title, "Song");

    sender.send(SpotifyWatcherEvent::Disconnected).unwrap();
    controller.tick();
    assert!(handle.current_track().is_none());
}

struct NoopLyricsView;

impl LyricsView for NoopLyricsView {
    fn set_song_info(&self, _value: &str) {}

    fn set_lyrics_document(&self, _document: LyricsDocument) {}

    fn show_lyrics(&self, _frame: LyricsFrame) {}

    fn show_status(&self, _key: Text) {}
}

fn snapshot(title: &str) -> PlaybackSnapshot {
    PlaybackSnapshot {
        state: SpotifyPlayerState {
            bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
            playback_status: PlaybackStatus::Paused,
            position_ms: Some(1_000),
            track: Some(TrackMetadata {
                title: title.to_string(),
                artists: vec!["Artist".to_string()],
                album: None,
                duration_ms: Some(60_000),
                mpris_track_id: None,
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
    }
}
