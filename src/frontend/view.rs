// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! GTK widget construction and the frontend overlay adapter.

use gtk::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use relm4::WidgetTemplate;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use floatlyrics_core::i18n::{I18n, Text};

use crate::{
    backend::LyricsView,
    shared::{config::AppConfig, presentation::LyricSlotText},
};
mod css;
mod positioning;
mod web_lyrics;

use super::AppMsg;
use super::localization::bind_button_tooltip;
use positioning::{
    SharedPlacement, WindowPlacement, apply_snap_css_classes, attach_floating_drag,
    available_panel_width, bottom_margin_from_placement, initial_x, left_margin_for_width,
};
use web_lyrics::{WebLyricsView, font_family, lyric_content_width};

const MIN_PANEL_WIDTH: i32 = 320;
const MAX_PANEL_WIDTH: i32 = 640;
const MAX_EXPANDED_PANEL_WIDTH: i32 = 960;
const PANEL_HORIZONTAL_GUTTER: i32 = 32;
const PANEL_CHROME_WIDTH: i32 = 28;
const TEXT_HORIZONTAL_PADDING: i32 = 24;
const MIN_KARAOKE_HEIGHT: i32 = 36;
const MIN_ROMANIZATION_HEIGHT: i32 = 18;
const MIN_TRANSLATION_HEIGHT: i32 = 18;
const FALLBACK_PANEL_HEIGHT: i32 = 84;

fn karaoke_line_height(lyric_font_px: i32) -> i32 {
    (lyric_font_px as f64 * 1.5).ceil() as i32
}

fn secondary_line_height(font_px: i32) -> i32 {
    (font_px as f64 * 1.5).ceil() as i32
}

fn viewport_height(
    lyric_font_px: i32,
    romanization_font_px: i32,
    translation_font_px: i32,
    show_romanization: bool,
) -> i32 {
    let romanization_height = if show_romanization {
        secondary_line_height(romanization_font_px).max(MIN_ROMANIZATION_HEIGHT)
    } else {
        0
    };
    karaoke_line_height(lyric_font_px).max(MIN_KARAOKE_HEIGHT)
        + romanization_height
        + secondary_line_height(translation_font_px).max(MIN_TRANSLATION_HEIGHT)
}

struct OverlayPanelInit {
    width: i32,
    viewport_height: i32,
    sender: relm4::Sender<AppMsg>,
}

#[relm4::widget_template]
impl WidgetTemplate for OverlayPanel {
    type Init = OverlayPanelInit;

    view! {
        #[name = "content"]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 4,
            set_halign: gtk::Align::Center,
            set_valign: gtk::Align::Center,
            set_size_request: (init.width, -1),
            add_css_class: "floating-panel",

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 6,

                #[name = "song_info"]
                gtk::Label {
                    set_label: "FloatLyrics",
                    set_halign: gtk::Align::Start,
                    set_valign: gtk::Align::Center,
                    set_hexpand: true,
                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                    set_max_width_chars: 48,
                    set_single_line_mode: true,
                    add_css_class: "floating-song-info",
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 4,

                    #[name = "manual_search_button"]
                    gtk::Button {
                        set_icon_name: "system-search-symbolic",
                        set_valign: gtk::Align::Center,
                        set_css_classes: &["flat", "circular", "floating-action-button"],
                        connect_clicked[sender = init.sender.clone()] => move |_| {
                            let _ = sender.send(AppMsg::OpenManualSearch);
                        },
                    },
                    #[name = "settings_button"]
                    gtk::Button {
                        set_icon_name: "emblem-system-symbolic",
                        set_valign: gtk::Align::Center,
                        set_css_classes: &["flat", "circular", "floating-action-button"],
                        connect_clicked[sender = init.sender.clone()] => move |_| {
                            let _ = sender.send(AppMsg::OpenSettings);
                        },
                    },
                    #[name = "close_button"]
                    gtk::Button {
                        set_icon_name: "window-close-symbolic",
                        set_valign: gtk::Align::Center,
                        set_css_classes: &["flat", "circular", "floating-action-button"],
                        connect_clicked[sender = init.sender.clone()] => move |_| {
                            let _ = sender.send(AppMsg::Quit);
                        },
                    },
                },
            },

            gtk::Separator {
                set_orientation: gtk::Orientation::Horizontal,
                add_css_class: "floating-separator",
            },

            #[name = "lyrics_viewport"]
            gtk::Box {
                set_size_request: (init.width, init.viewport_height),
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
            },
        }
    }
}

/// Message-only handle to the overlay component state.
///
/// Keeping GTK widgets out of the playback controller makes `AppModel::update`
/// the single place where the concrete view is mutated.
#[derive(Clone)]
pub(super) struct OverlaySender {
    sender: relm4::Sender<AppMsg>,
}

impl OverlaySender {
    pub(super) fn new(sender: relm4::Sender<AppMsg>) -> Self {
        Self { sender }
    }
}

impl LyricsView for OverlaySender {
    fn set_song_info(&self, value: &str) {
        let _ = self.sender.send(AppMsg::SetSongInfo(value.to_string()));
    }

    fn show_lyrics(&self, value: LyricSlotText, key: &str) {
        let _ = self.sender.send(AppMsg::ShowLyrics(value, key.to_string()));
    }

    fn show_status(&self, key: Text) {
        let _ = self.sender.send(AppMsg::ShowStatus(key));
    }
}

#[derive(Clone)]
pub(super) struct OverlayView {
    window: gtk::ApplicationWindow,
    content: gtk::Box,
    compact_width: Rc<Cell<i32>>,
    opacity_provider: gtk::CssProvider,
    font_family: Rc<RefCell<String>>,
    font_provider: gtk::CssProvider,
    placement: SharedPlacement,
    song_info: gtk::Label,
    lyrics_viewport: gtk::Box,
    web_lyrics: WebLyricsView,
    last_lyrics_layout: Rc<RefCell<Option<(String, String)>>>,
    i18n: I18n,
    static_status: Rc<RefCell<Option<Text>>>,
    lyric_font_size: Rc<Cell<i32>>,
    romanization_font_size: Rc<Cell<i32>>,
    translation_font_size: Rc<Cell<i32>>,
}

pub(super) fn build(
    window: &gtk::ApplicationWindow,
    config: &AppConfig,
    i18n: I18n,
    sender: relm4::Sender<AppMsg>,
) -> OverlayView {
    let panel_width = compact_panel_width(config.window.width);
    let initial_placement = if config.window.remember_position {
        config.window.position.map(WindowPlacement::from_position)
    } else {
        None
    };
    let viewport_h = viewport_height(
        config.lyrics.lyric_font_size,
        config.lyrics.romanization_font_size,
        config.lyrics.translation_font_size,
        config.lyrics.show_romanization,
    );
    let font_family = Rc::new(RefCell::new(font_family(&config.lyrics.font_order)));
    window.set_title(Some("FloatLyrics Overlay"));
    window.set_decorated(false);
    window.set_resizable(false);

    window.init_layer_shell();
    window.set_namespace(Some("floatlyrics"));
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::None);
    window.set_anchor(Edge::Bottom, true);
    window.set_anchor(Edge::Left, true);
    window.set_anchor(Edge::Right, false);
    window.set_anchor(Edge::Top, false);
    window.set_margin(
        Edge::Left,
        initial_placement
            .and_then(|placement| {
                left_margin_for_width(
                    window,
                    &placement,
                    panel_width.saturating_add(PANEL_CHROME_WIDTH),
                )
            })
            .or_else(|| initial_x(panel_width.saturating_add(PANEL_CHROME_WIDTH)))
            .unwrap_or_default(),
    );
    window.set_margin(
        Edge::Bottom,
        initial_placement
            .and_then(|placement| {
                bottom_margin_from_placement(
                    window,
                    &placement,
                    panel_width.saturating_add(PANEL_CHROME_WIDTH),
                    FALLBACK_PANEL_HEIGHT,
                )
            })
            .unwrap_or_else(|| effective_bottom_margin(config)),
    );
    window.set_exclusive_zone(-1);
    window.add_css_class("floating-window");

    let panel = OverlayPanel::init(OverlayPanelInit {
        width: panel_width,
        viewport_height: viewport_h,
        sender: sender.clone(),
    });
    let song_info = panel.song_info.clone();
    let manual_search_button = panel.manual_search_button.clone();
    let settings_button = panel.settings_button.clone();
    let close_button = panel.close_button.clone();
    let lyrics_viewport = panel.lyrics_viewport.clone();
    let content = panel.content.clone();

    bind_button_tooltip(&manual_search_button, &i18n, Text::ManualSearchTooltip);
    bind_button_tooltip(&settings_button, &i18n, Text::OpenSettingsTooltip);
    bind_button_tooltip(&close_button, &i18n, Text::CloseTooltip);

    let lyric_font_size = Rc::new(Cell::new(config.lyrics.lyric_font_size));
    let romanization_font_size = Rc::new(Cell::new(config.lyrics.romanization_font_size));
    let translation_font_size = Rc::new(Cell::new(config.lyrics.translation_font_size));
    let web_lyrics = WebLyricsView::new(config, i18n.text(Text::OpenSpotify));
    lyrics_viewport.append(&web_lyrics.widget());
    let placement = attach_floating_drag(
        window,
        &content,
        panel_width.saturating_add(PANEL_CHROME_WIDTH),
        FALLBACK_PANEL_HEIGHT,
        initial_placement,
        move |position| {
            let _ = sender.send(AppMsg::WindowMoved(position));
        },
    );

    let provider = gtk::CssProvider::new();
    let panel_alpha = config.window.opacity.clamp(0.18, 0.72);
    let css = css::panel_css(panel_alpha);
    provider.load_from_string(&css);

    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let opacity_provider = gtk::CssProvider::new();
    load_opacity_css(&opacity_provider, config.window.opacity);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &opacity_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
    }

    let font_provider = gtk::CssProvider::new();
    load_font_css(&font_provider, &font_family.borrow());
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &font_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2,
        );
    }

    window.set_child(Some(&content));
    let overlay = OverlayView {
        window: window.clone(),
        content,
        compact_width: Rc::new(Cell::new(panel_width)),
        opacity_provider,
        font_family,
        font_provider,
        placement,
        song_info,
        lyrics_viewport,
        web_lyrics,
        last_lyrics_layout: Rc::new(RefCell::new(None)),
        i18n: i18n.clone(),
        static_status: Rc::new(RefCell::new(Some(Text::OpenSpotify))),
        lyric_font_size,
        romanization_font_size,
        translation_font_size,
    };
    {
        let overlay = overlay.clone();
        i18n.subscribe(move |language| {
            if let Some(key) = *overlay.static_status.borrow() {
                set_status_lyrics(&overlay, language.text(key), key);
            }
        });
    }
    overlay
}

fn compact_panel_width(configured_width: i32) -> i32 {
    configured_width.clamp(MIN_PANEL_WIDTH, MAX_PANEL_WIDTH)
}

fn effective_bottom_margin(config: &AppConfig) -> i32 {
    config
        .window
        .margin
        .max(config.window.bottom_panel_height)
        .max(0)
}

fn load_opacity_css(provider: &gtk::CssProvider, opacity: f64) {
    let opacity = opacity.clamp(0.15, 1.0);
    provider.load_from_string(&format!(
        ".floating-panel {{ background: rgba(10, 12, 16, {opacity:.3}); }}"
    ));
}

fn load_font_css(provider: &gtk::CssProvider, family: &str) {
    let css_families = family
        .split(',')
        .map(str::trim)
        .map(|name| format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(", ");
    provider.load_from_string(&format!(
        ".floating-panel {{ font-family: {css_families}; }}"
    ));
}

fn expanded_panel_width(compact_width: i32, content_width: i32, maximum_width: i32) -> i32 {
    content_width
        .max(compact_width)
        .min(maximum_width.max(compact_width))
}

fn set_status_lyrics(floating: &OverlayView, message: &str, key: Text) {
    floating.render_lyrics(LyricSlotText::message(message), &format!("status:{key:?}"));
}

impl OverlayView {
    pub(super) fn set_song_info(&self, value: &str) {
        self.song_info.set_label(value);
    }

    pub(super) fn show_lyrics(&self, value: LyricSlotText, key: &str) {
        *self.static_status.borrow_mut() = None;
        self.render_lyrics(value, key);
    }

    pub(super) fn show_status(&self, key: Text) {
        *self.static_status.borrow_mut() = Some(key);
        set_status_lyrics(self, self.i18n.text(key), key);
    }

    pub(super) fn tick_widget(&self) -> gtk::Widget {
        self.lyrics_viewport.clone().upcast()
    }

    pub(super) fn apply_config(&self, config: &AppConfig) {
        let width = compact_panel_width(config.window.width);
        self.compact_width.set(width);
        self.content.set_width_request(width);
        self.lyrics_viewport.set_width_request(width);
        self.window.set_margin(
            Edge::Bottom,
            bottom_margin_from_placement(
                &self.window,
                &self.placement.borrow(),
                width.saturating_add(PANEL_CHROME_WIDTH),
                FALLBACK_PANEL_HEIGHT,
            )
            .unwrap_or_else(|| effective_bottom_margin(config)),
        );
        if let Some(left_margin) = left_margin_for_width(
            &self.window,
            &self.placement.borrow(),
            width.saturating_add(PANEL_CHROME_WIDTH),
        ) {
            self.window.set_margin(Edge::Left, left_margin);
        }
        load_opacity_css(&self.opacity_provider, config.window.opacity);
        self.lyric_font_size.set(config.lyrics.lyric_font_size);
        self.romanization_font_size
            .set(config.lyrics.romanization_font_size);
        self.translation_font_size
            .set(config.lyrics.translation_font_size);
        let karaoke_h = karaoke_line_height(config.lyrics.lyric_font_size).max(MIN_KARAOKE_HEIGHT);
        let romanization_h = secondary_line_height(config.lyrics.romanization_font_size)
            .max(MIN_ROMANIZATION_HEIGHT);
        let translation_h =
            secondary_line_height(config.lyrics.translation_font_size).max(MIN_TRANSLATION_HEIGHT);
        let romanization_viewport_h = if config.lyrics.show_romanization {
            romanization_h
        } else {
            0
        };
        self.lyrics_viewport
            .set_height_request(karaoke_h + romanization_viewport_h + translation_h);
        let family = font_family(&config.lyrics.font_order);
        *self.font_family.borrow_mut() = family.clone();
        load_font_css(&self.font_provider, &family);
        self.web_lyrics.apply_config(config);
        *self.last_lyrics_layout.borrow_mut() = None;
        self.sync_snap_classes();
    }

    fn render_lyrics(&self, value: LyricSlotText, key: &str) {
        let layout_key = (key.to_string(), value.romanization.clone());
        if self.last_lyrics_layout.borrow().as_ref() != Some(&layout_key) {
            self.resize_for_lyrics(&value);
            *self.last_lyrics_layout.borrow_mut() = Some(layout_key);
        }
        self.web_lyrics.show(value, key);
    }

    fn resize_for_lyrics(&self, value: &LyricSlotText) {
        let measured_width = lyric_content_width(
            &self.song_info,
            value,
            &self.font_family.borrow(),
            self.lyric_font_size.get(),
            self.romanization_font_size.get(),
            self.translation_font_size.get(),
        )
        .saturating_add(TEXT_HORIZONTAL_PADDING);
        let available_width = available_panel_width(&self.window, PANEL_HORIZONTAL_GUTTER)
            .unwrap_or(MAX_EXPANDED_PANEL_WIDTH)
            .saturating_sub(PANEL_CHROME_WIDTH)
            .min(MAX_EXPANDED_PANEL_WIDTH);
        let width = expanded_panel_width(self.compact_width.get(), measured_width, available_width);
        let next_surface_width = width.saturating_add(PANEL_CHROME_WIDTH);
        let next_left_margin =
            left_margin_for_width(&self.window, &self.placement.borrow(), next_surface_width);

        self.content.set_width_request(width);
        self.lyrics_viewport.set_width_request(width);
        if let Some(left_margin) = next_left_margin {
            self.window.set_margin(Edge::Left, left_margin);
        }
        self.sync_snap_classes();
    }

    fn sync_snap_classes(&self) {
        apply_snap_css_classes(&self.content, &self.placement.borrow());
    }
}

#[cfg(test)]
#[path = "../test/view_test.rs"]
mod tests;
