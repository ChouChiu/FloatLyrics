// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Shared serializable preferences and atomic file persistence.

use serde::{Deserialize, Serialize};

use floatlyrics_core::i18n::Language;
pub use floatlyrics_lyrics::lyrics::{ChineseRomanizationMode, LyricsProvider};

mod persistence;
mod recovery;
mod validation;

pub(crate) use validation::ConfigLimits;

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
    /// MPRIS player discovery and selection preferences.
    #[serde(alias = "spotify")]
    pub player: PlayerConfig,
    /// AMLL WebSocket sender preferences.
    pub amll: AmllConfig,
    /// System tray preferences.
    pub tray: TrayConfig,
}

impl AppConfig {
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        validation::validate(self)
    }
}

/// General application preferences.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct GeneralConfig {
    /// Active user-interface language.
    pub language: Language,
    /// Run mode selected for this installation.
    pub mode: AppMode,
}

/// Mutually exclusive application run modes.
///
/// The floating overlay and the AMLL WebSocket sender present the same
/// playback state in two different ways, so only one of them can own the
/// lyrics output at a time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppMode {
    /// Draw lyrics in the GTK layer-shell overlay.
    #[default]
    Floating,
    /// Forward playback state and lyrics to an AMLL WebSocket listener.
    Amll,
}

/// AMLL WebSocket sender preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct AmllConfig {
    /// Host and port of the AMLL player listener, for example `localhost:11444`.
    pub address: String,
}

impl Default for AmllConfig {
    fn default() -> Self {
        Self {
            address: default_amll_address(),
        }
    }
}

/// System tray preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct TrayConfig {
    /// Whether the StatusNotifierItem tray icon is published.
    pub enabled: bool,
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Overlay window geometry and appearance preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct WindowConfig {
    /// Logical anchor used by the overlay.
    pub anchor: WindowAnchor,
    /// Whether the overlay position is restored after restarting.
    pub remember_position: bool,
    /// Last position selected by dragging, expressed relative to the monitor.
    pub position: Option<WindowPosition>,
    /// Distance from the selected screen edge, in the inclusive range `0..=500` pixels.
    pub margin: i32,
    /// Preferred compact overlay width, in the inclusive range `320..=640` pixels.
    pub width: i32,
    /// Background alpha in the inclusive range `0.15..=1.0`.
    pub opacity: f64,
    /// Height of a reserved bottom desktop panel, in the inclusive range `0..=200` pixels.
    pub bottom_panel_height: i32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            anchor: default_anchor(),
            remember_position: true,
            position: None,
            margin: default_margin(),
            width: default_width(),
            opacity: default_opacity(),
            bottom_panel_height: 36,
        }
    }
}

/// Monitor-relative center point used to restore the floating overlay.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WindowPosition {
    /// Horizontal center as a fraction of the monitor width.
    pub horizontal: f64,
    /// Vertical center as a fraction of the monitor height.
    pub vertical: f64,
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
    /// Whether the single-current-line Apple Music-like lyrics renderer is enabled.
    pub apple_music_style: bool,
    /// Global playback offset in the inclusive range `-10000..=10000` milliseconds.
    pub offset_ms: i64,
    /// Automatic search priority.
    pub provider_order: Vec<LyricsProvider>,
    /// Whether translated text is displayed and fetched.
    pub show_translation: bool,
    /// Whether romanized text is displayed.
    pub show_romanization: bool,
    /// Pronunciation system used for Chinese lyrics.
    pub chinese_romanization: ChineseRomanizationMode,
    /// Ordered font-family fallback list used to render lyrics.
    pub font_order: Vec<String>,
    /// Font size in the inclusive range `12..=56` pixels for the current lyric line.
    pub lyric_font_size: i32,
    /// Font size in the inclusive range `8..=36` pixels for translation text.
    pub translation_font_size: i32,
    /// Font size in the inclusive range `8..=36` pixels for romanized text.
    pub romanization_font_size: i32,
    /// Color for played (filled) karaoke syllables, as `#RRGGBBAA` hex.
    pub played_color: String,
    /// Color for unplayed karaoke syllables, as `#RRGGBBAA` hex.
    pub unplayed_color: String,
    /// Color for translation text, as `#RRGGBBAA` hex.
    pub translation_color: String,
    /// Color for romanized text, as `#RRGGBBAA` hex.
    pub romanization_color: String,
}

impl Default for LyricsConfig {
    fn default() -> Self {
        Self {
            apple_music_style: false,
            offset_ms: 0,
            provider_order: default_provider_order(),
            show_translation: true,
            show_romanization: false,
            chinese_romanization: ChineseRomanizationMode::Auto,
            font_order: default_font_order(),
            lyric_font_size: default_lyric_font_size(),
            translation_font_size: default_translation_font_size(),
            romanization_font_size: default_romanization_font_size(),
            played_color: default_played_color(),
            unplayed_color: default_unplayed_color(),
            translation_color: default_translation_color(),
            romanization_color: default_romanization_color(),
        }
    }
}

/// Parses an `#RRGGBBAA` hex color string into an `(r, g, b, a)` tuple with
/// each channel in `0.0..=1.0`. Returns white and logs a warning when the
/// input is invalid.
pub fn parse_hex_color(hex: &str) -> (f64, f64, f64, f64) {
    let Some(hex) = normalized_hex_color(hex) else {
        tracing::warn!(%hex, "color must contain 6 or 8 hex digits, falling back to white");
        return (1.0, 1.0, 1.0, 1.0);
    };
    let parse_byte = |offset: usize| {
        u8::from_str_radix(&hex[offset..offset + 2], 16).expect("validated hexadecimal color byte")
            as f64
            / 255.0
    };
    let r = parse_byte(0);
    let g = parse_byte(2);
    let b = parse_byte(4);
    let a = if hex.len() >= 8 { parse_byte(6) } else { 1.0 };
    (r, g, b, a)
}

fn normalized_hex_color(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
    (matches!(hex.len(), 6 | 8) && hex.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(hex)
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

/// MPRIS player discovery and selection preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct PlayerConfig {
    /// Player selectors preferred when several players have the same playback state.
    ///
    /// A selector matches either a complete MPRIS bus name, its suffix after
    /// `org.mpris.MediaPlayer2.`, or the player's MPRIS identity.
    pub preferred_players: Vec<String>,
    /// Player selectors excluded from automatic discovery.
    pub ignored_players: Vec<String>,
    #[serde(default, rename = "mpris_prefix", skip_serializing)]
    legacy_mpris_prefix: Option<String>,
}

impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            preferred_players: vec!["spotify".to_string()],
            ignored_players: Vec::new(),
            legacy_mpris_prefix: None,
        }
    }
}

impl PlayerConfig {
    /// Returns the effective preferred-player selectors, including the former
    /// `spotify.mpris_prefix` representation when loading an older config.
    pub fn effective_preferred_players(&self) -> Vec<String> {
        self.legacy_mpris_prefix.as_ref().map_or_else(
            || self.preferred_players.clone(),
            |prefix| vec![prefix.clone()],
        )
    }

    fn normalize_legacy(&mut self) {
        if let Some(prefix) = self.legacy_mpris_prefix.take() {
            self.preferred_players = vec![prefix];
        }
    }
}

fn default_amll_address() -> String {
    "localhost:11444".to_string()
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

fn default_romanization_font_size() -> i32 {
    12
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

fn default_romanization_color() -> String {
    "#B8D8F0E6".to_string()
}

#[cfg(test)]
#[path = "../test/config_test.rs"]
mod tests;
