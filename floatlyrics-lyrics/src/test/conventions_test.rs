use super::*;

fn syllable(start_ms: u64, end_ms: u64, text: &str) -> TimedSyllable {
    TimedSyllable {
        start_ms,
        end_ms,
        text: text.to_string(),
        romanization: String::new(),
        furigana: String::new(),
    }
}

fn line(
    start_ms: u64,
    end_ms: Option<u64>,
    text: &str,
    syllables: Vec<TimedSyllable>,
) -> TimedLine {
    TimedLine {
        start_ms,
        end_ms,
        text: text.to_string(),
        syllables,
        translation: None,
        romanization: None,
        romanization_segments: Vec::new(),
        background: None,
        voice: Voice::Primary,
    }
}

fn background_text(line: &TimedLine) -> Option<&str> {
    line.background
        .as_ref()
        .map(|background| background.text.as_str())
}

fn texts(line: &TimedLine) -> Vec<&str> {
    line.syllables
        .iter()
        .map(|syllable| syllable.text.as_str())
        .collect()
}

fn artists(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

#[test]
fn a_bracketed_tail_becomes_a_background_vocal() {
    // The shape NetEase writes "Umbrella"'s backing part in: a bracketed tail on
    // the line it answers, with the brackets as zero-length syllables of their own.
    let mut lines = vec![line(
        0,
        Some(4_650),
        "Ahuh Ahuh （Yea Rihanna）",
        vec![
            syllable(0, 180, "Ahuh "),
            syllable(180, 270, "Ahuh "),
            syllable(270, 270, "（"),
            syllable(270, 3_990, "Yea "),
            syllable(3_990, 4_650, "Rihanna"),
            syllable(4_650, 4_650, "）"),
        ],
    )];

    split_background_vocals(&mut lines);

    assert_eq!(lines[0].text, "Ahuh Ahuh");
    assert_eq!(background_text(&lines[0]), Some("Yea Rihanna"));
    assert_eq!(texts(&lines[0]), vec!["Ahuh ", "Ahuh "]);
    let background = lines[0].background.as_ref().expect("the tail is split off");
    assert_eq!(
        (background.start_ms, background.end_ms),
        (270, Some(4_650)),
        "the tail is sung where its own words are"
    );
}

#[test]
fn a_bracketed_tail_inside_a_syllable_is_divided_by_its_characters() {
    let mut lines = vec![line(
        1_000,
        Some(3_000),
        "Hold on (ohyeah)",
        vec![
            syllable(1_000, 1_500, "Hold "),
            syllable(1_500, 2_500, "on (ohyeah)"),
        ],
    )];

    split_background_vocals(&mut lines);

    assert_eq!(lines[0].text, "Hold on");
    assert_eq!(background_text(&lines[0]), Some("ohyeah"));
    assert_eq!(texts(&lines[0]), vec!["Hold ", "on "]);
    assert_eq!(
        lines[0]
            .background
            .as_ref()
            .map(|background| (background.start_ms, background.end_ms)),
        Some((1_772, Some(2_500))),
        "the half the tail keeps starts where the characters it kept do"
    );
    // The divided half stays contiguous with the syllable it was cut from.
    assert_eq!(lines[0].syllables[1].end_ms, 1_500 + 1_000 * 3 / 11);
}

#[test]
fn a_bracketed_phrase_inside_the_line_becomes_a_background_vocal() {
    // "I'm In Love With a Monster" writes the words the two sing together in the
    // middle of the line they answer from, and the line carries on after them.
    let mut lines = vec![line(
        1_849,
        Some(5_627),
        "I'm in love (we're in love) with a monster",
        vec![
            syllable(1_849, 2_029, "I'm "),
            syllable(2_029, 2_219, "in "),
            syllable(2_219, 2_928, "love "),
            syllable(2_928, 3_308, "("),
            syllable(3_308, 3_488, "we're "),
            syllable(3_488, 3_918, "in "),
            syllable(3_918, 4_437, "love) "),
            syllable(4_437, 4_607, "with "),
            syllable(4_607, 4_787, "a "),
            syllable(4_787, 5_627, "monster"),
        ],
    )];

    split_background_vocals(&mut lines);

    assert_eq!(lines[0].text, "I'm in love with a monster");
    assert_eq!(
        texts(&lines[0]),
        vec!["I'm ", "in ", "love ", "with ", "a ", "monster"],
        "the words the line is drawn with are the words around the phrase"
    );
    let background = lines[0]
        .background
        .as_ref()
        .expect("the phrase is split off");
    assert_eq!(background.text, "we're in love");
    assert_eq!(
        background
            .syllables
            .iter()
            .map(|syllable| syllable.text.as_str())
            .collect::<Vec<_>>(),
        vec!["we're ", "in ", "love"],
        "the brackets are not sung, so they are not words of the phrase"
    );
    assert_eq!(
        (background.start_ms, background.end_ms),
        (3_308, Some(4_350)),
        "the phrase is sung where its own words are, inside the line"
    );
}

#[test]
fn a_bracketed_phrase_the_line_opens_with_becomes_a_background_vocal() {
    let mut lines = vec![line(
        0,
        Some(2_000),
        "(Oh) I love it",
        vec![
            syllable(0, 300, "(Oh) "),
            syllable(300, 700, "I "),
            syllable(700, 1_200, "love "),
            syllable(1_200, 2_000, "it"),
        ],
    )];

    split_background_vocals(&mut lines);

    assert_eq!(lines[0].text, "I love it");
    assert_eq!(texts(&lines[0]), vec!["I ", "love ", "it"]);
    assert_eq!(background_text(&lines[0]), Some("Oh"));
}

#[test]
fn a_bracketed_tail_without_words_is_left_alone() {
    let mut lines = vec![line(
        1_000,
        Some(3_000),
        "Wait (...)",
        vec![
            syllable(1_000, 2_000, "Wait "),
            syllable(2_000, 3_000, "(...)"),
        ],
    )];

    split_background_vocals(&mut lines);

    assert_eq!(lines[0].text, "Wait (...)");
    assert!(lines[0].background.is_none());
}

#[test]
fn a_line_timed_source_keeps_its_brackets() {
    // A plain LRC line has no word timing to split a tail off with.
    let mut lines = vec![line(0, None, "Know the way (My way)", Vec::new())];

    split_background_vocals(&mut lines);

    assert_eq!(lines[0].text, "Know the way (My way)");
    assert!(lines[0].background.is_none());
}

#[test]
fn a_wholly_bracketed_line_joins_the_line_it_echoes() {
    // The shape QQ Music writes "hate that i made you love me" in: a bracketed
    // line of its own, timed to start where the line it answers runs out.
    let mut lines = vec![
        line(
            1_000,
            Some(3_000),
            "Know the way",
            vec![
                syllable(1_000, 2_000, "Know "),
                syllable(2_000, 3_000, "the way"),
            ],
        ),
        line(
            3_000,
            Some(4_000),
            "(My way)",
            vec![
                syllable(3_000, 3_100, "("),
                syllable(3_100, 4_000, "My way)"),
            ],
        ),
    ];

    fold_bracketed_echoes(&mut lines);

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "Know the way");
    assert_eq!(background_text(&lines[0]), Some("My way"));
}

#[test]
fn a_folded_echo_keeps_its_own_translation() {
    // The echo is translated where it is sung, so its translation travels with it
    // rather than being lost with the line it stops being.
    let mut lines = vec![
        line(
            1_000,
            Some(3_000),
            "Know the way",
            vec![syllable(1_000, 3_000, "Know the way")],
        ),
        line(
            3_000,
            Some(4_000),
            "(My way)",
            vec![syllable(3_000, 4_000, "(My way)")],
        ),
    ];
    lines[0].translation = Some("要知道方法".to_string());
    lines[1].translation = Some("（从我身边离开的方法）".to_string());

    fold_bracketed_echoes(&mut lines);

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].translation.as_deref(), Some("要知道方法"));
    let background = lines[0].background.as_ref().expect("the echo is folded in");
    assert_eq!(background.text, "My way");
    assert_eq!(
        (background.start_ms, background.end_ms),
        (3_000, Some(4_000)),
        "the echo is sung where its own words are, not where its parent runs"
    );
    assert_eq!(
        background
            .syllables
            .iter()
            .map(|syllable| (syllable.text.as_str(), syllable.start_ms))
            .collect::<Vec<_>>(),
        vec![("My way", 3_000)],
        "the bracket is not sung, so it is not a word of the echo"
    );
    assert_eq!(
        background.translation.as_deref(),
        Some("从我身边离开的方法"),
        "the bracket the phrase was written in is not repeated in its translation"
    );
}

#[test]
fn a_bracketed_echo_answers_the_rest_its_line_leaves() {
    // Saddle Up's backing vocal: `Are you man enough to hold it down` runs out at
    // 109072ms and the echo of it answers 1.5s later, after the rest between them.
    let mut lines = vec![
        line(
            1_000,
            Some(3_172),
            "Are you man enough to hold it down",
            vec![syllable(1_000, 3_172, "Are you man enough to hold it down")],
        ),
        line(
            4_639,
            Some(6_751),
            "(I wanna see you hold it down for me)",
            vec![syllable(
                4_639,
                6_751,
                "(I wanna see you hold it down for me)",
            )],
        ),
    ];

    fold_bracketed_echoes(&mut lines);

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "Are you man enough to hold it down");
    assert_eq!(
        background_text(&lines[0]),
        Some("I wanna see you hold it down for me")
    );
}

#[test]
fn a_bracketed_phrase_written_across_rows_joins_the_line_it_answers() {
    // The way Saddle Up repeats its chorus under the outro: the phrase opens on the
    // row it starts on, continues on the rows between, and closes on the row it
    // ends on, and the line after it is a line of its own again.
    let mut lines = vec![
        line(
            1_000,
            Some(2_586),
            "come and drive me crazy",
            vec![syllable(1_000, 2_586, "come and drive me crazy")],
        ),
        line(
            2_586,
            Some(3_000),
            "(If you walk it",
            vec![
                syllable(2_586, 2_700, "(If "),
                syllable(2_700, 2_800, "you "),
                syllable(2_800, 2_900, "walk "),
                syllable(2_900, 3_000, "it"),
            ],
        ),
        line(
            3_000,
            Some(3_300),
            "Baby, stand up",
            vec![
                syllable(3_000, 3_100, "Baby, "),
                syllable(3_100, 3_200, "stand "),
                syllable(3_200, 3_300, "up"),
            ],
        ),
        line(
            3_300,
            Some(3_391),
            "Baby, we go up)",
            vec![
                syllable(3_300, 3_330, "Baby, "),
                syllable(3_330, 3_360, "we "),
                syllable(3_360, 3_390, "go "),
                syllable(3_390, 3_391, "up)"),
            ],
        ),
        line(
            4_000,
            Some(5_000),
            "Put your money",
            vec![syllable(4_000, 5_000, "Put your money")],
        ),
    ];
    lines[1].translation = Some("如果你言行一致".to_string());
    lines[2].translation = Some("宝贝 请挺身而出".to_string());

    fold_bracketed_echoes(&mut lines);

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1].text, "Put your money");
    assert_eq!(lines[0].text, "come and drive me crazy");
    let background = lines[0]
        .background
        .as_ref()
        .expect("the phrase is folded in");
    assert_eq!(
        background.text,
        "If you walk it Baby, stand up Baby, we go up"
    );
    assert_eq!(
        background
            .syllables
            .iter()
            .map(|syllable| syllable.text.as_str())
            .collect::<String>(),
        background.text,
        "the words spell the phrase the phrase is written as"
    );
    assert_eq!(
        (background.start_ms, background.end_ms),
        (2_586, Some(3_391)),
        "the phrase is sung from where its first row starts to where its last ends"
    );
    assert_eq!(
        background.translation.as_deref(),
        Some("如果你言行一致 宝贝 请挺身而出"),
        "a phrase written across rows is translated on each of them"
    );
}

#[test]
fn an_unmatched_bracket_does_not_reach_across_the_song() {
    // A bracket standing open over the rows that follow it is not a phrase those
    // rows belong to: they are too far after the line that would have answered.
    let mut lines = vec![
        line(
            1_000,
            Some(2_000),
            "Know the way",
            vec![syllable(1_000, 2_000, "Know the way")],
        ),
        line(
            2_000,
            Some(3_000),
            "(hold on",
            vec![syllable(2_000, 3_000, "(hold on")],
        ),
        line(
            20_000,
            Some(21_000),
            "sing it",
            vec![syllable(20_000, 21_000, "sing it")],
        ),
        line(
            21_000,
            Some(22_000),
            "again)",
            vec![syllable(21_000, 22_000, "again)")],
        ),
    ];

    fold_bracketed_echoes(&mut lines);

    assert_eq!(lines.len(), 4);
    assert!(lines[0].background.is_none());
}

#[test]
fn an_unbracketed_translation_keeps_its_text() {
    assert_eq!(unwrap_brackets("来吧 尽管…"), "来吧 尽管…");
    assert_eq!(unwrap_brackets("  (来吧 尽管…)  "), "来吧 尽管…");
    assert_eq!(unwrap_brackets("【来吧 尽管…】"), "来吧 尽管…");
    assert_eq!(
        unwrap_brackets("(来吧"),
        "(来吧",
        "a bracket without its pair is part of the text"
    );
    assert_eq!(unwrap_brackets("("), "(");
}

#[test]
fn a_bracketed_line_far_from_the_previous_one_stays_its_own_line() {
    // Bracketed text elsewhere in the song answers nothing; only a line timed to
    // the one before it is an echo of it.
    let mut lines = vec![
        line(
            1_000,
            Some(2_000),
            "Know the way",
            vec![syllable(1_000, 2_000, "Know the way")],
        ),
        line(
            9_000,
            Some(10_000),
            "(Instrumental)",
            vec![syllable(9_000, 10_000, "(Instrumental)")],
        ),
    ];

    fold_bracketed_echoes(&mut lines);

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1].text, "(Instrumental)");
    assert!(lines[0].background.is_none());
}

#[test]
fn a_bracketed_opening_line_stays_one_ordinary_line() {
    // Nothing precedes it, so there is no line for it to belong to.
    let mut lines = vec![line(
        0,
        Some(1_000),
        "（Ella ella）",
        vec![syllable(0, 1_000, "（Ella ella）")],
    )];

    fold_bracketed_echoes(&mut lines);

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "（Ella ella）");
    assert!(lines[0].background.is_none());
}

#[test]
fn a_joint_label_takes_its_side_from_the_performer_it_names_first() {
    // QQ Music divides "Problem" with joint credits, and Big Sean appears in one
    // of them without being listed as an artist.
    let mut lines = vec![
        line(0, Some(500), "Iggy Azalea/Ariana Grande：", Vec::new()),
        line(1_000, Some(2_000), "Uh-huh it's Iggy", Vec::new()),
        line(2_000, Some(2_500), "Big Sean/Ariana Grande：", Vec::new()),
        line(3_000, Some(4_000), "One less problem", Vec::new()),
    ];

    apply_speaker_labels(&mut lines, &artists(&["Ariana Grande", "Iggy Azalea"]));

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].text, "Uh-huh it's Iggy");
    assert_eq!(lines[0].voice, Voice::Secondary);
    assert_eq!(lines[1].text, "One less problem");
    assert_eq!(lines[1].voice, Voice::Primary);
}

#[test]
fn a_label_naming_nobody_on_the_record_is_not_a_label() {
    let mut lines = vec![line(
        1_000,
        Some(2_000),
        "Big Sean/Some Guy: line",
        vec![
            syllable(1_000, 1_500, "Big Sean/Some Guy: "),
            syllable(1_500, 2_000, "line"),
        ],
    )];

    apply_speaker_labels(&mut lines, &artists(&["Ariana Grande", "Iggy Azalea"]));

    assert_eq!(lines[0].text, "Big Sean/Some Guy: line");
    assert_eq!(lines[0].voice, Voice::Primary);
}

#[test]
fn an_inline_label_leaves_the_word_timing_behind() {
    let mut lines = vec![line(
        1_000,
        Some(2_000),
        "Doja Cat: sing it",
        vec![
            syllable(1_000, 1_200, "Doja "),
            syllable(1_200, 1_500, "Cat: "),
            syllable(1_500, 2_000, "sing it"),
        ],
    )];

    apply_speaker_labels(&mut lines, &artists(&["Doja Cat", "SZA"]));

    assert_eq!(lines[0].text, "sing it");
    assert_eq!(lines[0].voice, Voice::Primary);
    assert_eq!(texts(&lines[0]), vec!["sing it"]);
}

#[test]
fn a_second_performer_switches_the_voice() {
    let mut lines = vec![
        line(0, Some(1_000), "The Weeknd：", Vec::new()),
        line(1_000, Some(2_000), "I can't feel my face", Vec::new()),
        line(2_000, Some(3_000), "Take my hand", Vec::new()),
    ];

    apply_speaker_labels(&mut lines, &artists(&["Ariana Grande", "The Weeknd"]));

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].voice, Voice::Secondary);
    assert_eq!(lines[1].voice, Voice::Secondary);
}

#[test]
fn a_sung_colon_without_a_label_keeps_the_line() {
    let mut lines = vec![line(0, Some(1_000), "Love: it hurts", Vec::new())];

    apply_speaker_labels(&mut lines, &artists(&["Ariana Grande"]));

    assert_eq!(lines[0].text, "Love: it hurts");
    assert_eq!(lines[0].voice, Voice::Primary);
}

#[test]
fn a_joint_label_gives_its_part_back_to_the_main_voice() {
    // "Save Your Tears (Remix)" divides its verses between the two performers and
    // gives the last chorus to both, which QQ Music writes as a label of its own.
    let mut lines = vec![
        line(0, Some(500), "The Weeknd：", Vec::new()),
        line(
            1_000,
            Some(2_000),
            "I saw you dancing in a crowded room",
            Vec::new(),
        ),
        line(2_000, Some(2_500), "Ariana Grande：", Vec::new()),
        line(
            3_000,
            Some(4_000),
            "Met you once under a Pisces moon",
            Vec::new(),
        ),
        line(4_000, Some(4_500), "Both：", Vec::new()),
        line(
            5_000,
            Some(6_000),
            "I don't know why I run away",
            Vec::new(),
        ),
    ];

    apply_speaker_labels(&mut lines, &artists(&["The Weeknd", "Ariana Grande"]));

    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].voice, Voice::Primary);
    assert_eq!(lines[1].voice, Voice::Secondary);
    assert_eq!(
        (lines[2].text.as_str(), lines[2].voice),
        ("I don't know why I run away", Voice::Primary),
        "the part the two of them sing goes back to the main voice"
    );
}

#[test]
fn a_joint_label_written_in_chinese_gives_its_part_back_to_the_main_voice() {
    // A Chinese transcription writes the same thing as `合：` or `合唱：`, either on a
    // row of its own or in front of the words the two of them sing.
    let mut lines = vec![
        line(0, Some(500), "合：", Vec::new()),
        line(1_000, Some(2_000), "合唱：我们一起走吧", Vec::new()),
    ];

    apply_speaker_labels(&mut lines, &artists(&["某人"]));

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "我们一起走吧");
    assert_eq!(lines[0].voice, Voice::Primary);
}

/// A word-timed row whose words are the words of `text`, timed evenly from
/// `start_ms`.
///
/// The transcription of "Saddle Up" times every word of every row, which is what a
/// sentence broken across rows is joined by.
fn word_timed_line(start_ms: u64, end_ms: u64, text: &str) -> TimedLine {
    let words = text.split(' ').collect::<Vec<_>>();
    let last = words.len() - 1;
    let step = (end_ms - start_ms) / words.len() as u64;
    let syllables = words
        .iter()
        .enumerate()
        .map(|(index, word)| {
            let word_start = start_ms + step * index as u64;
            let word_end = if index == last {
                end_ms
            } else {
                word_start + step
            };
            let text = if index == last {
                (*word).to_string()
            } else {
                format!("{word} ")
            };
            syllable(word_start, word_end, &text)
        })
        .collect();
    line(start_ms, Some(end_ms), text, syllables)
}

fn echo(text: &str) -> BackgroundVocal {
    BackgroundVocal {
        text: text.to_string(),
        translation: None,
        start_ms: 0,
        end_ms: None,
        syllables: Vec::new(),
    }
}

#[test]
fn a_sentence_broken_at_a_comma_is_joined_into_one_line() {
    // Saddle Up writes "Don't be shy, come and drive me crazy" as two rows, the
    // second timed to start where the first runs out.
    let mut lines = vec![
        word_timed_line(194_619, 195_413, "Don't be shy,"),
        word_timed_line(195_413, 196_999, "come and drive me crazy"),
    ];
    lines[0].translation = Some("不要害羞".to_string());
    lines[1].translation = Some("让我陷入疯狂".to_string());

    merge_continued_lines(&mut lines);

    assert_eq!(lines.len(), 1, "the rows are one sentence");
    assert_eq!(lines[0].text, "Don't be shy, come and drive me crazy");
    assert_eq!(
        lines[0].translation.as_deref(),
        Some("不要害羞 让我陷入疯狂"),
        "the translation of each row is translated with it"
    );
    assert_eq!(
        texts(&lines[0]),
        vec![
            "Don't ", "be ", "shy, ", "come ", "and ", "drive ", "me ", "crazy"
        ],
        "the words of the sentence are the words both rows were drawn with"
    );
    assert_eq!(lines[0].start_ms, 194_619);
    assert_eq!(lines[0].end_ms, Some(196_999));
}

#[test]
fn the_english_pronoun_carries_the_rest_of_a_sentence() {
    let mut lines = vec![
        word_timed_line(108_000, 108_582, "A tragedy, Ms. RIP,"),
        word_timed_line(108_582, 109_800, "I came for a reason"),
    ];

    merge_continued_lines(&mut lines);

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "A tragedy, Ms. RIP, I came for a reason");
}

#[test]
fn the_rest_of_a_sentence_is_joined_without_punctuation_at_the_break() {
    // Saddle Up writes `Put your money` and `where your mouth is` as two rows: the
    // break falls where the line ran out rather than at a mark of its own.
    let mut lines = vec![
        word_timed_line(197_804, 198_300, "Put your money"),
        word_timed_line(198_300, 199_100, "where your mouth is"),
    ];

    merge_continued_lines(&mut lines);

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "Put your money where your mouth is");
    assert_eq!(lines[0].end_ms, Some(199_100));
}

#[test]
fn a_hook_timed_apart_from_the_row_before_it_keeps_its_rows() {
    // "WDA (Whole Different Animal)" writes its hook as rows of half a second each,
    // a rest apart from one another, and translates each of them; only the rest of a
    // sentence begins where the row it continues runs out.
    let mut lines = vec![
        word_timed_line(28_415, 29_417, "She a Whole Different Animal"),
        word_timed_line(29_953, 30_779, "different animal"),
    ];

    merge_continued_lines(&mut lines);

    assert_eq!(lines.len(), 2, "the rows are the hook and not one sentence");
}

#[test]
fn a_row_the_punctuation_left_open_is_joined_however_late_it_is_timed() {
    // The rows of "LEMONADE" are a second apart and the comma the first one ends in
    // is what says the sentence goes on, so the timing has nothing to say about them.
    let mut lines = vec![
        word_timed_line(39_491, 39_991, "Like zip,"),
        word_timed_line(40_572, 40_984, "I don't care"),
    ];

    merge_continued_lines(&mut lines);

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "Like zip, I don't care");
}

#[test]
fn a_line_timed_payload_joins_its_rows_by_case_alone() {
    // A line-timed row states no end time, so it says nothing about when the row
    // after it begins and the case of the letters is all there is to read.
    let mut lines = vec![
        line(1_000, None, "Put your money", Vec::new()),
        line(5_000, None, "where your mouth is", Vec::new()),
    ];

    merge_continued_lines(&mut lines);

    assert_eq!(lines.len(), 1);
}

#[test]
fn a_sentence_the_punctuation_closed_keeps_its_rows() {
    let mut lines = vec![
        word_timed_line(0, 1_000, "I'm not your enemy."),
        word_timed_line(1_000, 2_000, "i already know"),
    ];

    merge_continued_lines(&mut lines);

    assert_eq!(lines.len(), 2, "the row before says its sentence ended");
}

#[test]
fn the_english_pronoun_begins_a_sentence_without_the_punctuation_to_carry_on() {
    // A row that says nothing either way leaves the decision to the row after it,
    // and `I` says as little as any capitalized word — 16 Bit writes
    // `These days` then `I can't picture my face` as two lines.
    let mut lines = vec![
        word_timed_line(0, 1_000, "These days"),
        word_timed_line(1_000, 2_000, "I can't picture my face"),
    ];

    merge_continued_lines(&mut lines);

    assert_eq!(lines.len(), 2);
}

#[test]
fn a_row_in_a_script_without_letter_case_says_nothing_about_the_sentence() {
    // A line-timed payload writes `[00:01.00]こんにちは世界` then `[00:05.00]bye`:
    // the row before the lowercase one ends in a script that has no case, so it says
    // nothing about a sentence continuing and the row after it stands alone.
    let mut lines = vec![
        line(1_000, Some(3_000), "こんにちは世界", Vec::new()),
        line(3_000, Some(5_000), "bye", Vec::new()),
    ];

    merge_continued_lines(&mut lines);

    assert_eq!(lines.len(), 2);
}

#[test]
fn a_row_that_begins_a_sentence_keeps_its_own_line() {
    let mut lines = vec![
        word_timed_line(158_851, 159_400, "Boy, Saddle Up,"),
        word_timed_line(159_400, 160_800, "Don't waste my time"),
    ];

    merge_continued_lines(&mut lines);

    assert_eq!(
        lines.len(),
        2,
        "a capitalized word begins a sentence of its own"
    );
}

#[test]
fn a_script_without_letter_case_keeps_its_rows() {
    let mut lines = vec![
        word_timed_line(148_870, 149_500, "这还远远不够，"),
        word_timed_line(149_500, 150_600, "我直言不讳"),
    ];

    merge_continued_lines(&mut lines);

    assert_eq!(
        lines.len(),
        2,
        "a comma breaks the line there rather than leaving it open"
    );
}

#[test]
fn a_duet_answer_keeps_its_own_row() {
    let mut lines = vec![
        word_timed_line(0, 1_000, "hold on,"),
        word_timed_line(1_000, 2_000, "i got you"),
    ];
    lines[1].voice = Voice::Secondary;

    merge_continued_lines(&mut lines);

    assert_eq!(
        lines.len(),
        2,
        "the other performer sings a line of their own"
    );
}

#[test]
fn a_sentence_broken_twice_is_joined_into_one_line() {
    let mut lines = vec![
        word_timed_line(0, 1_000, "Take the reins,"),
        word_timed_line(1_000, 2_000, "buckle up,"),
        word_timed_line(2_000, 3_000, "my baby"),
    ];

    merge_continued_lines(&mut lines);

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "Take the reins, buckle up, my baby");
    assert_eq!(lines[0].end_ms, Some(3_000));
}

#[test]
fn the_rest_of_a_sentence_keeps_the_echo_it_answers_with() {
    let mut lines = vec![
        word_timed_line(194_619, 195_413, "Don't be shy,"),
        word_timed_line(195_413, 196_999, "come and drive me crazy"),
    ];
    lines[1].background = Some(echo("If you walk it like you talk it"));

    merge_continued_lines(&mut lines);

    assert_eq!(lines.len(), 1);
    assert_eq!(
        background_text(&lines[0]),
        Some("If you walk it like you talk it"),
        "the echo answers the sentence it was written under"
    );
}

#[test]
fn rows_timed_differently_keep_their_own_lines() {
    // A line-timed row has no words of its own, so the merged line could not be
    // drawn from the words of both rows.
    let mut lines = vec![
        word_timed_line(0, 1_000, "hold on,"),
        line(1_000, Some(2_000), "i got you", Vec::new()),
    ];

    merge_continued_lines(&mut lines);

    assert_eq!(lines.len(), 2);
}

#[test]
fn two_echoes_are_not_joined_into_one_line() {
    let mut lines = vec![
        word_timed_line(0, 1_000, "hold on,"),
        word_timed_line(1_000, 2_000, "i got you"),
    ];
    lines[0].background = Some(echo("yeah"));
    lines[1].background = Some(echo("oh"));

    merge_continued_lines(&mut lines);

    assert_eq!(lines.len(), 2, "a line carries one background vocal");
}
