use adw::prelude::*;
use anyhow::{Context, Result};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::mpsc,
    time::{Duration, Instant},
};

use crate::{
    cache::{Cache, CachedLyrics, ProviderResultInsert},
    config::AppConfig,
    lyrics::{
        FetchedLyrics, SearchPlan, TimedLine, active_line_index, search_best_lyrics,
        timed_lines_from_raw,
    },
    mpris::{PlaybackStatus, SpotifyPlayerState, SpotifyWatcherEvent, spawn_spotify_watcher},
    paths::AppPaths,
    track::TrackMetadata,
};

pub fn run(paths: AppPaths, config: AppConfig) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("floatlyrics-worker")
        .build()
        .context("creating Tokio runtime")?;
    let _runtime_guard = runtime.enter();
    let runtime_handle = runtime.handle().clone();

    let cache = Cache::open(&paths.database_file)?;

    let app = adw::Application::builder()
        .application_id("io.github.chouchiu.FloatLyrics")
        .build();

    let config = Rc::new(config);
    let paths = Rc::new(paths);
    let cache = Rc::new(cache);

    app.connect_activate(move |app| {
        let settings = build_settings_window(app, &paths);
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
            settings,
            floating,
            Rc::clone(&cache),
            Rc::clone(&config),
        );
    });

    app.run();
    Ok(())
}

#[derive(Clone)]
struct SettingsWidgets {
    player_row: adw::ActionRow,
}

#[derive(Clone)]
struct FloatingWidgets {
    song_info: gtk::Label,
    progress: gtk::ProgressBar,
    progress_label: gtk::Label,
    previous_line: gtk::Label,
    current_line: gtk::Label,
    next_line: gtk::Label,
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
    source_label: Option<String>,
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
    settings: &'a SettingsWidgets,
    floating: &'a FloatingWidgets,
    cache: &'a Cache,
    config: &'a AppConfig,
    runtime: &'a tokio::runtime::Handle,
    lyrics_sender: &'a mpsc::Sender<LyricsFetchEvent>,
    latest: &'a Rc<RefCell<Option<UiPlaybackSnapshot>>>,
    lyrics_state: &'a Rc<RefCell<LyricsDisplayState>>,
}

fn build_settings_window(app: &adw::Application, paths: &AppPaths) -> SettingsWidgets {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("FloatLyrics")
        .default_width(560)
        .default_height(420)
        .build();

    let header = adw::HeaderBar::new();
    let toolbar = gtk::Box::new(gtk::Orientation::Vertical, 0);
    toolbar.append(&header);

    let status_group = adw::PreferencesGroup::builder().title("Spotify").build();
    let player_row = adw::ActionRow::builder()
        .title("Player")
        .subtitle("Waiting for Spotify MPRIS")
        .build();
    status_group.add(&player_row);

    let cache_group = adw::PreferencesGroup::builder().title("Cache").build();
    let db_row = adw::ActionRow::builder()
        .title("Database")
        .subtitle(paths.database_file.display().to_string())
        .build();
    cache_group.add(&db_row);

    let page = adw::PreferencesPage::new();
    page.add(&status_group);
    page.add(&cache_group);
    toolbar.append(&page);

    window.set_content(Some(&toolbar));
    window.present();

    SettingsWidgets { player_row }
}

fn build_floating_window(app: &adw::Application, config: &AppConfig) -> FloatingWidgets {
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

    let previous_line = gtk::Label::builder()
        .label("")
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(56)
        .single_line_mode(true)
        .css_classes(["floating-lyric-adjacent"])
        .build();

    let current_line = gtk::Label::builder()
        .label("Open Spotify to start tracking")
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(50)
        .single_line_mode(true)
        .css_classes(["floating-lyric-current"])
        .build();

    let next_line = gtk::Label::builder()
        .label("")
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(56)
        .single_line_mode(true)
        .css_classes(["floating-lyric-adjacent"])
        .build();

    let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
    content.set_halign(gtk::Align::Center);
    content.set_valign(gtk::Align::Center);
    content.set_size_request(panel_width, -1);
    content.add_css_class("floating-panel");
    content.append(&song_info);
    content.append(&progress_row);
    content.append(&separator);
    content.append(&previous_line);
    content.append(&current_line);
    content.append(&next_line);
    attach_floating_drag(&window, &content, panel_width, 150);

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
            padding: 10px 18px 8px 18px;
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
            font-size: 26px;
            font-weight: 750;
            text-shadow: 0 2px 8px rgba(0,0,0,0.85);
            min-height: 34px;
        }

        .floating-lyric-adjacent {
            color: rgba(255,255,255,0.66);
            font-size: 17px;
            text-shadow: 0 2px 8px rgba(0,0,0,0.85);
            min-height: 23px;
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
        previous_line,
        current_line,
        next_line,
    }
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
    settings: SettingsWidgets,
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

    gtk::glib::timeout_add_local(Duration::from_millis(250), move || {
        let ctx = SpotifyUiContext {
            settings: &settings,
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
                ctx.settings,
                ctx.floating,
                ctx.cache,
                ctx.config,
                ctx.latest,
                ctx.lyrics_state,
            );
        }

        if let Some(snapshot) = ctx.latest.borrow().as_ref() {
            refresh_progress_from_clock(
                snapshot,
                ctx.settings,
                ctx.floating,
                ctx.config,
                ctx.lyrics_state,
            );
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
        SpotifyWatcherEvent::Disconnected => {
            *ctx.latest.borrow_mut() = None;
            *ctx.lyrics_state.borrow_mut() = LyricsDisplayState::default();
            ctx.settings
                .player_row
                .set_subtitle("Waiting for Spotify MPRIS");
            ctx.floating.song_info.set_label("FloatLyrics");
            set_lyrics_labels(ctx.floating, "", "Open Spotify to start tracking", "");
            reset_progress(ctx.floating);
        }
        SpotifyWatcherEvent::Error(message) => {
            *ctx.latest.borrow_mut() = None;
            *ctx.lyrics_state.borrow_mut() = LyricsDisplayState::default();
            ctx.settings
                .player_row
                .set_subtitle(&format!("Listener error: {message}"));
            ctx.floating.song_info.set_label("FloatLyrics");
            set_lyrics_labels(ctx.floating, "", "Spotify listener needs attention", "");
            reset_progress(ctx.floating);
        }
    }
}

fn update_spotify_state(state: &SpotifyPlayerState, ctx: &SpotifyUiContext<'_>) {
    let status = playback_status_label(&state.playback_status);

    if let Some(track) = &state.track {
        if let Err(error) = ctx.cache.upsert_track(track) {
            ctx.settings
                .player_row
                .set_subtitle(&format!("Spotify tracked, cache error: {error}"));
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
            ctx.settings,
            ctx.floating,
            ctx.config,
            ctx.lyrics_state,
            state.position_ms,
        );
    } else {
        ctx.settings
            .player_row
            .set_subtitle(&format!("{status} via {}", state.bus_name));
        ctx.floating.song_info.set_label("FloatLyrics");
        set_lyrics_labels(ctx.floating, "", "Waiting for Spotify metadata", "");
        reset_progress(ctx.floating);
    }
}

fn refresh_progress_from_clock(
    snapshot: &UiPlaybackSnapshot,
    settings: &SettingsWidgets,
    floating: &FloatingWidgets,
    config: &AppConfig,
    lyrics_state: &Rc<RefCell<LyricsDisplayState>>,
) {
    if snapshot.state.track.is_some() {
        update_track_display(
            &snapshot.state,
            settings,
            floating,
            config,
            lyrics_state,
            effective_position_ms(snapshot),
        );
    }
}

fn update_track_display(
    state: &SpotifyPlayerState,
    settings: &SettingsWidgets,
    floating: &FloatingWidgets,
    config: &AppConfig,
    lyrics_state: &Rc<RefCell<LyricsDisplayState>>,
    position_ms: Option<u64>,
) {
    let Some(track) = &state.track else {
        return;
    };

    let status = playback_status_label(&state.playback_status);
    let artist = track.display_artist();
    let progress_suffix = progress_text(position_ms, track.duration_ms)
        .map(|text| format!(" ({text})"))
        .unwrap_or_default();
    let lyrics_suffix = lyrics_state
        .borrow()
        .source_label
        .as_deref()
        .map(|source| format!(" · lyrics: {source}"))
        .unwrap_or_default();
    settings.player_row.set_subtitle(&format!(
        "{status} via {}: {} - {}{}{}",
        state.bus_name, track.title, artist, progress_suffix, lyrics_suffix
    ));
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

    let state = lyrics_state_from_cached(track, fingerprint.clone(), cached);
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

fn lyrics_state_from_cached(
    track: &TrackMetadata,
    fingerprint: String,
    cached: CachedLyrics,
) -> LyricsDisplayState {
    let lines = match timed_lines_from_raw(&cached.raw_lyrics) {
        Ok(lines) => lines,
        Err(error) => {
            return LyricsDisplayState {
                track_fingerprint: Some(fingerprint),
                source_label: Some(format!("{} #{}", cached.provider, cached.id)),
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
        source_label: Some(format!(
            "{} #{} for {}",
            cached.provider, cached.id, track.title
        )),
        status_message,
    }
}

fn handle_lyrics_fetch_event(
    event: LyricsFetchEvent,
    settings: &SettingsWidgets,
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
                    load_cached_lyrics_after_fetch(track, cache, config, event.track_fingerprint);
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
        settings,
        floating,
        config,
        lyrics_state,
        effective_position_ms(&snapshot),
    );
}

fn load_cached_lyrics_after_fetch(
    track: &TrackMetadata,
    cache: &Cache,
    config: &AppConfig,
    fingerprint: String,
) -> LyricsDisplayState {
    let provider_order = active_provider_order(config);
    match cache.lyrics_for_track(&fingerprint, &provider_order) {
        Ok(Some(cached)) => lyrics_state_from_cached(track, fingerprint, cached),
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
        set_lyrics_labels(floating, "", message, "");
        return;
    }

    if lyrics_state.lines.is_empty() {
        set_lyrics_labels(floating, "", "Waiting for lyrics", "");
        return;
    }

    let Some(position_ms) = position_ms else {
        set_lyrics_labels(floating, "", "Waiting for playback position", "");
        return;
    };

    let Some(index) = active_line_index(&lyrics_state.lines, position_ms, config.lyrics.offset_ms)
    else {
        let next = line_text(lyrics_state.lines.first(), config);
        set_lyrics_labels(floating, "", "…", &next);
        return;
    };

    let previous = line_text(
        index.checked_sub(1).and_then(|i| lyrics_state.lines.get(i)),
        config,
    );
    let current = line_text(lyrics_state.lines.get(index), config);
    let next = line_text(lyrics_state.lines.get(index + 1), config);

    set_lyrics_labels(floating, &previous, &current, &next);
}

fn line_text(line: Option<&TimedLine>, config: &AppConfig) -> String {
    let Some(line) = line else {
        return String::new();
    };

    let mut parts = Vec::new();
    if !line.text.trim().is_empty() {
        parts.push(line.text.trim());
    }
    if config.lyrics.show_translation {
        if let Some(translation) = line.translation.as_deref().map(str::trim) {
            if !translation.is_empty() {
                parts.push(translation);
            }
        }
    }
    if config.lyrics.show_romanization {
        if let Some(romanization) = line.romanization.as_deref().map(str::trim) {
            if !romanization.is_empty() {
                parts.push(romanization);
            }
        }
    }

    parts.join("  /  ")
}

fn set_lyrics_labels(floating: &FloatingWidgets, previous: &str, current: &str, next: &str) {
    floating.previous_line.set_label(previous);
    floating.current_line.set_label(current);
    floating.next_line.set_label(next);
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

fn playback_status_label(status: &PlaybackStatus) -> &str {
    match status {
        PlaybackStatus::Playing => "Playing",
        PlaybackStatus::Paused => "Paused",
        PlaybackStatus::Stopped => "Stopped",
        PlaybackStatus::Unknown(_) => "Unknown",
    }
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
