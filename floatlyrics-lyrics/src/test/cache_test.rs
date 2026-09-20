use super::*;

fn track() -> TrackMetadata {
    TrackMetadata {
        title: "A Song".to_string(),
        artists: vec!["Alice".to_string()],
        album: Some("Record".to_string()),
        duration_ms: Some(123_000),
        mpris_track_id: Some("/org/mpris/MediaPlayer2/Track/1".to_string()),
        art_url: None,
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

#[test]
fn per_track_offset_defaults_to_zero_and_round_trips() {
    let cache = Cache::open_memory().unwrap();
    let fingerprint = cache.upsert_track(&track()).unwrap();

    assert_eq!(cache.track_offset_ms(&fingerprint).unwrap(), 0);

    cache.set_track_offset_ms(&fingerprint, -350).unwrap();
    assert_eq!(cache.track_offset_ms(&fingerprint).unwrap(), -350);

    cache.set_track_offset_ms(&fingerprint, 725).unwrap();
    assert_eq!(cache.track_offset_ms(&fingerprint).unwrap(), 725);
}

#[test]
fn per_track_offset_rejects_out_of_range_values() {
    let cache = Cache::open_memory().unwrap();
    let fingerprint = cache.upsert_track(&track()).unwrap();

    assert!(
        cache
            .set_track_offset_ms(&fingerprint, TRACK_OFFSET_MS_MIN - 1)
            .is_err()
    );
    assert!(
        cache
            .set_track_offset_ms(&fingerprint, TRACK_OFFSET_MS_MAX + 1)
            .is_err()
    );
    assert_eq!(cache.track_offset_ms(&fingerprint).unwrap(), 0);
}

#[test]
fn existing_database_gains_track_offsets_without_losing_tracks() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lyrics.db");
    let fingerprint = track().fingerprint();
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                r#"
                CREATE TABLE tracks (
                    fingerprint TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    artists_json TEXT NOT NULL,
                    album TEXT,
                    duration_ms INTEGER,
                    mpris_track_id TEXT,
                    last_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                "#,
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO tracks (fingerprint, title, artists_json) VALUES (?1, ?2, ?3)",
                params![fingerprint, "A Song", "[\"Alice\"]"],
            )
            .unwrap();
    }

    let cache = Cache::open(&path).unwrap();

    assert_eq!(cache.track_offset_ms(&fingerprint).unwrap(), 0);
    cache.set_track_offset_ms(&fingerprint, 300).unwrap();
    assert_eq!(cache.track_offset_ms(&fingerprint).unwrap(), 300);
}

/// A payload that carries the word timings of a track beside its transcription is
/// stored as it was written and read back the same way, which is how the
/// application reads every document it caches.
#[test]
fn stores_a_payload_that_carries_word_timings() {
    let cache = Cache::open_memory().unwrap();
    let track = track();
    let fingerprint = cache.upsert_track(&track).unwrap();
    let payload = crate::lyrics::combine_word_timing(
        "[90,2070](90,330,0)Ugh(690,540,0)you're (1230,900,0)a monster",
        "[00:00.396]Ugh, you're a monster\n",
        Some("[00:00.396]呕，你真是只怪兽\n"),
    );

    cache
        .insert_provider_result(ProviderResultInsert {
            track_fingerprint: &fingerprint,
            provider: LyricsProvider::NetEase,
            provider_track_id: Some("123"),
            title: "A Song",
            artists: &track.artists,
            score: 0.99,
            raw_lyrics: Some(&payload),
        })
        .unwrap();

    let stored = cache
        .lyrics_for_track(&fingerprint, &LyricsProvider::default_order())
        .unwrap()
        .unwrap();
    let lines = crate::lyrics::timed_lines_from_raw(&stored.raw_lyrics, &stored.artists).unwrap();

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "Ugh, you're a monster");
    assert_eq!(lines[0].translation.as_deref(), Some("呕，你真是只怪兽"));
    assert_eq!(
        lines[0]
            .syllables
            .iter()
            .map(|syllable| (syllable.start_ms, syllable.text.as_str()))
            .collect::<Vec<_>>(),
        vec![(90, "Ugh, "), (690, "you're "), (1230, "a monster")]
    );
}
