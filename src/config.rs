// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Serializable user preferences and atomic file persistence.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use floatlyrics_core::i18n::Language;
use floatlyrics_lyrics::lyrics::LyricsProvider;

/// Complete application configuration persisted as TOML.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    /// General application preferences.
    pub general: GeneralConfig,
    /// Overlay window preferences.
    pub window: WindowConfig,
    /// Lyrics display and provider preferences.
    pub lyrics: LyricsConfig,
    /// Spotify-compatible MPRIS preferences.
    pub spotify: SpotifyConfig,
}

/// General application preferences.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct GeneralConfig {
    /// Active user-interface language.
    pub language: Language,
}

impl AppConfig {
    /// Loads `path`, creating and saving defaults when it does not exist.
    ///
    /// # Errors
    /// Returns an error when the file cannot be read, parsed, or initially saved.
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            let config = Self::default();
            config.save(path)?;
            return Ok(config);
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("reading config file {}", path.display()))?;
        toml::from_str(&content).with_context(|| format!("parsing config file {}", path.display()))
    }

    /// Atomically replaces the configuration at `path`.
    ///
    /// # Errors
    /// Returns an error when serialization, directory creation, writing, or
    /// replacement fails. A failed write cleans up its temporary file.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating config directory {}", parent.display()))?;
        }

        let content = toml::to_string_pretty(self).context("serializing config")?;
        let temporary = temporary_config_path(path)?;
        if let Err(error) = fs::write(&temporary, content)
            .with_context(|| format!("writing temporary config file {}", temporary.display()))
            .and_then(|()| {
                fs::rename(&temporary, path)
                    .with_context(|| format!("replacing config file {}", path.display()))
            })
        {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }

        Ok(())
    }
}

fn temporary_config_path(path: &Path) -> Result<PathBuf> {
    static NEXT_TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

    let file_name = path
        .file_name()
        .context("config path must point to a file")?
        .to_string_lossy();
    let id = NEXT_TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
    Ok(path.with_file_name(format!(".{file_name}.{}.{}.tmp", std::process::id(), id)))
}

/// Overlay window geometry and appearance preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct WindowConfig {
    /// Logical anchor used by the overlay.
    pub anchor: WindowAnchor,
    /// Distance from the selected screen edge, in pixels.
    pub margin: i32,
    /// Preferred compact overlay width, in pixels.
    pub width: i32,
    /// Background alpha in the inclusive range `0.0..=1.0`.
    pub opacity: f64,
    /// Height of a reserved bottom desktop panel, in pixels.
    pub bottom_panel_height: i32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            anchor: default_anchor(),
            margin: default_margin(),
            width: default_width(),
            opacity: default_opacity(),
            bottom_panel_height: 36,
        }
    }
}

/// Logical overlay anchor persisted in configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WindowAnchor {
    /// Center the overlay along the bottom screen edge.
    BottomCenter,
}

/// Lyrics timing, secondary-text, and provider preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct LyricsConfig {
    /// Global playback offset in milliseconds.
    pub offset_ms: i64,
    /// Automatic search priority.
    pub provider_order: Vec<LyricsProvider>,
    /// Whether translated text is displayed and fetched.
    pub show_translation: bool,
    /// Whether romanized text is displayed.
    pub show_romanization: bool,
    /// Ordered font-family fallback list used to render lyrics.
    pub font_order: Vec<String>,
    /// Font size in pixels for the current lyric line.
    pub lyric_font_size: i32,
    /// Font size in pixels for translation text.
    pub translation_font_size: i32,
    /// Color for played (filled) karaoke syllables, as `#RRGGBBAA` hex.
    pub played_color: String,
    /// Color for unplayed karaoke syllables, as `#RRGGBBAA` hex.
    pub unplayed_color: String,
    /// Color for translation text, as `#RRGGBBAA` hex.
    pub translation_color: String,
}

impl Default for LyricsConfig {
    fn default() -> Self {
        Self {
            offset_ms: 0,
            provider_order: default_provider_order(),
            show_translation: true,
            show_romanization: false,
            font_order: default_font_order(),
            lyric_font_size: default_lyric_font_size(),
            translation_font_size: default_translation_font_size(),
            played_color: default_played_color(),
            unplayed_color: default_unplayed_color(),
            translation_color: default_translation_color(),
        }
    }
}

/// Parses an `#RRGGBBAA` hex color string into an `(r, g, b, a)` tuple with
/// each channel in `0.0..=1.0`. Returns white when the input is invalid.
pub fn parse_hex_color(hex: &str) -> (f64, f64, f64, f64) {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() < 6 {
        return (1.0, 1.0, 1.0, 1.0);
    }
    let parse_byte = |offset: usize| {
        u8::from_str_radix(&hex[offset..offset + 2], 16).unwrap_or(0xff) as f64 / 255.0
    };
    let r = parse_byte(0);
    let g = parse_byte(2);
    let b = parse_byte(4);
    let a = if hex.len() >= 8 {
        parse_byte(6)
    } else {
        1.0
    };
    (r, g, b, a)
}

/// Formats an `(r, g, b, a)` tuple as an `#RRGGBBAA` hex string.
pub fn format_hex_color(color: (f64, f64, f64, f64)) -> String {
    let to_byte = |channel: f64| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        to_byte(color.0),
        to_byte(color.1),
        to_byte(color.2),
        to_byte(color.3),
    )
}

/// Spotify-compatible MPRIS discovery preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct SpotifyConfig {
    /// D-Bus well-known-name prefix accepted as a player instance.
    pub mpris_prefix: String,
}

impl Default for SpotifyConfig {
    fn default() -> Self {
        Self {
            mpris_prefix: default_spotify_prefix(),
        }
    }
}

fn default_anchor() -> WindowAnchor {
    WindowAnchor::BottomCenter
}

fn default_margin() -> i32 {
    96
}

fn default_width() -> i32 {
    350
}

fn default_opacity() -> f64 {
    0.78
}

fn default_provider_order() -> Vec<LyricsProvider> {
    LyricsProvider::default_order()
}

fn default_font_order() -> Vec<String> {
    vec!["Sans".to_string()]
}

fn default_lyric_font_size() -> i32 {
    24
}

fn default_translation_font_size() -> i32 {
    13
}

fn default_played_color() -> String {
    "#FFFFFFFF".to_string()
}

fn default_unplayed_color() -> String {
    "#9EA6B3FF".to_string()
}

fn default_translation_color() -> String {
    "#FFFFFFC7".to_string()
}

fn default_spotify_prefix() -> String {
    "org.mpris.MediaPlayer2.spotify".to_string()
}

#[cfg(test)]
#[path = "test/config_test.rs"]
mod tests;
