use super::*;

fn track(art_url: Option<&str>) -> TrackMetadata {
    TrackMetadata {
        title: "Song".to_string(),
        artists: vec!["Artist".to_string(), "Guest".to_string()],
        album: Some("Album".to_string()),
        duration_ms: Some(200_000),
        mpris_track_id: Some("/org/mpris/MediaPlayer2/Track/7".to_string()),
        art_url: art_url.map(str::to_string),
    }
}

#[test]
fn maps_player_metadata_onto_the_protocol_track() {
    let music = music_from_track(&track(Some("file:///tmp/cover.jpg")));

    assert_eq!(music.id, "/org/mpris/MediaPlayer2/Track/7");
    assert_eq!(music.name, "Song");
    assert_eq!(music.album, "Album");
    assert_eq!(music.artists, vec!["Artist", "Guest"]);
    assert_eq!(music.duration_ms, 200_000);
    assert!(matches!(music.cover, Some(Cover::File(_))));
}

#[test]
fn tolerates_metadata_the_player_did_not_supply() {
    let mut sparse = track(None);
    sparse.album = None;
    sparse.duration_ms = None;
    sparse.mpris_track_id = None;

    let music = music_from_track(&sparse);
    assert_eq!(music.album, "");
    assert_eq!(music.duration_ms, 0);
    assert_eq!(music.cover, None);
    assert_eq!(
        music.id,
        sparse.fingerprint(),
        "a track without a player identifier falls back to its fingerprint"
    );
}
