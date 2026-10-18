use super::*;

#[test]
fn font_family_ignores_blank_entries_and_has_a_safe_fallback() {
    assert_eq!(font_family(&[]), "Sans");
    assert_eq!(
        font_family(&[" Noto Sans ".to_string(), "".to_string()]),
        "Noto Sans"
    );
}

fn syllable(text: &str, romanization: &str) -> TimedSyllable {
    TimedSyllable {
        start_ms: 0,
        end_ms: 100,
        text: text.to_string(),
        romanization: romanization.to_string(),
        furigana: String::new(),
    }
}

/// One pixel per character on either row, so the arithmetic stays readable.
fn by_characters(text: &str, _row: WordRow) -> i32 {
    text.chars().count() as i32
}

#[test]
fn a_word_wider_below_than_above_widens_the_line() {
    // "힘없이 " is four characters wide but its reading "him eop si" is ten,
    // so the reading row sets the width of that word.
    let syllables = [
        syllable("힘없이 ", "him eop si"),
        syllable("늘 ", "neul"),
        syllable("but", ""),
    ];

    assert_eq!(
        word_reading_row_width(&syllables, by_characters),
        10 + 4 + 3
    );
}

#[test]
fn a_line_without_readings_is_left_to_the_text_measurement() {
    let syllables = [syllable("but ", " "), syllable("I rise", "")];

    assert_eq!(word_reading_row_width(&syllables, by_characters), 0);
}
