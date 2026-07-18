// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Domain validation for persisted application preferences.

use anyhow::{Result, bail};

use super::AppConfig;

/// Numeric limits enforced at both persistence and presentation boundaries.
pub(crate) struct ConfigLimits;

impl ConfigLimits {
    pub(crate) const WINDOW_WIDTH_MIN: i32 = 320;
    pub(crate) const WINDOW_WIDTH_MAX: i32 = 640;
    pub(crate) const WINDOW_MARGIN_MIN: i32 = 0;
    pub(crate) const WINDOW_MARGIN_MAX: i32 = 500;
    pub(crate) const BOTTOM_PANEL_HEIGHT_MIN: i32 = 0;
    pub(crate) const BOTTOM_PANEL_HEIGHT_MAX: i32 = 200;
    pub(crate) const OPACITY_MIN: f64 = 0.15;
    pub(crate) const OPACITY_MAX: f64 = 1.0;
    pub(crate) const OFFSET_MS_MIN: i64 = -10_000;
    pub(crate) const OFFSET_MS_MAX: i64 = 10_000;
    pub(crate) const LYRIC_FONT_SIZE_MIN: i32 = 12;
    pub(crate) const LYRIC_FONT_SIZE_MAX: i32 = 56;
    pub(crate) const SECONDARY_FONT_SIZE_MIN: i32 = 8;
    pub(crate) const SECONDARY_FONT_SIZE_MAX: i32 = 36;
}

pub(super) fn validate(config: &AppConfig) -> Result<()> {
    validate_i32(
        "window.width",
        config.window.width,
        ConfigLimits::WINDOW_WIDTH_MIN,
        ConfigLimits::WINDOW_WIDTH_MAX,
    )?;
    validate_i32(
        "window.margin",
        config.window.margin,
        ConfigLimits::WINDOW_MARGIN_MIN,
        ConfigLimits::WINDOW_MARGIN_MAX,
    )?;
    validate_i32(
        "window.bottom_panel_height",
        config.window.bottom_panel_height,
        ConfigLimits::BOTTOM_PANEL_HEIGHT_MIN,
        ConfigLimits::BOTTOM_PANEL_HEIGHT_MAX,
    )?;
    validate_f64(
        "window.opacity",
        config.window.opacity,
        ConfigLimits::OPACITY_MIN,
        ConfigLimits::OPACITY_MAX,
    )?;
    if let Some(position) = config.window.position {
        validate_f64("window.position.horizontal", position.horizontal, 0.0, 1.0)?;
        validate_f64("window.position.vertical", position.vertical, 0.0, 1.0)?;
    }
    validate_i64(
        "lyrics.offset_ms",
        config.lyrics.offset_ms,
        ConfigLimits::OFFSET_MS_MIN,
        ConfigLimits::OFFSET_MS_MAX,
    )?;
    validate_i32(
        "lyrics.lyric_font_size",
        config.lyrics.lyric_font_size,
        ConfigLimits::LYRIC_FONT_SIZE_MIN,
        ConfigLimits::LYRIC_FONT_SIZE_MAX,
    )?;
    validate_i32(
        "lyrics.translation_font_size",
        config.lyrics.translation_font_size,
        ConfigLimits::SECONDARY_FONT_SIZE_MIN,
        ConfigLimits::SECONDARY_FONT_SIZE_MAX,
    )?;
    validate_i32(
        "lyrics.romanization_font_size",
        config.lyrics.romanization_font_size,
        ConfigLimits::SECONDARY_FONT_SIZE_MIN,
        ConfigLimits::SECONDARY_FONT_SIZE_MAX,
    )?;
    if config.lyrics.font_order.is_empty()
        || config
            .lyrics
            .font_order
            .iter()
            .any(|font| font.trim().is_empty())
    {
        bail!("lyrics.font_order must contain only non-empty font names");
    }
    for (name, color) in [
        ("lyrics.played_color", &config.lyrics.played_color),
        ("lyrics.unplayed_color", &config.lyrics.unplayed_color),
        ("lyrics.translation_color", &config.lyrics.translation_color),
        (
            "lyrics.romanization_color",
            &config.lyrics.romanization_color,
        ),
    ] {
        if super::normalized_hex_color(color).is_none() {
            bail!("{name} must contain 6 or 8 hexadecimal digits");
        }
    }
    Ok(())
}

fn validate_i32(name: &str, value: i32, minimum: i32, maximum: i32) -> Result<()> {
    if !(minimum..=maximum).contains(&value) {
        bail!("{name} must be between {minimum} and {maximum}, got {value}");
    }
    Ok(())
}

fn validate_i64(name: &str, value: i64, minimum: i64, maximum: i64) -> Result<()> {
    if !(minimum..=maximum).contains(&value) {
        bail!("{name} must be between {minimum} and {maximum}, got {value}");
    }
    Ok(())
}

fn validate_f64(name: &str, value: f64, minimum: f64, maximum: f64) -> Result<()> {
    if !value.is_finite() || !(minimum..=maximum).contains(&value) {
        bail!("{name} must be between {minimum} and {maximum}, got {value}");
    }
    Ok(())
}
