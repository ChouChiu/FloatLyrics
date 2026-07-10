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
    position::{TrackPositionSync, player_track_identity, position_us_to_ms},
};

pub const SPOTIFY_MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.spotify";
const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER_IFACE: &str = "org.mpris.MediaPlayer2.Player";
const PLAYBACK_POSITION_POLL_INTERVAL: Duration = Duration::from_millis(250);
const PLAYER_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(30);

pub fn is_spotify_mpris_name(name: &str) -> bool {
    name == SPOTIFY_MPRIS_PREFIX || name.starts_with("org.mpris.MediaPlayer2.spotify.")
}

pub async fn spotify_mpris_names(connection: &Connection) -> zbus::Result<Vec<String>> {
    let proxy = DBusProxy::new(connection).await?;
    let names = proxy.list_names().await?;

    Ok(names
        .into_iter()
        .map(|name| name.to_string())
        .filter(|name| is_spotify_mpris_name(name))
        .collect())
}

pub fn spawn_spotify_watcher(
    runtime: &tokio::runtime::Handle,
    sender: Sender<SpotifyWatcherEvent>,
) {
    runtime.spawn(async move {
        if let Err(error) = watch_spotify(sender.clone()).await {
            let _ = sender.send(SpotifyWatcherEvent::Error(error.to_string()));
        }
    });
}

async fn watch_spotify(sender: Sender<SpotifyWatcherEvent>) -> Result<()> {
    let connection = Connection::session()
        .await
        .context("connecting to session D-Bus")?;

    loop {
        let names = spotify_mpris_names(&connection)
            .await
            .context("listing MPRIS names")?;

        if let Some(name) = names.into_iter().next() {
            if let Err(error) = watch_player(&connection, name.clone(), &sender).await {
                let _ = sender.send(SpotifyWatcherEvent::Error(format!(
                    "Spotify listener reset: {error}"
                )));
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

    let mut state = read_player_state(&player, &bus_name).await?;
    let mut position_sync = TrackPositionSync::new(&state, Instant::now());
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
                    let mut next_state = read_player_state(&player, &bus_name).await?;
                    let observed_at = Instant::now();
                    let track_changed = position_sync.observe_track(&next_state, observed_at);
                    if !position_sync.accepts(
                        next_state.position_ms,
                        &next_state.playback_status,
                        observed_at,
                    ) {
                        next_state.position_ms = if track_changed {
                            Some(0)
                        } else {
                            position_sync
                                .estimated_position(observed_at)
                                .or(state.position_ms)
                        };
                    }
                    state = next_state;
                    let _ = sender.send(SpotifyWatcherEvent::Updated(state.clone()));
                }
            }
            signal = seeked.next() => {
                let Some(signal) = signal else {
                    let _ = sender.send(SpotifyWatcherEvent::Disconnected);
                    return Ok(());
                };

                if let Some(position_ms) = seeked_position_ms(&signal) {
                    position_sync.trust_position();
                    state.position_ms = Some(position_ms);
                    let _ = sender.send(SpotifyWatcherEvent::Updated(state.clone()));
                }
            }
            _ = position_poll.tick() => {
                if let Some(position_ms) = read_player_position(&player).await {
                    let sampled_at = Instant::now();
                    if position_sync.accepts(
                        Some(position_ms),
                        &state.playback_status,
                        sampled_at,
                    ) {
                        state.position_ms = Some(position_ms);
                        let _ = sender.send(SpotifyWatcherEvent::PositionUpdated {
                            track_identity: player_track_identity(&state),
                            position_ms,
                            sampled_at,
                        });
                    }
                }
            }
            _ = health_check.tick() => {
                match read_player_state(&player, &bus_name).await {
                    Ok(mut next_state) => {
                        let observed_at = Instant::now();
                        let track_changed = position_sync.observe_track(&next_state, observed_at);
                        if !position_sync.accepts(
                            next_state.position_ms,
                            &next_state.playback_status,
                            observed_at,
                        ) {
                            next_state.position_ms = if track_changed {
                                Some(0)
                            } else {
                                position_sync
                                    .estimated_position(observed_at)
                                    .or(state.position_ms)
                            };
                        }
                        state = next_state;
                        let _ = sender.send(SpotifyWatcherEvent::Updated(state.clone()));
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
