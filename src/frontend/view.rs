// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! GTK widget construction and the frontend overlay adapter.

use cairo::RectangleInt;
use gtk::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::Cell;

use floatlyrics_core::i18n::{I18n, Text};

use crate::shared::{
    config::AppConfig,
    presentation::{LyricSlotText, LyricsDocument, LyricsFrame},
};
mod adapter;
mod css;
mod layout;
mod panel;
mod positioning;
mod state;
mod web_lyrics;

use super::AppMsg;
use super::localization::bind_button_tooltip;
pub(super) use adapter::OverlaySender;
use css::OverlayStyle;
use layout::{
    MAX_EXPANDED_PANEL_WIDTH, PANEL_RESIZE_DURATION_US, animated_panel_width, compact_panel_width,
    effective_bottom_margin, expanded_panel_width, fallback_panel_height,
    lyrics_horizontal_padding, maximum_lyrics_width, viewport_height,
};
use positioning::{
    PlacementState, WindowPlacement, apply_snap_css_classes, attach_floating_drag,
    available_panel_width, bottom_margin_from_placement, initial_x, left_margin_for_width,
};
use state::OverlayStateHandle;
use web_lyrics::{WebLyricsView, font_family, lyric_content_width};

const PANEL_HORIZONTAL_GUTTER: i32 = 32;
const PANEL_CHROME_WIDTH: i32 = 28;

/// Restricts the surface input region to the interactive header area
/// (song-info label, action buttons, and separator) so that clicks on the
/// lyrics viewport pass through to windows below.
fn setup_input_region(window: &gtk::ApplicationWindow) {
    let Some(surface) = window.surface() else {
        return;
    };
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    if !display.supports_input_shapes() {
        return;
    }

    let Some(content) = window.child() else {
        return;
    };
    let Some(header) = content.first_child() else {
        return;
    };

    let Some(header_bounds) = header.compute_bounds(window) else {
        return;
    };

    let separator_bottom = header
        .next_sibling()
        .and_then(|sep| sep.compute_bounds(window))
        .map(|b| b.y() + b.height())
        .unwrap_or_else(|| header_bounds.y() + header_bounds.height());

    let x = header_bounds.x() as i32;
    let y = header_bounds.y() as i32;
    let width = header_bounds.width() as i32;
    let height = (separator_bottom - header_bounds.y()) as i32;

    if width <= 0 || height <= 0 {
        surface.set_input_region(None::<&cairo::Region>);
        return;
    }

    let region = cairo::Region::create_rectangle(&RectangleInt::new(x, y, width, height));
    surface.set_input_region(Some(&region));
}

#[derive(Clone)]
pub(super) struct OverlayView {
    window: gtk::ApplicationWindow,
    content: gtk::Box,
    state: OverlayStateHandle,
    style: OverlayStyle,
    placement: PlacementState,
    song_info: gtk::Label,
    lyrics_viewport: gtk::Box,
    web_lyrics: WebLyricsView,
    i18n: I18n,
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
        config.lyrics.apple_music_style,
    );
    let fallback_height = fallback_panel_height(viewport_h);
    let initial_font_family = font_family(&config.lyrics.font_order);
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
                    fallback_height,
                )
            })
            .unwrap_or_else(|| effective_bottom_margin(config)),
    );
    window.set_exclusive_zone(-1);
    window.add_css_class("floating-window");

    let panel = panel::build(panel_width, viewport_h, sender.clone());
    let song_info = panel.song_info;
    let manual_search_button = panel.manual_search_button;
    let settings_button = panel.settings_button;
    let close_button = panel.close_button;
    let lyrics_viewport = panel.lyrics_viewport;
    let content = panel.content;

    bind_button_tooltip(&manual_search_button, &i18n, Text::ManualSearchTooltip);
    bind_button_tooltip(&settings_button, &i18n, Text::OpenSettingsTooltip);
    bind_button_tooltip(&close_button, &i18n, Text::CloseTooltip);

    let web_lyrics = WebLyricsView::new(config, i18n.text(Text::OpenSpotify));
    lyrics_viewport.append(&web_lyrics.widget());
    let placement = attach_floating_drag(
        window,
        &content,
        panel_width.saturating_add(PANEL_CHROME_WIDTH),
        fallback_height,
        initial_placement,
        move |position| {
            let _ = sender.send(AppMsg::WindowMoved(position));
        },
    );

    let style = OverlayStyle::install(config.window.opacity, initial_font_family);

    window.set_child(Some(&content));
    window.connect_map(|window| {
        setup_input_region(window);
    });
    let overlay = OverlayView {
        window: window.clone(),
        content,
        state: OverlayStateHandle::new(config, panel_width),
        style,
        placement,
        song_info,
        lyrics_viewport,
        web_lyrics,
        i18n: i18n.clone(),
    };
    {
        let overlay = overlay.clone();
        i18n.subscribe(move |language| {
            let static_status = overlay.state.static_status();
            if let Some(key) = static_status {
                set_status_lyrics(&overlay, language.text(key), key);
            }
        });
    }
    overlay
}

fn apply_panel_width(
    content: &gtk::Box,
    lyrics_viewport: &gtk::Box,
    window: &gtk::ApplicationWindow,
    placement: &PlacementState,
    width: i32,
) {
    content.set_width_request(width);
    lyrics_viewport.set_width_request(width);
    if let Some(left_margin) = left_margin_for_width(
        window,
        &placement.current(),
        width.saturating_add(PANEL_CHROME_WIDTH),
    ) {
        window.set_margin(Edge::Left, left_margin);
    }
    apply_snap_css_classes(content, &placement.current());
    setup_input_region(window);
}

fn set_status_lyrics(floating: &OverlayView, message: &str, key: Text) {
    floating.render_lyrics(LyricsFrame {
        key: format!("status:{key:?}"),
        content: LyricSlotText::message(message),
        position_ms: None,
        playing: false,
        seeking: false,
    });
}

impl OverlayView {
    pub(super) fn set_song_info(&self, value: &str) {
        self.song_info.set_label(value);
    }

    pub(super) fn set_lyrics_document(&self, document: &LyricsDocument) {
        self.web_lyrics.set_document(document);
    }

    pub(super) fn show_lyrics(&self, frame: LyricsFrame) {
        self.state.show_content();
        self.render_lyrics(frame);
    }

    pub(super) fn show_status(&self, key: Text) {
        self.state.show_status(key);
        set_status_lyrics(self, self.i18n.text(key), key);
    }

    pub(super) fn tick_widget(&self) -> gtk::Widget {
        self.lyrics_viewport.clone().upcast()
    }

    pub(super) fn apply_config(&self, config: &AppConfig) {
        let width = compact_panel_width(config.window.width);
        let viewport_h = viewport_height(
            config.lyrics.lyric_font_size,
            config.lyrics.romanization_font_size,
            config.lyrics.translation_font_size,
            config.lyrics.show_romanization,
            config.lyrics.apple_music_style,
        );
        let fallback_height = fallback_panel_height(viewport_h);
        self.state.apply_config(config, width);
        self.content.set_width_request(width);
        self.lyrics_viewport.set_width_request(width);
        self.window.set_margin(
            Edge::Bottom,
            bottom_margin_from_placement(
                &self.window,
                &self.placement.current(),
                width.saturating_add(PANEL_CHROME_WIDTH),
                fallback_height,
            )
            .unwrap_or_else(|| effective_bottom_margin(config)),
        );
        if let Some(left_margin) = left_margin_for_width(
            &self.window,
            &self.placement.current(),
            width.saturating_add(PANEL_CHROME_WIDTH),
        ) {
            self.window.set_margin(Edge::Left, left_margin);
        }
        self.lyrics_viewport.set_height_request(viewport_h);
        let family = font_family(&config.lyrics.font_order);
        self.style.apply(config.window.opacity, family);
        self.web_lyrics.apply_config(config);
        self.sync_snap_classes();

        let window = self.window.clone();
        let placement = self.placement.clone();
        gtk::glib::idle_add_local_once(move || {
            if let Some(bottom_margin) = bottom_margin_from_placement(
                &window,
                &placement.current(),
                width.saturating_add(PANEL_CHROME_WIDTH),
                fallback_height,
            ) {
                window.set_margin(Edge::Bottom, bottom_margin);
            }
            setup_input_region(&window);
        });
    }

    fn render_lyrics(&self, frame: LyricsFrame) {
        let resize = self.state.register_frame(&frame);
        if let Some(animate) = resize {
            self.resize_for_lyrics(&frame.content, animate);
        }
        self.web_lyrics.show(frame);
    }

    fn resize_for_lyrics(&self, value: &LyricSlotText, animate: bool) {
        let metrics = self.state.metrics();
        let lyric_font_px = metrics.lyric_font_size;
        let font_family = self.style.font_family();
        let measured_width = lyric_content_width(
            &self.song_info,
            value,
            &font_family,
            lyric_font_px,
            metrics.romanization_font_size,
            metrics.translation_font_size,
        )
        .saturating_add(lyrics_horizontal_padding(
            metrics.apple_music_style,
            lyric_font_px,
        ));
        self.resize_to_measured_width(measured_width, animate);
    }

    fn resize_to_measured_width(&self, measured_width: i32, animate: bool) {
        let metrics = self.state.metrics();
        let available_width = available_panel_width(&self.window, PANEL_HORIZONTAL_GUTTER)
            .unwrap_or(MAX_EXPANDED_PANEL_WIDTH)
            .saturating_sub(PANEL_CHROME_WIDTH);
        let available_width = maximum_lyrics_width(available_width, metrics.apple_music_style);
        let width = expanded_panel_width(metrics.compact_width, measured_width, available_width);
        if animate {
            self.animate_panel_width(width);
        } else {
            self.cancel_width_animation();
            apply_panel_width(
                &self.content,
                &self.lyrics_viewport,
                &self.window,
                &self.placement,
                width,
            );
        }
    }

    fn animate_panel_width(&self, target_width: i32) {
        let generation = self.state.cancel_animation();
        let compact_width = self.state.metrics().compact_width;
        let start_width = self.content.width_request().max(compact_width);
        if start_width == target_width {
            apply_panel_width(
                &self.content,
                &self.lyrics_viewport,
                &self.window,
                &self.placement,
                target_width,
            );
            return;
        }

        let content = self.content.clone();
        let lyrics_viewport = self.lyrics_viewport.clone();
        let window = self.window.clone();
        let placement = self.placement.clone();
        let state = self.state.clone();
        let start_time_us = Cell::new(None);
        self.content.add_tick_callback(move |_, frame_clock| {
            if state.animation_generation() != generation {
                return gtk::glib::ControlFlow::Break;
            }

            let now_us = frame_clock.frame_time();
            let animation_start_us = start_time_us.get().unwrap_or_else(|| {
                start_time_us.set(Some(now_us));
                now_us
            });
            let elapsed_us = now_us.saturating_sub(animation_start_us);
            let width = animated_panel_width(start_width, target_width, elapsed_us);
            apply_panel_width(&content, &lyrics_viewport, &window, &placement, width);

            if elapsed_us >= PANEL_RESIZE_DURATION_US {
                gtk::glib::ControlFlow::Break
            } else {
                gtk::glib::ControlFlow::Continue
            }
        });
    }

    fn cancel_width_animation(&self) {
        self.state.cancel_animation();
    }

    fn sync_snap_classes(&self) {
        apply_snap_css_classes(&self.content, &self.placement.current());
    }
}
