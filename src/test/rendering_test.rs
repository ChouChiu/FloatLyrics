use super::*;

#[test]
fn syllable_ranges_track_repeated_words_in_order() {
    let syllables = vec![syllable("Please"), syllable(" "), syllable("Please")];

    assert_eq!(
        syllable_byte_range("Please Please", &syllables, 0),
        Some(0..6)
    );
    assert_eq!(
        syllable_byte_range("Please Please", &syllables, 1),
        Some(6..7)
    );
    assert_eq!(
        syllable_byte_range("Please Please", &syllables, 2),
        Some(7..13)
    );
}

#[test]
fn syllable_ranges_use_utf8_byte_offsets() {
    let syllables = vec![syllable("你"), syllable("好")];

    assert_eq!(syllable_byte_range("你好", &syllables, 0), Some(0..3));
    assert_eq!(syllable_byte_range("你好", &syllables, 1), Some(3..6));
}

fn syllable(text: &str) -> TimedSyllable {
    TimedSyllable {
        start_ms: 0,
        end_ms: 100,
        text: text.to_string(),
    }
}
