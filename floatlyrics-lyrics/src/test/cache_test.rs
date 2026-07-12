use super::*;

fn track() -> TrackMetadata {
    TrackMetadata {
        title: "A Song".to_string(),
        artists: vec!["Alice".to_string()],
        album: Some("Record".to_string()),
        duration_ms: Some(123_000),
        mpris_track_id: Some("/org/mpris/MediaPlayer2/Track/1".to_string()),
    }
}

#[test]
fn manual_match_wins_over_provider_cache() {
    let cache = Cache::open_memory().unwrap();
    let track = track();
    let fingerprint = cache.upsert_track(&track).unwrap();

    cache
        .insert_provider_result(ProviderResultInsert {
            track_fingerprint: &fingerprint,
            provider: LyricsProvider::QqMusic,
            provider_track_id: Some("qq-1"),
            title: "A Song",
            artists: &track.artists,
            score: 0.99,
            raw_lyrics: Some("[00:01.00]provider"),
        })
        .unwrap();

    let manual_id = cache
        .insert_lyrics(LyricsInsert {
            provider: LyricsProvider::NetEase,
            provider_track_id: Some("manual-1"),
            title: "A Song",
            artists: &track.artists,
            raw_lyrics: "[00:01.00]manual",
        })
        .unwrap();
    cache.bind_manual_match(&fingerprint, manual_id).unwrap();

    let lyrics = cache
        .lyrics_for_track(&fingerprint, &LyricsProvider::default_order())
        .unwrap()
        .unwrap();

    assert_eq!(lyrics.provider, LyricsProvider::NetEase);
    assert!(lyrics.raw_lyrics.contains("manual"));
}

#[test]
fn oversized_duration_is_stored_as_unknown() {
    let cache = Cache::open_memory().unwrap();
    let mut track = track();
    track.duration_ms = Some(u64::MAX);
    let fingerprint = cache.upsert_track(&track).unwrap();

    let stored: Option<i64> = cache
        .conn
        .query_row(
            "SELECT duration_ms FROM tracks WHERE fingerprint = ?1",
            params![fingerprint],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(stored, None);
}
