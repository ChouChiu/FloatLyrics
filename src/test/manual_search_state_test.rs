use super::*;
use crate::shared::manual_search::LyricsProvider;

#[test]
fn starting_a_new_search_invalidates_old_candidates_and_preview() {
    let mut state = ManualSearchState::default();
    let first_generation = state.begin_search(track("First"));
    let candidates = vec![candidate("first")];
    assert!(state.accept_candidates(first_generation, Ok(candidates.clone())));
    assert!(state.begin_preview(0).is_some());

    let next_generation = state.begin_search(track("Second"));

    assert_ne!(next_generation, first_generation);
    assert!(!state.accept_candidates(first_generation, Ok(candidates)));
    assert!(state.begin_preview(0).is_none());
    assert!(state.selection().is_none());
}

#[test]
fn preview_result_must_match_generation_and_selected_index() {
    let mut state = ManualSearchState::default();
    let generation = state.begin_search(track("Song"));
    assert!(state.accept_candidates(generation, Ok(vec![candidate("candidate")])));
    assert!(state.begin_preview(0).is_some());

    assert!(!state.accept_preview(generation.wrapping_add(1), 0, Ok(Some(fetched("stale"))),));
    assert!(!state.accept_preview(generation, 1, Ok(Some(fetched("wrong row")))));
    assert!(state.accept_preview(generation, 0, Ok(Some(fetched("current")))));
    assert_eq!(state.selection().unwrap().1.raw_lyrics, "current");
}

#[test]
fn search_input_is_normalized_without_copying_playback_identity() {
    let target = TrackMetadata {
        title: "Playing title".to_string(),
        artists: vec!["Playing artist".to_string()],
        album: Some("Playing album".to_string()),
        duration_ms: Some(180_000),
        mpris_track_id: Some("spotify:track:playing".to_string()),
    };

    let query = build_search_track(&target, "  Query title  ", " First, , Second ").unwrap();

    assert_eq!(query.title, "Query title");
    assert_eq!(query.artists, ["First", "Second"]);
    assert_eq!(query.duration_ms, target.duration_ms);
    assert_eq!(query.album, None);
    assert_eq!(query.mpris_track_id, None);
    assert_eq!(
        build_search_track(&target, "  ", "Artist"),
        Err(SearchInputError::EmptyTitle)
    );
}

#[test]
fn lifecycle_state_drives_busy_preview_and_apply_presentation() {
    let mut state = ManualSearchState::default();
    let generation = state.begin_search(track("Song"));
    assert!(state.is_searching());
    assert!(!state.can_apply());
    assert_eq!(
        state.preview_text(Language::English),
        Language::English.text(Text::SearchingCandidates)
    );

    assert!(state.accept_candidates(generation, Ok(vec![candidate("candidate")])));
    assert!(!state.is_searching());
    assert!(state.status_text(Language::English).contains('1'));

    assert!(state.begin_preview(0).is_some());
    assert!(!state.can_apply());
    assert!(state.accept_preview(generation, 0, Ok(Some(fetched("lyrics")))));
    assert!(state.can_apply());
    assert_eq!(state.preview_text(Language::English), "lyrics");

    assert!(state.accept_preview(generation, 0, Ok(None)));
    assert!(!state.can_apply());
    assert_eq!(
        state.preview_text(Language::English),
        Language::English.text(Text::CandidateUnavailable)
    );
}

#[test]
fn failed_search_finishes_busy_state_and_keeps_diagnostic_detail() {
    let mut state = ManualSearchState::default();
    let generation = state.begin_search(track("Song"));

    assert!(state.accept_candidates(generation, Err("provider offline".to_string())));

    assert!(!state.is_searching());
    assert!(!state.can_apply());
    assert!(
        state
            .status_text(Language::English)
            .contains("provider offline")
    );
    assert_eq!(
        state.preview_text(Language::English),
        Language::English.text(Text::LyricsSearchPreviewFailed)
    );
}

#[test]
fn apply_completion_must_match_the_active_search_generation() {
    let mut state = ManualSearchState::default();
    let generation = state.begin_search(track("First"));
    assert!(state.accept_candidates(generation, Ok(vec![candidate("candidate")])));
    assert!(state.begin_preview(0).is_some());
    assert!(state.accept_preview(generation, 0, Ok(Some(fetched("lyrics")))));
    let (apply_generation, _, _) = state.begin_apply().unwrap();
    assert!(!state.can_apply());
    assert!(state.begin_apply().is_none());

    state.begin_search(track("Second"));

    assert!(!state.finish_apply(apply_generation, Ok(())));
    assert!(state.is_searching());
}

#[test]
fn successful_apply_consumes_the_selection_while_failure_allows_retry() {
    let mut state = ManualSearchState::default();
    let generation = state.begin_search(track("Song"));
    assert!(state.accept_candidates(generation, Ok(vec![candidate("candidate")])));
    assert!(state.begin_preview(0).is_some());
    assert!(state.accept_preview(generation, 0, Ok(Some(fetched("lyrics")))));

    let (apply_generation, _, _) = state.begin_apply().unwrap();
    assert!(state.finish_apply(apply_generation, Err("disk full".to_string())));
    assert!(state.can_apply());

    let (apply_generation, _, _) = state.begin_apply().unwrap();
    assert!(state.finish_apply(apply_generation, Ok(())));
    assert!(!state.can_apply());
    assert!(state.selection().is_none());
}

fn track(title: &str) -> TrackMetadata {
    TrackMetadata {
        title: title.to_string(),
        artists: vec!["Artist".to_string()],
        album: None,
        duration_ms: Some(60_000),
        mpris_track_id: None,
    }
}

fn candidate(title: &str) -> LyricsCandidate {
    LyricsCandidate {
        provider: LyricsProvider::QqMusic,
        provider_track_id: title.to_string(),
        numeric_id: None,
        title: title.to_string(),
        artists: vec!["Artist".to_string()],
        album: String::new(),
        duration_ms: Some(60_000),
        match_score: 100,
    }
}

fn fetched(raw_lyrics: &str) -> FetchedLyrics {
    FetchedLyrics {
        provider: LyricsProvider::QqMusic,
        provider_track_id: Some("id".to_string()),
        title: "Song".to_string(),
        artists: vec!["Artist".to_string()],
        score: 100.0,
        raw_lyrics: raw_lyrics.to_string(),
    }
}
