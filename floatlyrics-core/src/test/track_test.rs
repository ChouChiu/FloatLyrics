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
        art_url: None,
    };
    let identity = track.playback_identity();

    track.album = Some("Album loaded later".to_string());
    track.duration_ms = Some(180_000);
    let fingerprint = track.fingerprint();
    track.art_url = Some("https://i.example.test/cover.jpg".to_string());

    assert_eq!(track.playback_identity(), identity);
    assert_eq!(
        track.fingerprint(),
        fingerprint,
        "cover art must not change the cache identity"
    );
}

fn track(title: &str, artists: &[&str]) -> TrackMetadata {
    TrackMetadata {
        title: title.to_string(),
        artists: artists.iter().map(|artist| artist.to_string()).collect(),
        album: None,
        duration_ms: None,
        mpris_track_id: None,
        art_url: None,
    }
}

#[test]
fn a_credited_artist_the_source_omits_is_added_after_it() {
    // The real shape of this: Spotify reports "Problem" as Ariana Grande alone,
    // while the provider holding the lyrics bills both performers.
    let track = track("Problem", &["Ariana Grande"]);
    let credited = ["ariana grande".to_string(), "Iggy Azalea".to_string()];

    assert_eq!(
        track.artists_including(&credited),
        Some(vec!["Ariana Grande".to_string(), "Iggy Azalea".to_string()])
    );
}

#[test]
fn a_billing_that_adds_nobody_leaves_the_display_alone() {
    let track = track("Problem", &["Ariana Grande", "Iggy Azalea"]);
    let credited = ["Iggy Azalea".to_string(), "Ariana Grande".to_string()];

    assert_eq!(track.artists_including(&credited), None);
}

#[test]
fn a_billing_naming_nobody_on_the_record_is_ignored() {
    // A cover matched on title and duration bills performers of its own; that
    // is a reason to doubt the match, not to rewrite who is playing.
    let track = track("Problem", &["Ariana Grande"]);
    let credited = ["Pentatonix".to_string()];

    assert_eq!(track.artists_including(&credited), None);
}

#[test]
fn credited_names_differing_only_in_casing_or_spacing_are_not_listed_twice() {
    let track = track("Problem", &["Ariana  Grande"]);
    let credited = ["  ariana grande ".to_string(), "Iggy Azalea".to_string()];

    assert_eq!(
        track.artists_including(&credited),
        Some(vec![
            "Ariana  Grande".to_string(),
            "Iggy Azalea".to_string()
        ]),
        "the track's own spelling is kept and the duplicate is skipped"
    );
}

#[test]
fn fingerprint_handles_maximum_duration_without_overflowing() {
    let artists = vec!["Artist".to_string()];

    let fingerprint = track_fingerprint("Song", &artists, None, Some(u64::MAX));

    assert_eq!(fingerprint.len(), 64);
}
