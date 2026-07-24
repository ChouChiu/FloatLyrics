use super::position::position_us_to_ms;
use super::*;
use floatlyrics_lyrics::lyrics::LyricsProvider;
use std::collections::HashMap;
use zvariant::{OwnedValue, Value};

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
fn accepts_title_only_metadata_from_minimal_mpris_players() {
    let mut metadata = HashMap::new();
    metadata.insert("xesam:title".to_string(), owned("Radio Stream"));
    let track = metadata_from_mpris(&metadata)
        .unwrap()
        .into_track_metadata()
        .unwrap();

    assert_eq!(track.title, "Radio Stream");
    assert!(track.artists.is_empty());
}

#[test]
fn infers_netease_ids_from_known_player_track_paths() {
    let metadata = hint_metadata(Some("/org/mpris/MediaPlayer2/track/123456"));

    for (identity, bus_name) in [
        ("ElectronNCM", "org.mpris.MediaPlayer2.electron-ncm"),
        ("Qcm", "org.mpris.MediaPlayer2.qcm"),
        ("Other", "org.mpris.MediaPlayer2.musicfox.instance42"),
        ("Other", "org.mpris.MediaPlayer2.NeteaseCloudMusicGtk4"),
    ] {
        let hint = super::compat::lyrics_lookup_hint(identity, bus_name, &metadata, None).unwrap();
        assert_eq!(hint.provider, LyricsProvider::NetEase);
        assert_eq!(hint.provider_track_id, "123456");
    }
}

#[test]
fn infers_feeluown_netease_and_qq_music_ids_from_source_urls() {
    for (url, provider, id) in [
        (
            "fuo://netease/songs/19723756",
            LyricsProvider::NetEase,
            "19723756",
        ),
        (
            "fuo://qqmusic/songs/0039MnYb0qxYhV",
            LyricsProvider::QqMusic,
            "0039MnYb0qxYhV",
        ),
    ] {
        let metadata = hint_metadata(None);
        let hint = super::compat::lyrics_lookup_hint(
            "feeluown",
            "org.mpris.MediaPlayer2.feeluown",
            &metadata,
            Some(url),
        )
        .unwrap();
        assert_eq!(hint.provider, provider);
        assert_eq!(hint.provider_track_id, id);
    }
}

#[test]
fn infers_yesplaymusic_id_and_rejects_invalid_provider_ids() {
    let metadata = hint_metadata(None);
    let hint = super::compat::lyrics_lookup_hint(
        "YesPlayMusic",
        "org.mpris.MediaPlayer2.yesplaymusic",
        &metadata,
        Some("/trackid/347230"),
    )
    .unwrap();
    assert_eq!(hint.provider, LyricsProvider::NetEase);
    assert_eq!(hint.provider_track_id, "347230");

    let invalid = hint_metadata(Some("/track/not-a-number"));
    assert!(
        super::compat::lyrics_lookup_hint(
            "YesPlayMusic",
            "org.mpris.MediaPlayer2.musicfox",
            &invalid,
            Some("/trackid/not-a-number"),
        )
        .is_none()
    );
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
    metadata.insert("xesam:url".to_string(), owned("fuo://netease/songs/123"));

    let parsed = spotify_metadata_from_mpris(&metadata).unwrap();

    assert_eq!(parsed.title, "Song");
    assert_eq!(parsed.artists, vec!["Alice", "Bob"]);
    assert_eq!(parsed.album.as_deref(), Some("Album"));
    assert_eq!(parsed.length_us, Some(215_000_000));
    assert_eq!(
        parsed.track_id.as_deref(),
        Some("/org/mpris/MediaPlayer2/Track/1")
    );
    assert_eq!(
        super::model::source_url_from_mpris(&metadata).as_deref(),
        Some("fuo://netease/songs/123")
    );
}

#[test]
fn accepts_string_track_ids_from_nonconforming_players() {
    let mut metadata = HashMap::new();
    metadata.insert("xesam:title".to_string(), owned("Song"));
    metadata.insert(
        "mpris:trackid".to_string(),
        owned("/org/mpris/MediaPlayer2/track/123"),
    );

    assert_eq!(
        metadata_from_mpris(&metadata).unwrap().track_id.as_deref(),
        Some("/org/mpris/MediaPlayer2/track/123")
    );
}

#[test]
fn converts_mpris_position_to_milliseconds() {
    assert_eq!(position_us_to_ms(12_345_678), Some(12_345));
    assert_eq!(position_us_to_ms(-1), None);
}

fn hint_metadata(track_id: Option<&str>) -> MprisMetadata {
    MprisMetadata {
        title: "Song".to_string(),
        artists: vec!["Artist".to_string()],
        album: None,
        length_us: None,
        track_id: track_id.map(str::to_string),
    }
}
