use super::*;

#[test]
fn configured_prefix_matches_only_its_instances() {
    let prefix = "org.mpris.MediaPlayer2.spotifyd";

    assert!(is_mpris_name_with_prefix(prefix, prefix));
    assert!(is_mpris_name_with_prefix(
        "org.mpris.MediaPlayer2.spotifyd.instance42",
        prefix
    ));
    assert!(!is_mpris_name_with_prefix(
        "org.mpris.MediaPlayer2.spotify",
        prefix
    ));
    assert!(!is_mpris_name_with_prefix(
        "org.mpris.MediaPlayer2.spotifydoppelganger",
        prefix
    ));
}
