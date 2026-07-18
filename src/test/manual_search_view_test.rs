use super::*;

#[test]
fn formats_candidate_duration() {
    assert_eq!(duration_text(Some(185_000)), "3:05");
    assert_eq!(duration_text(None), "0:00");
}
