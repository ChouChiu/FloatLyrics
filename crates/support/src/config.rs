use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

use floatlyrics_core::lyrics::LyricsProvider;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    #[serde(default)]
    pub window: WindowConfig,
    #[serde(default)]
    pub lyrics: LyricsConfig,
    #[serde(default)]
    pub spotify: SpotifyConfig,
}

impl AppConfig {
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

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating config directory {}", parent.display()))?;
        }

        let content = toml::to_string_pretty(self).context("serializing config")?;
        fs::write(path, content).with_context(|| format!("writing config file {}", path.display()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowConfig {
    #[serde(default = "default_anchor")]
    pub anchor: WindowAnchor,
    #[serde(default = "default_margin")]
    pub margin: i32,
    #[serde(default = "default_width")]
    pub width: i32,
    #[serde(default = "default_opacity")]
    pub opacity: f64,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            anchor: default_anchor(),
            margin: default_margin(),
            width: default_width(),
            opacity: default_opacity(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WindowAnchor {
    BottomCenter,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LyricsConfig {
    #[serde(default)]
    pub offset_ms: i64,
    #[serde(default = "default_provider_order")]
    pub provider_order: Vec<LyricsProvider>,
    #[serde(default = "default_show_translation")]
    pub show_translation: bool,
    #[serde(default)]
    pub show_romanization: bool,
}

impl Default for LyricsConfig {
    fn default() -> Self {
        Self {
            offset_ms: 0,
            provider_order: default_provider_order(),
            show_translation: true,
            show_romanization: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpotifyConfig {
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
    960
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

fn default_spotify_prefix() -> String {
    "org.mpris.MediaPlayer2.spotify".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_provider_order_matches_plan() {
        assert_eq!(
            AppConfig::default().lyrics.provider_order,
            vec![LyricsProvider::QqMusic, LyricsProvider::NetEase]
        );
    }
}
