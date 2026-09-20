use super::super::{AmllEvent, Cover, Music};
use super::*;

use crate::{
    backend::mpris::MediaCommand,
    shared::presentation::{LoopStatus, LyricsDocument, PlayerControl, PresentedLyricLine},
};
use floatlyrics_lyrics::lyrics::Voice;

fn music(name: &str) -> Music {
    Music {
        id: format!("id:{name}"),
        name: name.to_string(),
        album: "Album".to_string(),
        artists: vec!["Artist".to_string()],
        duration_ms: 200_000,
        cover: None,
    }
}

fn document(revision: u64) -> LyricsDocument {
    LyricsDocument {
        revision,
        duration_ms: Some(200_000),
        lines: vec![PresentedLyricLine {
            start_ms: 0,
            end_ms: Some(1_000),
            text: "line".to_string(),
            syllables: Vec::new(),
            romanization: String::new(),
            translation: String::new(),
            background: String::new(),
            background_translation: String::new(),
            background_start_ms: 0,
            background_end_ms: None,
            background_syllables: Vec::new(),
            voice: Voice::Primary,
        }],
    }
}

/// A player that reports a volume, both playback modes, and full capabilities.
fn control(volume: f64) -> PlayerControl {
    PlayerControl {
        volume: Some(volume),
        loop_status: Some(LoopStatus::Off),
        shuffle: Some(false),
        ..PlayerControl::default()
    }
}

fn playback(position_ms: Option<u64>, playing: bool) -> AmllEvent {
    AmllEvent::Playback {
        position_ms,
        playing,
    }
}

/// The same track, restated with the billing the lyrics provider added.
fn credited_track(track: &Music) -> Music {
    let mut credited = track.clone();
    credited.artists = vec!["Artist".to_string(), "Featured".to_string()];
    credited
}

#[test]
fn skips_updates_that_do_not_change_the_state() {
    let mut state = State::default();

    assert!(matches!(
        state.apply(AmllEvent::Music(Some(music("Song")))),
        Some(Change::Music)
    ));
    assert!(state.apply(AmllEvent::Music(Some(music("Song")))).is_none());
    assert!(state.apply(AmllEvent::Lyrics(document(1))).is_some());
    assert!(state.apply(AmllEvent::Lyrics(document(1))).is_none());
    assert!(state.apply(playback(Some(1_000), true)).is_some());
    assert!(state.apply(playback(Some(1_000), true)).is_none());
}

#[test]
fn reports_the_play_state_only_when_it_changes() {
    let mut state = State::default();

    let Some(Change::Playback {
        position_ms,
        play_state,
    }) = state.apply(playback(Some(1_000), true))
    else {
        panic!("the first playback update is emitted");
    };
    assert_eq!(position_ms, Some(1_000));
    assert_eq!(play_state, Some(true));

    let Some(Change::Playback {
        position_ms,
        play_state,
    }) = state.apply(playback(Some(1_400), true))
    else {
        panic!("a new position is emitted");
    };
    assert_eq!(position_ms, Some(1_400));
    assert_eq!(play_state, None, "playback continued");

    let Some(Change::Playback { play_state, .. }) = state.apply(playback(Some(1_400), false))
    else {
        panic!("a pause is emitted");
    };
    assert_eq!(play_state, Some(false));

    assert!(
        state.apply(playback(None, false)).is_none(),
        "an unchanged state without a position produces no message"
    );
}

#[test]
fn restates_the_clock_after_a_track_info_message() {
    let mut state = State::default();
    state.apply(AmllEvent::Music(Some(music("Song"))));
    state.apply(playback(Some(1_000), false));
    assert!(
        state.apply(playback(Some(1_000), false)).is_none(),
        "an unchanged clock is deduplicated"
    );

    // The provider's billing arrives mid-track and restates the metadata, which
    // zeroes the listener's playback position.
    assert!(matches!(
        state.apply(AmllEvent::Music(Some(credited_track(&music("Song"))))),
        Some(Change::Music)
    ));
    let Some(Change::Playback {
        position_ms,
        play_state,
    }) = state.apply(playback(Some(1_000), false))
    else {
        panic!("the clock follows the restated track info");
    };
    assert_eq!(position_ms, Some(1_000));
    assert_eq!(play_state, None, "the paused state did not change");

    assert!(
        state.apply(playback(Some(1_000), false)).is_none(),
        "the clock is restated once, not per update"
    );
}

#[test]
fn clears_the_listener_when_playback_stops() {
    let mut state = State::default();
    state.apply(AmllEvent::Music(Some(music("Song"))));
    state.apply(AmllEvent::Lyrics(document(1)));
    state.apply(playback(Some(1_000), true));

    assert!(matches!(
        state.apply(AmllEvent::Music(None)),
        Some(Change::Clear)
    ));
    assert!(state.apply(AmllEvent::Music(None)).is_none());
}

#[test]
fn primes_a_new_connection_with_the_current_state() {
    let mut state = State::default();
    state.apply(AmllEvent::Music(Some(music("Song"))));
    state.apply(AmllEvent::Lyrics(document(1)));
    state.apply(playback(Some(1_000), true));

    state.apply(AmllEvent::Control(Some(control(0.4))));

    let priming = state.priming();
    assert_eq!(priming.len(), 4);
    assert!(matches!(priming[0], Change::Music));
    assert!(matches!(priming[1], Change::Lyrics));
    assert!(matches!(
        priming[2],
        Change::Playback {
            position_ms: Some(1_000),
            play_state: Some(true)
        }
    ));
    assert!(matches!(
        priming[3],
        Change::Control {
            volume: Some(volume),
            mode: Some(_)
        } if volume == 0.4
    ));
}

#[test]
fn reports_only_the_control_values_that_changed() {
    let mut state = State::default();

    let Some(Change::Control { volume, mode }) =
        state.apply(AmllEvent::Control(Some(control(0.4))))
    else {
        panic!("the first control update is emitted");
    };
    assert_eq!(volume, Some(0.4));
    assert_eq!(mode, Some((RepeatMode::Off, false)));

    assert!(
        state
            .apply(AmllEvent::Control(Some(control(0.4))))
            .is_none(),
        "an unchanged control state produces no message"
    );

    let Some(Change::Control { volume, mode }) =
        state.apply(AmllEvent::Control(Some(control(0.2))))
    else {
        panic!("a new volume is emitted");
    };
    assert_eq!(volume, Some(0.2));
    assert_eq!(mode, None, "the playback mode continued");

    assert!(
        state
            .apply(AmllEvent::Control(Some(PlayerControl {
                shuffle: None,
                ..control(0.2)
            })))
            .is_none(),
        "a player that stops reporting its mode keeps the last one"
    );
}

#[test]
fn forwards_the_commands_the_listener_sends() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address").to_string();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("connection");
            let mut socket = tokio_tungstenite::accept_async(stream)
                .await
                .expect("handshake");
            let handshake = socket.next().await.expect("message").expect("frame");
            socket
                .send(Message::text(r#"{"type":"ping"}"#))
                .await
                .expect("ping");
            socket
                .send(Message::text(
                    r#"{"type":"command","value":{"command":"setVolume","volume":0.25}}"#,
                ))
                .await
                .expect("command");
            let reply = socket.next().await.expect("message").expect("frame");
            (handshake, reply)
        });

        let (events, receiver) = mpsc::unbounded_channel();
        let (commands, mut command_receiver) = mpsc::unbounded_channel();
        let client = tokio::spawn(run(address, receiver, commands));

        let received = tokio::time::timeout(Duration::from_secs(2), command_receiver.recv())
            .await
            .expect("the command arrives")
            .expect("the command channel stays open");
        assert_eq!(received, MediaCommand::SetVolume(0.25));

        let (handshake, reply) = server.await.expect("server task");
        assert_eq!(handshake, Message::text(r#"{"type":"initialize"}"#));
        assert_eq!(reply, Message::text(r#"{"type":"pong"}"#));

        drop(events);
        client.await.expect("client task ends with the sender");
    });
}

#[test]
fn streams_the_handshake_track_lyrics_and_progress_to_a_listener() {
    let cover = tempfile::NamedTempFile::new().expect("temporary cover");
    std::fs::write(cover.path(), [0xAB, 0xCD]).expect("cover bytes");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address").to_string();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("connection");
            let mut socket = tokio_tungstenite::accept_async(stream)
                .await
                .expect("handshake");
            let mut received = Vec::new();
            while received.len() < 6 {
                let Some(Ok(message)) = socket.next().await else {
                    break;
                };
                received.push(message);
            }
            received
        });

        let (events, receiver) = mpsc::unbounded_channel();
        let (commands, _command_receiver) = mpsc::unbounded_channel();
        let client = tokio::spawn(run(address, receiver, commands));
        let mut track = music("Song");
        track.cover = Some(Cover::File(cover.path().to_path_buf()));
        events
            .send(AmllEvent::Music(Some(track)))
            .expect("track event");
        events
            .send(AmllEvent::Lyrics(document(1)))
            .expect("lyrics event");
        events
            .send(playback(Some(1_500), true))
            .expect("playback event");
        drop(events);

        let received = server.await.expect("server task");
        assert_eq!(received.len(), 6);
        assert_eq!(
            json(&received[0]),
            serde_json::json!({ "type": "initialize" })
        );
        assert_eq!(
            json(&received[1])["value"]["update"],
            serde_json::json!("setMusic")
        );
        assert_eq!(
            received[2],
            Message::binary(vec![1, 0, 2, 0, 0, 0, 0xAB, 0xCD])
        );
        assert_eq!(
            json(&received[3])["value"]["update"],
            serde_json::json!("setLyric")
        );
        assert_eq!(
            json(&received[4]),
            serde_json::json!({ "type": "state", "value": { "update": "progress", "progress": 1_500 } })
        );
        assert_eq!(
            json(&received[5]),
            serde_json::json!({ "type": "state", "value": { "update": "resumed" } })
        );

        client.await.expect("client task ends with the sender");
    });
}

#[test]
fn writes_the_clock_again_after_a_restated_track() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address").to_string();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("connection");
            let mut socket = tokio_tungstenite::accept_async(stream)
                .await
                .expect("handshake");
            let mut received = Vec::new();
            while received.len() < 6 {
                let Some(Ok(message)) = socket.next().await else {
                    break;
                };
                received.push(message);
            }
            // A clock that repeats the retained one is deduplicated, so nothing
            // further may arrive.
            while let Ok(Some(Ok(message))) =
                tokio::time::timeout(Duration::from_millis(50), socket.next()).await
            {
                received.push(message);
            }
            received
        });

        let (events, receiver) = mpsc::unbounded_channel();
        let (commands, _command_receiver) = mpsc::unbounded_channel();
        let client = tokio::spawn(run(address, receiver, commands));
        events
            .send(AmllEvent::Music(Some(music("Song"))))
            .expect("track event");
        events
            .send(playback(Some(1_500), false))
            .expect("playback event");
        events
            .send(playback(Some(1_500), false))
            .expect("repeated playback event");
        // The provider's billing arrives while the track stays paused, so this
        // restatement is the only chance to put the clock back.
        events
            .send(AmllEvent::Music(Some(credited_track(&music("Song")))))
            .expect("credited track event");
        events
            .send(playback(Some(1_500), false))
            .expect("repeated playback event");
        events
            .send(playback(Some(1_500), false))
            .expect("repeated playback event");
        drop(events);

        let received = server.await.expect("server task");
        assert_eq!(received.len(), 6, "only the repeated clocks are missing");
        assert_eq!(
            json(&received[1])["value"]["artists"],
            serde_json::json!([{ "id": "", "name": "Artist" }])
        );
        assert_eq!(
            json(&received[2]),
            serde_json::json!({ "type": "state", "value": { "update": "progress", "progress": 1_500 } })
        );
        assert_eq!(
            json(&received[3]),
            serde_json::json!({ "type": "state", "value": { "update": "paused" } })
        );
        assert_eq!(
            json(&received[4])["value"]["artists"],
            serde_json::json!([
                { "id": "", "name": "Artist" },
                { "id": "", "name": "Featured" }
            ]),
            "the restated billing reaches the listener"
        );
        assert_eq!(
            json(&received[5]),
            serde_json::json!({ "type": "state", "value": { "update": "progress", "progress": 1_500 } }),
            "the clock follows the restated track info"
        );

        client.await.expect("client task ends with the sender");
    });
}

#[test]
fn reconnects_after_the_listener_closes_the_connection() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address").to_string();
        let server = tokio::spawn(async move {
            let mut handshakes = Vec::new();
            for _ in 0..2 {
                let (stream, _) = listener.accept().await.expect("connection");
                let mut socket = tokio_tungstenite::accept_async(stream)
                    .await
                    .expect("handshake");
                handshakes.push(socket.next().await.expect("message").expect("frame"));
            }
            handshakes
        });

        let (events, receiver) = mpsc::unbounded_channel();
        let (commands, _command_receiver) = mpsc::unbounded_channel();
        let client = tokio::spawn(run_with_delay(
            address,
            receiver,
            commands,
            Duration::from_millis(10),
        ));

        let handshakes = server.await.expect("server task");
        assert_eq!(handshakes[0], Message::text(r#"{"type":"initialize"}"#));
        assert_eq!(handshakes[1], Message::text(r#"{"type":"initialize"}"#));

        drop(events);
        client.await.expect("client task ends with the sender");
    });
}

#[test]
fn keeps_the_latest_state_while_disconnected() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let (events, mut receiver) = mpsc::unbounded_channel();
        events
            .send(AmllEvent::Music(Some(music("Song"))))
            .expect("track event");
        let mut state = State::default();

        let outcome = tokio::time::timeout(
            Duration::from_millis(50),
            wait_for_reconnect(&mut receiver, &mut state, RECONNECT_DELAY),
        )
        .await;
        assert!(
            outcome.is_err(),
            "an offline sender waits for the reconnect delay"
        );
        assert_eq!(
            state.music.as_ref().map(|music| music.name.as_str()),
            Some("Song")
        );
    });
}

fn json(message: &Message) -> serde_json::Value {
    serde_json::from_str(message.to_text().expect("text frame")).expect("valid json")
}
