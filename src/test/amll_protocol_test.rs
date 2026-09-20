use super::*;

use crate::{backend::mpris::MediaCommand, shared::presentation::LoopStatus};

#[test]
fn serializes_the_handshake_and_heartbeat() {
    assert_eq!(
        serde_json::to_value(Message::Initialize).unwrap(),
        serde_json::json!({ "type": "initialize" })
    );
    assert_eq!(
        serde_json::to_value(Message::Pong).unwrap(),
        serde_json::json!({ "type": "pong" })
    );
}

#[test]
fn serializes_music_and_cover_updates_with_protocol_names() {
    let update = StateUpdate::SetMusic {
        music_id: "id:1",
        music_name: "Song",
        album_id: "",
        album_name: "Album",
        artists: vec![Artist {
            id: "",
            name: "Artist",
        }],
        duration: 200_000,
    };

    assert_eq!(
        serde_json::to_value(Message::State(&update)).unwrap(),
        serde_json::json!({
            "type": "state",
            "value": {
                "update": "setMusic",
                "musicId": "id:1",
                "musicName": "Song",
                "albumId": "",
                "albumName": "Album",
                "artists": [{ "id": "", "name": "Artist" }],
                "duration": 200_000,
            },
        })
    );

    let cover = StateUpdate::SetCover {
        source: "uri",
        url: "https://example.test/cover.jpg",
    };
    assert_eq!(
        serde_json::to_value(Message::State(&cover)).unwrap(),
        serde_json::json!({
            "type": "state",
            "value": {
                "update": "setCover",
                "source": "uri",
                "url": "https://example.test/cover.jpg",
            },
        })
    );
}

#[test]
fn serializes_progress_and_play_state_updates() {
    let progress = StateUpdate::Progress { progress: 4_200 };
    assert_eq!(
        serde_json::to_value(Message::State(&progress)).unwrap(),
        serde_json::json!({
            "type": "state",
            "value": { "update": "progress", "progress": 4_200 },
        })
    );

    let paused = StateUpdate::Paused;
    assert_eq!(
        serde_json::to_value(Message::State(&paused)).unwrap(),
        serde_json::json!({ "type": "state", "value": { "update": "paused" } })
    );
}

#[test]
fn encodes_binary_cover_frames_with_the_set_cover_magic() {
    let frame = cover_frame(&[0xAB, 0xCD]).expect("small payloads are encodable");

    assert_eq!(frame, vec![1, 0, 2, 0, 0, 0, 0xAB, 0xCD]);
    assert_eq!(cover_frame(&[]), Some(vec![1, 0, 0, 0, 0, 0]));
}

#[test]
fn serializes_the_volume_and_mode_updates() {
    assert_eq!(
        serde_json::to_value(Message::State(&StateUpdate::Volume { volume: 0.35 })).unwrap(),
        serde_json::json!({ "type": "state", "value": { "update": "volume", "volume": 0.35 } })
    );
    assert_eq!(
        serde_json::to_value(Message::State(&StateUpdate::ModeChanged {
            repeat: RepeatMode::One,
            shuffle: true,
        }))
        .unwrap(),
        serde_json::json!({
            "type": "state",
            "value": { "update": "modeChanged", "repeat": "one", "shuffle": true }
        })
    );
}

#[test]
fn maps_every_control_command_the_listener_can_send() {
    let commands = [
        (
            r#"{"type":"command","value":{"command":"pause"}}"#,
            MediaCommand::Pause,
        ),
        (
            r#"{"type":"command","value":{"command":"resume"}}"#,
            MediaCommand::Play,
        ),
        (
            r#"{"type":"command","value":{"command":"forwardSong"}}"#,
            MediaCommand::Next,
        ),
        (
            r#"{"type":"command","value":{"command":"backwardSong"}}"#,
            MediaCommand::Previous,
        ),
        (
            r#"{"type":"command","value":{"command":"setVolume","volume":0.5}}"#,
            MediaCommand::SetVolume(0.5),
        ),
        (
            r#"{"type":"command","value":{"command":"seekPlayProgress","progress":42000}}"#,
            MediaCommand::SeekTo(42_000),
        ),
        (
            r#"{"type":"command","value":{"command":"setRepeatMode","mode":"off"}}"#,
            MediaCommand::SetLoopStatus(LoopStatus::Off),
        ),
        (
            r#"{"type":"command","value":{"command":"setRepeatMode","mode":"all"}}"#,
            MediaCommand::SetLoopStatus(LoopStatus::Playlist),
        ),
        (
            r#"{"type":"command","value":{"command":"setRepeatMode","mode":"one"}}"#,
            MediaCommand::SetLoopStatus(LoopStatus::Track),
        ),
        (
            r#"{"type":"command","value":{"command":"setShuffleMode","enabled":true}}"#,
            MediaCommand::SetShuffle(true),
        ),
    ];

    for (message, expected) in commands {
        assert_eq!(
            incoming(message),
            Some(Incoming::Command(expected)),
            "{message}"
        );
    }
}

#[test]
fn answers_protocol_heartbeats() {
    assert_eq!(incoming(r#"{"type":"ping"}"#), Some(Incoming::Ping));
}

#[test]
fn ignores_inbound_messages_that_carry_no_command() {
    for message in [
        r#"{"type":"pong"}"#,
        r#"{"type":"state","value":{"update":"paused"}}"#,
        r#"{"type":"command","value":{"command":"teleport"}}"#,
        r#"{"type":"command","value":{"command":"setVolume"}}"#,
        r#"{"type":"command"}"#,
        "not json",
    ] {
        assert_eq!(incoming(message), None, "{message}");
    }
}
