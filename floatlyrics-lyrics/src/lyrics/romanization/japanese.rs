// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Japanese readings, furigana alignment, and Hepburn Romaji.
//!
//! The word a line is read with comes from IPADIC, and the part of that reading
//! each character carries comes from the JmdictFurigana data, which
//! `jmdict-furigana` embeds. That data is published under CC-BY-SA 4.0 (<https://github.com/Doublevil/JmdictFurigana>); the crate itself is
//! MIT or Apache-2.0.

use std::borrow::Cow;
use std::panic::AssertUnwindSafe;
use std::sync::LazyLock;

use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::token::Token;

use super::super::model::RomanizationSegment;
use super::push_separator;

/// Fields the IPADIC details put the kana readings in.
const READING: usize = 7;
const PRONUNCIATION: usize = 8;

/// Small kana: they are read as part of the mora the character before them
/// starts, so they never carry a reading of their own.
const SMALL_KANA: &str = "ぁぃぅぇぉっゃゅょゎゕゖァィゥェォッャュョヮヵヶ";

/// Small tsu: it is heard as the consonant of the mora after it.
const SMALL_TSU: &str = "っッ";

/// Prolonged sound mark: it lengthens the vowel of the mora before it.
const PROLONGED_SOUND_MARK: char = 'ー';

/// The analyzer every Japanese line is read with.
static ANALYZER: LazyLock<Option<Segmenter>> =
    LazyLock::new(|| match load_dictionary("embedded://ipadic") {
        Ok(dictionary) => Some(Segmenter::new(Mode::Normal, dictionary, None)),
        Err(error) => {
            tracing::warn!(%error, "the embedded IPADIC dictionary could not be loaded");
            None
        }
    });

/// The shared analyzer, or `None` when the embedded dictionary cannot be read.
///
/// The dictionary is compiled into the binary, so a failure here means the crate
/// was built without it; Japanese lines then stay without readings instead of
/// bringing the process down.
fn analyzer() -> Option<&'static Segmenter> {
    ANALYZER.as_ref()
}

/// Whether the embedded furigana dictionary could be read.
///
/// The dictionary is parsed once, on the first Japanese word that needs it, and
/// is what tells a word's reading apart by the character it belongs to. Its crate
/// panics when the embedded archive cannot be read, which only a broken build
/// could cause, so a failure leaves every word read by the kana it is spelled
/// with instead of bringing the process down.
static FURIGANA: LazyLock<bool> = LazyLock::new(|| {
    let read = std::panic::catch_unwind(AssertUnwindSafe(|| {
        futures::executor::block_on(jmdict_furigana::init());
    }))
    .is_ok();
    if !read {
        tracing::warn!("the embedded furigana dictionary could not be read");
    }
    read
});

/// Whether `text` contains kana, which makes it Japanese rather than Chinese.
pub(super) fn contains_kana(text: &str) -> bool {
    text.chars().any(is_kana)
}

/// Whether every Han character of `text` is read by the Japanese dictionary.
///
/// A Han-only line is Japanese when the dictionary knows all of it and Chinese
/// otherwise, which is what tells the two apart when a document mixes them.
pub(super) fn reads_han_text(text: &str) -> bool {
    let Some(analyzer) = analyzer() else {
        return false;
    };
    let Ok(mut tokens) = analyzer.segment(Cow::Borrowed(text)) else {
        return false;
    };
    tokens
        .iter_mut()
        .all(|token| !token.surface.chars().any(is_han) || !reading_of(token).is_empty())
}

/// Replaces the Japanese characters of `text` with their reading.
///
/// Returns `None` when the embedded dictionary is unavailable. Every character of
/// `text` gets a segment whether or not it has a reading, so the caller can align
/// them with the syllables of the line.
///
/// Whitespace, digits, and punctuation survive in the reading so a mixed line
/// still reads like the text it came from; a Han character the dictionary cannot
/// read contributes nothing, the way unreadable text does in the other languages.
pub(super) fn romanize(text: &str) -> Option<(String, Vec<RomanizationSegment>)> {
    let analyzer = analyzer()?;
    let characters = text.chars().collect::<Vec<_>>();
    let mut readings = vec![String::new(); characters.len()];
    let mut furigana = vec![String::new(); characters.len()];
    let mut line = String::with_capacity(text.len());
    let mut tokens = analyzer.segment(Cow::Borrowed(text)).ok()?;

    // The words are collected first because a word is read with the one after it:
    // a small tsu at the end of a word is heard as the consonant the next word
    // starts with, so `がっ` and `こう` together read "gakkou".
    let mut words = Vec::with_capacity(tokens.len());
    let mut byte_cursor = 0;
    let mut char_cursor = 0;
    for token in &mut tokens {
        let reading = reading_of(token);
        let surface = token.surface.as_ref();
        let Some(start) = text
            .get(byte_cursor..)
            .and_then(|rest| rest.find(surface))
            .map(|offset| byte_cursor + offset)
        else {
            continue;
        };
        let end = start + surface.len();
        let index = char_cursor + text[byte_cursor..start].chars().count();
        byte_cursor = end;
        char_cursor = index + surface.chars().count();
        words.push(Word {
            reading,
            byte_range: start..end,
            characters: index..char_cursor,
        });
    }

    let mut read = false;
    let mut continues = false;
    for (position, word) in words.iter().enumerate() {
        if word.reading.is_empty() {
            for character in text[word.byte_range.clone()].chars() {
                if is_han(character) {
                    continue;
                }
                if read && character.is_alphanumeric() {
                    push_separator(&mut line);
                }
                line.push(character);
                read = false;
            }
            continues = false;
            continue;
        }

        let next = words[position + 1..]
            .iter()
            .find(|word| !word.reading.is_empty());
        let piece = word_readings(
            &characters[word.characters.clone()],
            &word.reading,
            next.map(|word| word.reading.as_str()),
        );
        if piece.iter().all(|piece| piece.romaji.is_empty()) {
            continue;
        }
        if !(read && continues) {
            push_separator(&mut line);
        }
        for (offset, piece) in piece.into_iter().enumerate() {
            let index = word.characters.start + offset;
            line.push_str(&piece.romaji);
            readings[index] = piece.romaji;
            // Only a character that is read rather than spelled carries kana
            // above it: a kana syllable reads itself.
            if is_han(characters[index]) {
                furigana[index] = piece.kana;
            }
        }
        // A small tsu, and an ん the next word starts with a vowel after, are
        // heard as part of that word, so the two are not separated: がっこう
        // reads "gakkou" and しんいち reads "shin'ichi". A word that starts with
        // a y is a word of its own, so 少年よ reads "shounen yo".
        let last = word.reading.chars().next_back();
        continues = last.is_some_and(is_small_tsu)
            || (last == Some('ん')
                && next
                    .and_then(|word| first_mora_romaji(&word.reading))
                    .is_some_and(|romaji| romaji.starts_with(is_vowel)));
        read = true;
    }

    let segments = characters
        .into_iter()
        .zip(readings)
        .zip(furigana)
        .map(
            |((character, romanization), furigana)| RomanizationSegment {
                text: character.to_string(),
                romanization,
                furigana,
            },
        )
        .collect();

    Some((line.trim().to_string(), segments))
}

/// One analyzed word: the kana it is read with, and where it sits in the line.
struct Word {
    reading: String,
    byte_range: std::ops::Range<usize>,
    characters: std::ops::Range<usize>,
}

/// The kana a token is read with, in hiragana, or an empty string.
///
/// The reading comes from the analyzer, so a word is read in its context: `今日`
/// is "kyou" where `日` alone is "nichi", and `君` is "kimi" where the standalone
/// kanji is read "kun". Where the dictionary says a word is pronounced
/// differently from how it is spelled, a `は` or `へ` that stands for a particle
/// is read as the sound it is heard as — `こんにちは` is "konnichiwa" and
/// `学校へ` is "gakkou e" — while an orthographic long vowel keeps its spelling,
/// so `今日` stays "kyou" rather than turning into the "kyoo" of the
/// pronunciation. Kana the dictionary does not list read themselves.
fn reading_of(token: &mut Token<'_>) -> String {
    let mut reading = "";
    let mut pronunciation = "";
    for (index, detail) in token.details_iter().map(str::trim).enumerate() {
        match index {
            READING => reading = detail,
            PRONUNCIATION => pronunciation = detail,
            _ => {}
        }
    }

    let kana = spoken_reading(reading, pronunciation);
    // A field the dictionary does not fill in holds a placeholder, not a reading.
    if !kana.is_empty() && kana.chars().all(is_kana) {
        return kana;
    }
    let surface = token.surface.as_ref();
    if surface.chars().all(is_kana) {
        return to_hiragana(surface);
    }
    String::new()
}

/// The reading of a word, with the particle sounds the dictionary spells
/// separately read in.
fn spoken_reading(reading: &str, pronunciation: &str) -> String {
    let spelling = to_hiragana(reading);
    let spoken = to_hiragana(pronunciation);
    if spelling.chars().count() != spoken.chars().count() {
        return spelling;
    }
    spelling
        .chars()
        .zip(spoken.chars())
        .map(|(spelled, heard)| match (spelled, heard) {
            ('は', 'わ') => 'わ',
            ('へ', 'え') => 'え',
            _ => spelled,
        })
        .collect()
}

/// The reading of every character of a word: the kana it is read with and the
/// Romaji that kana spells.
///
/// The reading of the word is generated as a whole and handed out character by
/// character, so the readings a view shows under a line add up to the reading it
/// shows for the line, and the kana a view shows above a character is the kana
/// that character is read with.
fn word_readings(surface: &[char], reading: &str, next: Option<&str>) -> Vec<Piece> {
    let kana = reading.chars().collect::<Vec<_>>();
    let romaji = kana_romaji(&kana, next);
    let mut pieces = vec![Piece::default(); surface.len()];

    for (position, index) in char_mapping(surface, &kana).into_iter().enumerate() {
        pieces[index].kana.push(kana[position]);
        pieces[index].romaji.push_str(&romaji[position]);
    }
    pieces
}

/// One character of a word and the reading it is handed.
#[derive(Clone, Default)]
struct Piece {
    /// Kana the character is read with.
    kana: String,
    /// Romaji that kana spells.
    romaji: String,
}

/// The character of `surface` each kana of `reading` is read with.
///
/// The reading belongs to the word as a whole, so the characters it is handed out
/// to are what the furigana dictionary decides: `運命` is read "un" and then "mei"
/// where `大人` keeps its "otona" in one piece. A word the dictionary does not
/// list is split at its own kana, which a kana character spells itself, and a run
/// of characters that have no reading of their own keeps its reading in one piece
/// on the character it starts at rather than being cut at the wrong character.
fn char_mapping(surface: &[char], kana: &[char]) -> Vec<usize> {
    if surface.len() == 1 {
        return vec![0; kana.len()];
    }
    furigana_mapping(surface, kana)
        .or_else(|| align(surface, kana))
        .unwrap_or_else(|| vec![0; kana.len()])
}

/// The character each kana of a word is read with, from the furigana dictionary.
///
/// Returns `None` when the dictionary does not list the word, or when the reading
/// it hands back does not add up to the one the analyzer gave, which keeps a
/// reading from being attached to the wrong character.
fn furigana_mapping(surface: &[char], kana: &[char]) -> Option<Vec<usize>> {
    if !*FURIGANA {
        return None;
    }
    let word = surface.iter().collect::<String>();
    let reading = kana.iter().collect::<String>();
    let segments = jmdict_furigana::get(&word, &reading)?;

    let mut mapping = Vec::with_capacity(kana.len());
    let mut index = 0;
    for (piece, piece_reading) in segments {
        let piece_length = piece.chars().count();
        if piece_reading.is_empty() {
            // Kana the word is written with: every character spells itself.
            mapping.extend(index..index + piece_length);
        } else {
            // A reading the dictionary puts on one piece of the word belongs to
            // the character that piece starts at.
            mapping.extend(std::iter::repeat_n(index, piece_reading.chars().count()));
        }
        index += piece_length;
    }

    (index == surface.len() && mapping.len() == kana.len()).then_some(mapping)
}

/// Maps every kana of `reading` to the character of `surface` it is read with.
///
/// A kana character of the surface spells its own kana and is where the reading
/// touches the word; the kana between two of them belongs to the kanji run before
/// it. Returns `None` when the reading cannot be told apart that way, which leaves
/// the caller to keep the reading in one piece.
fn align(surface: &[char], reading: &[char]) -> Option<Vec<usize>> {
    let mut anchors = Vec::with_capacity(reading.len());
    let mut position = 0;
    let mut index = 0;
    while index < surface.len() {
        if is_kana(surface[index]) {
            let kana = to_hiragana_char(surface[index]);
            if !reading
                .get(position)
                .is_some_and(|reading| spells(kana, *reading))
            {
                return None;
            }
            anchors.push(index);
            position += 1;
            index += 1;
            continue;
        }

        let run_end = surface[index..]
            .iter()
            .position(|character| is_kana(*character))
            .map_or(surface.len(), |offset| index + offset);
        let anchor = surface[run_end..]
            .iter()
            .take_while(|character| is_kana(**character))
            .map(|character| to_hiragana_char(*character))
            .collect::<Vec<_>>();
        let offset = find(&reading[position..], &anchor)?;
        anchors.extend(std::iter::repeat_n(index, offset));
        position += offset;
        index = run_end;
    }

    (anchors.len() == reading.len()).then_some(anchors)
}

/// The position where `needle` spells the start of `haystack`.
fn find(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    (0..=haystack.len().saturating_sub(needle.len())).find(|start| {
        needle
            .iter()
            .enumerate()
            .all(|(offset, kana)| spells(*kana, haystack[start + offset]))
    })
}

/// Whether a kana of a word's surface spells the kana its reading is written
/// with.
///
/// A `は` or a `へ` that stands for a particle is read as the sound it is heard
/// as, so `こんにちは` is the reading "konnichiwa" of the spelling "こんにちは".
fn spells(kana: char, reading: char) -> bool {
    kana == reading || matches!((kana, reading), ('は', 'わ') | ('へ', 'え'))
}

/// The Romaji of the first mora of a reading, which is what decides how a word
/// before it is joined to it.
fn first_mora_romaji(reading: &str) -> Option<String> {
    let kana = reading.chars().collect::<Vec<_>>();
    morae(&kana)
        .into_iter()
        .find(|(_, _, romaji)| !romaji.is_empty())
        .map(|(_, _, romaji)| romaji)
}

/// The Romaji of every kana character of a reading.
///
/// Morae are the unit a reading is handed out in, so they are what the Romaji is
/// generated per: a digraph is one mora, a small tsu takes the consonant of the
/// mora after it, and a prolonged sound mark takes the vowel of the mora before
/// it. `next` is the kana of the word that follows, which is what a small tsu at
/// the end of this one is heard as.
fn kana_romaji(kana: &[char], next: Option<&str>) -> Vec<String> {
    let list = morae(kana);
    let following = next.and_then(first_mora_romaji);
    let mut romaji = vec![String::new(); kana.len()];

    for (position, (start, end, reading)) in list.iter().enumerate() {
        let previous = list[..position]
            .iter()
            .rev()
            .find(|(_, _, reading)| !reading.is_empty());
        let next_reading = list[position + 1..]
            .iter()
            .find(|(_, _, reading)| !reading.is_empty())
            .map(|(_, _, romaji)| romaji.as_str());
        let after = next_reading.or(following.as_deref());
        let reading = match kana[*start] {
            character if is_small_tsu(character) => sokuon(after.unwrap_or("")),
            PROLONGED_SOUND_MARK => prolonged(previous.map_or("", |(_, _, r)| r.as_str())),
            // An ん before a vowel or a y is written with an apostrophe, so
            // しんいち reads "shin'ichi" and こんや reads "kon'ya". The mora of
            // the word after this one is heard with it only when that word
            // starts with a vowel, because a word of its own is not carried into
            // the one before it: 少年よ reads "shounen yo".
            'ん' => match next_reading {
                Some(romaji) if is_vowel_or_y(romaji) => "n'".to_string(),
                Some(_) => reading.clone(),
                None if following
                    .as_deref()
                    .is_some_and(|romaji| romaji.starts_with(is_vowel)) =>
                {
                    "n'".to_string()
                }
                None => reading.clone(),
            },
            _ => reading.clone(),
        };
        romaji[*start] = reading;
        debug_assert!(romaji[*start..*end].iter().skip(1).all(String::is_empty));
    }
    romaji
}

/// Splits a reading into its morae, each with the Romaji it is read with.
///
/// A mora is a kana plus the small kana that follow it, so `きょ` is one mora and
/// whatever is written for it is what the reading is handed out in.
fn morae(kana: &[char]) -> Vec<(usize, usize, String)> {
    let mut morae = Vec::with_capacity(kana.len());
    let mut index = 0;
    while index < kana.len() {
        let start = index;
        let mut text = String::new();
        text.push(kana[index]);
        index += 1;
        if is_small_tsu(kana[start]) || kana[start] == PROLONGED_SOUND_MARK {
            morae.push((start, index, String::new()));
            continue;
        }
        let mut reading = syllable(&text);
        while index < kana.len() && is_small_kana(kana[index]) {
            text.push(kana[index]);
            let Some(combined) = syllable(&text) else {
                text.pop();
                break;
            };
            reading = Some(combined);
            index += 1;
        }
        morae.push((start, index, reading.unwrap_or_default().to_string()));
    }
    morae
}

/// The consonant a small tsu is heard as, which is the one the next mora starts
/// with — `ちょっと` reads "cho", "t", "to" and `まっちゃ` reads "ma", "t", "cha".
fn sokuon(next: &str) -> String {
    if next.starts_with("ch") {
        return "t".to_string();
    }
    next.chars()
        .next()
        .filter(char::is_ascii_alphabetic)
        .unwrap_or('t')
        .to_string()
}

/// The vowel a prolonged sound mark lengthens the mora before it with.
fn prolonged(previous: &str) -> String {
    previous
        .chars()
        .rev()
        .find(|character| is_vowel(*character))
        .map_or_else(String::new, |vowel| vowel.to_string())
}

/// Hepburn Romaji for one or two kana.
fn syllable(kana: &str) -> Option<&'static str> {
    Some(match kana {
        "あ" => "a",
        "い" => "i",
        "う" => "u",
        "え" => "e",
        "お" => "o",
        "ゐ" => "i",
        "ゑ" => "e",
        "か" => "ka",
        "き" => "ki",
        "く" => "ku",
        "け" => "ke",
        "こ" => "ko",
        "きゃ" => "kya",
        "きゅ" => "kyu",
        "きょ" => "kyo",
        "が" => "ga",
        "ぎ" => "gi",
        "ぐ" => "gu",
        "げ" => "ge",
        "ご" => "go",
        "ぎゃ" => "gya",
        "ぎゅ" => "gyu",
        "ぎょ" => "gyo",
        "さ" => "sa",
        "し" => "shi",
        "す" => "su",
        "せ" => "se",
        "そ" => "so",
        "しゃ" => "sha",
        "しゅ" => "shu",
        "しょ" => "sho",
        "しぇ" => "she",
        "ざ" => "za",
        "じ" => "ji",
        "ず" => "zu",
        "ぜ" => "ze",
        "ぞ" => "zo",
        "じゃ" => "ja",
        "じゅ" => "ju",
        "じょ" => "jo",
        "じぇ" => "je",
        "た" => "ta",
        "ち" => "chi",
        "つ" => "tsu",
        "て" => "te",
        "と" => "to",
        "ちゃ" => "cha",
        "ちゅ" => "chu",
        "ちょ" => "cho",
        "ちぇ" => "che",
        "てぃ" => "ti",
        "とぅ" => "tu",
        "つぁ" => "tsa",
        "つぃ" => "tsi",
        "つぇ" => "tse",
        "つぉ" => "tso",
        "だ" => "da",
        "ぢ" => "ji",
        "づ" => "zu",
        "で" => "de",
        "ど" => "do",
        "ぢゃ" => "ja",
        "ぢゅ" => "ju",
        "ぢょ" => "jo",
        "でぃ" => "di",
        "どぅ" => "du",
        "な" => "na",
        "に" => "ni",
        "ぬ" => "nu",
        "ね" => "ne",
        "の" => "no",
        "にゃ" => "nya",
        "にゅ" => "nyu",
        "にょ" => "nyo",
        "は" => "ha",
        "ひ" => "hi",
        "ふ" => "fu",
        "へ" => "he",
        "ほ" => "ho",
        "ひゃ" => "hya",
        "ひゅ" => "hyu",
        "ひょ" => "hyo",
        "ふぁ" => "fa",
        "ふぃ" => "fi",
        "ふぇ" => "fe",
        "ふぉ" => "fo",
        "ば" => "ba",
        "び" => "bi",
        "ぶ" => "bu",
        "べ" => "be",
        "ぼ" => "bo",
        "びゃ" => "bya",
        "びゅ" => "byu",
        "びょ" => "byo",
        "ぱ" => "pa",
        "ぴ" => "pi",
        "ぷ" => "pu",
        "ぺ" => "pe",
        "ぽ" => "po",
        "ぴゃ" => "pya",
        "ぴゅ" => "pyu",
        "ぴょ" => "pyo",
        "ま" => "ma",
        "み" => "mi",
        "む" => "mu",
        "め" => "me",
        "も" => "mo",
        "みゃ" => "mya",
        "みゅ" => "myu",
        "みょ" => "myo",
        "や" => "ya",
        "ゆ" => "yu",
        "よ" => "yo",
        "ら" => "ra",
        "り" => "ri",
        "る" => "ru",
        "れ" => "re",
        "ろ" => "ro",
        "りゃ" => "rya",
        "りゅ" => "ryu",
        "りょ" => "ryo",
        "わ" => "wa",
        "を" => "wo",
        "ゔ" => "vu",
        "ゔぁ" => "va",
        "ゔぃ" => "vi",
        "ゔぇ" => "ve",
        "ゔぉ" => "vo",
        "ん" => "n",
        "うぃ" => "wi",
        "うぇ" => "we",
        "うぉ" => "wo",
        "くぁ" => "kwa",
        "くぃ" => "kwi",
        "くぇ" => "kwe",
        "くぉ" => "kwo",
        "ぐぁ" => "gwa",
        _ => return None,
    })
}

/// Converts every katakana of `text` to hiragana, leaving the rest alone.
fn to_hiragana(text: &str) -> String {
    text.chars().map(to_hiragana_char).collect()
}

/// Converts one katakana character to hiragana, leaving the rest alone.
fn to_hiragana_char(character: char) -> char {
    match character as u32 {
        // Katakana and hiragana are the same sounds written in two scripts, one
        // offset apart; the prolonged sound mark has no hiragana counterpart.
        0x30A1..=0x30F6 => char::from_u32(character as u32 - 0x60).unwrap_or(character),
        _ => character,
    }
}

fn is_kana(character: char) -> bool {
    matches!(character as u32,
        0x3041..=0x3096   // Hiragana
        | 0x30A1..=0x30FA // Katakana
        | 0x30FC          // Prolonged sound mark
    )
}

fn is_small_kana(character: char) -> bool {
    SMALL_KANA.contains(character)
}

fn is_small_tsu(character: char) -> bool {
    SMALL_TSU.contains(character)
}

fn is_han(character: char) -> bool {
    matches!(character as u32,
        0x3400..=0x4DBF      // CJK unified ideographs extension A
        | 0x4E00..=0x9FFF    // CJK unified ideographs
        | 0xF900..=0xFAFF    // CJK compatibility ideographs
        | 0x20000..=0x3FFFF  // CJK unified ideographs extensions B and later
    )
}

/// Whether a mora is written with a vowel or a y, which is what an ん before it
/// is separated from with an apostrophe.
fn is_vowel_or_y(mora: &str) -> bool {
    mora.starts_with(is_vowel) || mora.starts_with('y')
}

fn is_vowel(character: char) -> bool {
    matches!(character, 'a' | 'e' | 'i' | 'o' | 'u')
}
