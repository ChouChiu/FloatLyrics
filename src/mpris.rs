use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::mpsc::Sender, time::Duration};
use zbus::{
    Connection, Proxy,
    fdo::{DBusProxy, PropertiesProxy},
    proxy::CacheProperties,
};
use zvariant::{OwnedObjectPath, OwnedValue};

use crate::track::TrackMetadata;

pub const SPOTIFY_MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.spotify";
const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER_IFACE: &str = "org.mpris.MediaPlayer2.Player";

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
    let _ = sender.send(SpotifyWatcherEvent::Connected(state.clone()));

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
                    state = read_player_state(&player, &bus_name).await?;
                    let _ = sender.send(SpotifyWatcherEvent::Updated(state.clone()));
                }
            }
            signal = seeked.next() => {
                let Some(signal) = signal else {
                    let _ = sender.send(SpotifyWatcherEvent::Disconnected);
                    return Ok(());
                };

                if let Some(position_ms) = seeked_position_ms(&signal) {
                    state.position_ms = Some(position_ms);
                    let _ = sender.send(SpotifyWatcherEvent::Updated(state.clone()));
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                match read_player_state(&player, &bus_name).await {
                    Ok(next_state) => {
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

fn position_us_to_ms(position_us: i64) -> Option<u64> {
    if position_us >= 0 {
        Some(position_us as u64 / 1_000)
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotifyWatcherEvent {
    Connected(SpotifyPlayerState),
    Updated(SpotifyPlayerState),
    Disconnected,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotifyPlayerState {
    pub bus_name: String,
    pub playback_status: PlaybackStatus,
    pub position_ms: Option<u64>,
    pub track: Option<TrackMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpotifyMetadata {
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub length_us: Option<u64>,
    pub track_id: Option<String>,
}

pub fn spotify_metadata_from_mpris(
    metadata: &HashMap<String, OwnedValue>,
) -> Option<SpotifyMetadata> {
    Some(SpotifyMetadata {
        title: string_value(metadata.get("xesam:title")?)?,
        artists: string_vec_value(metadata.get("xesam:artist")?).unwrap_or_default(),
        album: metadata.get("xesam:album").and_then(string_value),
        length_us: metadata.get("mpris:length").and_then(u64_value),
        track_id: metadata.get("mpris:trackid").and_then(object_path_value),
    })
}

fn string_value(value: &OwnedValue) -> Option<String> {
    String::try_from(value.try_clone().ok()?).ok()
}

fn string_vec_value(value: &OwnedValue) -> Option<Vec<String>> {
    Vec::<String>::try_from(value.try_clone().ok()?).ok()
}

fn u64_value(value: &OwnedValue) -> Option<u64> {
    u64::try_from(value.try_clone().ok()?)
        .ok()
        .or_else(|| i64::try_from(value.try_clone().ok()?).ok()?.try_into().ok())
}

fn object_path_value(value: &OwnedValue) -> Option<String> {
    OwnedObjectPath::try_from(value.try_clone().ok()?)
        .ok()
        .map(|path| path.to_string())
}

impl SpotifyMetadata {
    pub fn into_track_metadata(self) -> Result<TrackMetadata> {
        if self.title.trim().is_empty() {
            anyhow::bail!("Spotify metadata did not include a title");
        }
        if self.artists.is_empty() {
            anyhow::bail!("Spotify metadata did not include artists");
        }

        let track = TrackMetadata {
            title: self.title.trim().to_string(),
            artists: self
                .artists
                .into_iter()
                .map(|artist| artist.trim().to_string())
                .filter(|artist| !artist.is_empty())
                .collect::<Vec<_>>(),
            album: self
                .album
                .map(|album| album.trim().to_string())
                .filter(|album| !album.is_empty()),
            duration_ms: self.length_us.map(|value| value / 1_000),
            mpris_track_id: self.track_id,
        };

        if track.artists.is_empty() {
            anyhow::bail!("Spotify metadata did not include usable artists");
        }

        Ok(track)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
    Unknown(String),
}

impl From<&str> for PlaybackStatus {
    fn from(value: &str) -> Self {
        match value {
            "Playing" => Self::Playing,
            "Paused" => Self::Paused,
            "Stopped" => Self::Stopped,
            other => Self::Unknown(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zvariant::Value;

    fn owned(value: impl Into<Value<'static>>) -> OwnedValue {
        OwnedValue::try_from(value.into()).unwrap()
    }

    #[test]
    fn filters_spotify_mpris_names_only() {
        assert!(is_spotify_mpris_name("org.mpris.MediaPlayer2.spotify"));
        assert!(is_spotify_mpris_name(
            "org.mpris.MediaPlayer2.spotify.instance123"
        ));
        assert!(!is_spotify_mpris_name("org.mpris.MediaPlayer2.vlc"));
        assert!(!is_spotify_mpris_name("org.example.spotify"));
    }

    #[test]
    fn converts_spotify_metadata_to_internal_track() {
        let track = SpotifyMetadata {
            title: " Track ".to_string(),
            artists: vec![" Alice ".to_string(), "Bob".to_string()],
            album: Some(" Album ".to_string()),
            length_us: Some(215_500_000),
            track_id: Some("/org/mpris/MediaPlayer2/Track/42".to_string()),
        }
        .into_track_metadata()
        .unwrap();

        assert_eq!(track.title, "Track");
        assert_eq!(track.artists, vec!["Alice", "Bob"]);
        assert_eq!(track.album.as_deref(), Some("Album"));
        assert_eq!(track.duration_ms, Some(215_500));
    }

    #[test]
    fn parses_mpris_metadata_map() {
        let mut metadata = HashMap::new();
        metadata.insert("xesam:title".to_string(), owned("Song"));
        metadata.insert(
            "xesam:artist".to_string(),
            owned(vec!["Alice".to_string(), "Bob".to_string()]),
        );
        metadata.insert("xesam:album".to_string(), owned("Album"));
        metadata.insert("mpris:length".to_string(), owned(215_000_000_i64));
        metadata.insert(
            "mpris:trackid".to_string(),
            owned(zvariant::ObjectPath::try_from("/org/mpris/MediaPlayer2/Track/1").unwrap()),
        );

        let parsed = spotify_metadata_from_mpris(&metadata).unwrap();

        assert_eq!(parsed.title, "Song");
        assert_eq!(parsed.artists, vec!["Alice", "Bob"]);
        assert_eq!(parsed.album.as_deref(), Some("Album"));
        assert_eq!(parsed.length_us, Some(215_000_000));
        assert_eq!(
            parsed.track_id.as_deref(),
            Some("/org/mpris/MediaPlayer2/Track/1")
        );
    }

    #[test]
    fn converts_mpris_position_to_milliseconds() {
        assert_eq!(position_us_to_ms(12_345_678), Some(12_345));
        assert_eq!(position_us_to_ms(-1), None);
    }
}
