// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Korean Revised Romanization with syllable-aligned readings.
//!
//! A Hangul syllable is read differently from how it is written — `신라` is
//! "silla", `종로` is "jongno", `닭` is "dak" — so the run is read the way it is
//! spoken first and the readings are then handed out one syllable at a time, in
//! the order the syllables are written.

use super::super::model::RomanizationSegment;

/// Initial jamo of a Hangul syllable, in the order the syllable blocks number them.
const ONSETS: [Onset; 19] = [
    Onset::G,
    Onset::Kk,
    Onset::N,
    Onset::D,
    Onset::Tt,
    Onset::R,
    Onset::M,
    Onset::B,
    Onset::Pp,
    Onset::S,
    Onset::Ss,
    Onset::None,
    Onset::J,
    Onset::Jj,
    Onset::Ch,
    Onset::K,
    Onset::T,
    Onset::P,
    Onset::H,
];

/// Vowel jamo, written the way the Revised Romanization spells them.
const VOWELS: [&str; 21] = [
    "a", "ae", "ya", "yae", "eo", "e", "yeo", "ye", "o", "wa", "wae", "oe", "yo", "u", "wo", "we",
    "wi", "yu", "eu", "ui", "i",
];

/// The final jamo of `밟`, which is read as ㅂ before a consonant (밟다 bapda)
/// where the other ㄼ words keep the ㄹ (여덟 yeodeol).
const BAP: u8 = 11;

/// Whether `text` contains a Hangul syllable.
pub(super) fn has_hangul(text: &str) -> bool {
    text.chars().any(is_hangul_syllable)
}

/// Builds the line reading and the per-syllable segments from one scan of the
/// script runs of `text`.
///
/// A run that contains an alphabetic character outside the Hangul syllable block
/// (for example a Latin word in a mixed Korean line) contributes no reading and is
/// dropped from the line, while runs without letters (whitespace, punctuation,
/// digits, symbols) survive as separators. Every run still yields its segments.
pub(super) fn romanize(text: &str) -> (String, Vec<RomanizationSegment>) {
    let mut output = String::with_capacity(text.len());
    let mut segments = Vec::new();

    for run in script_runs_by(text, is_hangul_syllable) {
        if run
            .chars()
            .any(|character| character.is_alphabetic() && !is_hangul_syllable(character))
        {
            segments.push(RomanizationSegment {
                text: run,
                romanization: String::new(),
                furigana: String::new(),
            });
            continue;
        }
        if run.chars().all(is_hangul_syllable) {
            let readings = run_readings(&run);
            super::push_separator(&mut output);
            for reading in &readings {
                output.push_str(reading);
            }
            segments.extend(run.chars().zip(readings).map(|(character, romanization)| {
                RomanizationSegment {
                    text: character.to_string(),
                    romanization,
                    furigana: String::new(),
                }
            }));
        } else {
            for character in run.chars() {
                if character.is_whitespace() {
                    super::push_separator(&mut output);
                } else {
                    output.push(character);
                }
            }
            segments.push(RomanizationSegment {
                text: run,
                romanization: String::new(),
                furigana: String::new(),
            });
        }
    }

    (output.trim().to_string(), segments)
}

fn script_runs_by(text: &str, predicate: impl Fn(char) -> bool) -> Vec<String> {
    let mut runs = Vec::new();
    let mut current = String::new();
    let mut current_matches = None;
    for character in text.chars() {
        let matches = predicate(character);
        if current_matches.is_some_and(|previous| previous != matches) {
            runs.push(std::mem::take(&mut current));
        }
        current.push(character);
        current_matches = Some(matches);
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs
}

/// The reading of every syllable of a run of Hangul.
///
/// The run is read as a whole because a syllable is read with the ones around it:
/// the final jamo of a syllable and the initial jamo of the next meet in the
/// middle, so `신라` reads "sil" and "la" and `종로` reads "jong" and "no".
fn run_readings(text: &str) -> Vec<String> {
    let syllables = text.chars().filter_map(Syllable::of).collect::<Vec<_>>();
    let mut onsets = syllables
        .iter()
        .map(|syllable| syllable.onset)
        .collect::<Vec<_>>();
    let mut codas = vec![Coda::None; syllables.len()];

    for (index, syllable) in syllables.iter().enumerate() {
        let next = syllables.get(index + 1);
        let vowel_next = onsets.get(index + 1) == Some(&Onset::None);
        let (mut coda, mut carried) = split_coda(syllable.coda, vowel_next);
        // 밟 before a consonant keeps the ㅂ where ㄼ otherwise keeps the ㄹ.
        if syllable.coda == BAP && syllable.text == '밟' && !vowel_next {
            coda = Coda::P;
            carried = None;
        }
        // A cluster is read with the syllable after it when that one starts with
        // a vowel, so the jamo it keeps is the one it is heard with there: 값이
        // reads "gapsi" and 닭이 reads "dalgi".
        if vowel_next && let Some(carried) = carried.filter(|carried| *carried != Onset::H) {
            onsets[index + 1] = carried;
        }

        let palatal =
            next.is_some_and(|next| next.onset == Onset::None && VOWELS[next.vowel] == "i");
        // A ㅎ, on its own or as the second jamo of ㄶ or ㅀ, merges into the
        // consonant after it: 좋고 reads "joko" and 놓다 reads "nota".
        let aspirating = coda == Coda::H || carried == Some(Onset::H);
        if coda == Coda::H {
            // A ㅎ at the end of a word is read as the ㄷ it becomes: 히읗 reads
            // "hieut".
            coda = if next.is_some() { Coda::None } else { Coda::T };
        }
        if aspirating {
            match onsets.get(index + 1).copied() {
                Some(Onset::G) => onsets[index + 1] = Onset::K,
                Some(Onset::D) => {
                    // The ㅌ a ㄷ and a ㅎ make is read as ㅊ before 이, so 닫히다
                    // reads "dachida".
                    onsets[index + 1] = if palatal && matches!(syllable.coda, 7 | 25) {
                        Onset::Ch
                    } else {
                        Onset::T
                    };
                }
                Some(Onset::J) => onsets[index + 1] = Onset::Ch,
                Some(Onset::S) => onsets[index + 1] = Onset::Ss,
                Some(Onset::N | Onset::M) => coda = Coda::N,
                _ => {}
            }
        }

        // ㄷ, ㅌ, and ㅊ are read as ㅈ and ㅊ before the vowel 이: 굳이 reads
        // "guji" and 같이 reads "gachi".
        if palatal && matches!(syllable.coda, 7 | 23 | 25) {
            coda = Coda::None;
            onsets[index + 1] = if syllable.coda == 7 {
                Onset::J
            } else {
                Onset::Ch
            };
        }

        // A ㅎ in the onset is absorbed by the jamo before it: 묵호 reads "mukho".
        if let Some(aspirated) = match (coda, onsets.get(index + 1)) {
            (Coda::K, Some(Onset::H)) => Some(Onset::K),
            (Coda::T, Some(Onset::H)) => Some(Onset::T),
            (Coda::P, Some(Onset::H)) => Some(Onset::P),
            _ => None,
        } {
            coda = Coda::None;
            onsets[index + 1] = aspirated;
        }

        // ㄱ, ㄷ, and ㅂ are read as ㅇ, ㄴ, and ㅁ before ㄴ and ㅁ: 백마 reads
        // "baengma" and 한국말 reads "hangungmal".
        if matches!(onsets.get(index + 1), Some(Onset::N | Onset::M)) {
            coda = nasal_coda(coda);
        }

        // A ㄹ next to a ㄴ is read as one long l: 신라 reads "silla" and 설날
        // reads "seollal". A ㄹ after ㅇ or ㅁ is read as ㄴ: 종로 reads "jongno".
        match (coda, onsets.get(index + 1)) {
            (Coda::N, Some(Onset::R)) => {
                coda = Coda::L;
                onsets[index + 1] = Onset::Ll;
            }
            (Coda::L, Some(Onset::N | Onset::R)) => onsets[index + 1] = Onset::Ll,
            (Coda::Ng | Coda::M, Some(Onset::R)) => onsets[index + 1] = Onset::N,
            (Coda::K | Coda::T | Coda::P, Some(Onset::R)) => {
                coda = nasal_coda(coda);
                onsets[index + 1] = Onset::N;
            }
            _ => {}
        }

        // A ㄱ, ㄷ, ㅂ, or ㄹ before a syllable that starts with a y sound takes
        // the ㄴ or the ㄹ that sound adds: 학여울 reads "hangnyeoul", 십육 reads
        // "simnyuk", and 알약 reads "allyak". This is the insertion the standard
        // describes for compounds, and it needs the word to be known, so only the
        // sounds it applies to unambiguously are read this way.
        if let Some(next) = next
            && next.onset == Onset::None
            && is_glide_vowel(VOWELS[next.vowel])
        {
            match coda {
                Coda::K | Coda::T | Coda::P => {
                    coda = nasal_coda(coda);
                    onsets[index + 1] = Onset::N;
                }
                Coda::L => onsets[index + 1] = Onset::Ll,
                _ => {}
            }
        }

        // A jamo that can stand before a vowel is read with the syllable after
        // it: 옷이 reads "osi", 있어 reads "isseo", and 물이 reads "muri".
        if vowel_next
            && onsets.get(index + 1) == Some(&Onset::None)
            && let Some(moved) = move_to_onset(syllable.coda)
        {
            coda = Coda::None;
            onsets[index + 1] = moved;
        }

        codas[index] = coda;
    }

    (0..syllables.len())
        .map(|index| {
            let mut reading = String::new();
            reading.push_str(onsets[index].romanize());
            reading.push_str(VOWELS[syllables[index].vowel]);
            reading.push_str(codas[index].romanize());
            reading
        })
        .collect()
}

/// One Hangul syllable, split into the jamo it is written with.
struct Syllable {
    /// The character the syllable is written with.
    text: char,
    /// Index of the initial jamo in [`ONSETS`].
    onset: Onset,
    /// Index of the vowel jamo in [`VOWELS`].
    vowel: usize,
    /// Index of the final jamo, or zero when the syllable ends with its vowel.
    coda: u8,
}

impl Syllable {
    fn of(character: char) -> Option<Self> {
        let code = character as u32;
        is_hangul_syllable(character).then(|| {
            let index = code - 0xAC00;
            Self {
                text: character,
                onset: ONSETS[(index / 588) as usize],
                vowel: ((index % 588) / 28) as usize,
                coda: (index % 28) as u8,
            }
        })
    }
}

/// The sound a syllable is left with, and the jamo a cluster hands to the next
/// syllable when that one starts with a vowel.
///
/// The jamo a cluster hands on is written with its plain letter: a sound that is
/// tensed because it follows another consonant is written the way it is spelled
/// (없어 reads "eopseo" and 밟다 reads "bapda").
fn split_coda(coda: u8, vowel_next: bool) -> (Coda, Option<Onset>) {
    match coda {
        // A ㄺ, ㄻ, or ㄿ keeps its ㄹ when a vowel follows and the other jamo
        // when it does not, so 닭 is "dak" and 닭이 is "dalgi".
        9 => (if vowel_next { Coda::L } else { Coda::K }, Some(Onset::G)),
        10 => (if vowel_next { Coda::L } else { Coda::M }, Some(Onset::M)),
        14 => (if vowel_next { Coda::L } else { Coda::P }, Some(Onset::P)),
        1 | 2 | 24 => (Coda::K, None),
        3 => (Coda::K, Some(Onset::S)),
        4 => (Coda::N, None),
        5 => (Coda::N, Some(Onset::J)),
        6 => (Coda::N, Some(Onset::H)),
        7 => (Coda::T, None),
        8 => (Coda::L, None),
        11 => (Coda::L, Some(Onset::B)),
        12 | 13 => (Coda::L, None),
        15 => (Coda::L, Some(Onset::H)),
        16 => (Coda::M, None),
        17 => (Coda::P, None),
        18 => (Coda::P, Some(Onset::S)),
        19 | 20 | 22 | 23 | 25 => (Coda::T, None),
        21 => (Coda::Ng, None),
        26 => (Coda::P, None),
        27 => (Coda::H, None),
        _ => (Coda::None, None),
    }
}

/// The jamo a syllable ending in one is read with when the syllable after it
/// starts with a vowel, or `None` when it stays where it is written: a ㄴ, a ㅁ,
/// and a ㅇ are heard as themselves at the end of a syllable.
fn move_to_onset(coda: u8) -> Option<Onset> {
    match coda {
        1 => Some(Onset::G),
        2 => Some(Onset::Kk),
        7 => Some(Onset::D),
        8 => Some(Onset::R),
        17 => Some(Onset::B),
        19 => Some(Onset::S),
        20 => Some(Onset::Ss),
        22 => Some(Onset::J),
        23 => Some(Onset::Ch),
        24 => Some(Onset::K),
        25 => Some(Onset::T),
        26 => Some(Onset::P),
        _ => None,
    }
}

/// The sound a ㄱ, ㄷ, or ㅂ ends up as before a ㄴ or a ㄹ.
fn nasal_coda(coda: Coda) -> Coda {
    match coda {
        Coda::K => Coda::Ng,
        Coda::T => Coda::N,
        Coda::P => Coda::M,
        _ => coda,
    }
}

/// Whether a vowel is written with a y sound, which is where the ㄴ of a compound
/// is heard.
fn is_glide_vowel(vowel: &str) -> bool {
    matches!(vowel, "ya" | "yae" | "yeo" | "ye" | "yo" | "yu")
}

fn is_hangul_syllable(character: char) -> bool {
    matches!(character as u32, 0xAC00..=0xD7A3)
}

/// The initial sound of a syllable after the pronunciation rules.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Onset {
    None,
    G,
    Kk,
    N,
    D,
    Tt,
    R,
    /// A ㄹ that is read as an l because a ㄹ or ㄴ came before it.
    Ll,
    M,
    B,
    Pp,
    S,
    Ss,
    J,
    Jj,
    Ch,
    K,
    T,
    P,
    H,
}

impl Onset {
    fn romanize(self) -> &'static str {
        match self {
            Self::None => "",
            Self::G => "g",
            Self::Kk => "kk",
            Self::N => "n",
            Self::D => "d",
            Self::Tt => "tt",
            Self::R => "r",
            Self::Ll => "l",
            Self::M => "m",
            Self::B => "b",
            Self::Pp => "pp",
            Self::S => "s",
            Self::Ss => "ss",
            Self::J => "j",
            Self::Jj => "jj",
            Self::Ch => "ch",
            Self::K => "k",
            Self::T => "t",
            Self::P => "p",
            Self::H => "h",
        }
    }
}

/// The final sound of a syllable after the pronunciation rules.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Coda {
    None,
    /// ㅎ, which is never heard as itself and is always read with what follows.
    H,
    K,
    N,
    T,
    L,
    M,
    P,
    Ng,
}

impl Coda {
    /// The letter for the sound a syllable ends with.
    fn romanize(self) -> &'static str {
        match self {
            Self::None | Self::H => "",
            Self::K => "k",
            Self::N => "n",
            Self::T => "t",
            Self::L => "l",
            Self::M => "m",
            Self::P => "p",
            Self::Ng => "ng",
        }
    }
}
