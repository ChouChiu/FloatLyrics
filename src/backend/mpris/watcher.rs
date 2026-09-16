// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Discovery, selection, and observation of MPRIS player instances.

use anyhow::{Context, Result};
use floatlyrics_lyrics::lyrics::LyricsLookupHint;
use futures_util::StreamExt;
use std::{
    collections::HashMap,
    sync::mpsc::Sender,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use zbus::{
    Connection, Proxy,
    fdo::{DBusProxy, PropertiesProxy},
    proxy::CacheProperties,
};
use zvariant::OwnedValue;

use crate::shared::presentation::PlayerControl;

use super::{
    compat::lyrics_lookup_hint,
    control::{self, ControlContext, MediaCommand, MediaControlHandle},
    model::{
        PlaybackStatus, PlayerState, PlayerWatcherEvent, metadata_from_mpris, source_url_from_mpris,
    },
    position::{player_track_identity, position_us_to_ms},
};

/// Prefix shared by all standard MPRIS well-known bus names.
pub const MPRIS_BUS_PREFIX: &str = "org.mpris.MediaPlayer2.";
/// Default D-Bus well-known-name prefix used by Spotify for Linux.
pub const SPOTIFY_MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.spotify";
const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
const ROOT_IFACE: &str = "org.mpris.MediaPlayer2";
const PLAYER_IFACE: &str = "org.mpris.MediaPlayer2.Player";
const PLAYBACK_POSITION_POLL_INTERVAL: Duration = Duration::from_millis(250);
const PLAYER_SELECTION_INTERVAL: Duration = Duration::from_secs(2);
const PLAYER_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(30);
const PLAYER_RECONNECT_DELAY: Duration = Duration::from_secs(1);

/// Preferences used to choose one player when several MPRIS instances exist.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerSelection {
    /// Bus-name suffixes, complete bus names, or MPRIS identities preferred
    /// when candidates have the same playback status.
    pub preferred_players: Vec<String>,
    /// Bus-name suffixes, complete bus names, or MPRIS identities excluded
    /// from automatic selection.
    pub ignored_players: Vec<String>,
    /// Optional complete bus-name prefixes restricting discovery.
    ///
    /// An empty list accepts every standard MPRIS player.
    pub allowed_bus_prefixes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlayerLyricsHintEvent {
    pub(crate) bus_name: String,
    pub(crate) track_fingerprint: Option<String>,
    pub(crate) hint: Option<LyricsLookupHint>,
}

#[derive(Debug, Clone)]
struct PlayerCandidate {
    bus_name: String,
    identity: String,
    playback_status: PlaybackStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayerWatchExit {
    Disconnected,
    Switch,
}

struct PlayerObservation {
    state: PlayerState,
    lyrics_hint: Option<LyricsLookupHint>,
}

/// Returns whether `name` is the Spotify MPRIS name or one of its instances.
pub fn is_spotify_mpris_name(name: &str) -> bool {
    is_mpris_name_with_prefix(name, SPOTIFY_MPRIS_PREFIX)
}

/// Lists Spotify MPRIS instances currently registered on `connection`.
///
/// # Errors
/// Returns a D-Bus error when names cannot be queried.
pub async fn spotify_mpris_names(connection: &Connection) -> zbus::Result<Vec<String>> {
    mpris_names_with_prefix(connection, SPOTIFY_MPRIS_PREFIX).await
}

/// Lists all standard MPRIS player names currently registered on `connection`.
///
/// # Errors
/// Returns a D-Bus error when names cannot be queried.
pub async fn mpris_player_names(connection: &Connection) -> zbus::Result<Vec<String>> {
    let proxy = DBusProxy::new(connection).await?;
    let names = proxy.list_names().await?;
    Ok(names
        .into_iter()
        .map(|name| name.to_string())
        .filter(|name| {
            name.strip_prefix(MPRIS_BUS_PREFIX)
                .is_some_and(|suffix| !suffix.is_empty())
        })
        .collect())
}

async fn mpris_names_with_prefix(
    connection: &Connection,
    prefix: &str,
) -> zbus::Result<Vec<String>> {
    Ok(mpris_player_names(connection)
        .await?
        .into_iter()
        .filter(|name| is_mpris_name_with_prefix(name, prefix))
        .collect())
}

fn is_mpris_name_with_prefix(name: &str, prefix: &str) -> bool {
    name == prefix
        || name
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

/// Spawns a watcher that automatically follows the most likely active MPRIS
/// player.
///
/// Events are delivered on `sender`; fatal background errors become
/// [`PlayerWatcherEvent::Error`]. Player hints inferred from exact metadata are
/// delivered on `hint_sender`.
pub(crate) fn spawn_player_watcher(
    runtime: &tokio::runtime::Handle,
    sender: Sender<PlayerWatcherEvent>,
    hint_sender: Sender<PlayerLyricsHintEvent>,
    selection: PlayerSelection,
) -> MediaControlHandle {
    let (commands, receiver) = mpsc::unbounded_channel();
    runtime.spawn(async move {
        if let Err(error) =
            watch_players(sender.clone(), &hint_sender, &selection, Some(receiver)).await
        {
            let _ = sender.send(PlayerWatcherEvent::Error(error.to_string()));
        }
    });
    MediaControlHandle::new(commands)
}

async fn watch_players(
    sender: Sender<PlayerWatcherEvent>,
    hint_sender: &Sender<PlayerLyricsHintEvent>,
    selection: &PlayerSelection,
    mut commands: Option<mpsc::UnboundedReceiver<MediaCommand>>,
) -> Result<()> {
    let connection = Connection::session()
        .await
        .context("connecting to session D-Bus")?;
    let mut disconnected_announced = false;

    loop {
        let selected = select_player_name(&connection, selection, None)
            .await
            .context("selecting an MPRIS player")?;
        let Some(bus_name) = selected else {
            if !disconnected_announced {
                let _ = sender.send(PlayerWatcherEvent::Disconnected);
                disconnected_announced = true;
            }
            tokio::time::sleep(PLAYER_SELECTION_INTERVAL).await;
            continue;
        };

        disconnected_announced = false;
        match watch_player(
            &connection,
            bus_name,
            &sender,
            hint_sender,
            selection,
            &mut commands,
        )
        .await
        {
            Ok(PlayerWatchExit::Switch) => {}
            Ok(PlayerWatchExit::Disconnected) => {
                let _ = sender.send(PlayerWatcherEvent::Disconnected);
                disconnected_announced = true;
                tokio::time::sleep(PLAYER_RECONNECT_DELAY).await;
            }
            Err(error) => {
                let _ = sender.send(PlayerWatcherEvent::Error(format!(
                    "MPRIS listener reset: {error}"
                )));
                tokio::time::sleep(PLAYER_RECONNECT_DELAY).await;
            }
        }
    }
}

async fn select_player_name(
    connection: &Connection,
    selection: &PlayerSelection,
    current_bus_name: Option<&str>,
) -> Result<Option<String>> {
    let names = mpris_player_names(connection)
        .await
        .context("listing MPRIS names")?;
    let mut candidates = Vec::new();
    for name in names {
        if !selection.allowed_bus_prefixes.is_empty()
            && !selection
                .allowed_bus_prefixes
                .iter()
                .any(|prefix| is_mpris_name_with_prefix(&name, prefix))
        {
            continue;
        }
        if let Ok(candidate) = player_candidate(connection, name).await
            && !matches_any_selector(&candidate, &selection.ignored_players)
        {
            candidates.push(candidate);
        }
    }

    Ok(choose_player(candidates, selection, current_bus_name).map(|candidate| candidate.bus_name))
}

fn choose_player(
    candidates: Vec<PlayerCandidate>,
    selection: &PlayerSelection,
    current_bus_name: Option<&str>,
) -> Option<PlayerCandidate> {
    candidates.into_iter().min_by_key(|candidate| {
        (
            playback_rank(candidate.playback_status),
            u8::from(!matches_any_selector(
                candidate,
                &selection.preferred_players,
            )),
            u8::from(current_bus_name != Some(candidate.bus_name.as_str())),
            candidate.bus_name.to_ascii_lowercase(),
        )
    })
}

fn playback_rank(status: PlaybackStatus) -> u8 {
    match status {
        PlaybackStatus::Playing => 0,
        PlaybackStatus::Paused => 1,
        PlaybackStatus::Stopped => 2,
    }
}

fn matches_any_selector(candidate: &PlayerCandidate, selectors: &[String]) -> bool {
    selectors
        .iter()
        .any(|selector| selector_matches(candidate, selector))
}

fn selector_matches(candidate: &PlayerCandidate, selector: &str) -> bool {
    let selector = selector.trim().to_ascii_lowercase();
    if selector.is_empty() {
        return false;
    }
    let bus_name = candidate.bus_name.to_ascii_lowercase();
    let bus_suffix = candidate
        .bus_name
        .strip_prefix(MPRIS_BUS_PREFIX)
        .unwrap_or(&candidate.bus_name)
        .to_ascii_lowercase();
    let identity = candidate.identity.trim().to_ascii_lowercase();

    bus_name == selector
        || bus_name
            .strip_prefix(&selector)
            .is_some_and(|suffix| suffix.starts_with('.'))
        || bus_suffix == selector
        || bus_suffix
            .strip_prefix(&selector)
            .is_some_and(|suffix| suffix.starts_with('.'))
        || identity == selector
}

async fn player_candidate(connection: &Connection, bus_name: String) -> Result<PlayerCandidate> {
    let player = player_proxy(connection, &bus_name).await?;
    let playback_status = read_playback_status(&player).await?;
    let identity = player_identity(connection, &bus_name).await;
    Ok(PlayerCandidate {
        bus_name,
        identity,
        playback_status,
    })
}

async fn player_identity(connection: &Connection, bus_name: &str) -> String {
    match root_proxy(connection, bus_name).await {
        Ok(proxy) => proxy.get_property::<String>("Identity").await.ok(),
        Err(_) => None,
    }
    .unwrap_or_else(|| {
        bus_name
            .strip_prefix(MPRIS_BUS_PREFIX)
            .unwrap_or(bus_name)
            .to_string()
    })
}

async fn watch_player(
    connection: &Connection,
    bus_name: String,
    sender: &Sender<PlayerWatcherEvent>,
    hint_sender: &Sender<PlayerLyricsHintEvent>,
    selection: &PlayerSelection,
    commands: &mut Option<mpsc::UnboundedReceiver<MediaCommand>>,
) -> Result<PlayerWatchExit> {
    let player = player_proxy(connection, &bus_name).await?;
    let properties = PropertiesProxy::builder(connection)
        .destination(bus_name.as_str())?
        .path(MPRIS_PATH)?
        .build()
        .await?;
    let mut changes = properties.receive_properties_changed().await?;
    let mut seeked = player.receive_signal("Seeked").await?;
    let identity = player_identity(connection, &bus_name).await;

    let mut control = control::read_player_control(&player).await;
    let observation = read_player_state(&player, &bus_name, &identity, &control).await?;
    // Identity of the observed track. The 250 ms position poll reuses it instead
    // of reading the metadata and re-deriving it four times a second.
    let mut track_identity = player_track_identity(&observation.state);
    let _ = sender.send(PlayerWatcherEvent::Connected(observation.state.clone()));
    send_hint(hint_sender, &observation);

    let mut position_poll = tokio::time::interval(PLAYBACK_POSITION_POLL_INTERVAL);
    position_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    position_poll.tick().await;
    let mut selection_poll = tokio::time::interval(PLAYER_SELECTION_INTERVAL);
    selection_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    selection_poll.tick().await;
    let mut health_check = tokio::time::interval(PLAYER_HEALTH_CHECK_INTERVAL);
    health_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    health_check.tick().await;

    loop {
        tokio::select! {
            command = next_command(commands) => {
                let Some(command) = command else {
                    // Every control handle is gone; stop watching the queue.
                    *commands = None;
                    continue;
                };
                let observation = read_player_state(&player, &bus_name, &identity, &control).await
                    .context("reading the player before applying a control command")?;
                let context = control_context(&observation.state);
                match control::apply(&player, command, &context).await {
                    Ok(()) => tracing::debug!(?command, "applied an MPRIS control command"),
                    Err(error) => {
                        tracing::debug!(%error, ?command, "MPRIS control command was not applied");
                    }
                }
            }
            changed = changes.next() => {
                let Some(changed) = changed else {
                    return Ok(PlayerWatchExit::Disconnected);
                };

                let args = changed.args()?;
                if args.interface_name().as_str() != PLAYER_IFACE {
                    continue;
                }

                let changed_properties = args.changed_properties();
                let invalidated_properties = args.invalidated_properties();
                let was_changed = |property: &str| {
                    changed_properties.contains_key(property)
                        || invalidated_properties.contains(&property)
                };
                let control_changed = control::CONTROL_PROPERTIES
                    .iter()
                    .any(|property| was_changed(property));
                let player_changed = ["Metadata", "PlaybackStatus", "Position"]
                    .iter()
                    .any(|property| was_changed(property));

                if control_changed || player_changed {
                    if control_changed {
                        control = control::read_player_control(&player).await;
                    }
                    let observation =
                        read_player_state(&player, &bus_name, &identity, &control).await?;
                    track_identity = player_track_identity(&observation.state);
                    let _ = sender.send(PlayerWatcherEvent::Updated(observation.state.clone()));
                    send_hint(hint_sender, &observation);
                }
            }
            signal = seeked.next() => {
                let Some(signal) = signal else {
                    return Ok(PlayerWatchExit::Disconnected);
                };

                if let Some(position_ms) = seeked_position_ms(&signal) {
                    let mut observation =
                        read_player_state(&player, &bus_name, &identity, &control).await?;
                    track_identity = player_track_identity(&observation.state);
                    observation.state.position_ms = Some(position_ms);
                    let _ = sender.send(PlayerWatcherEvent::Updated(observation.state.clone()));
                    send_hint(hint_sender, &observation);
                }
            }
            _ = position_poll.tick() => {
                if let Some(position_ms) = read_player_position(&player).await {
                    let _ = sender.send(PlayerWatcherEvent::PositionUpdated {
                        track_identity: track_identity.clone(),
                        position_ms,
                        sampled_at: Instant::now(),
                    });
                }
            }
            _ = selection_poll.tick() => {
                match select_player_name(connection, selection, Some(&bus_name)).await? {
                    Some(selected) if selected != bus_name => {
                        return Ok(PlayerWatchExit::Switch);
                    }
                    Some(_) => {}
                    None => return Ok(PlayerWatchExit::Disconnected),
                }
            }
            _ = health_check.tick() => {
                control = control::read_player_control(&player).await;
                match read_player_state(&player, &bus_name, &identity, &control).await {
                    Ok(observation) => {
                        track_identity = player_track_identity(&observation.state);
                        let _ = sender.send(PlayerWatcherEvent::Updated(observation.state.clone()));
                        send_hint(hint_sender, &observation);
                    }
                    Err(_) => return Ok(PlayerWatchExit::Disconnected),
                }
            }
        }
    }
}

/// Waits for the next control command.
///
/// Never completes once the last control handle is dropped, so the caller's
/// select loop keeps running without spinning on a closed channel.
async fn next_command(
    commands: &mut Option<mpsc::UnboundedReceiver<MediaCommand>>,
) -> Option<MediaCommand> {
    match commands {
        Some(commands) => commands.recv().await,
        None => std::future::pending().await,
    }
}

/// Extracts the player state a control command is resolved against.
fn control_context(state: &PlayerState) -> ControlContext {
    ControlContext {
        control: state.control,
        track_id: state
            .track
            .as_ref()
            .and_then(|track| track.mpris_track_id.clone()),
        position_ms: state.position_ms,
    }
}

fn seeked_position_ms(signal: &zbus::Message) -> Option<u64> {
    let position_us = signal.body().deserialize::<i64>().ok()?;
    position_us_to_ms(position_us)
}

async fn player_proxy<'a>(
    connection: &'a Connection,
    bus_name: &'a str,
) -> zbus::Result<Proxy<'a>> {
    zbus::proxy::Builder::<Proxy<'a>>::new(connection)
        .destination(bus_name)?
        .path(MPRIS_PATH)?
        .interface(PLAYER_IFACE)?
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

async fn root_proxy<'a>(connection: &'a Connection, bus_name: &'a str) -> zbus::Result<Proxy<'a>> {
    zbus::proxy::Builder::<Proxy<'a>>::new(connection)
        .destination(bus_name)?
        .path(MPRIS_PATH)?
        .interface(ROOT_IFACE)?
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

async fn read_playback_status(player: &Proxy<'_>) -> Result<PlaybackStatus> {
    let status = player
        .get_property::<String>("PlaybackStatus")
        .await
        .context("reading MPRIS playback status")?;
    PlaybackStatus::try_from(status.as_str())
}

async fn read_player_state(
    player: &Proxy<'_>,
    bus_name: &str,
    identity: &str,
    control: &PlayerControl,
) -> Result<PlayerObservation> {
    let metadata = player
        .get_property::<HashMap<String, OwnedValue>>("Metadata")
        .await
        .context("reading MPRIS metadata")?;
    let playback_status = read_playback_status(player).await?;
    let position_us = player.get_property::<i64>("Position").await.ok();

    let source_url = source_url_from_mpris(&metadata);
    let metadata = metadata_from_mpris(&metadata);
    let lyrics_hint = metadata.as_ref().and_then(|metadata| {
        lyrics_lookup_hint(identity, bus_name, metadata, source_url.as_deref())
    });
    let track = metadata.and_then(|metadata| metadata.into_track_metadata().ok());

    Ok(PlayerObservation {
        state: PlayerState {
            bus_name: bus_name.to_string(),
            playback_status,
            position_ms: position_us.and_then(position_us_to_ms),
            track,
            control: *control,
        },
        lyrics_hint,
    })
}

fn send_hint(sender: &Sender<PlayerLyricsHintEvent>, observation: &PlayerObservation) {
    let _ = sender.send(PlayerLyricsHintEvent {
        bus_name: observation.state.bus_name.clone(),
        track_fingerprint: observation
            .state
            .track
            .as_ref()
            .map(|track| track.fingerprint()),
        hint: observation.lyrics_hint.clone(),
    });
}

async fn read_player_position(player: &Proxy<'_>) -> Option<u64> {
    player
        .get_property::<i64>("Position")
        .await
        .ok()
        .and_then(position_us_to_ms)
}

#[cfg(test)]
#[path = "../../test/watcher_test.rs"]
mod tests;
