// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Typed JavaScript command protocol for the embedded lyrics frontend.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::shared::{
    config::{AppConfig, parse_hex_color},
    presentation::{LyricsDocument, LyricsFrame},
};

const TRANSITION_DURATION_MS: u32 = 180;

#[derive(Debug, Clone, Serialize)]
struct LyricsStyle {
    font_family: String,
    lyric_font_px: i32,
    romanization_font_px: i32,
    translation_font_px: i32,
    played_color: String,
    unplayed_color: String,
    romanization_color: String,
    translation_color: String,
    transition_ms: u32,
}

impl LyricsStyle {
    fn from_config(config: &AppConfig) -> Self {
        Self {
            font_family: super::metrics::font_family(&config.lyrics.font_order),
            lyric_font_px: config.lyrics.lyric_font_size,
            romanization_font_px: config.lyrics.romanization_font_size,
            translation_font_px: config.lyrics.translation_font_size,
            played_color: css_color(&config.lyrics.played_color),
            unplayed_color: css_color(&config.lyrics.unplayed_color),
            romanization_color: css_color(&config.lyrics.romanization_color),
            translation_color: css_color(&config.lyrics.translation_color),
            transition_ms: TRANSITION_DURATION_MS,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum LyricsCommand<'a> {
    Configure {
        apple_music_style: bool,
        style: LyricsStyle,
    },
    Document {
        document: &'a LyricsDocument,
    },
    Frame {
        frame: &'a LyricsFrame,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(in crate::frontend) enum UiSurface {
    Overlay,
    ControlCenter,
    ManualSearch,
    FontPicker,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(in crate::frontend) enum ControlPage {
    General,
    Display,
    Sources,
    About,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum UiCommand<'a> {
    Bootstrap {
        surface: UiSurface,
        config: &'a AppConfig,
        strings: BTreeMap<&'static str, &'static str>,
        version: &'static str,
        about: &'a Value,
        available_fonts: &'a [String],
    },
    ConfigState {
        config: &'a AppConfig,
        saved: bool,
        error: Option<&'a str>,
    },
    SearchState {
        state: &'a Value,
    },
    Navigate {
        page: ControlPage,
    },
    OverlayState {
        song_info: &'a str,
        track_offset: &'a str,
    },
    OverlayPlacement {
        classes: &'a [&'a str],
    },
    OverlayAppearance {
        opacity: f64,
    },
}

pub(super) fn configure_script(config: &AppConfig) -> serde_json::Result<String> {
    render_script(&LyricsCommand::Configure {
        apple_music_style: config.lyrics.apple_music_style,
        style: LyricsStyle::from_config(config),
    })
}

pub(super) fn document_script(document: &LyricsDocument) -> serde_json::Result<String> {
    render_script(&LyricsCommand::Document { document })
}

pub(super) fn frame_script(frame: &LyricsFrame) -> serde_json::Result<String> {
    render_script(&LyricsCommand::Frame { frame })
}

pub(super) fn bootstrap_script(
    surface: UiSurface,
    config: &AppConfig,
    strings: BTreeMap<&'static str, &'static str>,
    about: &Value,
    available_fonts: &[String],
) -> serde_json::Result<String> {
    render_script(&UiCommand::Bootstrap {
        surface,
        config,
        strings,
        version: env!("CARGO_PKG_VERSION"),
        about,
        available_fonts,
    })
}

pub(super) fn config_state_script(
    config: &AppConfig,
    saved: bool,
    error: Option<&str>,
) -> serde_json::Result<String> {
    render_script(&UiCommand::ConfigState {
        config,
        saved,
        error,
    })
}

pub(super) fn search_state_script(state: &Value) -> serde_json::Result<String> {
    render_script(&UiCommand::SearchState { state })
}

pub(super) fn navigate_script(page: ControlPage) -> serde_json::Result<String> {
    render_script(&UiCommand::Navigate { page })
}

pub(super) fn overlay_state_script(
    song_info: &str,
    track_offset: &str,
) -> serde_json::Result<String> {
    render_script(&UiCommand::OverlayState {
        song_info,
        track_offset,
    })
}

pub(super) fn overlay_placement_script(classes: &[&str]) -> serde_json::Result<String> {
    render_script(&UiCommand::OverlayPlacement { classes })
}

pub(super) fn overlay_appearance_script(opacity: f64) -> serde_json::Result<String> {
    render_script(&UiCommand::OverlayAppearance {
        opacity: opacity.clamp(0.15, 1.0),
    })
}

fn render_script(command: &impl Serialize) -> serde_json::Result<String> {
    serde_json::to_string(command).map(|json| {
        format!(
            "((command) => {{ if (window.floatLyrics) {{ window.floatLyrics.dispatch(command); }} else {{ (window.floatLyricsPendingCommands ??= []).push(command); }} }})({json});"
        )
    })
}

fn css_color(value: &str) -> String {
    let (red, green, blue, alpha) = parse_hex_color(value);
    format!(
        "rgba({},{},{},{alpha:.4})",
        (red * 255.0).round() as u8,
        (green * 255.0).round() as u8,
        (blue * 255.0).round() as u8,
    )
}

#[cfg(test)]
#[path = "../../../test/web_lyrics_command_test.rs"]
mod tests;
