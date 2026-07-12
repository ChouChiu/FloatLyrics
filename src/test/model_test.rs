use super::*;
use std::time::Duration;

#[test]
fn advances_local_clock_only_while_playing() {
    let playing = snapshot(PlaybackStatus::Playing, Duration::from_millis(1_500));
    let paused = snapshot(PlaybackStatus::Paused, Duration::from_millis(1_500));

    assert!(effective_position_ms(&playing).unwrap() >= 11_000);
    assert_eq!(effective_position_ms(&paused), Some(10_000));
}

#[test]
fn authoritative_sample_reanchors_matching_track() {
    let mut snapshot = snapshot(PlaybackStatus::Playing, Duration::from_secs(2));
    let identity = snapshot.state.track.as_ref().unwrap().playback_identity();
    let sampled_at = Instant::now();

    assert!(apply_position_sample(
        &mut snapshot,
        Some(&identity),
        10_500,
        sampled_at,
    ));
    assert!(effective_position_ms(&snapshot).unwrap() < 10_600);
    assert_eq!(snapshot.received_at, sampled_at);
}

#[test]
fn sample_from_another_track_is_ignored() {
    let mut snapshot = snapshot(PlaybackStatus::Playing, Duration::ZERO);
    let received_at = snapshot.received_at;

    assert!(!apply_position_sample(
        &mut snapshot,
        Some("another-track"),
        500,
        Instant::now(),
    ));
    assert_eq!(snapshot.state.position_ms, Some(10_000));
    assert_eq!(snapshot.received_at, received_at);
}

#[test]
fn syllable_progress_is_clamped_to_its_time_range() {
    let syllable = TimedSyllable {
        start_ms: 1_000,
        end_ms: 1_500,
        text: "hello".to_string(),
    };

    assert_eq!(syllable_progress(&syllable, 900), 0.0);
    assert_eq!(syllable_progress(&syllable, 1_250), 0.5);
    assert_eq!(syllable_progress(&syllable, 1_700), 1.0);
}

#[test]
fn placeholder_translation_is_hidden() {
    let mut line = test_line();
    line.translation = Some("//".to_string());
    let text = line_text(Some(&line), &AppConfig::default());
    assert_eq!(text.text, "Hello");
    assert!(text.translation.is_empty());

    line.translation = Some("你好".to_string());
    let text = line_text(Some(&line), &AppConfig::default());
    assert_eq!(text.translation, "你好");
}

#[test]
fn lyric_frame_uses_stable_key_for_active_line() {
    let state = LyricsDisplayState {
        lines: vec![test_line()],
        ..LyricsDisplayState::default()
    };

    let frame = lyrics_frame(
        &state,
        &AppConfig::default(),
        Some(1_500),
        Language::English,
    );
    assert_eq!(frame.key, "line:0");
    assert_eq!(frame.content.text, "Hello");
}

#[test]
fn adjusted_position_is_saturated_at_both_bounds() {
    assert_eq!(adjusted_position_ms(0, -1), 0);
    assert_eq!(adjusted_position_ms(u64::MAX, i64::MAX), u64::MAX);
}

fn snapshot(status: PlaybackStatus, elapsed: Duration) -> PlaybackSnapshot {
    PlaybackSnapshot {
        state: SpotifyPlayerState {
            bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
            playback_status: status,
            position_ms: Some(10_000),
            track: Some(TrackMetadata {
                title: "Song".to_string(),
                artists: vec!["Artist".to_string()],
                album: None,
                duration_ms: Some(20_000),
                mpris_track_id: None,
            }),
        },
        received_at: Instant::now() - elapsed,
    }
}

fn test_line() -> TimedLine {
    TimedLine {
        start_ms: 1_000,
        end_ms: Some(2_000),
        text: "Hello".to_string(),
        syllables: Vec::new(),
        translation: None,
        romanization: None,
        background: None,
    }
}
