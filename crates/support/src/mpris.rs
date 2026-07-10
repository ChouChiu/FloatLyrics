use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
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
use zvariant::{OwnedObjectPath, OwnedValue};

use floatlyrics_core::track::TrackMetadata;

pub const SPOTIFY_MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.spotify";
const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER_IFACE: &str = "org.mpris.MediaPlayer2.Player";
const PLAYBACK_POSITION_POLL_INTERVAL: Duration = Duration::from_millis(250);
const PLAYER_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(30);
const NEW_TRACK_POSITION_TOLERANCE: Duration = Duration::from_millis(1_500);

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

fn position_us_to_ms(position_us: i64) -> Option<u64> {
    if position_us >= 0 {
        Some(position_us as u64 / 1_000)
    } else {
        None
    }
}

fn player_track_identity(state: &SpotifyPlayerState) -> Option<String> {
    state.track.as_ref().map(TrackMetadata::playback_identity)
}

#[derive(Debug, Clone)]
struct TrackPositionSync {
    track_identity: Option<String>,
    detected_at: Instant,
    synchronized: bool,
}

impl TrackPositionSync {
    fn new(state: &SpotifyPlayerState, now: Instant) -> Self {
        Self {
            track_identity: player_track_identity(state),
            detected_at: now,
            synchronized: true,
        }
    }

    fn observe_track(&mut self, state: &SpotifyPlayerState, now: Instant) -> bool {
        let identity = player_track_identity(state);
        if self.track_identity == identity {
            return false;
        }

        self.track_identity = identity;
        self.detected_at = now;
        self.synchronized = false;
        true
    }

    fn accepts(
        &mut self,
        position_ms: Option<u64>,
        playback_status: &PlaybackStatus,
        now: Instant,
    ) -> bool {
        let Some(position_ms) = position_ms else {
            return false;
        };
        if self.synchronized || !matches!(playback_status, PlaybackStatus::Playing) {
            self.synchronized = true;
            return true;
        }

        let elapsed_ms = now.duration_since(self.detected_at).as_millis() as u64;
        let tolerance_ms = NEW_TRACK_POSITION_TOLERANCE.as_millis() as u64;
        if position_ms <= elapsed_ms.saturating_add(tolerance_ms) {
            self.synchronized = true;
            return true;
        }

        false
    }

    fn trust_position(&mut self) {
        self.synchronized = true;
    }

    fn estimated_position(&self, now: Instant) -> Option<u64> {
        (!self.synchronized).then(|| now.duration_since(self.detected_at).as_millis() as u64)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotifyWatcherEvent {
    Connected(SpotifyPlayerState),
    Updated(SpotifyPlayerState),
    PositionUpdated {
        track_identity: Option<String>,
        position_ms: u64,
        sampled_at: Instant,
    },
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

    #[test]
    fn rejects_a_stale_position_when_the_track_changes() {
        let now = Instant::now();
        let mut sync = TrackPositionSync::new(&test_state("old", 120_000), now);
        let next = test_state("new", 120_250);

        assert!(sync.observe_track(&next, now));
        assert!(!sync.accepts(next.position_ms, &PlaybackStatus::Playing, now));
        assert_eq!(sync.estimated_position(now), Some(0));

        let settled_at = now + Duration::from_millis(500);
        assert!(sync.accepts(Some(500), &PlaybackStatus::Playing, settled_at));
    }

    #[test]
    fn paused_or_seeked_positions_are_trusted_immediately() {
        let now = Instant::now();
        let mut sync = TrackPositionSync::new(&test_state("old", 120_000), now);
        assert!(sync.observe_track(&test_state("new", 120_250), now));
        assert!(sync.accepts(Some(60_000), &PlaybackStatus::Paused, now));

        sync.synchronized = false;
        sync.trust_position();
        assert!(sync.accepts(Some(90_000), &PlaybackStatus::Playing, now));
    }

    fn test_state(track_id: &str, position_ms: u64) -> SpotifyPlayerState {
        SpotifyPlayerState {
            bus_name: SPOTIFY_MPRIS_PREFIX.to_string(),
            playback_status: PlaybackStatus::Playing,
            position_ms: Some(position_ms),
            track: Some(TrackMetadata {
                title: track_id.to_string(),
                artists: vec!["Artist".to_string()],
                album: None,
                duration_ms: Some(180_000),
                mpris_track_id: Some(format!("/track/{track_id}")),
            }),
        }
    }
}
