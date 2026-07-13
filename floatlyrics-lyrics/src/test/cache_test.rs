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
    assert!(lyrics.manually_selected);
    assert!(lyrics.raw_lyrics.contains("manual"));
}

#[test]
fn provider_cache_is_not_marked_as_manually_selected() {
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

    let lyrics = cache
        .lyrics_for_track(&fingerprint, &LyricsProvider::default_order())
        .unwrap()
        .unwrap();

    assert!(!lyrics.manually_selected);
}

#[test]
fn repeated_provider_result_updates_existing_entry() {
    let cache = Cache::open_memory().unwrap();
    let track = track();
    let fingerprint = cache.upsert_track(&track).unwrap();

    let first_id = cache
        .insert_provider_result(ProviderResultInsert {
            track_fingerprint: &fingerprint,
            provider: LyricsProvider::QqMusic,
            provider_track_id: Some("qq-1"),
            title: "Old title",
            artists: &track.artists,
            score: 0.5,
            raw_lyrics: Some("[00:01.00]provider"),
        })
        .unwrap();
    let second_id = cache
        .insert_provider_result(ProviderResultInsert {
            track_fingerprint: &fingerprint,
            provider: LyricsProvider::QqMusic,
            provider_track_id: Some("qq-1"),
            title: "Updated title",
            artists: &track.artists,
            score: 0.99,
            raw_lyrics: Some("[00:01.00]provider"),
        })
        .unwrap();

    assert_eq!(second_id, first_id);
    let (row_count, title, score): (i64, String, f64) = cache
        .conn
        .query_row(
            "SELECT COUNT(*), title, score FROM provider_results",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(row_count, 1);
    assert_eq!(title, "Updated title");
    assert_eq!(score, 0.99);
}

#[test]
fn migration_deduplicates_provider_results_before_creating_index() {
    let cache = Cache::open_memory().unwrap();
    let track = track();
    let fingerprint = cache.upsert_track(&track).unwrap();
    cache
        .conn
        .execute("DROP INDEX provider_results_content_unique", [])
        .unwrap();

    for score in [0.5, 0.99] {
        cache
            .insert_provider_result(ProviderResultInsert {
                track_fingerprint: &fingerprint,
                provider: LyricsProvider::QqMusic,
                provider_track_id: Some("qq-1"),
                title: "A Song",
                artists: &track.artists,
                score,
                raw_lyrics: Some("[00:01.00]provider"),
            })
            .unwrap();
    }

    cache.migrate().unwrap();

    let row_count: i64 = cache
        .conn
        .query_row("SELECT COUNT(*) FROM provider_results", [], |row| {
            row.get(0)
        })
        .unwrap();
    let has_index: bool = cache
        .conn
        .query_row(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM sqlite_master
                WHERE type = 'index' AND name = 'provider_results_content_unique'
            )
            "#,
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(row_count, 1);
    assert!(has_index);
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
