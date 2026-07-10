use anyhow::{Context, Result};
use gtk::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::{cell::RefCell, rc::Rc, sync::mpsc, time::Instant};

use crate::{
    cache::{Cache, CachedLyrics, ProviderResultInsert},
    config::AppConfig,
    lyrics::{
        FetchedLyrics, SearchPlan, TimedLine, active_line_index, line_index_at_or_before,
        search_best_lyrics, timed_lines_from_raw,
    },
    mpris::{PlaybackStatus, SpotifyPlayerState, SpotifyWatcherEvent, spawn_spotify_watcher},
    paths::AppPaths,
    track::TrackMetadata,
};

const CURRENT_LYRIC_FONT_PX: i32 = 24;
const CURRENT_KARAOKE_HEIGHT: i32 = 42;
const CURRENT_TRANSLATION_HEIGHT: i32 = 20;
const LYRICS_VIEWPORT_HEIGHT: i32 = CURRENT_KARAOKE_HEIGHT + CURRENT_TRANSLATION_HEIGHT;
const LYRICS_TRANSITION_DURATION_MS: u32 = 180;

pub fn run(paths: AppPaths, config: AppConfig) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("floatlyrics-worker")
        .build()
        .context("creating Tokio runtime")?;
    let _runtime_guard = runtime.enter();
    let runtime_handle = runtime.handle().clone();

    let cache = Cache::open(&paths.database_file)?;

    let app = gtk::Application::builder()
        .application_id("io.github.chouchiu.FloatLyrics")
        .build();

    let config = Rc::new(config);
    let cache = Rc::new(cache);

    app.connect_activate(move |app| {
        let floating = build_floating_window(app, &config);
        let (spotify_sender, spotify_receiver) = mpsc::channel();
        let (lyrics_sender, lyrics_receiver) = mpsc::channel();

        spawn_spotify_watcher(&runtime_handle, spotify_sender);
        attach_spotify_events(
            spotify_receiver,
            LyricsFetchHandles {
                receiver: lyrics_receiver,
                sender: lyrics_sender,
                runtime: runtime_handle.clone(),
            },
            floating,
            Rc::clone(&cache),
            Rc::clone(&config),
        );
    });

    app.run();
    Ok(())
}

#[derive(Clone)]
struct FloatingWidgets {
    song_info: gtk::Label,
    progress: gtk::ProgressBar,
    progress_label: gtk::Label,
    lyrics_stack: gtk::Stack,
    lyric_slots: [LyricSlotWidgets; 2],
    lyrics_transition: Rc<RefCell<LyricsTransitionState>>,
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
struct KaraokeRenderState {
    text: String,
    syllables: Vec<crate::lyrics::TimedSyllable>,
    position_ms: u64,
}

#[derive(Debug, Clone)]
struct TextLineRenderState {
    text: String,
    style: TextLineStyle,
}

#[derive(Debug, Clone, Copy)]
struct TextLineStyle {
    font_px: i32,
    color: (f64, f64, f64, f64),
}

#[derive(Debug, Clone, Default)]
struct LyricsTransitionState {
    current_key: Option<String>,
    active_slot: usize,
}

impl Default for TextLineRenderState {
    fn default() -> Self {
        Self {
            text: String::new(),
            style: TextLineStyle {
                font_px: 14,
                color: (1.0, 1.0, 1.0, 1.0),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct DragOrigin {
    x: i32,
    y: i32,
    geometry: FloatingGeometry,
}

#[derive(Debug, Clone, Copy, Default)]
struct FloatingGeometry {
    viewport_width: i32,
    viewport_height: i32,
    surface_width: i32,
    surface_height: i32,
}

#[derive(Clone)]
struct UiPlaybackSnapshot {
    state: SpotifyPlayerState,
    received_at: Instant,
}

#[derive(Debug, Clone, Default)]
struct LyricsDisplayState {
    track_fingerprint: Option<String>,
    lines: Vec<TimedLine>,
    status_message: Option<String>,
}

#[derive(Debug)]
struct LyricsFetchEvent {
    track_fingerprint: String,
    result: std::result::Result<FetchedLyrics, String>,
}

struct LyricsFetchHandles {
    receiver: mpsc::Receiver<LyricsFetchEvent>,
    sender: mpsc::Sender<LyricsFetchEvent>,
    runtime: tokio::runtime::Handle,
}

struct SpotifyUiContext<'a> {
    floating: &'a FloatingWidgets,
    cache: &'a Cache,
    config: &'a AppConfig,
    runtime: &'a tokio::runtime::Handle,
    lyrics_sender: &'a mpsc::Sender<LyricsFetchEvent>,
    latest: &'a Rc<RefCell<Option<UiPlaybackSnapshot>>>,
    lyrics_state: &'a Rc<RefCell<LyricsDisplayState>>,
}

fn build_floating_window(app: &gtk::Application, config: &AppConfig) -> FloatingWidgets {
    let panel_width = config.window.width.clamp(360, 720);
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
    window.set_margin(Edge::Left, initial_x(panel_width).unwrap_or_default());
    window.set_margin(Edge::Bottom, config.window.margin);
    window.set_exclusive_zone(-1);
    window.add_css_class("floating-window");

    let song_info = gtk::Label::builder()
        .label("FloatLyrics")
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(58)
        .single_line_mode(true)
        .css_classes(["floating-song-info"])
        .build();

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
        .width_chars(13)
        .single_line_mode(true)
        .css_classes(["floating-progress-label"])
        .build();

    let progress_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    progress_row.set_halign(gtk::Align::Fill);
    progress_row.append(&progress);
    progress_row.append(&progress_label);

    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    separator.add_css_class("floating-separator");

    let primary = lyric_slot(
        ["floating-slot-current"],
        ["floating-lyric-current"],
        ["floating-translation-current"],
        "Open Spotify to start tracking",
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

    let content = gtk::Box::new(gtk::Orientation::Vertical, 5);
    content.set_halign(gtk::Align::Center);
    content.set_valign(gtk::Align::Center);
    content.set_size_request(panel_width, -1);
    content.add_css_class("floating-panel");
    content.append(&song_info);
    content.append(&progress_row);
    content.append(&separator);
    content.append(&lyrics_stack);
    attach_floating_drag(&window, &content, panel_width, 96);

    let provider = gtk::CssProvider::new();
    let panel_alpha = config.window.opacity.clamp(0.18, 0.72);
    let css = r#"
        window.floating-window,
        window.floating-window > contents,
        .floating-window {
            background: transparent;
            box-shadow: none;
        }

        .floating-panel {
            padding: 9px 18px 10px 18px;
            border-radius: 8px;
            background: rgba(10, 12, 16, __PANEL_ALPHA__);
        }

        .floating-song-info {
            color: rgba(255,255,255,0.86);
            font-size: 16px;
            font-weight: 650;
            text-shadow: 0 2px 8px rgba(0,0,0,0.85);
        }

        .floating-lyric-current {
            color: white;
            font-size: 24px;
            font-weight: 750;
            text-shadow: 0 2px 8px rgba(0,0,0,0.85);
        }

        .floating-lyric-adjacent {
            color: rgba(255,255,255,0.66);
            font-size: 15px;
            text-shadow: 0 2px 8px rgba(0,0,0,0.85);
        }

        .floating-translation-current {
            color: rgba(255,255,255,0.78);
            font-size: 14px;
            font-weight: 500;
            text-shadow: none;
        }

        .floating-translation-adjacent {
            color: rgba(255,255,255,0.50);
            font-size: 12px;
            text-shadow: none;
        }

        .floating-slot-current {
            margin: 2px 0;
        }

        .floating-slot-adjacent {
            margin: 0;
        }

        .floating-progress {
            min-height: 5px;
            margin-top: 4px;
        }

        .floating-progress trough {
            min-height: 5px;
            border-radius: 3px;
            background: rgba(255,255,255,0.22);
        }

        .floating-progress progress {
            min-height: 5px;
            border-radius: 3px;
            background: rgba(255,255,255,0.82);
        }

        .floating-progress-label {
            color: rgba(255,255,255,0.74);
            font-size: 13px;
            text-shadow: 0 2px 8px rgba(0,0,0,0.85);
        }

        .floating-separator {
            margin: 5px 0 2px 0;
            background: rgba(255,255,255,0.24);
        }
        "#
    .replace("__PANEL_ALPHA__", &format!("{panel_alpha:.3}"));
    provider.load_from_string(&css);

    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    window.set_child(Some(&content));
    window.present();

    FloatingWidgets {
        song_info,
        progress,
        progress_label,
        lyrics_stack,
        lyric_slots: [primary, secondary],
        lyrics_transition,
    }
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
        .max_width_chars(56)
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
            font_px: 14,
            color: (1.0, 1.0, 1.0, 0.78),
        }
    } else {
        TextLineStyle {
            font_px: 12,
            color: (1.0, 1.0, 1.0, 0.50),
        }
    }
}

fn translation_line_height(style: TextLineStyle) -> i32 {
    if style.font_px >= 14 {
        CURRENT_TRANSLATION_HEIGHT
    } else {
        18
    }
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

fn text_line_area(
    width: i32,
    height: i32,
    style: TextLineStyle,
) -> (gtk::DrawingArea, Rc<RefCell<TextLineRenderState>>) {
    let state = Rc::new(RefCell::new(TextLineRenderState {
        text: String::new(),
        style,
    }));
    let area = gtk::DrawingArea::builder()
        .width_request(width)
        .height_request(height)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .visible(false)
        .build();
    {
        let state = Rc::clone(&state);
        area.set_draw_func(move |area, cr, width, height| {
            draw_text_line(area, cr, width, height, &state.borrow());
        });
    }

    (area, state)
}

fn lyric_text_widget(
    text: &gtk::Label,
    karaoke_size: Option<(i32, i32)>,
) -> (
    gtk::Widget,
    Option<gtk::DrawingArea>,
    Option<Rc<RefCell<KaraokeRenderState>>>,
) {
    let Some((width, height)) = karaoke_size else {
        return (text.clone().upcast(), None, None);
    };

    let state = Rc::new(RefCell::new(KaraokeRenderState::default()));
    let area = gtk::DrawingArea::builder()
        .width_request(width)
        .height_request(height)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .visible(false)
        .build();
    {
        let state = Rc::clone(&state);
        area.set_draw_func(move |area, cr, width, height| {
            draw_karaoke_line(area, cr, width, height, &state.borrow());
        });
    }

    let stack = gtk::Stack::new();
    stack.set_halign(gtk::Align::Center);
    stack.set_valign(gtk::Align::Center);
    stack.add_child(text);
    stack.add_child(&area);

    (stack.upcast(), Some(area), Some(state))
}

fn draw_text_line(
    area: &gtk::DrawingArea,
    cr: &gtk::cairo::Context,
    width: i32,
    _height: i32,
    state: &TextLineRenderState,
) {
    if state.text.trim().is_empty() {
        return;
    }

    let layout = area.create_pango_layout(Some(&state.text));
    let mut font = gtk::pango::FontDescription::from_string("Sans");
    font.set_absolute_size(state.style.font_px as f64 * gtk::pango::SCALE as f64);
    layout.set_font_description(Some(&font));
    layout.set_single_paragraph_mode(true);
    layout.set_ellipsize(gtk::pango::EllipsizeMode::End);
    layout.set_alignment(gtk::pango::Alignment::Center);
    layout.set_width(width.saturating_mul(gtk::pango::SCALE));

    draw_pango_layout(cr, &layout, 0.0, 0.0, state.style.color);
}

fn draw_karaoke_line(
    area: &gtk::DrawingArea,
    cr: &gtk::cairo::Context,
    width: i32,
    height: i32,
    state: &KaraokeRenderState,
) {
    if state.text.trim().is_empty() {
        return;
    }

    let layout = area.create_pango_layout(Some(&state.text));
    let mut font = gtk::pango::FontDescription::from_string("Sans Bold");
    font.set_absolute_size(CURRENT_LYRIC_FONT_PX as f64 * gtk::pango::SCALE as f64);
    layout.set_font_description(Some(&font));
    layout.set_single_paragraph_mode(true);

    let (text_width, text_height) = layout.pixel_size();
    let x = ((width - text_width).max(0) as f64) / 2.0;
    let y = ((height - text_height).max(0) as f64) / 2.0;
    let fill_width = karaoke_fill_width(&layout, state);

    draw_pango_layout(cr, &layout, x, y, (0.62, 0.65, 0.70, 1.0));
    if fill_width > 0.0 {
        let _ = cr.save();
        cr.rectangle(x, 0.0, fill_width, height as f64);
        cr.clip();
        draw_pango_layout(cr, &layout, x, y, (1.0, 1.0, 1.0, 1.0));
        let _ = cr.restore();
    }
}

fn draw_pango_layout(
    cr: &gtk::cairo::Context,
    layout: &gtk::pango::Layout,
    x: f64,
    y: f64,
    color: (f64, f64, f64, f64),
) {
    cr.set_source_rgba(color.0, color.1, color.2, color.3);
    cr.move_to(x, y);
    pangocairo::functions::show_layout(cr, layout);
}

fn karaoke_fill_width(layout: &gtk::pango::Layout, state: &KaraokeRenderState) -> f64 {
    for (index, syllable) in state.syllables.iter().enumerate() {
        if state.position_ms < syllable.start_ms {
            return index
                .checked_sub(1)
                .and_then(|previous| syllable_byte_range(&state.text, &state.syllables, previous))
                .map_or(0.0, |range| layout_x_at_byte(layout, range.end));
        }

        if state.position_ms < syllable.end_ms {
            let Some(range) = syllable_byte_range(&state.text, &state.syllables, index) else {
                return fallback_syllable_fill_width(layout, &state.syllables, index);
            };
            let start_x = layout_x_at_byte(layout, range.start);
            let end_x = layout_x_at_byte(layout, range.end);
            let progress = active_syllable_fraction(syllable, state.position_ms);

            return start_x + (end_x - start_x).max(0.0) * progress;
        }
    }

    layout.pixel_size().0.max(0) as f64
}

fn syllable_byte_range(
    full_text: &str,
    syllables: &[crate::lyrics::TimedSyllable],
    target_index: usize,
) -> Option<std::ops::Range<i32>> {
    let mut search_from = 0usize;

    for (index, syllable) in syllables.iter().enumerate() {
        let syllable_text = syllable.text.as_str();
        if syllable_text.is_empty() {
            if index == target_index {
                let byte = byte_index_i32(search_from.min(full_text.len()));
                return Some(byte..byte);
            }
            continue;
        }

        let start = full_text
            .get(search_from..)
            .and_then(|remaining| remaining.find(syllable_text))
            .map(|offset| search_from.saturating_add(offset));

        let Some(start) = start else {
            return fallback_syllable_byte_range(full_text, syllables, target_index);
        };
        let end = start
            .saturating_add(syllable_text.len())
            .min(full_text.len());

        if index == target_index {
            return Some(byte_index_i32(start)..byte_index_i32(end));
        }

        search_from = end;
    }

    None
}

fn fallback_syllable_byte_range(
    full_text: &str,
    syllables: &[crate::lyrics::TimedSyllable],
    target_index: usize,
) -> Option<std::ops::Range<i32>> {
    let mut byte_index = 0usize;

    for (index, syllable) in syllables.iter().enumerate() {
        let start = byte_index.min(full_text.len());
        byte_index = byte_index.saturating_add(syllable.text.len());
        let end = byte_index.min(full_text.len());

        if index == target_index {
            return Some(byte_index_i32(start)..byte_index_i32(end));
        }
    }

    None
}

fn fallback_syllable_fill_width(
    layout: &gtk::pango::Layout,
    syllables: &[crate::lyrics::TimedSyllable],
    target_index: usize,
) -> f64 {
    let byte_index = syllables
        .iter()
        .take(target_index + 1)
        .map(|syllable| syllable.text.len())
        .sum::<usize>();

    layout_x_at_byte(layout, byte_index_i32(byte_index))
}

fn byte_index_i32(byte_index: usize) -> i32 {
    byte_index.try_into().unwrap_or(i32::MAX)
}

fn layout_x_at_byte(layout: &gtk::pango::Layout, byte_index: i32) -> f64 {
    layout.index_to_pos(byte_index).x() as f64 / gtk::pango::SCALE as f64
}

fn attach_floating_drag(
    window: &gtk::ApplicationWindow,
    content: &gtk::Box,
    fallback_width: i32,
    fallback_height: i32,
) {
    let drag_origin = Rc::new(RefCell::new(DragOrigin::default()));
    let gesture = gtk::GestureDrag::new();

    {
        let window = window.clone();
        let drag_origin = Rc::clone(&drag_origin);
        gesture.connect_drag_begin(move |_, _, _| {
            let geometry = floating_geometry(&window, fallback_width, fallback_height)
                .unwrap_or_else(|| fallback_geometry(fallback_width, fallback_height));
            let bottom_margin = window.margin(Edge::Bottom);
            *drag_origin.borrow_mut() = DragOrigin {
                x: window.margin(Edge::Left),
                y: y_from_bottom_margin(bottom_margin, geometry),
                geometry,
            };
        });
    }

    {
        let window = window.clone();
        let drag_origin = Rc::clone(&drag_origin);
        gesture.connect_drag_update(move |_, offset_x, offset_y| {
            let origin = *drag_origin.borrow();
            let (next_left, next_bottom) = dragged_margins(origin, offset_x, offset_y);

            window.set_margin(Edge::Left, next_left);
            window.set_margin(Edge::Bottom, next_bottom);
        });
    }

    content.add_controller(gesture);
}

fn initial_x(window_width: i32) -> Option<i32> {
    let monitor = first_monitor()?;
    let monitor_width = monitor.geometry().width();

    Some(((monitor_width - window_width) / 2).max(0))
}

fn dragged_margins(origin: DragOrigin, offset_x: f64, offset_y: f64) -> (i32, i32) {
    let max_x = (origin.geometry.viewport_width - origin.geometry.surface_width).max(0);
    let max_y = (origin.geometry.viewport_height - origin.geometry.surface_height).max(0);
    let next_x = origin
        .x
        .saturating_add(offset_x.round() as i32)
        .clamp(0, max_x);
    let next_y = origin
        .y
        .saturating_add(offset_y.round() as i32)
        .clamp(0, max_y);

    (next_x, bottom_margin_from_y(next_y, origin.geometry))
}

fn floating_geometry(
    window: &gtk::ApplicationWindow,
    fallback_width: i32,
    fallback_height: i32,
) -> Option<FloatingGeometry> {
    let monitor = window_monitor(window).or_else(first_monitor)?;
    let geometry = monitor.geometry();
    let surface_width = effective_surface_size(window.width(), fallback_width);
    let surface_height = effective_surface_size(window.height(), fallback_height);

    Some(FloatingGeometry {
        viewport_width: geometry.width().max(0),
        viewport_height: geometry.height().max(0),
        surface_width,
        surface_height,
    })
}

fn fallback_geometry(fallback_width: i32, fallback_height: i32) -> FloatingGeometry {
    FloatingGeometry {
        viewport_width: fallback_width.max(0),
        viewport_height: fallback_height.max(0),
        surface_width: fallback_width.max(0),
        surface_height: fallback_height.max(0),
    }
}

fn effective_surface_size(actual: i32, fallback: i32) -> i32 {
    if actual > 0 { actual } else { fallback.max(0) }
}

fn y_from_bottom_margin(bottom_margin: i32, geometry: FloatingGeometry) -> i32 {
    (geometry.viewport_height - geometry.surface_height - bottom_margin).clamp(
        0,
        (geometry.viewport_height - geometry.surface_height).max(0),
    )
}

fn bottom_margin_from_y(y: i32, geometry: FloatingGeometry) -> i32 {
    (geometry.viewport_height - geometry.surface_height - y).clamp(
        0,
        (geometry.viewport_height - geometry.surface_height).max(0),
    )
}

fn window_monitor(window: &gtk::ApplicationWindow) -> Option<gtk::gdk::Monitor> {
    let display = gtk::gdk::Display::default()?;
    let surface = window.surface()?;
    display.monitor_at_surface(&surface)
}

fn first_monitor() -> Option<gtk::gdk::Monitor> {
    gtk::gdk::Display::default()?
        .monitors()
        .item(0)?
        .downcast::<gtk::gdk::Monitor>()
        .ok()
}

fn attach_spotify_events(
    receiver: mpsc::Receiver<SpotifyWatcherEvent>,
    lyrics: LyricsFetchHandles,
    floating: FloatingWidgets,
    cache: Rc<Cache>,
    config: Rc<AppConfig>,
) {
    let receiver = Rc::new(RefCell::new(receiver));
    let lyrics_receiver = Rc::new(RefCell::new(lyrics.receiver));
    let lyrics_sender = lyrics.sender;
    let runtime = lyrics.runtime;
    let latest = Rc::new(RefCell::new(None::<UiPlaybackSnapshot>));
    let lyrics_state = Rc::new(RefCell::new(LyricsDisplayState::default()));

    let tick_widget = floating.lyrics_stack.clone();
    tick_widget.add_tick_callback(move |_, _| {
        let ctx = SpotifyUiContext {
            floating: &floating,
            cache: &cache,
            config: &config,
            runtime: &runtime,
            lyrics_sender: &lyrics_sender,
            latest: &latest,
            lyrics_state: &lyrics_state,
        };

        for event in receiver.borrow().try_iter() {
            handle_spotify_event(&event, &ctx);
        }

        for event in lyrics_receiver.borrow().try_iter() {
            handle_lyrics_fetch_event(
                event,
                ctx.floating,
                ctx.cache,
                ctx.config,
                ctx.latest,
                ctx.lyrics_state,
            );
        }

        if let Some(snapshot) = ctx.latest.borrow().as_ref() {
            refresh_progress_from_clock(snapshot, ctx.floating, ctx.config, ctx.lyrics_state);
        }

        gtk::glib::ControlFlow::Continue
    });
}

fn handle_spotify_event(event: &SpotifyWatcherEvent, ctx: &SpotifyUiContext<'_>) {
    match event {
        SpotifyWatcherEvent::Connected(state) | SpotifyWatcherEvent::Updated(state) => {
            *ctx.latest.borrow_mut() = Some(UiPlaybackSnapshot {
                state: state.clone(),
                received_at: Instant::now(),
            });
            update_spotify_state(state, ctx);
        }
        SpotifyWatcherEvent::PositionUpdated {
            track_fingerprint,
            position_ms,
            sampled_at,
        } => {
            if let Some(snapshot) = ctx.latest.borrow_mut().as_mut() {
                apply_position_sample(
                    snapshot,
                    track_fingerprint.as_deref(),
                    *position_ms,
                    *sampled_at,
                );
            }
        }
        SpotifyWatcherEvent::Disconnected => {
            *ctx.latest.borrow_mut() = None;
            *ctx.lyrics_state.borrow_mut() = LyricsDisplayState::default();
            ctx.floating.song_info.set_label("FloatLyrics");
            set_status_lyrics(ctx.floating, "Open Spotify to start tracking");
            reset_progress(ctx.floating);
        }
        SpotifyWatcherEvent::Error(message) => {
            *ctx.latest.borrow_mut() = None;
            *ctx.lyrics_state.borrow_mut() = LyricsDisplayState::default();
            tracing::warn!(%message, "Spotify listener error");
            ctx.floating.song_info.set_label("FloatLyrics");
            set_status_lyrics(ctx.floating, "Spotify listener needs attention");
            reset_progress(ctx.floating);
        }
    }
}

fn update_spotify_state(state: &SpotifyPlayerState, ctx: &SpotifyUiContext<'_>) {
    if let Some(track) = &state.track {
        if let Err(error) = ctx.cache.upsert_track(track) {
            tracing::warn!(%error, "failed to cache Spotify track");
        }
        ensure_lyrics_loaded(
            track,
            ctx.cache,
            ctx.config,
            ctx.runtime,
            ctx.lyrics_sender,
            ctx.lyrics_state,
        );
        update_track_display(
            state,
            ctx.floating,
            ctx.config,
            ctx.lyrics_state,
            state.position_ms,
        );
    } else {
        ctx.floating.song_info.set_label("FloatLyrics");
        set_status_lyrics(ctx.floating, "Waiting for Spotify metadata");
        reset_progress(ctx.floating);
    }
}

fn refresh_progress_from_clock(
    snapshot: &UiPlaybackSnapshot,
    floating: &FloatingWidgets,
    config: &AppConfig,
    lyrics_state: &Rc<RefCell<LyricsDisplayState>>,
) {
    if snapshot.state.track.is_some() {
        update_track_display(
            &snapshot.state,
            floating,
            config,
            lyrics_state,
            effective_position_ms(snapshot),
        );
    }
}

fn update_track_display(
    state: &SpotifyPlayerState,
    floating: &FloatingWidgets,
    config: &AppConfig,
    lyrics_state: &Rc<RefCell<LyricsDisplayState>>,
    position_ms: Option<u64>,
) {
    let Some(track) = &state.track else {
        return;
    };

    let artist = track.display_artist();
    floating
        .song_info
        .set_label(&format!("{} - {}", track.title, artist));
    update_progress(floating, position_ms, track.duration_ms);
    update_lyrics_display(floating, &lyrics_state.borrow(), config, position_ms);
}

fn ensure_lyrics_loaded(
    track: &TrackMetadata,
    cache: &Cache,
    config: &AppConfig,
    runtime: &tokio::runtime::Handle,
    lyrics_sender: &mpsc::Sender<LyricsFetchEvent>,
    lyrics_state: &Rc<RefCell<LyricsDisplayState>>,
) {
    let fingerprint = track.fingerprint();
    if lyrics_state.borrow().track_fingerprint.as_deref() == Some(fingerprint.as_str()) {
        return;
    }

    *lyrics_state.borrow_mut() =
        load_lyrics_for_track(track, cache, config, runtime, lyrics_sender, fingerprint);
}

fn load_lyrics_for_track(
    track: &TrackMetadata,
    cache: &Cache,
    config: &AppConfig,
    runtime: &tokio::runtime::Handle,
    lyrics_sender: &mpsc::Sender<LyricsFetchEvent>,
    fingerprint: String,
) -> LyricsDisplayState {
    let provider_order = active_provider_order(config);
    let cached = match cache.lyrics_for_track(&fingerprint, &provider_order) {
        Ok(cached) => cached,
        Err(error) => {
            return LyricsDisplayState {
                track_fingerprint: Some(fingerprint),
                status_message: Some(format!("Lyrics cache error: {error}")),
                ..LyricsDisplayState::default()
            };
        }
    };

    let Some(cached) = cached else {
        spawn_lyrics_fetch(
            runtime,
            lyrics_sender.clone(),
            track.clone(),
            provider_order,
            fingerprint.clone(),
        );
        return LyricsDisplayState {
            track_fingerprint: Some(fingerprint),
            status_message: Some("Searching lyrics...".to_string()),
            ..LyricsDisplayState::default()
        };
    };

    let state = lyrics_state_from_cached(fingerprint.clone(), cached);
    if config.lyrics.show_translation && !has_cached_translation(&state) {
        spawn_lyrics_fetch(
            runtime,
            lyrics_sender.clone(),
            track.clone(),
            provider_order,
            fingerprint,
        );
    }
    state
}

fn lyrics_state_from_cached(fingerprint: String, cached: CachedLyrics) -> LyricsDisplayState {
    let lines = match timed_lines_from_raw(&cached.raw_lyrics) {
        Ok(lines) => lines,
        Err(error) => {
            return LyricsDisplayState {
                track_fingerprint: Some(fingerprint),
                status_message: Some(format!("Lyrics parse error: {error}")),
                ..LyricsDisplayState::default()
            };
        }
    };

    let status_message = if lines.is_empty() {
        Some("Cached lyrics are not time-synced".to_string())
    } else {
        None
    };

    LyricsDisplayState {
        track_fingerprint: Some(fingerprint),
        lines,
        status_message,
    }
}

fn handle_lyrics_fetch_event(
    event: LyricsFetchEvent,
    floating: &FloatingWidgets,
    cache: &Cache,
    config: &AppConfig,
    latest: &Rc<RefCell<Option<UiPlaybackSnapshot>>>,
    lyrics_state: &Rc<RefCell<LyricsDisplayState>>,
) {
    let Some(snapshot) = latest.borrow().as_ref().cloned() else {
        return;
    };
    let Some(track) = snapshot.state.track.as_ref() else {
        return;
    };
    if track.fingerprint() != event.track_fingerprint {
        return;
    }

    match event.result {
        Ok(fetched) => {
            if let Err(error) = cache.insert_provider_result(ProviderResultInsert {
                track_fingerprint: &event.track_fingerprint,
                provider: fetched.provider,
                provider_track_id: fetched.provider_track_id.as_deref(),
                title: &fetched.title,
                artists: &fetched.artists,
                score: fetched.score,
                raw_lyrics: Some(&fetched.raw_lyrics),
            }) {
                *lyrics_state.borrow_mut() = LyricsDisplayState {
                    track_fingerprint: Some(event.track_fingerprint),
                    status_message: Some(format!("Lyrics cache write error: {error}")),
                    ..LyricsDisplayState::default()
                };
            } else {
                *lyrics_state.borrow_mut() =
                    load_cached_lyrics_after_fetch(cache, config, event.track_fingerprint);
            }
        }
        Err(message) => {
            *lyrics_state.borrow_mut() = LyricsDisplayState {
                track_fingerprint: Some(event.track_fingerprint),
                status_message: Some(format!("Lyrics search failed: {message}")),
                ..LyricsDisplayState::default()
            };
        }
    }

    update_track_display(
        &snapshot.state,
        floating,
        config,
        lyrics_state,
        effective_position_ms(&snapshot),
    );
}

fn load_cached_lyrics_after_fetch(
    cache: &Cache,
    config: &AppConfig,
    fingerprint: String,
) -> LyricsDisplayState {
    let provider_order = active_provider_order(config);
    match cache.lyrics_for_track(&fingerprint, &provider_order) {
        Ok(Some(cached)) => lyrics_state_from_cached(fingerprint, cached),
        Ok(None) => LyricsDisplayState {
            track_fingerprint: Some(fingerprint),
            status_message: Some("Downloaded lyrics were not stored".to_string()),
            ..LyricsDisplayState::default()
        },
        Err(error) => LyricsDisplayState {
            track_fingerprint: Some(fingerprint),
            status_message: Some(format!("Lyrics cache error: {error}")),
            ..LyricsDisplayState::default()
        },
    }
}

fn spawn_lyrics_fetch(
    runtime: &tokio::runtime::Handle,
    sender: mpsc::Sender<LyricsFetchEvent>,
    track: TrackMetadata,
    provider_order: Vec<crate::lyrics::LyricsProvider>,
    track_fingerprint: String,
) {
    runtime.spawn(async move {
        let result = match search_best_lyrics(&track, &provider_order).await {
            Ok(Some(fetched)) => Ok(fetched),
            Ok(None) => Err("No lyrics found from configured providers".to_string()),
            Err(error) => Err(error.to_string()),
        };

        let _ = sender.send(LyricsFetchEvent {
            track_fingerprint,
            result,
        });
    });
}

fn has_cached_translation(state: &LyricsDisplayState) -> bool {
    state.lines.iter().any(|line| {
        line.translation
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    })
}

fn active_provider_order(config: &AppConfig) -> Vec<crate::lyrics::LyricsProvider> {
    SearchPlan::new(config.lyrics.provider_order.clone())
        .providers()
        .to_vec()
}

fn update_lyrics_display(
    floating: &FloatingWidgets,
    lyrics_state: &LyricsDisplayState,
    config: &AppConfig,
    position_ms: Option<u64>,
) {
    if let Some(message) = &lyrics_state.status_message {
        set_status_lyrics(floating, message);
        return;
    }

    if lyrics_state.lines.is_empty() {
        set_status_lyrics(floating, "Waiting for lyrics");
        return;
    }

    let Some(position_ms) = position_ms else {
        set_status_lyrics(floating, "Waiting for playback position");
        return;
    };

    let Some(index) = active_line_index(&lyrics_state.lines, position_ms, config.lyrics.offset_ms)
    else {
        if let Some(index) =
            line_index_at_or_before(&lyrics_state.lines, position_ms, config.lyrics.offset_ms)
        {
            set_surrounding_lyrics_labels(
                floating,
                &lyrics_state.lines,
                index,
                config,
                position_ms,
            );
        } else {
            set_lyrics_slots(floating, LyricSlotText::message("…"), "before-first-line");
        }
        return;
    };

    set_surrounding_lyrics_labels(floating, &lyrics_state.lines, index, config, position_ms);
}

fn set_surrounding_lyrics_labels(
    floating: &FloatingWidgets,
    lines: &[TimedLine],
    index: usize,
    config: &AppConfig,
    position_ms: u64,
) {
    let current = current_line_text(lines.get(index), config, position_ms);
    set_lyrics_slots(floating, current, &format!("line:{index}"));
}

#[derive(Debug, Clone, Default)]
struct LyricSlotText {
    text: String,
    karaoke: Option<KaraokeRenderState>,
    translation: String,
}

impl LyricSlotText {
    fn empty() -> Self {
        Self::default()
    }

    fn message(message: &str) -> Self {
        Self {
            text: message.to_string(),
            karaoke: None,
            translation: String::new(),
        }
    }
}

fn line_text(line: Option<&TimedLine>, config: &AppConfig) -> LyricSlotText {
    let Some(line) = line else {
        return LyricSlotText::empty();
    };

    let mut text = line.text.trim().to_string();
    if config.lyrics.show_translation {
        if let Some(translation) = line.translation.as_deref().map(str::trim) {
            if !translation.is_empty() && !is_placeholder_text(translation) {
                return LyricSlotText {
                    text,
                    karaoke: None,
                    translation: translation.to_string(),
                };
            }
        }
    }
    if config.lyrics.show_romanization {
        if let Some(romanization) = line.romanization.as_deref().map(str::trim) {
            if !romanization.is_empty() {
                text = format!("{text}  /  {romanization}");
            }
        }
    }

    LyricSlotText {
        text,
        karaoke: None,
        translation: String::new(),
    }
}

fn current_line_text(
    line: Option<&TimedLine>,
    config: &AppConfig,
    position_ms: u64,
) -> LyricSlotText {
    let mut value = line_text(line, config);
    let Some(line) = line else {
        return value;
    };
    if !line.syllables.is_empty() {
        let adjusted_position = adjusted_position_ms(position_ms, config.lyrics.offset_ms);
        value.karaoke = Some(KaraokeRenderState {
            text: line.text.clone(),
            syllables: line.syllables.clone(),
            position_ms: adjusted_position,
        });
    }
    value
}

fn adjusted_position_ms(position_ms: u64, offset_ms: i64) -> u64 {
    (position_ms as i128 + offset_ms as i128).max(0) as u64
}

fn active_syllable_fraction(syllable: &crate::lyrics::TimedSyllable, position_ms: u64) -> f64 {
    let duration = syllable.end_ms.saturating_sub(syllable.start_ms);
    if duration == 0 {
        return 1.0;
    }

    let elapsed = position_ms.saturating_sub(syllable.start_ms).min(duration);
    elapsed as f64 / duration as f64
}

fn is_placeholder_text(value: &str) -> bool {
    let normalized = value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();

    matches!(normalized.as_str(), "//" | "/" | "／" | "／／")
}

fn set_lyrics_slots(floating: &FloatingWidgets, current: LyricSlotText, animation_key: &str) {
    let (slot_index, should_transition) =
        select_lyric_slot(&mut floating.lyrics_transition.borrow_mut(), animation_key);
    let slot = &floating.lyric_slots[slot_index];
    set_lyric_slot(slot, &current);

    if should_transition {
        floating.lyrics_stack.set_visible_child(&slot.container);
    }
}

fn set_status_lyrics(floating: &FloatingWidgets, message: &str) {
    set_lyrics_slots(
        floating,
        LyricSlotText::message(message),
        &format!("status:{message}"),
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

fn effective_position_ms(snapshot: &UiPlaybackSnapshot) -> Option<u64> {
    let base = snapshot.state.position_ms?;
    let position = match snapshot.state.playback_status {
        PlaybackStatus::Playing => {
            base.saturating_add(snapshot.received_at.elapsed().as_millis() as u64)
        }
        _ => base,
    };

    Some(
        snapshot
            .state
            .track
            .as_ref()
            .and_then(|track| track.duration_ms)
            .map_or(position, |duration| position.min(duration)),
    )
}

fn apply_position_sample(
    snapshot: &mut UiPlaybackSnapshot,
    track_fingerprint: Option<&str>,
    position_ms: u64,
    sampled_at: Instant,
) -> bool {
    let current_fingerprint = snapshot
        .state
        .track
        .as_ref()
        .map(TrackMetadata::fingerprint);
    if current_fingerprint.as_deref() != track_fingerprint {
        return false;
    }

    snapshot.state.position_ms = Some(position_ms);
    snapshot.received_at = sampled_at;
    true
}

fn update_progress(floating: &FloatingWidgets, position_ms: Option<u64>, duration_ms: Option<u64>) {
    floating
        .progress
        .set_fraction(progress_fraction(position_ms, duration_ms).unwrap_or(0.0));
    floating.progress_label.set_label(
        progress_text(position_ms, duration_ms)
            .as_deref()
            .unwrap_or(""),
    );
}

fn reset_progress(floating: &FloatingWidgets) {
    floating.progress.set_fraction(0.0);
    floating.progress_label.set_label("");
}

fn progress_fraction(position_ms: Option<u64>, duration_ms: Option<u64>) -> Option<f64> {
    let position_ms = position_ms?;
    let duration_ms = duration_ms?;
    if duration_ms == 0 {
        return None;
    }

    Some((position_ms as f64 / duration_ms as f64).clamp(0.0, 1.0))
}

fn progress_text(position_ms: Option<u64>, duration_ms: Option<u64>) -> Option<String> {
    let position = position_ms?;
    Some(match duration_ms {
        Some(duration) if duration > 0 => {
            format!(
                "{} / {}",
                format_duration(position),
                format_duration(duration)
            )
        }
        _ => format_duration(position),
    })
}

fn format_duration(ms: u64) -> String {
    let total_seconds = ms / 1_000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes}:{seconds:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::TrackMetadata;
    use std::time::Duration;

    #[test]
    fn formats_progress_text() {
        assert_eq!(
            progress_text(Some(65_000), Some(185_000)).as_deref(),
            Some("1:05 / 3:05")
        );
        assert_eq!(progress_text(Some(5_000), None).as_deref(), Some("0:05"));
        assert_eq!(progress_text(None, Some(10_000)), None);
    }

    #[test]
    fn clamps_progress_fraction() {
        assert_eq!(progress_fraction(Some(50), Some(100)), Some(0.5));
        assert_eq!(progress_fraction(Some(150), Some(100)), Some(1.0));
        assert_eq!(progress_fraction(Some(50), Some(0)), None);
    }

    #[test]
    fn drag_offsets_update_layer_margins() {
        let geometry = FloatingGeometry {
            viewport_width: 800,
            viewport_height: 600,
            surface_width: 300,
            surface_height: 100,
        };
        let origin = DragOrigin {
            x: 100,
            y: 420,
            geometry,
        };

        assert_eq!(dragged_margins(origin, 25.4, -10.2), (125, 90));
        assert_eq!(dragged_margins(origin, -150.0, -500.0), (0, 500));
        assert_eq!(dragged_margins(origin, 500.0, 500.0), (500, 0));
    }

    #[test]
    fn converts_between_top_y_and_bottom_margin() {
        let geometry = FloatingGeometry {
            viewport_width: 800,
            viewport_height: 600,
            surface_width: 300,
            surface_height: 100,
        };

        assert_eq!(y_from_bottom_margin(0, geometry), 500);
        assert_eq!(y_from_bottom_margin(500, geometry), 0);
        assert_eq!(bottom_margin_from_y(0, geometry), 500);
        assert_eq!(bottom_margin_from_y(500, geometry), 0);
    }

    #[test]
    fn advances_progress_with_local_clock_only_while_playing() {
        let playing = UiPlaybackSnapshot {
            state: test_state(PlaybackStatus::Playing),
            received_at: Instant::now() - Duration::from_millis(1_500),
        };
        let paused = UiPlaybackSnapshot {
            state: test_state(PlaybackStatus::Paused),
            received_at: Instant::now() - Duration::from_millis(1_500),
        };

        assert!(effective_position_ms(&playing).unwrap() >= 11_000);
        assert_eq!(effective_position_ms(&paused), Some(10_000));
    }

    #[test]
    fn authoritative_position_sample_reanchors_the_local_clock() {
        let mut snapshot = UiPlaybackSnapshot {
            state: test_state(PlaybackStatus::Playing),
            received_at: Instant::now() - Duration::from_secs(2),
        };
        assert!(effective_position_ms(&snapshot).unwrap() >= 12_000);

        let fingerprint = snapshot.state.track.as_ref().unwrap().fingerprint();
        let sampled_at = Instant::now();
        assert!(apply_position_sample(
            &mut snapshot,
            Some(&fingerprint),
            10_500,
            sampled_at,
        ));
        assert!(effective_position_ms(&snapshot).unwrap() < 10_600);
        assert_eq!(snapshot.received_at, sampled_at);
    }

    #[test]
    fn position_sample_from_another_track_is_ignored() {
        let mut snapshot = UiPlaybackSnapshot {
            state: test_state(PlaybackStatus::Playing),
            received_at: Instant::now(),
        };
        let received_at = snapshot.received_at;

        assert!(!apply_position_sample(
            &mut snapshot,
            Some("another-track"),
            500,
            Instant::now(),
        ));
        assert_eq!(snapshot.state.position_ms, Some(10_000));
        assert_eq!(snapshot.received_at, received_at);
    }

    #[test]
    fn active_syllable_fraction_tracks_progress_inside_syllable() {
        let syllable = crate::lyrics::TimedSyllable {
            start_ms: 1_000,
            end_ms: 1_500,
            text: "hello".to_string(),
        };

        assert_eq!(active_syllable_fraction(&syllable, 900), 0.0);
        assert_eq!(active_syllable_fraction(&syllable, 1_250), 0.5);
        assert_eq!(active_syllable_fraction(&syllable, 1_700), 1.0);
    }

    #[test]
    fn lyric_slot_selection_only_switches_when_the_key_changes() {
        let mut state = LyricsTransitionState::default();

        assert_eq!(select_lyric_slot(&mut state, "line:0"), (0, false));
        assert_eq!(select_lyric_slot(&mut state, "line:0"), (0, false));
        assert_eq!(select_lyric_slot(&mut state, "line:1"), (1, true));
        assert_eq!(select_lyric_slot(&mut state, "line:2"), (0, true));
    }

    #[test]
    fn syllable_byte_range_tracks_repeated_words_in_order() {
        let syllables = vec![
            test_syllable(0, 100, "Please"),
            test_syllable(100, 200, " "),
            test_syllable(200, 300, "Please"),
        ];

        assert_eq!(
            syllable_byte_range("Please Please", &syllables, 0),
            Some(0..6)
        );
        assert_eq!(
            syllable_byte_range("Please Please", &syllables, 1),
            Some(6..7)
        );
        assert_eq!(
            syllable_byte_range("Please Please", &syllables, 2),
            Some(7..13)
        );
    }

    #[test]
    fn syllable_byte_range_handles_multibyte_text() {
        let syllables = vec![test_syllable(0, 100, "你"), test_syllable(100, 200, "好")];

        assert_eq!(syllable_byte_range("你好", &syllables, 0), Some(0..3));
        assert_eq!(syllable_byte_range("你好", &syllables, 1), Some(3..6));
    }

    #[test]
    fn line_text_hides_placeholder_translation() {
        let mut line = TimedLine {
            start_ms: 1_000,
            end_ms: Some(2_000),
            text: "Hello".to_string(),
            syllables: Vec::new(),
            translation: Some("//".to_string()),
            romanization: None,
            background: None,
        };

        let text = line_text(Some(&line), &AppConfig::default());
        assert_eq!(text.text, "Hello");
        assert!(text.translation.is_empty());

        line.translation = Some("你好".to_string());
        let text = line_text(Some(&line), &AppConfig::default());
        assert_eq!(text.translation, "你好");
    }

    fn test_syllable(start_ms: u64, end_ms: u64, text: &str) -> crate::lyrics::TimedSyllable {
        crate::lyrics::TimedSyllable {
            start_ms,
            end_ms,
            text: text.to_string(),
        }
    }

    fn test_state(playback_status: PlaybackStatus) -> SpotifyPlayerState {
        SpotifyPlayerState {
            bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
            playback_status,
            position_ms: Some(10_000),
            track: Some(TrackMetadata {
                title: "Song".to_string(),
                artists: vec!["Artist".to_string()],
                album: None,
                duration_ms: Some(20_000),
                mpris_track_id: None,
            }),
        }
    }
}
