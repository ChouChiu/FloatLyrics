// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Pango font selection and lyric group width measurement.

use gtk::prelude::*;

use floatlyrics_lyrics::lyrics::TimedSyllable;

use crate::shared::presentation::LyricSlotText;

/// Measures the widest row of `value` as the overlay draws it.
///
/// With `word_readings`, the Apple Music style renderer draws the reading of
/// each word below that word whenever one word has a reading, so a word takes
/// the width of the wider of the two and the line is measured word by word.
/// That renderer sizes a reading from the lyric font rather than from the
/// configured romanization size: half its size, at its weight, followed by
/// 0.3 em of its own size.
pub(in crate::frontend::view) fn lyric_content_width(
    measure_widget: &gtk::Label,
    value: &LyricSlotText,
    font_family: &str,
    lyric_font_px: i32,
    romanization_font_px: i32,
    translation_font_px: i32,
    word_readings: bool,
) -> i32 {
    let fonts = LyricsFontMetrics {
        family: font_family,
        lyric_px: lyric_font_px,
        romanization_px: romanization_font_px,
        translation_px: translation_font_px,
    };
    let lyric_text = value
        .karaoke
        .as_ref()
        .map_or(value.text.as_str(), |karaoke| karaoke.text.as_str());
    let group_width = lyric_text_group_width(
        measure_widget,
        lyric_text,
        &value.romanization,
        &value.translation,
        fonts,
    );
    let Some(karaoke) = value.karaoke.as_ref().filter(|_| word_readings) else {
        return group_width;
    };
    group_width.max(word_reading_row_width(
        &karaoke.syllables,
        |text, row| match row {
            WordRow::Word => {
                raw_text_pixel_width(measure_widget, text, fonts.lyric_px, true, fonts.family)
            }
            WordRow::Reading => {
                let reading_px = (fonts.lyric_px + 1) / 2;
                match text_pixel_width(measure_widget, text, reading_px, true, fonts.family) {
                    0 => 0,
                    width => width.saturating_add(reading_px * 3 / 10),
                }
            }
        },
    ))
}

/// The row of a word a width is measured for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WordRow {
    Word,
    Reading,
}

/// Width of a line whose words are drawn above their readings.
///
/// Returns zero when no word has a reading, because the renderer then draws
/// the line as one run of text.
fn word_reading_row_width(
    syllables: &[TimedSyllable],
    measure: impl Fn(&str, WordRow) -> i32,
) -> i32 {
    if syllables
        .iter()
        .all(|syllable| syllable.romanization.trim().is_empty())
    {
        return 0;
    }
    syllables
        .iter()
        .filter(|syllable| !syllable.text.is_empty())
        .map(|syllable| {
            measure(&syllable.text, WordRow::Word)
                .max(measure(syllable.romanization.trim(), WordRow::Reading))
        })
        .fold(0, i32::saturating_add)
}

#[derive(Clone, Copy)]
struct LyricsFontMetrics<'a> {
    family: &'a str,
    lyric_px: i32,
    romanization_px: i32,
    translation_px: i32,
}

fn lyric_text_group_width(
    measure_widget: &gtk::Label,
    lyric_text: &str,
    romanization: &str,
    translation: &str,
    fonts: LyricsFontMetrics<'_>,
) -> i32 {
    text_pixel_width(
        measure_widget,
        lyric_text,
        fonts.lyric_px,
        true,
        fonts.family,
    )
    .max(text_pixel_width(
        measure_widget,
        romanization,
        fonts.romanization_px,
        false,
        fonts.family,
    ))
    .max(text_pixel_width(
        measure_widget,
        translation,
        fonts.translation_px,
        false,
        fonts.family,
    ))
}

fn text_pixel_width(
    widget: &gtk::Label,
    text: &str,
    font_px: i32,
    bold: bool,
    font_family: &str,
) -> i32 {
    if text.trim().is_empty() {
        return 0;
    }
    raw_text_pixel_width(widget, text, font_px, bold, font_family)
}

/// Measures `text` including any whitespace, which separates the words of a
/// line drawn word by word.
fn raw_text_pixel_width(
    widget: &gtk::Label,
    text: &str,
    font_px: i32,
    bold: bool,
    font_family: &str,
) -> i32 {
    if text.is_empty() {
        return 0;
    }

    let layout = widget.create_pango_layout(Some(text));
    let mut font = gtk::pango::FontDescription::new();
    font.set_family(font_family);
    font.set_weight(if bold {
        gtk::pango::Weight::Bold
    } else {
        gtk::pango::Weight::Normal
    });
    font.set_absolute_size(font_px as f64 * gtk::pango::SCALE as f64);
    layout.set_font_description(Some(&font));
    layout.set_single_paragraph_mode(true);
    layout.pixel_size().0.max(0)
}

pub(in crate::frontend::view) fn font_family(order: &[String]) -> String {
    let families = order
        .iter()
        .map(|family| family.trim())
        .filter(|family| !family.is_empty())
        .collect::<Vec<_>>();
    if families.is_empty() {
        "Sans".to_string()
    } else {
        families.join(", ")
    }
}

#[cfg(test)]
#[path = "../../../test/web_lyrics_metrics_test.rs"]
mod tests;
