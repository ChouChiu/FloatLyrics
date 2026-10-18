// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Domain validation for persisted application preferences.

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use serde::Serialize;

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
    pub(crate) const TRANSITION_MS_MIN: u32 = 0;
    pub(crate) const TRANSITION_MS_MAX: u32 = 1000;
    pub(crate) const WORD_FADE_WIDTH_MIN: f64 = 0.0;
    pub(crate) const WORD_FADE_WIDTH_MAX: f64 = 2.0;
    pub(crate) const TRACK_OFFSET_STEP_MS_MIN: u32 = 10;
    pub(crate) const TRACK_OFFSET_STEP_MS_MAX: u32 = 1000;

    /// Inclusive range of every numeric preference, keyed by its dotted TOML path.
    ///
    /// The settings page reads the same table, so a limit is declared once.
    pub(crate) fn ranges() -> BTreeMap<&'static str, NumericRange> {
        NUMERIC_FIELDS
            .iter()
            .map(|field| (field.name, field.range))
            .collect()
    }

    /// Whether `value` lies inside the range declared for the field `name`.
    ///
    /// An unknown field name is a programming error and is never in range.
    pub(crate) fn contains(name: &str, value: impl Numeric) -> bool {
        NUMERIC_FIELDS
            .iter()
            .find(|field| field.name == name)
            .is_some_and(|field| field.range.contains(value.as_f64()))
    }
}

/// Inclusive bounds of a numeric preference.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(crate) struct NumericRange {
    pub(crate) min: f64,
    pub(crate) max: f64,
}

impl NumericRange {
    fn contains(self, value: f64) -> bool {
        value.is_finite() && (self.min..=self.max).contains(&value)
    }
}

/// Numeric types persisted in the configuration.
pub(crate) trait Numeric: Copy {
    /// Converts the value without loss for every range declared here.
    fn as_f64(self) -> f64;
}

impl Numeric for i32 {
    fn as_f64(self) -> f64 {
        self.into()
    }
}

impl Numeric for u32 {
    fn as_f64(self) -> f64 {
        self.into()
    }
}

impl Numeric for i64 {
    fn as_f64(self) -> f64 {
        // Every declared range lies well inside the 53-bit integer range of `f64`.
        self as f64
    }
}

impl Numeric for f64 {
    fn as_f64(self) -> f64 {
        self
    }
}

struct NumericField {
    name: &'static str,
    range: NumericRange,
    read: fn(&AppConfig) -> f64,
}

const fn range(min: f64, max: f64) -> NumericRange {
    NumericRange { min, max }
}

const NUMERIC_FIELDS: &[NumericField] = &[
    NumericField {
        name: "window.width",
        range: range(
            ConfigLimits::WINDOW_WIDTH_MIN as f64,
            ConfigLimits::WINDOW_WIDTH_MAX as f64,
        ),
        read: |config| config.window.width.as_f64(),
    },
    NumericField {
        name: "window.margin",
        range: range(
            ConfigLimits::WINDOW_MARGIN_MIN as f64,
            ConfigLimits::WINDOW_MARGIN_MAX as f64,
        ),
        read: |config| config.window.margin.as_f64(),
    },
    NumericField {
        name: "window.bottom_panel_height",
        range: range(
            ConfigLimits::BOTTOM_PANEL_HEIGHT_MIN as f64,
            ConfigLimits::BOTTOM_PANEL_HEIGHT_MAX as f64,
        ),
        read: |config| config.window.bottom_panel_height.as_f64(),
    },
    NumericField {
        name: "window.opacity",
        range: range(ConfigLimits::OPACITY_MIN, ConfigLimits::OPACITY_MAX),
        read: |config| config.window.opacity,
    },
    NumericField {
        name: "lyrics.offset_ms",
        range: range(
            ConfigLimits::OFFSET_MS_MIN as f64,
            ConfigLimits::OFFSET_MS_MAX as f64,
        ),
        read: |config| config.lyrics.offset_ms.as_f64(),
    },
    NumericField {
        name: "lyrics.lyric_font_size",
        range: range(
            ConfigLimits::LYRIC_FONT_SIZE_MIN as f64,
            ConfigLimits::LYRIC_FONT_SIZE_MAX as f64,
        ),
        read: |config| config.lyrics.lyric_font_size.as_f64(),
    },
    NumericField {
        name: "lyrics.translation_font_size",
        range: range(
            ConfigLimits::SECONDARY_FONT_SIZE_MIN as f64,
            ConfigLimits::SECONDARY_FONT_SIZE_MAX as f64,
        ),
        read: |config| config.lyrics.translation_font_size.as_f64(),
    },
    NumericField {
        name: "lyrics.romanization_font_size",
        range: range(
            ConfigLimits::SECONDARY_FONT_SIZE_MIN as f64,
            ConfigLimits::SECONDARY_FONT_SIZE_MAX as f64,
        ),
        read: |config| config.lyrics.romanization_font_size.as_f64(),
    },
    NumericField {
        name: "lyrics.transition_ms",
        range: range(
            ConfigLimits::TRANSITION_MS_MIN as f64,
            ConfigLimits::TRANSITION_MS_MAX as f64,
        ),
        read: |config| config.lyrics.transition_ms.as_f64(),
    },
    NumericField {
        name: "lyrics.word_fade_width",
        range: range(
            ConfigLimits::WORD_FADE_WIDTH_MIN,
            ConfigLimits::WORD_FADE_WIDTH_MAX,
        ),
        read: |config| config.lyrics.word_fade_width,
    },
    NumericField {
        name: "lyrics.track_offset_step_ms",
        range: range(
            ConfigLimits::TRACK_OFFSET_STEP_MS_MIN as f64,
            ConfigLimits::TRACK_OFFSET_STEP_MS_MAX as f64,
        ),
        read: |config| config.lyrics.track_offset_step_ms.as_f64(),
    },
];

pub(super) fn validate(config: &AppConfig) -> Result<()> {
    for field in NUMERIC_FIELDS {
        let value = (field.read)(config);
        if !field.range.contains(value) {
            let NumericRange { min, max } = field.range;
            bail!(
                "{} must be between {min} and {max}, got {value}",
                field.name
            );
        }
    }
    if let Some(position) = config.window.position {
        validate_fraction("window.position.horizontal", position.horizontal)?;
        validate_fraction("window.position.vertical", position.vertical)?;
    }
    if config.lyrics.font_order.is_empty()
        || config
            .lyrics
            .font_order
            .iter()
            .any(|font| font.trim().is_empty())
    {
        bail!("lyrics.font_order must contain only non-empty font names");
    }
    let effective_preferred_players = config.player.effective_preferred_players();
    for (name, selectors) in [
        ("player.preferred_players", &effective_preferred_players),
        ("player.ignored_players", &config.player.ignored_players),
    ] {
        if selectors.iter().any(|selector| selector.trim().is_empty()) {
            bail!("{name} must contain only non-empty selectors");
        }
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
    validate_listener_address("amll.address", &config.amll.address)?;
    Ok(())
}

/// Rejects addresses that cannot be turned into a `ws://host:port` URL.
pub(super) fn validate_listener_address(name: &str, value: &str) -> Result<()> {
    let address = value.trim();
    let Some((host, port)) = address.rsplit_once(':') else {
        bail!("{name} must be written as host:port, got {value:?}");
    };
    let host = host.trim_matches(['[', ']']);
    if host.is_empty()
        || host
            .chars()
            .any(|character| character.is_whitespace() || character == '/')
    {
        bail!("{name} must start with a host name or address, got {value:?}");
    }
    match port.parse::<u16>() {
        Ok(port) if port > 0 => Ok(()),
        _ => bail!("{name} must end with a TCP port between 1 and 65535, got {value:?}"),
    }
}

fn validate_fraction(name: &str, value: f64) -> Result<()> {
    if !range(0.0, 1.0).contains(value) {
        bail!("{name} must be between 0 and 1, got {value}");
    }
    Ok(())
}
