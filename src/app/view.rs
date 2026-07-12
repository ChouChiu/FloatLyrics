// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! GTK widget construction and the narrow UI API used by the controller.

use gtk::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use relm4::WidgetTemplate;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use floatlyrics_core::i18n::{I18n, Text};

use crate::config::AppConfig;
mod css;
mod positioning;
mod rendering;

use super::AppMsg;
use super::localization::bind_button_tooltip;
use super::model::{KaraokeRenderState, LyricSlotText};
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

struct OverlayPanelInit {
    width: i32,
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

            #[name = "lyrics_stack"]
            gtk::Stack {
                set_size_request: (init.width, LYRICS_VIEWPORT_HEIGHT),
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
                set_hhomogeneous: true,
                set_vhomogeneous: true,
                set_transition_type: gtk::StackTransitionType::Crossfade,
                set_transition_duration: LYRICS_TRANSITION_DURATION_MS,
            },
        }
    }
}

/// Narrow interface used by the playback controller.
///
/// Production updates are sent back through Relm4 as [`AppMsg`] values; tests
/// can still provide a small in-memory implementation of this interface.
pub(super) trait LyricsView {
    fn set_song_info(&self, value: &str);
    fn show_lyrics(&self, value: LyricSlotText, key: &str);
    fn show_status(&self, key: Text);
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
    font_size_provider: gtk::CssProvider,
    placement: SharedPlacement,
    song_info: gtk::Label,
    lyrics_stack: gtk::Stack,
    lyric_slots: [LyricSlotWidgets; 2],
    lyrics_transition: Rc<RefCell<LyricsTransitionState>>,
    i18n: I18n,
    static_status: Rc<RefCell<Option<Text>>>,
    lyric_font_size: Rc<Cell<i32>>,
    translation_font_size: Rc<Cell<i32>>,
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

pub(super) fn build(
    window: &gtk::ApplicationWindow,
    config: &AppConfig,
    i18n: I18n,
    sender: relm4::Sender<AppMsg>,
) -> OverlayView {
    let panel_width = compact_panel_width(config.window.width);
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
        initial_x(panel_width.saturating_add(PANEL_CHROME_WIDTH)).unwrap_or_default(),
    );
    window.set_margin(Edge::Bottom, effective_bottom_margin(config));
    window.set_exclusive_zone(-1);
    window.add_css_class("floating-window");

    let panel = OverlayPanel::init(OverlayPanelInit {
        width: panel_width,
        sender,
    });
    let song_info = panel.song_info.clone();
    let manual_search_button = panel.manual_search_button.clone();
    let settings_button = panel.settings_button.clone();
    let close_button = panel.close_button.clone();
    let lyrics_stack = panel.lyrics_stack.clone();
    let content = panel.content.clone();

    bind_button_tooltip(&manual_search_button, &i18n, Text::ManualSearchTooltip);
    bind_button_tooltip(&settings_button, &i18n, Text::OpenSettingsTooltip);
    bind_button_tooltip(&close_button, &i18n, Text::CloseTooltip);

    let lyric_font_size = Rc::new(Cell::new(config.lyrics.lyric_font_size));
    let translation_font_size = Rc::new(Cell::new(config.lyrics.translation_font_size));

    let primary = lyric_slot(
        ["floating-slot-current"],
        ["floating-lyric-current"],
        i18n.text(Text::OpenSpotify),
        Some((panel_width, CURRENT_KARAOKE_HEIGHT)),
        translation_style(true, config.lyrics.translation_font_size),
        panel_width,
        Rc::clone(&font_family),
        Rc::clone(&lyric_font_size),
    );
    let secondary = lyric_slot(
        ["floating-slot-current"],
        ["floating-lyric-current"],
        "",
        Some((panel_width, CURRENT_KARAOKE_HEIGHT)),
        translation_style(true, config.lyrics.translation_font_size),
        panel_width,
        Rc::clone(&font_family),
        Rc::clone(&lyric_font_size),
    );

    lyrics_stack.add_named(&primary.container, Some("primary"));
    lyrics_stack.add_named(&secondary.container, Some("secondary"));
    lyrics_stack.set_visible_child(&primary.container);
    let lyrics_transition = Rc::new(RefCell::new(LyricsTransitionState::default()));
    let placement = attach_floating_drag(
        window,
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

    let font_provider = gtk::CssProvider::new();
    load_font_css(&font_provider, &font_family.borrow());
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &font_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2,
        );
    }

    let font_size_provider = gtk::CssProvider::new();
    load_font_size_css(
        &font_size_provider,
        config.lyrics.lyric_font_size,
        config.lyrics.translation_font_size,
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &font_size_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 3,
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
        font_size_provider,
        placement,
        song_info,
        lyrics_stack,
        lyric_slots: [primary, secondary],
        lyrics_transition,
        i18n: i18n.clone(),
        static_status: Rc::new(RefCell::new(Some(Text::OpenSpotify))),
        lyric_font_size,
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

#[allow(clippy::too_many_arguments)]
fn lyric_slot(
    container_classes: [&str; 1],
    text_classes: [&str; 1],
    initial_text: &str,
    karaoke_size: Option<(i32, i32)>,
    translation_style: TextLineStyle,
    panel_width: i32,
    font_family: Rc<RefCell<String>>,
    lyric_font_size: Rc<Cell<i32>>,
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

    let (text_widget, karaoke_area, karaoke_state) =
        lyric_text_widget(&text, karaoke_size, Rc::clone(&font_family), lyric_font_size);
    let (translation_area, translation_state) = text_line_area(
        panel_width,
        translation_line_height(translation_style),
        translation_style,
        &font_family.borrow(),
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

fn translation_style(is_current: bool, translation_font_px: i32) -> TextLineStyle {
    let (font_px, color) = if is_current {
        (translation_font_px, (1.0, 1.0, 1.0, 0.78))
    } else {
        ((translation_font_px * 5 / 6).max(8), (1.0, 1.0, 1.0, 0.50))
    };
    TextLineStyle { font_px, color }
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

fn font_family(order: &[String]) -> String {
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

fn load_font_size_css(provider: &gtk::CssProvider, lyric_px: i32, translation_px: i32) {
    provider.load_from_string(&css::font_size_css(lyric_px, translation_px));
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

impl OverlayView {
    pub(super) fn set_song_info(&self, value: &str) {
        self.song_info.set_label(value);
    }

    pub(super) fn show_lyrics(&self, value: LyricSlotText, key: &str) {
        *self.static_status.borrow_mut() = None;
        set_lyrics_slots(self, value, key);
    }

    pub(super) fn show_status(&self, key: Text) {
        *self.static_status.borrow_mut() = Some(key);
        set_status_lyrics(self, self.i18n.text(key), key);
    }

    pub(super) fn tick_widget(&self) -> gtk::Stack {
        self.lyrics_stack.clone()
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
        load_font_size_css(
            &self.font_size_provider,
            config.lyrics.lyric_font_size,
            config.lyrics.translation_font_size,
        );
        self.lyric_font_size.set(config.lyrics.lyric_font_size);
        self.translation_font_size.set(config.lyrics.translation_font_size);
        let family = font_family(&config.lyrics.font_order);
        *self.font_family.borrow_mut() = family.clone();
        load_font_css(&self.font_provider, &family);
        for slot in &self.lyric_slots {
            slot.translation_state.borrow_mut().font_family = family.clone();
            let new_translation_style =
                translation_style(true, config.lyrics.translation_font_size);
            slot.translation_state.borrow_mut().style = new_translation_style;
            slot.translation_area.queue_draw();
            if let Some(area) = &slot.karaoke_area {
                area.queue_draw();
            }
        }
        self.lyrics_transition.borrow_mut().current_key = None;
        self.sync_snap_classes();
    }

    fn resize_for_lyrics(&self, value: &LyricSlotText) {
        let measured_width =
            lyric_content_width(
                &self.song_info,
                value,
                &self.font_family.borrow(),
                self.lyric_font_size.get(),
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
