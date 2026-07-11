use super::*;

#[test]
fn fingerprint_ignores_case_whitespace_and_artist_order() {
    let artists_a = vec!["Alice".to_string(), "Bob".to_string()];
    let artists_b = vec![" bob ".to_string(), "ALICE".to_string()];
    let a = track_fingerprint("  Song  Name ", &artists_a, Some(" Album "), Some(180_100));
    let b = track_fingerprint("song name", &artists_b, Some("album"), Some(180_400));

    assert_eq!(a, b);
}

#[test]
fn playback_identity_prefers_the_stable_mpris_track_id() {
    let mut track = TrackMetadata {
        title: "Song".to_string(),
        artists: vec!["Artist".to_string()],
        album: None,
        duration_ms: None,
        mpris_track_id: Some("/org/mpris/MediaPlayer2/track/42".to_string()),
    };
    let identity = track.playback_identity();

    track.album = Some("Album loaded later".to_string());
    track.duration_ms = Some(180_000);

    assert_eq!(track.playback_identity(), identity);
}

#[test]
fn fingerprint_handles_maximum_duration_without_overflowing() {
    let artists = vec!["Artist".to_string()];

    let fingerprint = track_fingerprint("Song", &artists, None, Some(u64::MAX));

    assert_eq!(fingerprint.len(), 64);
}
