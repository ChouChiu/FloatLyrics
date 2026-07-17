// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Frontend WebKitGTK renderer for the lyrics viewport.

use gtk::prelude::*;
use serde::Serialize;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use webkit6::prelude::*;

use crate::shared::{
    config::{AppConfig, parse_hex_color},
    presentation::{LyricSlotText, LyricsDocument, LyricsFrame},
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
            font_family: font_family(&config.lyrics.font_order),
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

#[derive(Default)]
struct BridgeState {
    ready: bool,
    in_flight: bool,
    pending_config: Option<String>,
    pending_document: Option<String>,
    pending_frame: Option<String>,
}

#[derive(Clone, Copy)]
enum CommandSlot {
    Config,
    Document,
    Frame,
}

/// A transparent, non-interactive WebKit view backed by packaged HTML.
#[derive(Clone)]
pub(super) struct WebLyricsView {
    web_view: webkit6::WebView,
    bridge: Rc<RefCell<BridgeState>>,
    document_revision: Rc<Cell<Option<u64>>>,
}

impl WebLyricsView {
    pub(super) fn new(config: &AppConfig, initial_text: &str) -> Self {
        let settings = webkit6::Settings::new();
        settings.set_auto_load_images(false);
        settings.set_enable_developer_extras(false);
        settings.set_enable_html5_database(false);
        settings.set_enable_html5_local_storage(false);
        settings.set_enable_javascript(true);
        settings.set_enable_media(false);
        settings.set_enable_page_cache(false);
        settings.set_enable_webgl(false);
        settings.set_javascript_can_access_clipboard(false);
        settings.set_javascript_can_open_windows_automatically(false);

        let web_view = webkit6::WebView::builder().settings(&settings).build();
        web_view.set_background_color(&gtk::gdk::RGBA::TRANSPARENT);
        web_view.set_can_target(false);
        web_view.set_focusable(false);
        web_view.set_hexpand(true);
        web_view.set_vexpand(true);
        web_view.connect_context_menu(|_, _, _| true);

        let bridge = Rc::new(RefCell::new(BridgeState::default()));

        {
            let bridge = Rc::clone(&bridge);
            web_view.connect_load_changed(move |view, event| match event {
                webkit6::LoadEvent::Started => bridge.borrow_mut().ready = false,
                webkit6::LoadEvent::Finished => {
                    bridge.borrow_mut().ready = true;
                    dispatch_pending(view, &bridge);
                }
                _ => {}
            });
        }

        web_view.load_html(include_str!(concat!(env!("OUT_DIR"), "/lyrics.html")), None);
        let renderer = Self {
            web_view,
            bridge,
            document_revision: Rc::new(Cell::new(None)),
        };
        renderer.apply_config(config);
        renderer.show(LyricsFrame {
            key: "initial".to_string(),
            content: LyricSlotText::message(initial_text),
            position_ms: None,
            playing: false,
            seeking: false,
        });
        renderer
    }

    pub(super) fn widget(&self) -> webkit6::WebView {
        self.web_view.clone()
    }

    pub(super) fn set_document(&self, document: &LyricsDocument) {
        if self.document_revision.get() == Some(document.revision) {
            return;
        }
        self.document_revision.set(Some(document.revision));
        self.submit(CommandSlot::Document, &LyricsCommand::Document { document });
    }

    pub(super) fn show(&self, frame: LyricsFrame) {
        self.submit(CommandSlot::Frame, &LyricsCommand::Frame { frame: &frame });
    }

    pub(super) fn apply_config(&self, config: &AppConfig) {
        self.submit(
            CommandSlot::Config,
            &LyricsCommand::Configure {
                apple_music_style: config.lyrics.apple_music_style,
                style: LyricsStyle::from_config(config),
            },
        );
    }

    fn submit(&self, slot: CommandSlot, command: &impl Serialize) {
        match render_script(command) {
            Ok(script) => {
                let mut bridge = self.bridge.borrow_mut();
                match slot {
                    CommandSlot::Config => bridge.pending_config = Some(script),
                    CommandSlot::Document => bridge.pending_document = Some(script),
                    CommandSlot::Frame => bridge.pending_frame = Some(script),
                }
                drop(bridge);
                dispatch_pending(&self.web_view, &self.bridge);
            }
            Err(error) => tracing::warn!(%error, "failed to serialize lyrics for WebKit"),
        }
    }
}

fn dispatch_pending(web_view: &webkit6::WebView, bridge: &Rc<RefCell<BridgeState>>) {
    let script = {
        let mut state = bridge.borrow_mut();
        if !state.ready || state.in_flight {
            return;
        }
        let scripts = [
            state.pending_config.take(),
            state.pending_document.take(),
            state.pending_frame.take(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        if scripts.is_empty() {
            return;
        }
        state.in_flight = true;
        scripts.join("\n")
    };

    let weak_view = web_view.downgrade();
    let bridge = Rc::clone(bridge);
    web_view.evaluate_javascript(
        &script,
        None,
        Some("floatlyrics://lyrics/update"),
        None::<&gtk::gio::Cancellable>,
        move |result| {
            bridge.borrow_mut().in_flight = false;
            if let Err(error) = result {
                tracing::warn!(%error, "failed to update the WebKit lyrics view");
            }
            if let Some(view) = weak_view.upgrade() {
                dispatch_pending(&view, &bridge);
            }
        },
    );
}

fn render_script(command: &impl Serialize) -> serde_json::Result<String> {
    serde_json::to_string(command).map(|json| {
        format!(
            "((command) => {{ if (window.floatLyrics) {{ window.floatLyrics.dispatch(command); }} else {{ (window.floatLyricsPendingCommands ??= []).push(command); }} }})({json});"
        )
    })
}

pub(super) fn lyric_content_width(
    measure_widget: &gtk::Label,
    value: &LyricSlotText,
    font_family: &str,
    lyric_font_px: i32,
    romanization_font_px: i32,
    translation_font_px: i32,
) -> i32 {
    let fonts = LyricsFontMetrics {
        family: font_family,
        lyric_px: lyric_font_px,
        romanization_px: romanization_font_px,
        translation_px: translation_font_px,
    };
    let lyric_text = value
        .karaoke
        .as_ref()
        .map_or(value.text.as_str(), |karaoke| karaoke.text.as_str());
    lyric_text_group_width(
        measure_widget,
        lyric_text,
        &value.romanization,
        &value.translation,
        fonts,
    )
}

#[derive(Clone, Copy)]
struct LyricsFontMetrics<'a> {
    family: &'a str,
    lyric_px: i32,
    romanization_px: i32,
    translation_px: i32,
}

fn lyric_text_group_width(
    measure_widget: &gtk::Label,
    lyric_text: &str,
    romanization: &str,
    translation: &str,
    fonts: LyricsFontMetrics<'_>,
) -> i32 {
    text_pixel_width(
        measure_widget,
        lyric_text,
        fonts.lyric_px,
        true,
        fonts.family,
    )
    .max(text_pixel_width(
        measure_widget,
        romanization,
        fonts.romanization_px,
        false,
        fonts.family,
    ))
    .max(text_pixel_width(
        measure_widget,
        translation,
        fonts.translation_px,
        false,
        fonts.family,
    ))
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

pub(super) fn font_family(order: &[String]) -> String {
    let families = order
        .iter()
        .map(|family| family.trim())
        .filter(|family| !family.is_empty())
        .collect::<Vec<_>>();
    if families.is_empty() {
        "Sans".to_string()
    } else {
        families.join(", ")
    }
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
mod tests {
    use super::*;

    #[test]
    fn render_script_serializes_lyrics_as_data() {
        let frame = LyricsFrame {
            key: "line:1".to_string(),
            content: LyricSlotText::message("'quoted' </script> 歌词"),
            position_ms: Some(1_000),
            playing: true,
            seeking: false,
        };

        let script = render_script(&LyricsCommand::Frame { frame: &frame }).unwrap();

        assert!(script.starts_with("((command) => {"));
        assert!(script.contains("window.floatLyrics.dispatch(command)"));
        assert!(script.contains("window.floatLyricsPendingCommands"));
        assert!(script.contains("\"type\":\"frame\""));
        assert!(script.contains("\"key\":\"line:1\""));
        assert!(script.contains("'quoted' </script> 歌词"));
    }

    #[test]
    fn invalid_config_color_uses_opaque_white() {
        assert_eq!(css_color("invalid"), "rgba(255,255,255,1.0000)");
    }

    #[test]
    fn embedded_html_contains_the_bridge_without_external_assets() {
        let html = include_str!(concat!(env!("OUT_DIR"), "/lyrics.html"));

        assert!(html.contains("Content-Security-Policy"));
        assert!(html.contains("floatLyrics"));
        assert!(!html.contains("<script src="));
        assert!(!html.contains("<link rel=\"stylesheet\""));
    }
}
