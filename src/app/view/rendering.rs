// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Pango/Cairo text rendering for translations and syllable-level karaoke.

use gtk::prelude::*;
use std::{cell::{Cell, RefCell}, rc::Rc};

use floatlyrics_lyrics::lyrics::TimedSyllable;

use super::super::model::{KaraokeRenderState, LyricSlotText, syllable_progress};

#[derive(Debug, Clone)]
pub(super) struct TextLineRenderState {
    pub(super) text: String,
    pub(super) style: TextLineStyle,
    pub(super) font_family: String,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TextLineStyle {
    pub(super) font_px: i32,
    pub(super) color: (f64, f64, f64, f64),
}

impl Default for TextLineRenderState {
    fn default() -> Self {
        Self {
            text: String::new(),
            style: TextLineStyle {
                font_px: 14,
                color: (1.0, 1.0, 1.0, 1.0),
            },
            font_family: "Sans".to_string(),
        }
    }
}

pub(super) fn lyric_content_width(
    measure_widget: &gtk::Label,
    value: &LyricSlotText,
    font_family: &str,
    lyric_font_px: i32,
    translation_font_px: i32,
) -> i32 {
    let lyric_text = value
        .karaoke
        .as_ref()
        .map_or(value.text.as_str(), |karaoke| karaoke.text.as_str());
    let lyric_width = text_pixel_width(
        measure_widget,
        lyric_text,
        lyric_font_px,
        true,
        font_family,
    );
    let translation_width = text_pixel_width(
        measure_widget,
        &value.translation,
        translation_font_px,
        false,
        font_family,
    );

    lyric_width.max(translation_width)
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

pub(super) fn text_line_area(
    width: i32,
    height: i32,
    style: TextLineStyle,
    font_family: &str,
) -> (gtk::DrawingArea, Rc<RefCell<TextLineRenderState>>) {
    let state = Rc::new(RefCell::new(TextLineRenderState {
        text: String::new(),
        style,
        font_family: font_family.to_string(),
    }));
    let area = gtk::DrawingArea::builder()
        .width_request(width)
        .height_request(height)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .visible(false)
        .build();
    {
        let state = Rc::clone(&state);
        area.set_draw_func(move |area, cr, width, height| {
            draw_text_line(area, cr, width, height, &state.borrow());
        });
    }

    (area, state)
}

pub(super) fn lyric_text_widget(
    text: &gtk::Label,
    karaoke_size: Option<(i32, i32)>,
    font_family: Rc<RefCell<String>>,
    lyric_font_size: Rc<Cell<i32>>,
    played_color: Rc<Cell<(f64, f64, f64, f64)>>,
    unplayed_color: Rc<Cell<(f64, f64, f64, f64)>>,
) -> (
    gtk::Widget,
    Option<gtk::DrawingArea>,
    Option<Rc<RefCell<KaraokeRenderState>>>,
) {
    let Some((width, height)) = karaoke_size else {
        return (text.clone().upcast(), None, None);
    };

    let state = Rc::new(RefCell::new(KaraokeRenderState::default()));
    let area = gtk::DrawingArea::builder()
        .width_request(width)
        .height_request(height)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .visible(false)
        .build();
    {
        let state = Rc::clone(&state);
        let font_size = Rc::clone(&lyric_font_size);
        area.set_draw_func(move |area, cr, width, height| {
            draw_karaoke_line(
                area,
                cr,
                width,
                height,
                &state.borrow(),
                &font_family.borrow(),
                font_size.get(),
                played_color.get(),
                unplayed_color.get(),
            );
        });
    }

    let stack = gtk::Stack::new();
    stack.set_halign(gtk::Align::Center);
    stack.set_valign(gtk::Align::Center);
    stack.add_child(text);
    stack.add_child(&area);

    (stack.upcast(), Some(area), Some(state))
}

fn draw_text_line(
    area: &gtk::DrawingArea,
    cr: &gtk::cairo::Context,
    width: i32,
    _height: i32,
    state: &TextLineRenderState,
) {
    if state.text.trim().is_empty() {
        return;
    }

    let layout = area.create_pango_layout(Some(&state.text));
    let mut font = gtk::pango::FontDescription::new();
    font.set_family(&state.font_family);
    font.set_absolute_size(state.style.font_px as f64 * gtk::pango::SCALE as f64);
    layout.set_font_description(Some(&font));
    layout.set_single_paragraph_mode(true);
    layout.set_ellipsize(gtk::pango::EllipsizeMode::End);
    layout.set_alignment(gtk::pango::Alignment::Center);
    layout.set_width(width.saturating_mul(gtk::pango::SCALE));

    draw_pango_layout(cr, &layout, 0.0, 0.0, state.style.color);
}

#[allow(clippy::too_many_arguments)]
fn draw_karaoke_line(
    area: &gtk::DrawingArea,
    cr: &gtk::cairo::Context,
    width: i32,
    height: i32,
    state: &KaraokeRenderState,
    font_family: &str,
    font_px: i32,
    played_color: (f64, f64, f64, f64),
    unplayed_color: (f64, f64, f64, f64),
) {
    if state.text.trim().is_empty() {
        return;
    }

    let layout = area.create_pango_layout(Some(&state.text));
    let mut font = gtk::pango::FontDescription::new();
    font.set_family(font_family);
    font.set_weight(gtk::pango::Weight::Bold);
    font.set_absolute_size(font_px as f64 * gtk::pango::SCALE as f64);
    layout.set_font_description(Some(&font));
    layout.set_single_paragraph_mode(true);

    let (text_width, text_height) = layout.pixel_size();
    let x = ((width - text_width).max(0) as f64) / 2.0;
    let y = ((height - text_height).max(0) as f64) / 2.0;
    let fill_width = karaoke_fill_width(&layout, state);

    draw_pango_layout(cr, &layout, x, y, unplayed_color);
    if fill_width > 0.0 {
        let _ = cr.save();
        cr.rectangle(x, 0.0, fill_width, height as f64);
        cr.clip();
        draw_pango_layout(cr, &layout, x, y, played_color);
        let _ = cr.restore();
    }
}

fn draw_pango_layout(
    cr: &gtk::cairo::Context,
    layout: &gtk::pango::Layout,
    x: f64,
    y: f64,
    color: (f64, f64, f64, f64),
) {
    cr.set_source_rgba(color.0, color.1, color.2, color.3);
    cr.move_to(x, y);
    pangocairo::functions::show_layout(cr, layout);
}

fn karaoke_fill_width(layout: &gtk::pango::Layout, state: &KaraokeRenderState) -> f64 {
    for (index, syllable) in state.syllables.iter().enumerate() {
        if state.position_ms < syllable.start_ms {
            return index
                .checked_sub(1)
                .and_then(|previous| syllable_byte_range(&state.text, &state.syllables, previous))
                .map_or(0.0, |range| layout_x_at_byte(layout, range.end));
        }

        if state.position_ms < syllable.end_ms {
            let Some(range) = syllable_byte_range(&state.text, &state.syllables, index) else {
                return fallback_syllable_fill_width(layout, &state.syllables, index);
            };
            let start_x = layout_x_at_byte(layout, range.start);
            let end_x = layout_x_at_byte(layout, range.end);
            let progress = syllable_progress(syllable, state.position_ms);

            return start_x + (end_x - start_x).max(0.0) * progress;
        }
    }

    layout.pixel_size().0.max(0) as f64
}

fn syllable_byte_range(
    full_text: &str,
    syllables: &[TimedSyllable],
    target_index: usize,
) -> Option<std::ops::Range<i32>> {
    let mut search_from = 0usize;

    for (index, syllable) in syllables.iter().enumerate() {
        let syllable_text = syllable.text.as_str();
        if syllable_text.is_empty() {
            if index == target_index {
                let byte = byte_index_i32(search_from.min(full_text.len()));
                return Some(byte..byte);
            }
            continue;
        }

        let start = full_text
            .get(search_from..)
            .and_then(|remaining| remaining.find(syllable_text))
            .map(|offset| search_from.saturating_add(offset));

        let Some(start) = start else {
            return fallback_syllable_byte_range(full_text, syllables, target_index);
        };
        let end = start
            .saturating_add(syllable_text.len())
            .min(full_text.len());

        if index == target_index {
            return Some(byte_index_i32(start)..byte_index_i32(end));
        }

        search_from = end;
    }

    None
}

fn fallback_syllable_byte_range(
    full_text: &str,
    syllables: &[TimedSyllable],
    target_index: usize,
) -> Option<std::ops::Range<i32>> {
    let mut byte_index = 0usize;

    for (index, syllable) in syllables.iter().enumerate() {
        let start = byte_index.min(full_text.len());
        byte_index = byte_index.saturating_add(syllable.text.len());
        let end = byte_index.min(full_text.len());

        if index == target_index {
            return Some(byte_index_i32(start)..byte_index_i32(end));
        }
    }

    None
}

fn fallback_syllable_fill_width(
    layout: &gtk::pango::Layout,
    syllables: &[TimedSyllable],
    target_index: usize,
) -> f64 {
    let byte_index = syllables
        .iter()
        .take(target_index + 1)
        .map(|syllable| syllable.text.len())
        .sum::<usize>();

    layout_x_at_byte(layout, byte_index_i32(byte_index))
}

fn byte_index_i32(byte_index: usize) -> i32 {
    byte_index.try_into().unwrap_or(i32::MAX)
}

fn layout_x_at_byte(layout: &gtk::pango::Layout, byte_index: i32) -> f64 {
    layout.index_to_pos(byte_index).x() as f64 / gtk::pango::SCALE as f64
}

#[cfg(test)]
#[path = "../../test/rendering_test.rs"]
mod tests;
