use super::*;

/// A karaoke renderer draws the words, not the line text, so every QRC line must
/// spell its text through its syllables.
fn spelled(syllables: &[TimedSyllable]) -> String {
    syllables
        .iter()
        .map(|syllable| syllable.text.as_str())
        .collect()
}

#[test]
fn keeps_a_repeated_phrase_parenthesis_in_the_syllables() {
    // QQ Music marks the repeated phrase with a parenthesised word group, so the
    // outer bracket is text and the syllable after it is the bracket itself.
    let lines = timed_lines_from_qrc(
        "[109473,7768]((109473,249)Come (109722,192)on, (109914,209)just…)(110123,7118)",
    );

    let line = &lines[0];
    assert_eq!(line.text, "(Come on, just…)");
    assert_eq!(spelled(&line.syllables), line.text);
    assert_eq!(
        line.syllables
            .iter()
            .map(|syllable| (syllable.start_ms, syllable.end_ms, syllable.text.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (109_473, 109_722, "("),
            (109_722, 109_914, "Come "),
            (109_914, 110_123, "on, "),
            (110_123, 117_241, "just…)"),
        ]
    );
}

#[test]
fn keeps_a_literal_bracket_inside_a_syllable() {
    let lines = timed_lines_from_qrc("[17456,1000]Love (you)(17456,500)me(17956,500)");

    let line = &lines[0];
    assert_eq!(line.text, "Love (you)me");
    assert_eq!(spelled(&line.syllables), line.text);
    assert_eq!(
        line.syllables
            .iter()
            .map(|syllable| (syllable.start_ms, syllable.text.as_str()))
            .collect::<Vec<_>>(),
        vec![(17_456, "Love (you)"), (17_956, "me")]
    );
}

#[test]
fn covers_text_after_the_last_timestamp_with_the_last_syllable() {
    let lines = timed_lines_from_qrc("[17456,1000]Hello(17456,500) world");

    let line = &lines[0];
    assert_eq!(line.text, "Hello world");
    assert_eq!(spelled(&line.syllables), line.text);
}
