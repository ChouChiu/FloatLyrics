use super::*;

#[test]
fn stale_save_completion_cannot_replace_the_latest_status() {
    let mut tracker = SaveTracker::default();
    let first = tracker.begin_save();
    let latest = tracker.begin_save();

    assert!(!tracker.complete(first, Err("old failure".to_string())));
    assert!(tracker.complete(latest, Ok(())));
    assert_eq!(
        tracker.render(Language::English),
        Language::English.text(Text::Saved)
    );
    assert!(!tracker.is_error());
}

#[test]
fn current_save_failure_retains_diagnostic_detail() {
    let mut tracker = SaveTracker::default();
    let revision = tracker.begin_save();

    assert!(tracker.complete(revision, Err("disk full".to_string())));

    assert!(tracker.render(Language::English).contains("disk full"));
    assert!(tracker.is_error());
}
