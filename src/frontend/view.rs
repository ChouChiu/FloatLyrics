// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! GTK widget construction and the frontend overlay adapter.

use cairo::RectangleInt;
use gtk::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use floatlyrics_core::i18n::{I18n, Text};

use crate::shared::{
    config::AppConfig,
    presentation::{LyricSlotText, LyricsDocument, LyricsFrame},
};
mod adapter;
mod layout;
mod positioning;
mod state;
pub(super) mod web_lyrics;

use super::AppMsg;
pub(super) use adapter::OverlaySender;
use layout::{
    MAX_EXPANDED_PANEL_WIDTH, PANEL_RESIZE_DURATION_US, animated_panel_width, compact_panel_width,
    effective_bottom_margin, expanded_panel_width, fallback_panel_height,
    lyrics_horizontal_padding, maximum_lyrics_width, viewport_height,
};
use positioning::{
    DragMode, FloatingDragLayout, PlacementState, WindowPlacement, attach_floating_drag,
    available_panel_width, bottom_margin_from_placement, initial_x, left_position_for_width,
    reposition_for_width, reposition_internal_content, snap_classes,
};
use state::OverlayStateHandle;
use web_lyrics::{UiSurface, WebLyricsView, font_family, lyric_content_width};

const PANEL_HORIZONTAL_GUTTER: i32 = 32;
const PANEL_HEADER_HEIGHT: i32 = 42;
const PANEL_ACTIONS_WIDTH: i32 = 232;

/// Restricts the surface input region to the React toolbar so that clicks on
/// the lyrics viewport pass through to windows below.
fn setup_input_region(window: &gtk::ApplicationWindow, content: &gtk::Box) {
    let Some(surface) = window.surface() else {
        return;
    };
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    if !display.supports_input_shapes() {
        return;
    }

    let Some(bounds) = content.compute_bounds(window) else {
        return;
    };
    let x = bounds.x() as i32;
    let y = bounds.y() as i32;
    let width = bounds.width() as i32;
    let height = PANEL_HEADER_HEIGHT.min(bounds.height() as i32);

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
    stage: gtk::Fixed,
    content: gtk::Box,
    state: OverlayStateHandle,
    placement: PlacementState,
    song_info: Rc<RefCell<String>>,
    track_offset_ms: Rc<Cell<i64>>,
    lyrics_viewport: gtk::Box,
    web_lyrics: WebLyricsView,
    measurement_label: gtk::Label,
    font_family: Rc<RefCell<String>>,
    i18n: I18n,
}

pub(super) fn build(
    window: &gtk::ApplicationWindow,
    config: &AppConfig,
    i18n: I18n,
    about: Rc<serde_json::Value>,
    sender: relm4::Sender<AppMsg>,
) -> OverlayView {
    let drag_mode = desktop_drag_mode(std::env::var("XDG_CURRENT_DESKTOP").ok().as_deref());
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
    window.set_title(Some("FloatLyrics Overlay"));
    window.set_decorated(false);
    window.set_resizable(false);

    window.init_layer_shell();
    window.set_namespace(Some("floatlyrics"));
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::None);
    window.set_anchor(Edge::Bottom, true);
    window.set_anchor(Edge::Left, true);
    // KWin can apply desktop effects whenever a layer surface moves. On KDE,
    // keep the surface fixed to the output and drag only its child panel.
    window.set_anchor(Edge::Right, drag_mode.is_internal());
    window.set_anchor(Edge::Top, drag_mode.is_internal());
    if !drag_mode.is_internal() {
        window.set_margin(
            Edge::Left,
            initial_placement
                .and_then(|placement| left_position_for_width(window, &placement, panel_width))
                .or_else(|| initial_x(panel_width))
                .unwrap_or_default(),
        );
        window.set_margin(
            Edge::Bottom,
            initial_placement
                .and_then(|placement| {
                    bottom_margin_from_placement(window, &placement, panel_width, fallback_height)
                })
                .unwrap_or_else(|| effective_bottom_margin(config)),
        );
    }
    window.set_exclusive_zone(-1);
    window.add_css_class("floating-window");

    super::style::install(
        "window.floating-window, window.floating-window > contents { background: transparent; box-shadow: none; }",
    );
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.set_size_request(panel_width, fallback_height);
    content.set_halign(gtk::Align::Center);
    content.set_valign(gtk::Align::Center);
    let lyrics_viewport = content.clone();
    let web_lyrics = WebLyricsView::new(
        config,
        i18n.text(Text::OpenPlayer),
        UiSurface::Overlay,
        about,
        sender.clone(),
    );
    let web_stack = gtk::Overlay::new();
    web_stack.set_hexpand(true);
    web_stack.set_vexpand(true);
    web_stack.set_child(Some(&web_lyrics.widget()));

    // WebKit owns its own event surface, so gestures attached only to GTK
    // ancestors do not reliably receive pointer motion. Put a native target
    // above the non-interactive song-title portion of the React toolbar.
    let drag_handle = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    drag_handle.set_hexpand(true);
    drag_handle.set_halign(gtk::Align::Fill);
    drag_handle.set_valign(gtk::Align::Start);
    drag_handle.set_height_request(PANEL_HEADER_HEIGHT);
    drag_handle.set_margin_end(PANEL_ACTIONS_WIDTH);
    drag_handle.set_widget_name("overlay-drag-handle");
    web_stack.add_overlay(&drag_handle);
    content.append(&web_stack);
    let stage = gtk::Fixed::new();
    stage.set_hexpand(true);
    stage.set_vexpand(true);
    let input_window = window.downgrade();
    let input_content = content.downgrade();
    let placement_view = web_lyrics.clone();
    let placement = attach_floating_drag(
        window,
        &content,
        &drag_handle,
        FloatingDragLayout {
            stage: stage.clone(),
            fallback_width: panel_width,
            fallback_height,
            initial_placement,
            initial_bottom_margin: effective_bottom_margin(config),
            mode: drag_mode,
        },
        move |placement| placement_view.set_overlay_placement(snap_classes(&placement)),
        move |position| {
            let _ = sender.send(AppMsg::WindowMoved(position));
            if drag_mode.is_internal() {
                let (Some(window), Some(content)) =
                    (input_window.upgrade(), input_content.upgrade())
                else {
                    return;
                };
                gtk::glib::idle_add_local_once(move || setup_input_region(&window, &content));
            }
        },
    );

    window.set_child(Some(&stage));
    {
        let stage = stage.downgrade();
        let content = content.downgrade();
        let web_stack = web_stack.downgrade();
        let drag_handle = drag_handle.downgrade();
        let placement = placement.clone();
        window.connect_map(move |window| {
            let (Some(stage), Some(content), Some(web_stack), Some(drag_handle)) = (
                stage.upgrade(),
                content.upgrade(),
                web_stack.upgrade(),
                drag_handle.upgrade(),
            ) else {
                return;
            };
            reposition_internal_content(
                window,
                &stage,
                &content,
                &placement,
                panel_width,
                fallback_height,
            );
            setup_input_region(window, &content);
            gtk::glib::idle_add_local_once(move || {
                let picked = web_stack.pick(16.0, 16.0, gtk::PickFlags::DEFAULT);
                if picked
                    .as_ref()
                    .is_some_and(|widget| widget == drag_handle.upcast_ref::<gtk::Widget>())
                {
                    tracing::debug!("overlay title drag target is active");
                } else {
                    tracing::warn!(
                        picked = picked.as_ref().map(gtk::Widget::widget_name).as_deref(),
                        "overlay title drag target is not receiving pointer events"
                    );
                }
            });
        });
    }
    let overlay = OverlayView {
        window: window.clone(),
        stage,
        content,
        state: OverlayStateHandle::new(config, panel_width),
        placement,
        song_info: Rc::new(RefCell::new("FloatLyrics".to_string())),
        track_offset_ms: Rc::new(Cell::new(0)),
        lyrics_viewport,
        web_lyrics,
        measurement_label: gtk::Label::new(None),
        font_family: Rc::new(RefCell::new(font_family(&config.lyrics.font_order))),
        i18n: i18n.clone(),
    };
    {
        let overlay = overlay.clone();
        i18n.subscribe(move |language| {
            overlay.render_overlay_state(language);
            let static_status = overlay.state.static_status();
            if let Some(key) = static_status {
                set_status_lyrics(&overlay, language.text(key), key);
            }
        });
    }
    overlay
}

fn apply_panel_width(
    stage: &gtk::Fixed,
    content: &gtk::Box,
    lyrics_viewport: &gtk::Box,
    window: &gtk::ApplicationWindow,
    placement: &PlacementState,
    width: i32,
) {
    content.set_width_request(width);
    lyrics_viewport.set_width_request(width);
    reposition_for_width(window, stage, content, placement, width);
    setup_input_region(window, content);
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
        if self.song_info.borrow().as_str() == value {
            return;
        }
        *self.song_info.borrow_mut() = value.to_string();
        self.render_overlay_state(self.i18n.language());
    }

    pub(super) fn set_track_offset(&self, offset_ms: i64) {
        self.track_offset_ms.set(offset_ms);
        self.render_overlay_state(self.i18n.language());
    }

    fn render_overlay_state(&self, language: floatlyrics_core::i18n::Language) {
        let label = track_offset_label(
            self.track_offset_ms.get(),
            language.text(Text::MillisecondsShort),
        );
        self.web_lyrics
            .set_overlay_state(&self.song_info.borrow(), &label);
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
        if self.placement.uses_internal_drag() {
            reposition_internal_content(
                &self.window,
                &self.stage,
                &self.content,
                &self.placement,
                width,
                fallback_height,
            );
        } else {
            self.window.set_margin(
                Edge::Bottom,
                bottom_margin_from_placement(
                    &self.window,
                    &self.placement.current(),
                    width,
                    fallback_height,
                )
                .unwrap_or_else(|| effective_bottom_margin(config)),
            );
        }
        reposition_for_width(
            &self.window,
            &self.stage,
            &self.content,
            &self.placement,
            width,
        );
        self.lyrics_viewport.set_height_request(viewport_h);
        self.content.set_height_request(fallback_height);
        *self.font_family.borrow_mut() = font_family(&config.lyrics.font_order);
        self.web_lyrics.refresh_bootstrap(config);
        self.web_lyrics.apply_config(config);
        self.web_lyrics
            .set_overlay_appearance(config.window.opacity);
        self.sync_snap_state();

        let window = self.window.clone();
        let stage = self.stage.clone();
        let content = self.content.clone();
        let placement = self.placement.clone();
        gtk::glib::idle_add_local_once(move || {
            if placement.uses_internal_drag() {
                reposition_internal_content(
                    &window,
                    &stage,
                    &content,
                    &placement,
                    width,
                    fallback_height,
                );
            } else if let Some(bottom_margin) =
                bottom_margin_from_placement(&window, &placement.current(), width, fallback_height)
            {
                window.set_margin(Edge::Bottom, bottom_margin);
            }
            setup_input_region(&window, &content);
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
        let font_family = self.font_family.borrow().clone();
        let measured_width = lyric_content_width(
            &self.measurement_label,
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
            .unwrap_or(MAX_EXPANDED_PANEL_WIDTH);
        let available_width = maximum_lyrics_width(available_width, metrics.apple_music_style);
        let width = expanded_panel_width(metrics.compact_width, measured_width, available_width);
        if animate {
            self.animate_panel_width(width);
        } else {
            self.cancel_width_animation();
            apply_panel_width(
                &self.stage,
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
                &self.stage,
                &self.content,
                &self.lyrics_viewport,
                &self.window,
                &self.placement,
                target_width,
            );
            return;
        }

        let stage = self.stage.clone();
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
            apply_panel_width(
                &stage,
                &content,
                &lyrics_viewport,
                &window,
                &placement,
                width,
            );

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

    fn sync_snap_state(&self) {
        self.web_lyrics
            .set_overlay_placement(snap_classes(&self.placement.current()));
    }
}

fn track_offset_label(offset_ms: i64, unit: &str) -> String {
    match offset_ms.cmp(&0) {
        std::cmp::Ordering::Greater => format!("+{offset_ms} {unit}"),
        std::cmp::Ordering::Less => format!("−{} {unit}", offset_ms.unsigned_abs()),
        std::cmp::Ordering::Equal => format!("0 {unit}"),
    }
}

fn desktop_drag_mode(desktop: Option<&str>) -> DragMode {
    if desktop.is_some_and(|desktop| {
        desktop
            .split([':', ';'])
            .any(|name| matches!(name.trim().to_ascii_lowercase().as_str(), "kde" | "plasma"))
    }) {
        DragMode::InternalPanel
    } else {
        DragMode::LayerSurface
    }
}

#[cfg(test)]
#[path = "../test/overlay_view_test.rs"]
mod tests;
