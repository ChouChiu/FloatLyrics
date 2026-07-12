// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Asynchronous discovery and observation of MPRIS player instances.

use anyhow::{Context, Result};
use futures_util::StreamExt;
use std::{
    collections::HashMap,
    sync::mpsc::Sender,
    time::{Duration, Instant},
};
use zbus::{
    Connection, Proxy,
    fdo::{DBusProxy, PropertiesProxy},
    proxy::CacheProperties,
};
use zvariant::OwnedValue;

use super::{
    model::{PlaybackStatus, SpotifyPlayerState, SpotifyWatcherEvent, spotify_metadata_from_mpris},
    position::{player_track_identity, position_us_to_ms},
};

/// Default D-Bus well-known-name prefix used by Spotify for Linux.
pub const SPOTIFY_MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.spotify";
const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER_IFACE: &str = "org.mpris.MediaPlayer2.Player";
const PLAYBACK_POSITION_POLL_INTERVAL: Duration = Duration::from_millis(250);
const PLAYER_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(30);
const PLAYER_RECONNECT_DELAY: Duration = Duration::from_secs(1);

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

async fn mpris_names_with_prefix(
    connection: &Connection,
    prefix: &str,
) -> zbus::Result<Vec<String>> {
    let proxy = DBusProxy::new(connection).await?;
    let names = proxy.list_names().await?;

    Ok(names
        .into_iter()
        .map(|name| name.to_string())
        .filter(|name| is_mpris_name_with_prefix(name, prefix))
        .collect())
}

fn is_mpris_name_with_prefix(name: &str, prefix: &str) -> bool {
    name == prefix
        || name
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

/// Spawns an MPRIS watcher using [`SPOTIFY_MPRIS_PREFIX`].
///
/// Events are delivered on `sender`; fatal background errors become
/// [`SpotifyWatcherEvent::Error`].
pub fn spawn_spotify_watcher(
    runtime: &tokio::runtime::Handle,
    sender: Sender<SpotifyWatcherEvent>,
) {
    spawn_spotify_watcher_with_prefix(runtime, sender, SPOTIFY_MPRIS_PREFIX.to_string());
}

/// Spawns an MPRIS watcher for player names matching `mpris_prefix`.
///
/// This extension point supports Spotify variants or compatible players without
/// coupling the application controller to D-Bus.
pub fn spawn_spotify_watcher_with_prefix(
    runtime: &tokio::runtime::Handle,
    sender: Sender<SpotifyWatcherEvent>,
    mpris_prefix: String,
) {
    runtime.spawn(async move {
        if let Err(error) = watch_spotify(sender.clone(), mpris_prefix).await {
            let _ = sender.send(SpotifyWatcherEvent::Error(error.to_string()));
        }
    });
}

async fn watch_spotify(
    sender: Sender<SpotifyWatcherEvent>,
    configured_prefix: String,
) -> Result<()> {
    let connection = Connection::session()
        .await
        .context("connecting to session D-Bus")?;
    let prefix = configured_prefix.trim();
    let prefix = if prefix.is_empty() {
        SPOTIFY_MPRIS_PREFIX
    } else {
        prefix
    };

    loop {
        let names = mpris_names_with_prefix(&connection, prefix)
            .await
            .context("listing MPRIS names")?;

        if let Some(name) = names.into_iter().next() {
            if let Err(error) = watch_player(&connection, name.clone(), &sender).await {
                let _ = sender.send(SpotifyWatcherEvent::Error(format!(
                    "Spotify listener reset: {error}"
                )));
                tokio::time::sleep(PLAYER_RECONNECT_DELAY).await;
            }
        } else {
            let _ = sender.send(SpotifyWatcherEvent::Disconnected);
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
}

async fn watch_player(
    connection: &Connection,
    bus_name: String,
    sender: &Sender<SpotifyWatcherEvent>,
) -> Result<()> {
    let player = player_proxy(connection, &bus_name).await?;
    let properties = PropertiesProxy::builder(connection)
        .destination(bus_name.as_str())?
        .path(MPRIS_PATH)?
        .build()
        .await?;
    let mut changes = properties.receive_properties_changed().await?;
    let mut seeked = player.receive_signal("Seeked").await?;

    let state = read_player_state(&player, &bus_name).await?;
    let _ = sender.send(SpotifyWatcherEvent::Connected(state.clone()));

    let mut position_poll = tokio::time::interval(PLAYBACK_POSITION_POLL_INTERVAL);
    position_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    position_poll.tick().await;
    let mut health_check = tokio::time::interval(PLAYER_HEALTH_CHECK_INTERVAL);
    health_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    health_check.tick().await;

    loop {
        tokio::select! {
            changed = changes.next() => {
                let Some(changed) = changed else {
                    let _ = sender.send(SpotifyWatcherEvent::Disconnected);
                    return Ok(());
                };

                let args = changed.args()?;
                if args.interface_name().as_str() != PLAYER_IFACE {
                    continue;
                }

                let changed_properties = args.changed_properties();
                let invalidated_properties = args.invalidated_properties();
                let player_changed = ["Metadata", "PlaybackStatus", "Position"].iter().any(|property| {
                    changed_properties.contains_key(*property)
                        || invalidated_properties.contains(property)
                });

                if player_changed {
                    let state = read_player_state(&player, &bus_name).await?;
                    let _ = sender.send(SpotifyWatcherEvent::Updated(state));
                }
            }
            signal = seeked.next() => {
                let Some(signal) = signal else {
                    let _ = sender.send(SpotifyWatcherEvent::Disconnected);
                    return Ok(());
                };

                if let Some(position_ms) = seeked_position_ms(&signal) {
                    // Re-read full state so the event carries the current track metadata.
                    let mut state = read_player_state(&player, &bus_name).await?;
                    state.position_ms = Some(position_ms);
                    let _ = sender.send(SpotifyWatcherEvent::Updated(state));
                }
            }
            _ = position_poll.tick() => {
                if let Some(position_ms) = read_player_position(&player).await {
                    let sampled_at = Instant::now();
                    // Re-read full state on each poll so track_identity stays current
                    // even when the D-Bus properties-changed signal is delayed.
                    let state = read_player_state(&player, &bus_name).await?;
                    let _ = sender.send(SpotifyWatcherEvent::PositionUpdated {
                        track_identity: player_track_identity(&state),
                        position_ms,
                        sampled_at,
                    });
                }
            }
            _ = health_check.tick() => {
                match read_player_state(&player, &bus_name).await {
                    Ok(state) => {
                        let _ = sender.send(SpotifyWatcherEvent::Updated(state));
                    }
                    Err(_) => {
                        let _ = sender.send(SpotifyWatcherEvent::Disconnected);
                        return Ok(());
                    }
                }
            }
        }
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

async fn read_player_state(player: &Proxy<'_>, bus_name: &str) -> Result<SpotifyPlayerState> {
    let metadata = player
        .get_property::<HashMap<String, OwnedValue>>("Metadata")
        .await
        .unwrap_or_default();
    let playback_status = player
        .get_property::<String>("PlaybackStatus")
        .await
        .unwrap_or_else(|_| "Stopped".to_string());
    let position_us = player.get_property::<i64>("Position").await.ok();

    let track = spotify_metadata_from_mpris(&metadata)
        .and_then(|metadata| metadata.into_track_metadata().ok());

    Ok(SpotifyPlayerState {
        bus_name: bus_name.to_string(),
        playback_status: PlaybackStatus::from(playback_status.as_str()),
        position_ms: position_us.and_then(position_us_to_ms),
        track,
    })
}

async fn read_player_position(player: &Proxy<'_>) -> Option<u64> {
    player
        .get_property::<i64>("Position")
        .await
        .ok()
        .and_then(position_us_to_ms)
}

#[cfg(test)]
#[path = "../test/watcher_test.rs"]
mod tests;
