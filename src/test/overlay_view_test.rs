use super::*;

#[test]
fn track_offset_label_uses_the_localized_unit_and_unicode_minus() {
    assert_eq!(track_offset_label(300, "milliseconds"), "+300 milliseconds");
    assert_eq!(track_offset_label(-300, "毫秒"), "−300 毫秒");
    assert_eq!(track_offset_label(0, "ms"), "0 ms");
}
