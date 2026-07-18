// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Pango font selection and lyric group width measurement.

use gtk::prelude::*;

use crate::shared::presentation::LyricSlotText;

pub(in crate::frontend::view) fn lyric_content_width(
    measure_widget: &gtk::Label,
    value: &LyricSlotText,
    font_family: &str,
    lyric_font_px: i32,
    romanization_font_px: i32,
    translation_font_px: i32,
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
    lyric_text_group_width(
        measure_widget,
        lyric_text,
        &value.romanization,
        &value.translation,
        fonts,
    )
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
