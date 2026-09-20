// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! AMLL WebSocket connection handling.
//!
//! One task owns the socket: it connects, primes the listener with the current
//! state, streams every subsequent change, and reconnects on its own until the
//! sender is dropped.

use std::{path::Path, time::Duration};

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::{net::TcpStream, sync::mpsc, time::sleep};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

use crate::{backend::mpris::MediaCommand, shared::presentation::LyricsDocument};

use super::{
    AmllEvent, Cover, Music,
    protocol::{self, ControlUpdate, Message as ProtocolMessage, RepeatMode, StateUpdate},
    ttml,
};

/// Delay between connection attempts after a failed or closed connection.
const RECONNECT_DELAY: Duration = Duration::from_secs(2);

/// Largest local cover streamed over the binary channel.
const MAX_COVER_BYTES: u64 = 8 * 1024 * 1024;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Streams lyrics and playback state to the AMLL listener at `address`,
/// reconnecting until `events` is closed, and forwards the control commands the
/// listener sends to `commands`.
pub(super) async fn run(
    address: String,
    events: mpsc::UnboundedReceiver<AmllEvent>,
    commands: mpsc::UnboundedSender<MediaCommand>,
) {
    run_with_delay(address, events, commands, RECONNECT_DELAY).await;
}

/// [`run`] with an explicit reconnect delay, so tests can keep it short.
async fn run_with_delay(
    address: String,
    mut events: mpsc::UnboundedReceiver<AmllEvent>,
    commands: mpsc::UnboundedSender<MediaCommand>,
    reconnect_delay: Duration,
) {
    let url = format!("ws://{address}");
    let mut state = State::default();
    loop {
        match connect_async(&url).await {
            Ok((socket, _)) => {
                tracing::debug!(%address, "connected to the AMLL listener");
                if !session(socket, &mut state, &mut events, &commands, &address).await {
                    return;
                }
            }
            Err(error) => tracing::warn!(%error, %address, "could not reach the AMLL listener"),
        }
        if !wait_for_reconnect(&mut events, &mut state, reconnect_delay).await {
            return;
        }
    }
}

/// Serves one connection until it fails or the sender is dropped.
///
/// Returns `false` only when `events` is closed, which ends the task.
async fn session(
    mut socket: Socket,
    state: &mut State,
    events: &mut mpsc::UnboundedReceiver<AmllEvent>,
    commands: &mpsc::UnboundedSender<MediaCommand>,
    address: &str,
) -> bool {
    if write_message(&mut socket, &ProtocolMessage::Initialize)
        .await
        .is_err()
    {
        return true;
    }
    for change in state.priming() {
        if write_change(&mut socket, change, state).await.is_err() {
            return true;
        }
    }

    loop {
        tokio::select! {
            event = events.recv() => {
                let Some(event) = event else {
                    return false;
                };
                if let Some(change) = state.apply(event)
                    && let Err(error) = write_change(&mut socket, change, state).await
                {
                    tracing::warn!(%error, %address, "AMLL update could not be delivered");
                    return true;
                }
            }
            incoming = socket.next() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    if handle_incoming(&mut socket, &text, commands).await.is_err() {
                        tracing::warn!(%address, "AMLL reply could not be delivered");
                        return true;
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(error)) => {
                    tracing::warn!(%error, %address, "AMLL connection failed");
                    return true;
                }
                None => return true,
            },
        }
    }
}

/// Handles one inbound frame: heartbeats are answered, control commands are
/// forwarded to the MPRIS control queue, and anything else is logged.
async fn handle_incoming(
    socket: &mut Socket,
    text: &str,
    commands: &mpsc::UnboundedSender<MediaCommand>,
) -> Result<()> {
    match protocol::incoming(text) {
        Some(protocol::Incoming::Ping) => write_message(socket, &ProtocolMessage::Pong).await,
        Some(protocol::Incoming::Command(command)) => {
            if commands.send(command).is_err() {
                tracing::debug!(?command, "AMLL control command has no receiver");
            }
            Ok(())
        }
        None => {
            tracing::debug!(%text, "ignored an AMLL message");
            Ok(())
        }
    }
}

/// Keeps the latest state while the connection is down.
///
/// Returns `false` when the sender is dropped.
async fn wait_for_reconnect(
    events: &mut mpsc::UnboundedReceiver<AmllEvent>,
    state: &mut State,
    reconnect_delay: Duration,
) -> bool {
    let timer = sleep(reconnect_delay);
    tokio::pin!(timer);
    loop {
        tokio::select! {
            () = &mut timer => return true,
            event = events.recv() => match event {
                Some(event) => {
                    state.apply(event);
                }
                None => return false,
            },
        }
    }
}

async fn write_message(socket: &mut Socket, message: &ProtocolMessage<'_>) -> Result<()> {
    let json = serde_json::to_string(message)?;
    socket.send(Message::text(json)).await?;
    Ok(())
}

async fn write_state(socket: &mut Socket, update: &StateUpdate<'_>) -> Result<()> {
    write_message(socket, &ProtocolMessage::State(update)).await
}

/// Writes the protocol update `change` asks for.
///
/// The never-`None` cases read the values [`State::apply`] stored before it
/// returned the change.
async fn write_change(socket: &mut Socket, change: Change, state: &State) -> Result<()> {
    match change {
        Change::Music => {
            let Some(music) = state.music.as_ref() else {
                return Ok(());
            };
            write_state(
                socket,
                &StateUpdate::SetMusic {
                    music_id: &music.id,
                    music_name: &music.name,
                    album_id: "",
                    album_name: &music.album,
                    artists: music
                        .artists
                        .iter()
                        .map(|artist| protocol::Artist {
                            id: "",
                            name: artist,
                        })
                        .collect(),
                    duration: music.duration_ms,
                },
            )
            .await?;
            match &music.cover {
                Some(Cover::Uri(url)) => {
                    write_state(socket, &StateUpdate::SetCover { source: "uri", url }).await?;
                }
                Some(Cover::File(path)) => {
                    if let Some(frame) = cover_frame(path).await {
                        socket.send(Message::binary(frame)).await?;
                    }
                }
                None => {}
            }
        }
        Change::Lyrics => {
            let Some(document) = state.lyrics.as_ref() else {
                return Ok(());
            };
            write_state(
                socket,
                &StateUpdate::SetLyric {
                    format: "ttml",
                    data: ttml::document(document),
                },
            )
            .await?;
        }
        Change::Playback {
            position_ms,
            play_state,
        } => {
            if let Some(progress) = position_ms {
                write_state(socket, &StateUpdate::Progress { progress }).await?;
            }
            match play_state {
                Some(true) => write_state(socket, &StateUpdate::Resumed).await?,
                Some(false) => write_state(socket, &StateUpdate::Paused).await?,
                None => {}
            }
        }
        Change::Control { volume, mode } => {
            if let Some(volume) = volume {
                write_state(socket, &StateUpdate::Volume { volume }).await?;
            }
            if let Some((repeat, shuffle)) = mode {
                write_state(socket, &StateUpdate::ModeChanged { repeat, shuffle }).await?;
            }
        }
        Change::Clear => {
            write_state(
                socket,
                &StateUpdate::SetLyric {
                    format: "ttml",
                    data: ttml::document(&LyricsDocument {
                        revision: 0,
                        duration_ms: None,
                        lines: Vec::new(),
                    }),
                },
            )
            .await?;
        }
    }
    Ok(())
}

/// Reads a local cover and encodes it for the protocol's binary channel.
async fn cover_frame(path: &Path) -> Option<Vec<u8>> {
    let path = path.to_path_buf();
    let data = tokio::task::spawn_blocking(move || read_cover(&path))
        .await
        .ok()??;
    protocol::cover_frame(&data)
}

fn read_cover(path: &Path) -> Option<Vec<u8>> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_COVER_BYTES {
        tracing::debug!(path = %path.display(), "skipping an unusable AMLL cover");
        return None;
    }
    match std::fs::read(path) {
        Ok(data) => Some(data),
        Err(error) => {
            tracing::debug!(%error, path = %path.display(), "could not read an AMLL cover");
            None
        }
    }
}

/// Latest published state, retained so a new connection starts in sync.
#[derive(Debug, Default)]
struct State {
    music: Option<Music>,
    lyrics: Option<LyricsDocument>,
    playback: Option<Playback>,
    control: Option<ControlUpdate>,
    /// Whether the next playback update must be written even when it repeats
    /// the retained one.
    ///
    /// AMLL Player zeroes its playback position on every track info message, so
    /// a listener told about the track again — the billing the lyrics provider
    /// adds arrives mid-track — needs the clock restated. While playing the next
    /// update carries a new position anyway; while paused nothing else would
    /// write one, and the lyrics would sit at the top of the song.
    restate_playback: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Playback {
    position_ms: Option<u64>,
    playing: bool,
}

/// One protocol change derived from an [`AmllEvent`].
///
/// The two retained variants carry no payload: [`State::apply`] stores the value
/// before it returns the change, so the writer reads it back from the state
/// instead of copying a whole lyrics document per update.
#[derive(Debug)]
enum Change {
    /// The current track changed; the state holds its metadata and cover.
    Music,
    /// The lyrics of the current track changed; the state holds the document.
    Lyrics,
    /// Playback position and, when it changed, the play state.
    Playback {
        position_ms: Option<u64>,
        play_state: Option<bool>,
    },
    /// Volume and playback modes of the active player.
    Control {
        volume: Option<f64>,
        /// Repeat and shuffle, in the order the protocol sends them.
        mode: Option<(RepeatMode, bool)>,
    },
    /// Playback stopped, so the listener should drop the current lyrics.
    Clear,
}

/// The value of a property the listener has not been told yet.
///
/// A value it was never told counts as changed, so a listener that missed an
/// update still receives one.
fn changed<T: PartialEq>(previous: Option<T>, next: Option<T>) -> Option<T> {
    if previous == next { None } else { next }
}

impl State {
    /// Applies an event, returning the change it requires, if any.
    fn apply(&mut self, event: AmllEvent) -> Option<Change> {
        match event {
            AmllEvent::Music(None) => {
                if self.music.is_none() && self.lyrics.is_none() {
                    return None;
                }
                self.music = None;
                self.lyrics = None;
                self.playback = None;
                Some(Change::Clear)
            }
            AmllEvent::Music(Some(music)) => {
                if self.music.as_ref() == Some(&music) {
                    return None;
                }
                self.music = Some(music);
                // The listener zeroes its position on track info, so the clock
                // has to follow it even if it did not move.
                self.restate_playback = true;
                Some(Change::Music)
            }
            AmllEvent::Lyrics(document) => {
                if self
                    .lyrics
                    .as_ref()
                    .is_some_and(|current| current.revision == document.revision)
                {
                    return None;
                }
                self.lyrics = Some(document);
                Some(Change::Lyrics)
            }
            AmllEvent::Control(control) => {
                let next = ControlUpdate::from_control(control);
                if self.control == next {
                    return None;
                }
                let previous = self.control;
                self.control = next;
                // Only the halves that changed are sent, but a listener that
                // reconnects receives both from the primed state.
                let volume = changed(
                    previous.and_then(|state| state.volume),
                    next.and_then(|state| state.volume),
                );
                let mode = changed(
                    previous.and_then(|state| state.mode),
                    next.and_then(|state| state.mode),
                );
                if volume.is_none() && mode.is_none() {
                    return None;
                }
                Some(Change::Control { volume, mode })
            }
            AmllEvent::Playback {
                position_ms,
                playing,
            } => {
                let next = Playback {
                    position_ms,
                    playing,
                };
                if self.playback == Some(next) && !self.restate_playback {
                    return None;
                }
                self.restate_playback = false;
                // A listener that never received a play state would show the
                // wrong one, so the first update always carries it.
                let play_state = changed(
                    self.playback.map(|previous| previous.playing),
                    Some(playing),
                );
                self.playback = Some(next);
                if position_ms.is_none() && play_state.is_none() {
                    return None;
                }
                Some(Change::Playback {
                    position_ms,
                    play_state,
                })
            }
        }
    }

    /// Changes that reproduce the current state for a new connection.
    fn priming(&self) -> Vec<Change> {
        let mut changes = Vec::with_capacity(4);
        if self.music.is_some() {
            changes.push(Change::Music);
        }
        if self.lyrics.is_some() {
            changes.push(Change::Lyrics);
        }
        if let Some(playback) = self.playback {
            changes.push(Change::Playback {
                position_ms: playback.position_ms,
                play_state: Some(playback.playing),
            });
        }
        if let Some(control) = self.control {
            changes.push(Change::Control {
                volume: control.volume,
                mode: control.mode,
            });
        }
        changes
    }
}

#[cfg(test)]
#[path = "../../test/amll_client_test.rs"]
mod tests;
