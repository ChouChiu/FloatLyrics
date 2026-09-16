use super::*;

use crate::shared::presentation::{LoopStatus, PlayerCapabilities};

fn context() -> ControlContext {
    ControlContext {
        control: PlayerControl {
            volume: Some(0.5),
            loop_status: Some(LoopStatus::Off),
            shuffle: Some(false),
            capabilities: PlayerCapabilities::default(),
        },
        track_id: Some("/org/mpris/MediaPlayer2/Track/7".to_string()),
        position_ms: Some(10_000),
    }
}

#[test]
fn refuses_actions_the_player_reports_as_unavailable() {
    let mut state = context();
    state.control.capabilities = PlayerCapabilities {
        can_control: false,
        can_play: false,
        can_pause: false,
        can_seek: false,
        can_next: false,
        can_previous: false,
    };

    for command in [
        MediaCommand::Play,
        MediaCommand::Pause,
        MediaCommand::Next,
        MediaCommand::Previous,
        MediaCommand::SeekTo(1_000),
        MediaCommand::SetVolume(0.5),
        MediaCommand::SetLoopStatus(LoopStatus::Track),
        MediaCommand::SetShuffle(true),
    ] {
        assert_eq!(resolve(command, &state), None, "{command:?}");
    }
}

#[test]
fn skips_tracks_only_when_the_player_can() {
    let mut state = context();
    state.control.capabilities.can_next = false;

    assert_eq!(resolve(MediaCommand::Next, &state), None);
    assert_eq!(
        resolve(MediaCommand::Previous, &state),
        Some(Call::Method("Previous"))
    );
    assert_eq!(
        resolve(MediaCommand::Play, &state),
        Some(Call::Method("Play"))
    );
}

#[test]
fn seeks_with_set_position_when_the_track_id_is_known() {
    let state = context();

    assert_eq!(
        resolve(MediaCommand::SeekTo(30_000), &state),
        Some(Call::SetPosition {
            track_id: "/org/mpris/MediaPlayer2/Track/7".to_string(),
            position_us: 30_000_000,
        })
    );
}

#[test]
fn seeks_relatively_without_a_track_id() {
    let mut state = context();
    state.track_id = None;

    assert_eq!(
        resolve(MediaCommand::SeekTo(15_000), &state),
        Some(Call::Seek(5_000_000))
    );

    state.position_ms = None;
    assert_eq!(resolve(MediaCommand::SeekTo(15_000), &state), None);
}

#[test]
fn clamps_the_requested_volume_to_the_mpris_range() {
    let state = context();

    assert_eq!(
        resolve(MediaCommand::SetVolume(1.5), &state),
        Some(Call::Volume(1.0))
    );
    assert_eq!(
        resolve(MediaCommand::SetVolume(-0.5), &state),
        Some(Call::Volume(0.0))
    );
    assert_eq!(
        resolve(MediaCommand::SetVolume(f64::NAN), &state),
        Some(Call::Volume(0.0))
    );
}

#[test]
fn maps_playback_modes_to_their_mpris_values() {
    let state = context();

    for (mode, expected) in [
        (LoopStatus::Off, "None"),
        (LoopStatus::Track, "Track"),
        (LoopStatus::Playlist, "Playlist"),
    ] {
        assert_eq!(
            resolve(MediaCommand::SetLoopStatus(mode), &state),
            Some(Call::LoopStatus(expected))
        );
        assert_eq!(LoopStatus::from_mpris(expected), Some(mode));
    }

    assert_eq!(LoopStatus::from_mpris("Shuffle"), None);
    assert_eq!(
        resolve(MediaCommand::SetShuffle(true), &state),
        Some(Call::Shuffle(true))
    );
}
