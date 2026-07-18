// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Pure overlay sizing and animation calculations.

use crate::shared::config::AppConfig;

const MIN_PANEL_WIDTH: i32 = 320;
const MAX_PANEL_WIDTH: i32 = 640;
pub(super) const MAX_EXPANDED_PANEL_WIDTH: i32 = 960;
const TEXT_HORIZONTAL_PADDING: i32 = 24;
const AMLL_NARROW_LINE_PADDING_PX: i32 = 20;
const MIN_KARAOKE_HEIGHT: i32 = 36;
const MIN_ROMANIZATION_HEIGHT: i32 = 18;
const MIN_TRANSLATION_HEIGHT: i32 = 18;
const PANEL_CHROME_HEIGHT: i32 = 28;
pub(super) const PANEL_RESIZE_DURATION_US: i64 = 180_000;

pub(super) fn fallback_panel_height(viewport_height: i32) -> i32 {
    viewport_height.saturating_add(PANEL_CHROME_HEIGHT)
}

fn karaoke_line_height(lyric_font_px: i32) -> i32 {
    (lyric_font_px as f64 * 1.5).ceil() as i32
}

fn secondary_line_height(font_px: i32) -> i32 {
    (font_px as f64 * 1.5).ceil() as i32
}

fn apple_music_line_height(
    lyric_font_px: i32,
    romanization_font_px: i32,
    translation_font_px: i32,
    show_romanization: bool,
) -> i32 {
    // AMLL uses 2 em for the primary line and wrapper, a 0.3 em flex gap,
    // and a 1.5 em line height for each configured secondary font.
    let lyric_font_px = lyric_font_px.max(1);
    let mut hundredths = lyric_font_px
        .saturating_mul(230)
        .saturating_add(translation_font_px.max(1).saturating_mul(150));
    if show_romanization {
        hundredths = hundredths
            .saturating_add(lyric_font_px.saturating_mul(30))
            .saturating_add(romanization_font_px.max(1).saturating_mul(150));
    }
    hundredths.saturating_add(99) / 100
}

pub(super) fn viewport_height(
    lyric_font_px: i32,
    romanization_font_px: i32,
    translation_font_px: i32,
    show_romanization: bool,
    apple_music_style: bool,
) -> i32 {
    if apple_music_style {
        return apple_music_line_height(
            lyric_font_px,
            romanization_font_px,
            translation_font_px,
            show_romanization,
        );
    }
    let romanization_height = if show_romanization {
        secondary_line_height(romanization_font_px).max(MIN_ROMANIZATION_HEIGHT)
    } else {
        0
    };
    karaoke_line_height(lyric_font_px).max(MIN_KARAOKE_HEIGHT)
        + romanization_height
        + secondary_line_height(translation_font_px).max(MIN_TRANSLATION_HEIGHT)
}

pub(super) fn compact_panel_width(configured_width: i32) -> i32 {
    configured_width.clamp(MIN_PANEL_WIDTH, MAX_PANEL_WIDTH)
}

pub(super) fn effective_bottom_margin(config: &AppConfig) -> i32 {
    config
        .window
        .margin
        .max(config.window.bottom_panel_height)
        .max(0)
}

pub(super) fn expanded_panel_width(
    compact_width: i32,
    content_width: i32,
    maximum_width: i32,
) -> i32 {
    content_width
        .max(compact_width)
        .min(maximum_width.max(compact_width))
}

pub(super) fn maximum_lyrics_width(available_width: i32, apple_music_style: bool) -> i32 {
    if apple_music_style {
        available_width
    } else {
        available_width.min(MAX_EXPANDED_PANEL_WIDTH)
    }
}

pub(super) fn lyrics_horizontal_padding(apple_music_style: bool, lyric_font_px: i32) -> i32 {
    if apple_music_style {
        // AMLL uses 1 em on each side, or 20 px per side in narrow viewports.
        lyric_font_px
            .max(AMLL_NARROW_LINE_PADDING_PX)
            .saturating_mul(2)
    } else {
        TEXT_HORIZONTAL_PADDING
    }
}

pub(super) fn lyrics_resize_animation(
    apple_music_style: bool,
    layout_changed: bool,
) -> Option<bool> {
    layout_changed.then_some(apple_music_style)
}

pub(super) fn animated_panel_width(start: i32, target: i32, elapsed_us: i64) -> i32 {
    if elapsed_us >= PANEL_RESIZE_DURATION_US {
        return target;
    }

    let progress = elapsed_us.max(0) as f64 / PANEL_RESIZE_DURATION_US as f64;
    let eased = if target >= start {
        1.0 - (1.0 - progress).powi(3)
    } else {
        progress.powi(3)
    };
    (start as f64 + (target - start) as f64 * eased).round() as i32
}

#[cfg(test)]
#[path = "../../test/view_test.rs"]
mod tests;
