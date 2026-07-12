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
