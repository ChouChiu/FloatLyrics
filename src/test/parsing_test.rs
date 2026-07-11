use super::*;

#[test]
fn overflowing_lrc_timestamp_is_ignored() {
    assert_eq!(parse_lrc_timestamp("18446744073709551615:00.00"), None);
    assert!(timed_lines_from_lrc("[18446744073709551615:00.00]Never").is_empty());
}
