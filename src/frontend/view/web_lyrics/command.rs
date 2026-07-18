// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Typed JavaScript command protocol for the embedded lyrics frontend.

use serde::Serialize;

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
