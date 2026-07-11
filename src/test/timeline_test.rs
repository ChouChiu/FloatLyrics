use super::*;

#[test]
fn playback_offset_clamps_at_numeric_boundaries() {
    assert_eq!(adjusted_playback_ms(0, i64::MIN), 0);
    assert_eq!(adjusted_playback_ms(u64::MAX, i64::MAX), u64::MAX);
}
