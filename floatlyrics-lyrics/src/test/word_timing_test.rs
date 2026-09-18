// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Word timings read onto the text of a row-timed transcription.

use crate::lyrics::model::{TimedLine, TimedSyllable};
use crate::lyrics::{combine_word_timing, timed_lines_from_raw};

fn syllable(start_ms: u64, end_ms: u64, text: &str) -> TimedSyllable {
    TimedSyllable {
        start_ms,
        end_ms,
        text: text.to_string(),
        romanization: String::new(),
        furigana: String::new(),
    }
}

fn word_line(start_ms: u64, text: &str, syllables: Vec<TimedSyllable>) -> TimedLine {
    TimedLine {
        start_ms,
        end_ms: Some(start_ms + 2_000),
        text: text.to_string(),
        syllables,
        translation: None,
        romanization: None,
        romanization_segments: Vec::new(),
        background: None,
        voice: crate::lyrics::model::Voice::Primary,
    }
}

fn row_line(start_ms: u64, text: &str) -> TimedLine {
    TimedLine {
        start_ms,
        end_ms: None,
        text: text.to_string(),
        syllables: Vec::new(),
        translation: None,
        romanization: None,
        romanization_segments: Vec::new(),
        background: None,
        voice: crate::lyrics::model::Voice::Primary,
    }
}

fn words_of(lines: &[TimedLine]) -> Vec<(u64, String)> {
    lines
        .iter()
        .flat_map(|line| {
            line.syllables
                .iter()
                .map(|syllable| (syllable.start_ms, syllable.text.clone()))
        })
        .collect()
}

/// The word-timed document of a provider spells its words without the separators
/// of the transcription, so the words are spelled from the transcription.
#[test]
fn spells_the_words_of_a_row_with_the_text_of_the_transcription() {
    let mut lines = vec![row_line(90, "Ugh, you're a monster")];
    let word_lines = vec![word_line(
        90,
        "Ughyou're a monster",
        vec![
            syllable(90, 420, "Ugh"),
            syllable(420, 690, "you're "),
            syllable(690, 960, "a "),
            syllable(960, 2_160, "monster"),
        ],
    )];

    super::apply_word_timings(&mut lines, &word_lines);

    assert_eq!(
        words_of(&lines),
        vec![
            (90, "Ugh, ".to_string()),
            (420, "you're ".to_string()),
            (690, "a ".to_string()),
            (960, "monster".to_string()),
        ]
    );
    assert_eq!(lines[0].text, "Ugh, you're a monster");
}

/// A row whose text the words cannot be read onto keeps its row timing: the words
/// of a provider that censored a word do not spell the word the row spent.
#[test]
fn a_row_whose_words_do_not_spell_it_keeps_its_row_timing() {
    let mut lines = vec![
        row_line(90, "Shady's in this bitch, I'm posse'd up"),
        row_line(18_540, "Consider it to cross me a costly mistake"),
    ];
    let word_lines = vec![
        word_line(
            90,
            "Shady's in this *****I'm posse'd up",
            vec![syllable(90, 400, "Shady's ")],
        ),
        word_line(
            18_540,
            "Consider it to cross me a costly mistake",
            vec![
                syllable(18_540, 18_700, "Consider "),
                syllable(18_700, 19_000, "it "),
                syllable(19_000, 19_300, "to "),
                syllable(19_300, 19_600, "cross "),
                syllable(19_600, 19_800, "me "),
                syllable(19_800, 19_900, "a "),
                syllable(19_900, 20_400, "costly "),
                syllable(20_400, 20_900, "mistake"),
            ],
        ),
    ];

    super::apply_word_timings(&mut lines, &word_lines);

    assert!(lines[0].syllables.is_empty());
    assert_eq!(lines[0].end_ms, None);
    assert_eq!(
        words_of(&lines[1..]),
        vec![
            (18_540, "Consider "),
            (18_700, "it "),
            (19_000, "to "),
            (19_300, "cross "),
            (19_600, "me "),
            (19_800, "a "),
            (19_900, "costly "),
            (20_400, "mistake"),
        ]
        .into_iter()
        .map(|(start_ms, text)| (start_ms, text.to_string()))
        .collect::<Vec<_>>()
    );
    // The words a view animates spell the row it draws.
    assert_eq!(
        lines[1]
            .syllables
            .iter()
            .map(|syllable| syllable.text.as_str())
            .collect::<String>(),
        lines[1].text
    );
}

/// The documents time the same row from moments of their own, so a row is read
/// onto the words that start nearest it, in order.
#[test]
fn reads_the_words_of_a_row_that_starts_a_little_earlier() {
    let mut lines = vec![
        row_line(396, "Ugh, you're a monster"),
        row_line(2_345, "Second row"),
    ];
    let word_lines = vec![
        word_line(
            90,
            "Ughyou're a monster",
            vec![
                syllable(90, 420, "Ugh"),
                syllable(420, 690, "you're "),
                syllable(690, 960, "a "),
                syllable(960, 2_160, "monster"),
            ],
        ),
        word_line(
            2_370,
            "Second row",
            vec![syllable(2_370, 3_000, "Second row")],
        ),
    ];

    super::apply_word_timings(&mut lines, &word_lines);

    assert_eq!(
        words_of(&lines),
        vec![
            (90, "Ugh, ".to_string()),
            (420, "you're ".to_string()),
            (690, "a ".to_string()),
            (960, "monster".to_string()),
            (2_370, "Second row".to_string()),
        ]
    );
}

/// The row-timed transcription of a track written by a provider that also times
/// its words is read with the words of that document, and with the translation of
/// the transcription rather than the one written for the words.
#[test]
fn parses_a_payload_that_carries_both_documents() {
    let payload = combine_word_timing(
        "[90,2070](90,330,0)Ugh(690,540,0)you're (1230,900,0)a monster",
        "[00:00.396]Ugh, you're a monster\n",
        Some("[00:00.396]呕，你真是只怪兽\n"),
    );

    let lines = timed_lines_from_raw(&payload, &[]).unwrap();

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "Ugh, you're a monster");
    assert_eq!(lines[0].translation.as_deref(), Some("呕，你真是只怪兽"));
    // The comma is the transcription's: the words are spelled from it and timed
    // by the document beside it.
    assert_eq!(
        words_of(&lines),
        vec![
            (90, "Ugh, ".to_string()),
            (690, "you're ".to_string()),
            (1230, "a monster".to_string()),
        ]
    );
}
