// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! GTK widget construction and the narrow UI API used by the controller.

use gtk::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use floatlyrics_core::i18n::{I18n, Text};

use crate::config::AppConfig;
mod css;
mod positioning;
mod rendering;

use super::localization::bind_button_tooltip;
use super::model::{KaraokeRenderState, LyricSlotText, progress_fraction, progress_text};
use positioning::{
    SharedPlacement, apply_snap_css_classes, attach_floating_drag, available_panel_width,
    bottom_margin_from_placement, initial_x, left_margin_for_width,
};
use rendering::{
    TextLineRenderState, TextLineStyle, lyric_content_width, lyric_text_widget, text_line_area,
};

const MIN_PANEL_WIDTH: i32 = 320;
const MAX_PANEL_WIDTH: i32 = 640;
const MAX_EXPANDED_PANEL_WIDTH: i32 = 960;
const PANEL_HORIZONTAL_GUTTER: i32 = 32;
const PANEL_CHROME_WIDTH: i32 = 28;
const TEXT_HORIZONTAL_PADDING: i32 = 24;
const CURRENT_KARAOKE_HEIGHT: i32 = 36;
const CURRENT_TRANSLATION_HEIGHT: i32 = 18;
const LYRICS_VIEWPORT_HEIGHT: i32 = CURRENT_KARAOKE_HEIGHT + CURRENT_TRANSLATION_HEIGHT;
const LYRICS_TRANSITION_DURATION_MS: u32 = 180;
const FALLBACK_PANEL_HEIGHT: i32 = 84;

/// Narrow interface the controller uses to update the display.
/// Decouples controller logic from the concrete [`OverlayView`].
pub(super) trait LyricsView {
    fn tick_widget(&self) -> gtk::Stack;
    fn set_song_info(&self, value: &str);
    fn show_lyrics(&self, value: LyricSlotText, key: &str);
    fn show_status(&self, key: Text);
    fn set_progress(&self, position_ms: Option<u64>, duration_ms: Option<u64>);
    fn reset_progress(&self);
}

#[derive(Clone)]
pub(super) struct OverlayView {
    window: gtk::ApplicationWindow,
    content: gtk::Box,
    compact_width: Rc<Cell<i32>>,
    opacity_provider: gtk::CssProvider,
    placement: SharedPlacement,
    song_info: gtk::Label,
    manual_search_button: gtk::Button,
    settings_button: gtk::Button,
    close_button: gtk::Button,
    progress: gtk::ProgressBar,
    progress_label: gtk::Label,
    lyrics_stack: gtk::Stack,
    lyric_slots: [LyricSlotWidgets; 2],
    lyrics_transition: Rc<RefCell<LyricsTransitionState>>,
    i18n: I18n,
    static_status: Rc<RefCell<Option<Text>>>,
}

#[derive(Clone)]
struct LyricSlotWidgets {
    text: gtk::Label,
    translation_area: gtk::DrawingArea,
    translation_state: Rc<RefCell<TextLineRenderState>>,
    translation_row: gtk::Box,
    container: gtk::Box,
    karaoke_area: Option<gtk::DrawingArea>,
    karaoke_state: Option<Rc<RefCell<KaraokeRenderState>>>,
}

#[derive(Debug, Clone, Default)]
struct LyricsTransitionState {
    current_key: Option<String>,
    active_slot: usize,
}

pub(super) fn build(app: &gtk::Application, config: &AppConfig, i18n: I18n) -> OverlayView {
    let panel_width = compact_panel_width(config.window.width);
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("FloatLyrics Overlay")
        .decorated(false)
        .resizable(false)
        .build();

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
        initial_x(panel_width.saturating_add(PANEL_CHROME_WIDTH)).unwrap_or_default(),
    );
    window.set_margin(Edge::Bottom, effective_bottom_margin(config));
    window.set_exclusive_zone(-1);
    window.add_css_class("floating-window");

    let song_info = gtk::Label::builder()
        .label("FloatLyrics")
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Center)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(48)
        .single_line_mode(true)
        .css_classes(["floating-song-info"])
        .build();

    let manual_search_button = gtk::Button::builder()
        .icon_name("system-search-symbolic")
        .valign(gtk::Align::Center)
        .css_classes(["flat", "circular", "floating-action-button"])
        .build();
    bind_button_tooltip(&manual_search_button, &i18n, Text::ManualSearchTooltip);
    let settings_button = gtk::Button::builder()
        .icon_name("preferences-system-symbolic")
        .valign(gtk::Align::Center)
        .css_classes(["flat", "circular", "floating-action-button"])
        .build();
    bind_button_tooltip(&settings_button, &i18n, Text::OpenSettingsTooltip);
    let close_button = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .valign(gtk::Align::Center)
        .css_classes(["flat", "circular", "floating-action-button"])
        .build();
    bind_button_tooltip(&close_button, &i18n, Text::CloseTooltip);
    let button_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    button_box.append(&manual_search_button);
    button_box.append(&settings_button);
    button_box.append(&close_button);
    let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    title_row.append(&song_info);
    title_row.append(&button_box);

    let progress = gtk::ProgressBar::builder()
        .fraction(0.0)
        .hexpand(true)
        .valign(gtk::Align::Center)
        .css_classes(["floating-progress"])
        .build();

    let progress_label = gtk::Label::builder()
        .label("")
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .width_chars(12)
        .single_line_mode(true)
        .css_classes(["floating-progress-label"])
        .build();

    let progress_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    progress_row.set_halign(gtk::Align::Fill);
    progress_row.append(&progress);
    progress_row.append(&progress_label);

    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    separator.add_css_class("floating-separator");

    let primary = lyric_slot(
        ["floating-slot-current"],
        ["floating-lyric-current"],
        ["floating-translation-current"],
        i18n.text(Text::OpenSpotify),
        Some((panel_width, CURRENT_KARAOKE_HEIGHT)),
        translation_style(true),
        panel_width,
    );
    let secondary = lyric_slot(
        ["floating-slot-current"],
        ["floating-lyric-current"],
        ["floating-translation-current"],
        "",
        Some((panel_width, CURRENT_KARAOKE_HEIGHT)),
        translation_style(true),
        panel_width,
    );

    let lyrics_stack = gtk::Stack::builder()
        .width_request(panel_width)
        .height_request(LYRICS_VIEWPORT_HEIGHT)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .hhomogeneous(true)
        .vhomogeneous(true)
        .transition_type(gtk::StackTransitionType::Crossfade)
        .transition_duration(LYRICS_TRANSITION_DURATION_MS)
        .build();
    lyrics_stack.add_named(&primary.container, Some("primary"));
    lyrics_stack.add_named(&secondary.container, Some("secondary"));
    lyrics_stack.set_visible_child(&primary.container);
    let lyrics_transition = Rc::new(RefCell::new(LyricsTransitionState::default()));

    let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
    content.set_halign(gtk::Align::Center);
    content.set_valign(gtk::Align::Center);
    content.set_size_request(panel_width, -1);
    content.add_css_class("floating-panel");
    content.append(&title_row);
    content.append(&progress_row);
    content.append(&separator);
    content.append(&lyrics_stack);
    let placement = attach_floating_drag(
        &window,
        &content,
        panel_width.saturating_add(PANEL_CHROME_WIDTH),
        FALLBACK_PANEL_HEIGHT,
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

    window.set_child(Some(&content));
    window.present();

    let overlay = OverlayView {
        window,
        content,
        compact_width: Rc::new(Cell::new(panel_width)),
        opacity_provider,
        placement,
        song_info,
        manual_search_button,
        settings_button,
        close_button,
        progress,
        progress_label,
        lyrics_stack,
        lyric_slots: [primary, secondary],
        lyrics_transition,
        i18n: i18n.clone(),
        static_status: Rc::new(RefCell::new(Some(Text::OpenSpotify))),
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

fn lyric_slot(
    container_classes: [&str; 1],
    text_classes: [&str; 1],
    _translation_classes: [&str; 1],
    initial_text: &str,
    karaoke_size: Option<(i32, i32)>,
    translation_style: TextLineStyle,
    panel_width: i32,
) -> LyricSlotWidgets {
    let text = gtk::Label::builder()
        .label(initial_text)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(46)
        .single_line_mode(true)
        .css_classes(text_classes)
        .build();

    let (text_widget, karaoke_area, karaoke_state) = lyric_text_widget(&text, karaoke_size);
    let (translation_area, translation_state) = text_line_area(
        panel_width,
        translation_line_height(translation_style),
        translation_style,
    );
    let text_row = lyric_line_row(&text_widget, 0, 0);
    let translation_widget: gtk::Widget = translation_area.clone().upcast();
    let translation_row = lyric_line_row(&translation_widget, 0, 0);

    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    container.set_halign(gtk::Align::Center);
    container.set_valign(gtk::Align::Center);
    container.add_css_class(container_classes[0]);
    container.append(&text_row);
    container.append(&translation_row);

    LyricSlotWidgets {
        text,
        translation_area,
        translation_state,
        translation_row,
        container,
        karaoke_area,
        karaoke_state,
    }
}

fn translation_style(is_current: bool) -> TextLineStyle {
    if is_current {
        TextLineStyle {
            font_px: 13,
            color: (1.0, 1.0, 1.0, 0.78),
        }
    } else {
        TextLineStyle {
            font_px: 11,
            color: (1.0, 1.0, 1.0, 0.50),
        }
    }
}

fn translation_line_height(style: TextLineStyle) -> i32 {
    if style.font_px >= 13 {
        CURRENT_TRANSLATION_HEIGHT
    } else {
        16
    }
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

fn expanded_panel_width(compact_width: i32, content_width: i32, maximum_width: i32) -> i32 {
    content_width
        .max(compact_width)
        .min(maximum_width.max(compact_width))
}

fn lyric_line_row(widget: &gtk::Widget, margin_top: i32, margin_bottom: i32) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.set_halign(gtk::Align::Center);
    row.set_valign(gtk::Align::Center);
    row.set_margin_top(margin_top);
    row.set_margin_bottom(margin_bottom);
    row.append(widget);
    row
}

fn set_lyrics_slots(floating: &OverlayView, current: LyricSlotText, animation_key: &str) {
    let key_changed =
        floating.lyrics_transition.borrow().current_key.as_deref() != Some(animation_key);
    if key_changed {
        floating.resize_for_lyrics(&current);
    }

    let (slot_index, should_transition) =
        select_lyric_slot(&mut floating.lyrics_transition.borrow_mut(), animation_key);
    let slot = &floating.lyric_slots[slot_index];
    set_lyric_slot(slot, &current);

    if should_transition {
        floating.lyrics_stack.set_visible_child(&slot.container);
    }
}

fn set_status_lyrics(floating: &OverlayView, message: &str, key: Text) {
    set_lyrics_slots(
        floating,
        LyricSlotText::message(message),
        &format!("status:{key:?}"),
    );
}

fn select_lyric_slot(state: &mut LyricsTransitionState, key: &str) -> (usize, bool) {
    if state.current_key.as_deref() == Some(key) {
        return (state.active_slot, false);
    }

    let is_first_value = state.current_key.is_none();
    state.current_key = Some(key.to_string());
    if !is_first_value {
        state.active_slot = 1 - state.active_slot;
    }

    (state.active_slot, !is_first_value)
}

fn set_lyric_slot(slot: &LyricSlotWidgets, value: &LyricSlotText) {
    if let Some(karaoke) = &value.karaoke {
        if let (Some(area), Some(state)) = (&slot.karaoke_area, &slot.karaoke_state) {
            *state.borrow_mut() = karaoke.clone();
            slot.text.set_visible(false);
            area.set_visible(true);
            area.queue_draw();
        } else {
            slot.text.set_label(&value.text);
        }
    } else {
        slot.text.set_label(&value.text);
        slot.text.set_visible(true);
        if let Some(area) = &slot.karaoke_area {
            area.set_visible(false);
        }
    }
    slot.translation_state.borrow_mut().text = value.translation.clone();
    slot.translation_area.set_visible(true);
    slot.translation_area.queue_draw();
    slot.translation_row.set_visible(true);
}

fn update_progress(floating: &OverlayView, position_ms: Option<u64>, duration_ms: Option<u64>) {
    floating
        .progress
        .set_fraction(progress_fraction(position_ms, duration_ms).unwrap_or(0.0));
    floating.progress_label.set_label(
        progress_text(position_ms, duration_ms)
            .as_deref()
            .unwrap_or(""),
    );
}

fn reset_progress(floating: &OverlayView) {
    floating.progress.set_fraction(0.0);
    floating.progress_label.set_label("");
}

impl LyricsView for OverlayView {
    fn tick_widget(&self) -> gtk::Stack {
        self.lyrics_stack.clone()
    }

    fn set_song_info(&self, value: &str) {
        self.song_info.set_label(value);
    }

    fn show_lyrics(&self, value: LyricSlotText, key: &str) {
        *self.static_status.borrow_mut() = None;
        set_lyrics_slots(self, value, key);
    }

    fn show_status(&self, key: Text) {
        *self.static_status.borrow_mut() = Some(key);
        set_status_lyrics(self, self.i18n.text(key), key);
    }

    fn set_progress(&self, position_ms: Option<u64>, duration_ms: Option<u64>) {
        update_progress(self, position_ms, duration_ms);
    }

    fn reset_progress(&self) {
        reset_progress(self);
    }
}

impl OverlayView {
    pub(super) fn connect_manual_search(&self, callback: impl Fn() + 'static) {
        self.manual_search_button
            .connect_clicked(move |_| callback());
    }

    pub(super) fn connect_settings(&self, callback: impl Fn() + 'static) {
        self.settings_button.connect_clicked(move |_| callback());
    }

    pub(super) fn connect_close(&self, callback: impl Fn() + 'static) {
        self.close_button.connect_clicked(move |_| callback());
    }

    pub(super) fn apply_config(&self, config: &AppConfig) {
        let width = compact_panel_width(config.window.width);
        self.compact_width.set(width);
        self.content.set_width_request(width);
        self.lyrics_stack.set_width_request(width);
        for slot in &self.lyric_slots {
            slot.translation_area.set_width_request(width);
            if let Some(area) = &slot.karaoke_area {
                area.set_width_request(width);
            }
        }
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
        self.lyrics_transition.borrow_mut().current_key = None;
        self.sync_snap_classes();
    }

    fn resize_for_lyrics(&self, value: &LyricSlotText) {
        let measured_width =
            lyric_content_width(&self.song_info, value).saturating_add(TEXT_HORIZONTAL_PADDING);
        let available_width = available_panel_width(&self.window, PANEL_HORIZONTAL_GUTTER)
            .unwrap_or(MAX_EXPANDED_PANEL_WIDTH)
            .saturating_sub(PANEL_CHROME_WIDTH)
            .min(MAX_EXPANDED_PANEL_WIDTH);
        let width = expanded_panel_width(self.compact_width.get(), measured_width, available_width);
        let next_surface_width = width.saturating_add(PANEL_CHROME_WIDTH);
        let next_left_margin =
            left_margin_for_width(&self.window, &self.placement.borrow(), next_surface_width);

        self.content.set_width_request(width);
        self.lyrics_stack.set_width_request(width);
        for slot in &self.lyric_slots {
            slot.translation_area.set_width_request(width);
            if let Some(area) = &slot.karaoke_area {
                area.set_width_request(width);
            }
        }
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
