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

#[test]
fn playing_player_wins_over_paused_preference() {
    let selection = PlayerSelection {
        preferred_players: vec!["spotify".to_string()],
        ..PlayerSelection::default()
    };

    let selected = choose_player(
        vec![
            candidate("spotify", "Spotify", PlaybackStatus::Paused),
            candidate("vlc", "VLC", PlaybackStatus::Playing),
        ],
        &selection,
        Some("org.mpris.MediaPlayer2.spotify"),
    )
    .unwrap();

    assert_eq!(selected.bus_name, "org.mpris.MediaPlayer2.vlc");
}

#[test]
fn preference_and_current_player_stabilize_equal_candidates() {
    let selection = PlayerSelection {
        preferred_players: vec!["VLC".to_string()],
        ..PlayerSelection::default()
    };
    let candidates = vec![
        candidate("spotify", "Spotify", PlaybackStatus::Playing),
        candidate("vlc.instance1", "VLC", PlaybackStatus::Playing),
    ];

    let preferred = choose_player(candidates.clone(), &selection, None).unwrap();
    assert_eq!(preferred.identity, "VLC");

    let sticky = choose_player(
        candidates,
        &PlayerSelection::default(),
        Some("org.mpris.MediaPlayer2.spotify"),
    )
    .unwrap();
    assert_eq!(sticky.identity, "Spotify");
}

#[test]
fn selectors_match_bus_suffix_instances_and_identity() {
    let player = candidate("vlc.instance42", "VLC media player", PlaybackStatus::Paused);

    assert!(selector_matches(&player, "vlc"));
    assert!(selector_matches(&player, "org.mpris.MediaPlayer2.vlc"));
    assert!(selector_matches(&player, "VLC media player"));
    assert!(!selector_matches(&player, "spotify"));
}

fn candidate(suffix: &str, identity: &str, playback_status: PlaybackStatus) -> PlayerCandidate {
    PlayerCandidate {
        bus_name: format!("{MPRIS_BUS_PREFIX}{suffix}"),
        identity: identity.to_string(),
        playback_status,
    }
}
