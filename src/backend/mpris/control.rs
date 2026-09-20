// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Outbound MPRIS control.
//!
//! Playback control is AMLL-only: the commands an AMLL client sends over the
//! WebSocket queue a [`MediaCommand`] on a channel, and the watcher task — the
//! only owner of the D-Bus connection — reads the player's current state,
//! resolves the command against it, and performs the MPRIS call. Neither the
//! floating overlay nor the tray asks for playback actions. [`resolve`] holds
//! the whole decision — which MPRIS operation a command maps to and when a
//! player cannot perform it — so it is testable without a bus.

use anyhow::{Context, Result};
use tokio::sync::mpsc;
use zbus::Proxy;
use zvariant::ObjectPath;

use crate::shared::presentation::{LoopStatus, PlayerCapabilities, PlayerControl};

/// One playback action requested by the AMLL listener.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum MediaCommand {
    /// Start or resume playback.
    Play,
    /// Pause playback.
    Pause,
    /// Skip to the next track.
    Next,
    /// Skip to the previous track.
    Previous,
    /// Seek to an absolute position in the current track.
    SeekTo(u64),
    /// Set the player volume; the value is clamped to `0.0..=1.0`.
    SetVolume(f64),
    /// Set the loop mode.
    SetLoopStatus(LoopStatus),
    /// Enable or disable shuffle.
    SetShuffle(bool),
}

/// Handle used to reach the active player from the GTK thread.
///
/// Commands are applied by the watcher task, so this handle is cheap to clone
/// and safe to use from the GTK thread.
#[derive(Clone)]
pub(crate) struct MediaControlHandle {
    commands: mpsc::UnboundedSender<MediaCommand>,
}

impl MediaControlHandle {
    pub(super) fn new(commands: mpsc::UnboundedSender<MediaCommand>) -> Self {
        Self { commands }
    }

    /// Queues `command` for the active player.
    pub(crate) fn send(&self, command: MediaCommand) {
        if self.commands.send(command).is_err() {
            tracing::debug!(?command, "the MPRIS watcher is no longer running");
        }
    }
}

/// Player state a command is resolved against.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ControlContext {
    pub(super) control: PlayerControl,
    /// `mpris:trackid` of the current track, when the player reports one.
    pub(super) track_id: Option<String>,
    pub(super) position_ms: Option<u64>,
}

/// The MPRIS operation one command turns into.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Call {
    /// A no-argument method of `org.mpris.MediaPlayer2.Player`.
    Method(&'static str),
    /// The `Seek` method with a relative offset in microseconds.
    Seek(i64),
    /// The `SetPosition` method with the track id and an absolute position.
    SetPosition { track_id: String, position_us: i64 },
    /// A write to the `Volume` property.
    Volume(f64),
    /// A write to the `LoopStatus` property, with its MPRIS value.
    LoopStatus(&'static str),
    /// A write to the `Shuffle` property.
    Shuffle(bool),
}

/// Resolves `command` for a player in `context`.
///
/// Returns `None` when the player reports that it cannot perform the action.
/// Play and pause are separate methods rather than `PlayPause`, because MPRIS
/// requires `PlayPause` to fail when the player cannot pause.
pub(super) fn resolve(command: MediaCommand, context: &ControlContext) -> Option<Call> {
    let capabilities = context.control.capabilities;
    match command {
        MediaCommand::Play if capabilities.can_play => Some(Call::Method("Play")),
        MediaCommand::Pause if capabilities.can_pause => Some(Call::Method("Pause")),
        MediaCommand::Next if capabilities.can_next => Some(Call::Method("Next")),
        MediaCommand::Previous if capabilities.can_previous => Some(Call::Method("Previous")),
        MediaCommand::SeekTo(position_ms) if capabilities.can_seek => seek_to(position_ms, context),
        MediaCommand::SetVolume(volume) if capabilities.can_control => {
            Some(Call::Volume(clamp_volume(volume)))
        }
        MediaCommand::SetLoopStatus(status) if capabilities.can_control => {
            Some(Call::LoopStatus(status.as_mpris()))
        }
        MediaCommand::SetShuffle(enabled) if capabilities.can_control => {
            Some(Call::Shuffle(enabled))
        }
        _ => None,
    }
}

/// Prefers `SetPosition`, which the player validates against the current track,
/// and falls back to a relative `Seek` when the track id is unknown.
fn seek_to(position_ms: u64, context: &ControlContext) -> Option<Call> {
    let position_us = i64::try_from(position_ms).ok()?.saturating_mul(1_000);
    if let Some(track_id) = context
        .track_id
        .as_deref()
        .filter(|track_id| !track_id.is_empty())
    {
        return Some(Call::SetPosition {
            track_id: track_id.to_string(),
            position_us,
        });
    }

    let current_us = i64::try_from(context.position_ms?)
        .ok()?
        .saturating_mul(1_000);
    Some(Call::Seek(position_us.saturating_sub(current_us)))
}

/// Clamps a requested volume to the MPRIS range, rejecting non-finite values.
fn clamp_volume(volume: f64) -> f64 {
    if volume.is_finite() {
        volume.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Performs `command` on `player`.
///
/// # Errors
///
/// Returns an error when the player does not allow the action, when the track
/// id it reported is not a D-Bus object path, or when the D-Bus call fails.
pub(super) async fn apply(
    player: &Proxy<'_>,
    command: MediaCommand,
    context: &ControlContext,
) -> Result<()> {
    let Some(call) = resolve(command, context) else {
        anyhow::bail!("the active player does not allow {command:?}");
    };

    let result = match call {
        Call::Method(name) => player.call_method(name, &()).await.map(|_| ()),
        Call::Seek(offset_us) => player.call_method("Seek", &offset_us).await.map(|_| ()),
        Call::SetPosition {
            track_id,
            position_us,
        } => {
            let track_id = ObjectPath::try_from(track_id.as_str())
                .context("the reported track id is not a D-Bus object path")?;
            player
                .call_method("SetPosition", &(&track_id, position_us))
                .await
                .map(|_| ())
        }
        // A property write reports an interface error where a method call
        // reports a zbus one, so the two are brought together here.
        Call::Volume(volume) => player
            .set_property("Volume", volume)
            .await
            .map_err(zbus::Error::from),
        Call::LoopStatus(status) => player
            .set_property("LoopStatus", status)
            .await
            .map_err(zbus::Error::from),
        Call::Shuffle(enabled) => player
            .set_property("Shuffle", enabled)
            .await
            .map_err(zbus::Error::from),
    };

    result.with_context(|| format!("applying {command:?} to the active MPRIS player"))
}

/// Properties a [`PlayerControl`] is read from.
///
/// A change to any of them makes the properties-changed handler read the control
/// state again.
pub(super) const CONTROL_PROPERTIES: [&str; 9] = [
    "Volume",
    "LoopStatus",
    "Shuffle",
    "CanControl",
    "CanPlay",
    "CanPause",
    "CanSeek",
    "CanGoNext",
    "CanGoPrevious",
];

/// Reads the control properties of `player`.
///
/// A capability the player does not expose is treated as available, so a player
/// that omits the property is not disabled by it. The proxy answers one
/// `Properties.Get` per read, so every read is issued before any is awaited and
/// the watcher task waits for a single round trip instead of nine.
pub(super) async fn read_player_control(player: &Proxy<'_>) -> PlayerControl {
    let (volume, loop_status, shuffle) = tokio::join!(
        read_property(player, "Volume"),
        read_property::<String>(player, "LoopStatus"),
        read_property(player, "Shuffle"),
    );
    let (can_control, can_play, can_pause, can_seek, can_next, can_previous) = tokio::join!(
        read_flag(player, "CanControl"),
        read_flag(player, "CanPlay"),
        read_flag(player, "CanPause"),
        read_flag(player, "CanSeek"),
        read_flag(player, "CanGoNext"),
        read_flag(player, "CanGoPrevious"),
    );

    PlayerControl {
        volume,
        loop_status: loop_status.and_then(|value| LoopStatus::from_mpris(&value)),
        shuffle,
        capabilities: PlayerCapabilities {
            can_control,
            can_play,
            can_pause,
            can_seek,
            can_next,
            can_previous,
        },
    }
}

async fn read_property<T>(player: &Proxy<'_>, name: &str) -> Option<T>
where
    T: TryFrom<zvariant::OwnedValue>,
    T::Error: Into<zbus::Error>,
{
    player.get_property::<T>(name).await.ok()
}

async fn read_flag(player: &Proxy<'_>, name: &str) -> bool {
    read_property(player, name).await.unwrap_or(true)
}

impl LoopStatus {
    /// MPRIS value of this loop mode.
    fn as_mpris(self) -> &'static str {
        match self {
            Self::Off => "None",
            Self::Track => "Track",
            Self::Playlist => "Playlist",
        }
    }

    /// Parses an MPRIS `LoopStatus`, ignoring values this app does not model.
    fn from_mpris(value: &str) -> Option<Self> {
        match value {
            "None" => Some(Self::Off),
            "Track" => Some(Self::Track),
            "Playlist" => Some(Self::Playlist),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "../../test/mpris_control_test.rs"]
mod tests;
