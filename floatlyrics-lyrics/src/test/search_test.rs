use super::*;

#[test]
fn manual_candidates_are_ranked_and_deduplicated() {
    let candidates = finalize_candidates(vec![
        candidate(LyricsProvider::QqMusic, "same", 70),
        candidate(LyricsProvider::NetEase, "other", 99),
        candidate(LyricsProvider::QqMusic, "same", 95),
    ]);

    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].provider, LyricsProvider::NetEase);
    assert_eq!(candidates[1].match_score, 95);
}

#[test]
fn manual_candidates_are_limited_after_ranking() {
    let candidates = (0..13)
        .map(|score| candidate(LyricsProvider::QqMusic, &format!("id-{score}"), score))
        .collect();

    let candidates = finalize_candidates(candidates);

    assert_eq!(candidates.len(), 12);
    assert_eq!(
        candidates.first().map(|candidate| candidate.match_score),
        Some(12)
    );
    assert_eq!(
        candidates.last().map(|candidate| candidate.match_score),
        Some(1)
    );
}

fn candidate(provider: LyricsProvider, id: &str, score: i32) -> LyricsCandidate {
    LyricsCandidate {
        provider,
        provider_track_id: id.to_string(),
        numeric_id: None,
        title: "Song".to_string(),
        artists: vec!["Artist".to_string()],
        album: String::new(),
        duration_ms: Some(180_000),
        match_score: score,
    }
}

#[test]
fn search_plan_removes_non_adjacent_duplicate_providers() {
    let plan = SearchPlan::new([
        LyricsProvider::QqMusic,
        LyricsProvider::NetEase,
        LyricsProvider::QqMusic,
    ]);

    assert_eq!(
        plan.providers(),
        &[LyricsProvider::QqMusic, LyricsProvider::NetEase]
    );
}

#[test]
fn provider_metadata_converts_traditional_chinese_for_search() {
    let track = TrackMetadata {
        title: "喜歡你".to_string(),
        artists: vec!["G.E.M.鄧紫棋".to_string()],
        album: Some("喜歡你".to_string()),
        duration_ms: Some(235_000),
        mpris_track_id: Some("spotify:track:example".to_string()),
        art_url: None,
    };

    let metadata = lyrics_helper_metadata(&track);

    assert_eq!(metadata.title.as_deref(), Some("喜欢你"));
    assert_eq!(metadata.artist.as_deref(), Some("G.E.M.邓紫棋"));
    assert_eq!(metadata.artists, Some(vec!["G.E.M.邓紫棋".to_string()]));
    assert_eq!(metadata.album.as_deref(), Some("喜欢你"));
    assert_eq!(track.title, "喜歡你");
    assert_eq!(track.artists, ["G.E.M.鄧紫棋"]);
}

fn helper_result(
    title: &str,
    artists: &[&str],
) -> lyrics_helper::searchers::search_result::SearchResult {
    use lyrics_helper::searchers::{Searchers, compare_helper::MatchType};

    lyrics_helper::searchers::search_result::SearchResult {
        searcher_type: Searchers::Kugou,
        title: title.to_string(),
        artists: artists.iter().map(|value| (*value).to_string()).collect(),
        album: String::new(),
        album_artists: None,
        duration_ms: None,
        match_type: Some(MatchType::Medium),
        id: "hash".to_string(),
        numeric_id: None,
    }
}

fn helper_metadata(title: &str, artists: &[&str]) -> lyrics_helper::models::TrackMetadata {
    let mut metadata = lyrics_helper::models::TrackMetadata::new();
    metadata.title = Some(title.to_string());
    metadata.artist = Some(artists.join(", "));
    metadata.ensure_artists();
    metadata
}

/// A source that scores another song of the same artist as this track must not
/// answer with its lyrics, which is what Kugou does for a title it does not hold.
#[test]
fn another_song_of_the_same_artist_is_not_this_tracks_lyrics() {
    let metadata = helper_metadata("Zzz Totally Made Up Song", &["Eminem"]);

    assert!(super::provider::is_another_song(
        &metadata,
        &helper_result("Godzilla", &["Eminem"])
    ));
    assert!(super::provider::is_another_song(
        &metadata,
        &helper_result("Zzz Totally Made Up Song", &["Somebody Else"])
    ));
}

/// A source that holds the track still answers it, whatever else it credits it
/// with, and however it decorates the title.
#[test]
fn a_source_that_holds_the_track_still_answers_it() {
    let metadata = helper_metadata("Zzz Totally Made Up Song", &["Eminem"]);

    assert!(!super::provider::is_another_song(
        &metadata,
        &helper_result("Zzz Totally Made Up Song", &["Eminem"])
    ));
    assert!(!super::provider::is_another_song(
        &metadata,
        &helper_result(
            "Zzz Totally Made Up Song (Explicit)",
            &["Eminem", "Juice WRLD"]
        )
    ));
}
