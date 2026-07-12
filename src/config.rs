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
pub struct AppConfig {
    /// General application preferences.
    #[serde(default)]
    pub general: GeneralConfig,
    /// Overlay window preferences.
    #[serde(default)]
    pub window: WindowConfig,
    /// Lyrics display and provider preferences.
    #[serde(default)]
    pub lyrics: LyricsConfig,
    /// Spotify-compatible MPRIS preferences.
    #[serde(default)]
    pub spotify: SpotifyConfig,
}

/// General application preferences.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeneralConfig {
    /// Active user-interface language.
    #[serde(default)]
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
pub struct WindowConfig {
    /// Logical anchor used by the overlay.
    #[serde(default = "default_anchor")]
    pub anchor: WindowAnchor,
    /// Distance from the selected screen edge, in pixels.
    #[serde(default = "default_margin")]
    pub margin: i32,
    /// Preferred compact overlay width, in pixels.
    #[serde(default = "default_width")]
    pub width: i32,
    /// Background alpha in the inclusive range `0.0..=1.0`.
    #[serde(default = "default_opacity")]
    pub opacity: f64,
    /// Height of a reserved bottom desktop panel, in pixels.
    #[serde(default)]
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
pub struct LyricsConfig {
    /// Global playback offset in milliseconds.
    #[serde(default)]
    pub offset_ms: i64,
    /// Automatic search priority.
    #[serde(default = "default_provider_order")]
    pub provider_order: Vec<LyricsProvider>,
    /// Whether translated text is displayed and fetched.
    #[serde(default = "default_show_translation")]
    pub show_translation: bool,
    /// Whether romanized text is displayed.
    #[serde(default)]
    pub show_romanization: bool,
    /// Ordered font-family fallback list used to render lyrics.
    #[serde(default = "default_font_order")]
    pub font_order: Vec<String>,
}

impl Default for LyricsConfig {
    fn default() -> Self {
        Self {
            offset_ms: 0,
            provider_order: default_provider_order(),
            show_translation: true,
            show_romanization: false,
            font_order: default_font_order(),
        }
    }
}

/// Spotify-compatible MPRIS discovery preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpotifyConfig {
    /// D-Bus well-known-name prefix accepted as a player instance.
    #[serde(default = "default_spotify_prefix")]
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

fn default_show_translation() -> bool {
    true
}

fn default_provider_order() -> Vec<LyricsProvider> {
    LyricsProvider::default_order()
}

fn default_font_order() -> Vec<String> {
    vec!["Sans".to_string()]
}

fn default_spotify_prefix() -> String {
    "org.mpris.MediaPlayer2.spotify".to_string()
}

#[cfg(test)]
#[path = "test/config_test.rs"]
mod tests;
