// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Wire types for the AMLL WebSocket protocol V2.
//!
//! The state updates a playback source publishes to a lyrics player, and the
//! control commands a lyrics player sends back. Field names and shapes follow
//! <https://github.com/amll-dev/ws-protocol/blob/main/PROTOCOL_V2.md>; the
//! binary channel uses the little-endian layout described there.

use serde::{Deserialize, Serialize};

use crate::{
    backend::mpris::MediaCommand,
    shared::presentation::{LoopStatus, PlayerControl},
};

/// Magic number of the `SetCoverData` binary message.
const COVER_DATA_MAGIC: u16 = 1;

/// One message sent to the AMLL listener.
#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub(super) enum Message<'a> {
    /// Handshake required as the first message of a V2 connection.
    Initialize,
    /// Answer to an AMLL `ping`.
    Pong,
    /// Playback state update.
    State(&'a StateUpdate<'a>),
}

/// Playback state update carried by [`Message::State`].
#[derive(Debug, Serialize)]
#[serde(tag = "update", rename_all = "camelCase")]
pub(super) enum StateUpdate<'a> {
    SetMusic {
        #[serde(rename = "musicId")]
        music_id: &'a str,
        #[serde(rename = "musicName")]
        music_name: &'a str,
        #[serde(rename = "albumId")]
        album_id: &'a str,
        #[serde(rename = "albumName")]
        album_name: &'a str,
        artists: Vec<Artist<'a>>,
        duration: u64,
    },
    SetCover {
        source: &'static str,
        url: &'a str,
    },
    SetLyric {
        format: &'static str,
        data: String,
    },
    Progress {
        progress: u64,
    },
    Paused,
    Resumed,
    Volume {
        volume: f64,
    },
    #[serde(rename = "modeChanged")]
    ModeChanged {
        repeat: RepeatMode,
        shuffle: bool,
    },
}

/// Loop mode as the protocol spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum RepeatMode {
    Off,
    All,
    One,
}

/// Control command sent by the listener.
#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "camelCase")]
enum Command {
    Pause,
    Resume,
    ForwardSong,
    BackwardSong,
    SetVolume { volume: f64 },
    SeekPlayProgress { progress: u64 },
    SetRepeatMode { mode: RepeatMode },
    SetShuffleMode { enabled: bool },
}

/// One frame the listener sent.
#[derive(Debug, PartialEq)]
pub(super) enum Incoming {
    /// Heartbeat, which the connection answers with [`Message::Pong`].
    Ping,
    /// Playback control command.
    Command(MediaCommand),
}

/// Parses one inbound frame, if it carries something the sender acts on.
///
/// Returns `None` for the message types this sender does not act on, for a
/// command it cannot map onto MPRIS, and for anything unparseable.
pub(super) fn incoming(text: &str) -> Option<Incoming> {
    let mut value = serde_json::from_str::<serde_json::Value>(text).ok()?;
    match value.get("type")?.as_str()? {
        "ping" => Some(Incoming::Ping),
        "command" => match serde_json::from_value(value.get_mut("value")?.take()) {
            Ok(command) => Some(Incoming::Command(map_command(command))),
            Err(_) => None,
        },
        _ => None,
    }
}

/// Maps one control command onto the MPRIS operation it asks for.
fn map_command(command: Command) -> MediaCommand {
    match command {
        Command::Pause => MediaCommand::Pause,
        Command::Resume => MediaCommand::Play,
        Command::ForwardSong => MediaCommand::Next,
        Command::BackwardSong => MediaCommand::Previous,
        Command::SetVolume { volume } => MediaCommand::SetVolume(volume),
        Command::SeekPlayProgress { progress } => MediaCommand::SeekTo(progress),
        Command::SetRepeatMode { mode } => MediaCommand::SetLoopStatus(match mode {
            RepeatMode::Off => LoopStatus::Off,
            RepeatMode::All => LoopStatus::Playlist,
            RepeatMode::One => LoopStatus::Track,
        }),
        Command::SetShuffleMode { enabled } => MediaCommand::SetShuffle(enabled),
    }
}

/// Loop mode the protocol expects for `status`.
pub(super) fn repeat_mode(status: LoopStatus) -> RepeatMode {
    match status {
        LoopStatus::Off => RepeatMode::Off,
        LoopStatus::Track => RepeatMode::One,
        LoopStatus::Playlist => RepeatMode::All,
    }
}

/// Playback state the protocol's `volume` and `modeChanged` updates need.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ControlUpdate {
    pub(super) volume: Option<f64>,
    /// Repeat and shuffle mode.
    ///
    /// The protocol reports both in one `modeChanged` message, so the pair is
    /// only known once the player described both properties.
    pub(super) mode: Option<(RepeatMode, bool)>,
}

impl ControlUpdate {
    /// Reads the protocol updates out of a published player control state.
    pub(super) fn from_control(control: Option<PlayerControl>) -> Option<Self> {
        let control = control?;
        Some(Self {
            volume: control.volume,
            mode: control
                .loop_status
                .zip(control.shuffle)
                .map(|(status, shuffle)| (repeat_mode(status), shuffle)),
        })
    }
}

/// One performer credited for the current track.
#[derive(Debug, Serialize)]
pub(super) struct Artist<'a> {
    pub(super) id: &'a str,
    pub(super) name: &'a str,
}

/// Encodes a binary `SetCoverData` frame, or `None` when the payload cannot
/// be described by its 32-bit length field.
pub(super) fn cover_frame(data: &[u8]) -> Option<Vec<u8>> {
    let size = u32::try_from(data.len()).ok()?;
    let mut frame = Vec::with_capacity(6 + data.len());
    frame.extend_from_slice(&COVER_DATA_MAGIC.to_le_bytes());
    frame.extend_from_slice(&size.to_le_bytes());
    frame.extend_from_slice(data);
    Some(frame)
}

#[cfg(test)]
#[path = "../../test/amll_protocol_test.rs"]
mod tests;
