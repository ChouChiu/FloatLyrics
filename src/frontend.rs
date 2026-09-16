// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Relm4 frontend composition root.
//!
//! Runtime and infrastructure dependencies are created here.  Relm4 owns the
//! top-level window and routes UI actions through `AppMsg`, while the
//! playback controller remains independent from the concrete widget tree.

mod about;
mod control_center;
mod font_picker_window;
mod manual_search;
mod manual_search_window;
mod settings;
mod style;
mod view;

use anyhow::Result;
use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, MessageBroker, RelmApp, SimpleComponent};
use serde::Deserialize;
use std::{
    ffi::OsStr,
    rc::Rc,
    sync::{Arc, mpsc},
    time::Duration,
};

use crate::{
    backend::{self},
    shared::{
        config::{AppConfig, WindowPosition},
        presentation::{LyricsDocument, LyricsFrame},
        runtime::LyricsRuntimeConfig,
    },
};
use floatlyrics_core::{i18n::I18n, paths::AppPaths};

static APP_BROKER: MessageBroker<AppMsg> = MessageBroker::new();

struct AppInit {
    config: AppConfig,
    backend: backend::Backend,
    config_saver: settings::ConfigSaveService,
}

struct AppModel {
    config: AppConfig,
    i18n: I18n,
    overlay: view::OverlayView,
    control_center: control_center::ControlCenterView,
    font_picker: font_picker_window::FontPickerView,
    manual_search: manual_search::ManualSearchCoordinator,
    manual_search_window: manual_search_window::ManualSearchView,
    config_saver: settings::ConfigSaveService,
    save_revision: u64,
    controller: backend::Controller,
    song_info: String,
    track_offset_ms: i64,
    lyrics: LyricsPresentation,
    lyrics_document: Option<LyricsDocument>,
    _backend: backend::Backend,
}

#[derive(Debug, Clone)]
enum LyricsPresentation {
    Content(Arc<LyricsFrame>),
    Status(floatlyrics_core::i18n::Text),
}

#[derive(Debug)]
enum AppMsg {
    Tick,
    SetSongInfo(String),
    SetTrackOffset(i64),
    SetLyricsDocument(LyricsDocument),
    ShowLyrics(Arc<LyricsFrame>),
    ShowStatus(floatlyrics_core::i18n::Text),
    OpenSettings,
    OpenManualSearch,
    UiAction(UiAction),
    SearchEvent(manual_search::SearchEvent),
    ConfigSaveFinished {
        revision: u64,
        result: settings::ConfigSaveResult,
    },
    WindowMoved(WindowPosition),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum UiAction {
    OpenSettings,
    OpenSearch,
    OpenSettingsPage { page: view::web_lyrics::ControlPage },
    OpenFontPicker,
    CloseFontPicker,
    Quit,
    AdjustTrackOffset { delta_ms: i64 },
    ResetTrackOffset,
    SaveConfig { config: Box<AppConfig> },
    SearchLyrics { title: String, artist: String },
    PreviewLyrics { index: usize },
    ApplyLyrics,
    OpenUrl { url: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppCommand {
    OpenSettings,
    OpenManualSearch,
}

impl AppCommand {
    fn from_argument(argument: &OsStr) -> Option<Self> {
        match argument.to_str()? {
            "--settings" => Some(Self::OpenSettings),
            "--select-lyrics" => Some(Self::OpenManualSearch),
            _ => None,
        }
    }

    fn message(self) -> AppMsg {
        match self {
            Self::OpenSettings => AppMsg::OpenSettings,
            Self::OpenManualSearch => AppMsg::OpenManualSearch,
        }
    }
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = AppInit;
    type Input = AppMsg;
    type Output = ();

    view! {
        main_window = gtk::ApplicationWindow {}
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let AppInit {
            config,
            backend,
            config_saver,
        } = init;
        let i18n = I18n::new(config.general.language);
        let overlay = view::build(
            &root,
            &config,
            i18n.clone(),
            Rc::new(serde_json::json!({ "dependencies": [], "licenses": [] })),
            sender.input_sender().clone(),
        );
        let (player_sender, player_receiver) = mpsc::channel();
        let (player_hint_sender, player_hint_receiver) = mpsc::channel();
        backend.spawn_player_watcher(
            player_sender,
            player_hint_sender,
            backend::mpris::PlayerSelection {
                preferred_players: config.player.effective_preferred_players(),
                ignored_players: config.player.ignored_players.clone(),
                allowed_bus_prefixes: Vec::new(),
            },
        );
        let controller_config = LyricsRuntimeConfig::from(&config);
        let controller = backend.controller(
            player_receiver,
            player_hint_receiver,
            Rc::new(view::OverlaySender::new(sender.input_sender().clone())),
            controller_config,
        );

        let manual_search = manual_search::ManualSearchCoordinator::new(
            backend.manual_search(),
            controller.handle(),
        );
        let control_center = control_center::ControlCenterView::new(
            &config,
            Rc::new(about::license_data_json()),
            sender.input_sender().clone(),
        );
        let manual_search_window =
            manual_search_window::ManualSearchView::new(&config, sender.input_sender().clone());
        let font_picker =
            font_picker_window::FontPickerView::new(&config, sender.input_sender().clone());

        {
            let input = sender.input_sender().clone();
            overlay.tick_widget().add_tick_callback(move |_, _| {
                let _ = input.send(AppMsg::Tick);
                gtk::glib::ControlFlow::Continue
            });
        }

        let model = Self {
            config,
            i18n,
            overlay,
            control_center,
            font_picker,
            manual_search,
            manual_search_window,
            config_saver,
            save_revision: 0,
            controller,
            song_info: "FloatLyrics".to_string(),
            track_offset_ms: 0,
            lyrics: LyricsPresentation::Status(floatlyrics_core::i18n::Text::OpenPlayer),
            lyrics_document: None,
            _backend: backend,
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            AppMsg::Tick => self.controller.tick(),
            AppMsg::SetSongInfo(value) => self.song_info = value,
            AppMsg::SetTrackOffset(value) => self.track_offset_ms = value,
            AppMsg::SetLyricsDocument(document) => self.lyrics_document = Some(document),
            AppMsg::ShowLyrics(frame) => self.lyrics = LyricsPresentation::Content(frame),
            AppMsg::ShowStatus(key) => self.lyrics = LyricsPresentation::Status(key),
            AppMsg::OpenSettings => {
                self.control_center
                    .show(view::web_lyrics::ControlPage::General);
            }
            AppMsg::OpenManualSearch => {
                self.manual_search_window.show();
                self.manual_search
                    .prepare_and_search(sender.input_sender().clone());
                self.render_search_state();
            }
            AppMsg::WindowMoved(position) => {
                if self.config.window.remember_position {
                    let mut next = self.config.clone();
                    next.window.position = Some(position);
                    self.save_config(next, sender.input_sender().clone());
                }
            }
            AppMsg::UiAction(action) => {
                self.handle_ui_action(action, sender.input_sender().clone())
            }
            AppMsg::SearchEvent(event) => {
                self.manual_search.handle_event(event);
                self.render_search_state();
            }
            AppMsg::ConfigSaveFinished { revision, result } => {
                self.handle_config_saved(revision, result);
            }
        }
    }

    fn post_view() {
        self.overlay.set_song_info(&self.song_info);
        self.overlay.set_track_offset(self.track_offset_ms);
        if let Some(document) = &self.lyrics_document {
            self.overlay.set_lyrics_document(document);
        }
        match &self.lyrics {
            LyricsPresentation::Content(frame) => {
                overlay.show_lyrics(Arc::clone(frame));
            }
            LyricsPresentation::Status(key) => self.overlay.show_status(*key),
        }
    }
}

impl AppModel {
    fn handle_ui_action(&mut self, action: UiAction, sender: relm4::Sender<AppMsg>) {
        match action {
            UiAction::OpenSettings => self
                .control_center
                .show(view::web_lyrics::ControlPage::General),
            UiAction::OpenSearch => {
                self.manual_search_window.show();
                self.manual_search.prepare_and_search(sender);
                self.render_search_state();
            }
            UiAction::OpenSettingsPage { page } => self.control_center.show(page),
            UiAction::OpenFontPicker => self.font_picker.show(),
            UiAction::CloseFontPicker => self.font_picker.hide(),
            UiAction::Quit => relm4::main_application().quit(),
            UiAction::AdjustTrackOffset { delta_ms } => {
                self.controller.handle().adjust_track_offset(delta_ms)
            }
            UiAction::ResetTrackOffset => self.controller.handle().reset_track_offset(),
            UiAction::SaveConfig { config } => self.save_config(*config, sender),
            UiAction::SearchLyrics { title, artist } => {
                self.manual_search.search(title, artist, sender);
                self.render_search_state();
            }
            UiAction::PreviewLyrics { index } => {
                self.manual_search.select(index, sender);
                self.render_search_state();
            }
            UiAction::ApplyLyrics => {
                self.manual_search.apply(sender);
                self.render_search_state();
            }
            UiAction::OpenUrl { url } => {
                if matches!(
                    url.as_str(),
                    "https://github.com/ChouChiu/FloatLyrics"
                        | "https://github.com/MxIris-LyricsX-Project/LyricsX"
                ) && let Err(error) = gtk::gio::AppInfo::launch_default_for_uri(
                    &url,
                    None::<&gtk::gio::AppLaunchContext>,
                ) {
                    tracing::warn!(%error, %url, "failed to open project link");
                }
            }
        }
    }

    fn save_config(&mut self, config: AppConfig, sender: relm4::Sender<AppMsg>) {
        if let Err(error) = config.validate() {
            self.control_center
                .set_config_state(&self.config, false, Some(&format!("{error:#}")));
            self.font_picker
                .set_config_state(&self.config, false, Some(&format!("{error:#}")));
            return;
        }
        self.save_revision = self.save_revision.wrapping_add(1);
        let revision = self.save_revision;
        self.apply_config(config.clone());
        self.control_center.set_config_state(&config, false, None);
        self.font_picker.set_config_state(&config, false, None);
        self.config_saver.save(config, move |result| {
            let _ = sender.send(AppMsg::ConfigSaveFinished { revision, result });
        });
    }

    fn handle_config_saved(&mut self, revision: u64, result: settings::ConfigSaveResult) {
        use settings::ConfigSaveResult;
        match result {
            ConfigSaveResult::Saved if revision == self.save_revision => {
                self.control_center
                    .set_config_state(&self.config, true, None);
                self.font_picker.set_config_state(&self.config, true, None);
            }
            ConfigSaveResult::Failed(error) if revision == self.save_revision => {
                self.control_center
                    .set_config_state(&self.config, false, Some(&error));
                self.font_picker
                    .set_config_state(&self.config, false, Some(&error));
            }
            ConfigSaveResult::Saved
            | ConfigSaveResult::Failed(_)
            | ConfigSaveResult::Superseded => {}
        }
    }

    fn apply_config(&mut self, next_config: AppConfig) {
        let reload_lyrics = should_reload_lyrics(&self.config, &next_config);
        self.overlay.apply_config(&next_config);
        self.i18n.set_language(next_config.general.language);
        self.controller
            .update_config(LyricsRuntimeConfig::from(&next_config));
        self.config = next_config;
        self.control_center.bootstrap(&self.config);
        self.font_picker.bootstrap(&self.config);
        self.manual_search_window.bootstrap(&self.config);
        self.controller.refresh_lyrics_presentation();
        self.render_search_state();
        if reload_lyrics {
            self.controller.reload_lyrics();
        }
    }

    fn render_search_state(&self) {
        self.manual_search_window
            .set_search_state(&self.manual_search.snapshot(self.i18n.language()));
    }
}

fn should_reload_lyrics(current: &AppConfig, next: &AppConfig) -> bool {
    current.lyrics.provider_order != next.lyrics.provider_order
        || (!current.lyrics.show_translation && next.lyrics.show_translation)
        || (!current.lyrics.show_romanization && next.lyrics.show_romanization)
        || (next.lyrics.show_romanization
            && current.lyrics.chinese_romanization != next.lyrics.chinese_romanization)
}

/// Starts the GTK application with resolved `paths` and loaded `config`.
///
/// The function blocks until the application exits.
///
/// # Errors
///
/// Returns an error when the lyrics cache, Tokio runtime, or configuration
/// save worker cannot initialize.
pub fn run(paths: AppPaths, config: AppConfig) -> Result<()> {
    // Open the cache before GTK starts so initialization errors remain
    // recoverable through the public `Result` API.
    let backend = backend::Backend::new(&paths.database_file)?;
    let config_saver = settings::ConfigSaveService::new(paths.config_file)?;

    let app = gtk::Application::builder()
        .application_id("io.github.chouchiu.floatlyrics")
        .flags(gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.connect_command_line(|app, command_line| {
        app.activate();
        let arguments = command_line.arguments();
        for command in requested_commands(&arguments) {
            APP_BROKER.send(command.message());
        }
        gtk::glib::ExitCode::SUCCESS
    });

    RelmApp::from_app(app)
        .with_broker(&APP_BROKER)
        .run::<AppModel>(AppInit {
            config,
            backend,
            config_saver,
        });
    Ok(())
}

fn requested_commands(arguments: &[std::ffi::OsString]) -> impl Iterator<Item = AppCommand> + '_ {
    arguments
        .iter()
        .filter_map(|argument| AppCommand::from_argument(argument))
}

#[cfg(test)]
#[path = "test/app_test.rs"]
mod tests;
