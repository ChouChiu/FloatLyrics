// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Relm4 frontend composition root.
//!
//! Runtime and infrastructure dependencies are created here.  Relm4 owns the
//! top-level window and routes UI actions through `AppMsg`, while the
//! playback controller remains independent from the concrete widget tree.

mod about;
mod manual_search;
mod messages;
mod output;
mod settings;
mod style;
mod tray;
mod view;

use anyhow::{Result, ensure};
use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, MessageBroker, RelmApp, SimpleComponent};
use std::{rc::Rc, sync::mpsc, time::Duration};

use crate::{
    backend::{self, mpris::MediaControlHandle},
    shared::{
        config::{AppConfig, AppMode},
        runtime::LyricsRuntimeConfig,
    },
};
use floatlyrics_core::{i18n::I18n, paths::AppPaths};
use manual_search::window::ManualSearchView;
use messages::requested_commands;
use output::{LyricsOutput, OutputChange, output_change, should_reload_lyrics};
use settings::{font_picker::FontPickerView, window::ControlCenterView};
use view::web_lyrics::ControlPage;

#[cfg(test)]
use messages::AppCommand;
use messages::{AppMsg, UiAction};

/// Delay before a new AMLL address reconnects, so typing it does not reconnect
/// on every keystroke.
const AMLL_RECONNECT_DELAY_MS: u64 = 500;

const APP_ID: &str = "io.github.chouchiu.floatlyrics";

static APP_BROKER: MessageBroker<AppMsg> = MessageBroker::new();

struct AppInit {
    config: AppConfig,
    backend: backend::Backend,
    config_saver: settings::ConfigSaveService,
}

struct AppModel {
    config: AppConfig,
    i18n: I18n,
    output: LyricsOutput,
    control_center: Option<ControlCenterView>,
    font_picker: Option<FontPickerView>,
    manual_search: manual_search::ManualSearchCoordinator,
    manual_search_window: Option<ManualSearchView>,
    config_saver: settings::ConfigSaveService,
    save_revision: u64,
    controller: backend::Controller,
    media_control: MediaControlHandle,
    tray: Option<tray::TrayHandle>,
    /// Bumped by every output change, so a delayed reconnect that another
    /// change superseded is ignored.
    amll_reconnect_generation: u64,
    backend: backend::Backend,
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
        let mut output = LyricsOutput::new(root.clone(), config.general.mode);
        let lyrics_view = output.activate(&config, &i18n, &backend, sender.input_sender());
        let tray = tray::spawn(&config, &backend.runtime());
        let (player_sender, player_receiver) = mpsc::channel();
        let (player_hint_sender, player_hint_receiver) = mpsc::channel();
        let media_control = backend.spawn_player_watcher(
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
            lyrics_view,
            controller_config,
        );

        let manual_search = manual_search::ManualSearchCoordinator::new(
            backend.manual_search(),
            controller.handle(),
        );
        let model = Self {
            config,
            i18n,
            output,
            control_center: None,
            font_picker: None,
            manual_search,
            manual_search_window: None,
            config_saver,
            save_revision: 0,
            controller,
            media_control,
            tray,
            amll_reconnect_generation: 0,
            backend,
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            AppMsg::Tick => {
                self.controller.tick();
                self.drain_media_commands();
                if self.manual_search_window.is_some()
                    && self
                        .manual_search
                        .observe_track(self.controller.current_fingerprint(), sender.input_sender())
                {
                    self.render_search_state();
                }
            }
            AppMsg::Activated => self.output.present(),
            AppMsg::ReconnectAmll { generation } => {
                if generation == self.amll_reconnect_generation && self.output.mode == AppMode::Amll
                {
                    self.switch_output(sender.input_sender());
                }
            }
            AppMsg::Overlay(update) => {
                if let Some(overlay) = self.output.overlay() {
                    update.apply(overlay);
                }
            }
            AppMsg::OpenSettings => {
                self.open_settings(ControlPage::General, sender.input_sender().clone());
            }
            AppMsg::OpenManualSearch => self.open_manual_search(sender.input_sender().clone()),
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
                self.manual_search
                    .handle_event(event, sender.input_sender().clone());
                self.render_search_state();
            }
            AppMsg::ConfigSaveFinished { revision, result } => {
                self.handle_config_saved(revision, result);
            }
        }
    }
}

impl AppModel {
    fn handle_ui_action(&mut self, action: UiAction, sender: relm4::Sender<AppMsg>) {
        match action {
            UiAction::OpenSettings => self.open_settings(ControlPage::General, sender),
            UiAction::OpenSearch => self.open_manual_search(sender),
            UiAction::OpenSettingsPage { page } => self.open_settings(page, sender),
            UiAction::OpenFontPicker => self
                .font_picker
                .get_or_insert_with(|| FontPickerView::new(&self.config, sender))
                .show(),
            UiAction::CloseFontPicker => {
                if let Some(window) = &self.font_picker {
                    window.hide();
                }
            }
            UiAction::Quit => relm4::main_application().quit(),
            UiAction::AdjustTrackOffset { delta_ms } => {
                self.controller.handle().adjust_track_offset(delta_ms)
            }
            UiAction::ResetTrackOffset => self.controller.handle().reset_track_offset(),
            UiAction::ReloadLyrics => self.controller.handle().reload_lyrics(),
            UiAction::SaveConfig { config } => self.save_config(*config, sender),
            UiAction::SearchLyrics { title, artist } => {
                self.manual_search.search(title, artist, sender);
                self.render_search_state();
            }
            UiAction::LookupQqLyrics { reference } => {
                self.manual_search.lookup_qq(reference, sender);
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

    /// Forwards playback requests that arrived over the AMLL WebSocket.
    fn drain_media_commands(&mut self) {
        let Some(receiver) = self.output.remote_commands.as_mut() else {
            return;
        };
        while let Ok(command) = receiver.try_recv() {
            self.media_control.send(command);
        }
    }

    fn save_config(&mut self, config: AppConfig, sender: relm4::Sender<AppMsg>) {
        if let Err(error) = config.validate() {
            self.render_config_state(false, Some(&format!("{error:#}")));
            return;
        }
        self.save_revision = self.save_revision.wrapping_add(1);
        let revision = self.save_revision;
        self.apply_config(config.clone(), &sender);
        self.render_config_state(false, None);
        self.config_saver.save(config, move |result| {
            let _ = sender.send(AppMsg::ConfigSaveFinished { revision, result });
        });
    }

    fn handle_config_saved(&mut self, revision: u64, result: settings::ConfigSaveResult) {
        use settings::ConfigSaveResult;
        match result {
            ConfigSaveResult::Saved if revision == self.save_revision => {
                self.render_config_state(true, None);
            }
            ConfigSaveResult::Failed(error) if revision == self.save_revision => {
                self.render_config_state(false, Some(&error));
            }
            ConfigSaveResult::Saved
            | ConfigSaveResult::Failed(_)
            | ConfigSaveResult::Superseded => {}
        }
    }

    fn apply_config(&mut self, next_config: AppConfig, sender: &relm4::Sender<AppMsg>) {
        let reload_lyrics = should_reload_lyrics(&self.config, &next_config);
        let output_change = output_change(&self.config, &next_config);
        if let Some(overlay) = &self.output.overlay {
            overlay.apply_config(&next_config);
        }
        self.i18n.set_language(next_config.general.language);
        if let Some(tray) = &self.tray {
            tray.set_language(next_config.general.language);
        }
        self.controller
            .update_config(LyricsRuntimeConfig::from(&next_config));
        self.config = next_config;
        if output_change != OutputChange::None {
            self.amll_reconnect_generation = self.amll_reconnect_generation.wrapping_add(1);
        }
        match output_change {
            OutputChange::None => {}
            OutputChange::Mode => self.switch_output(sender),
            OutputChange::AmllAddress => {
                let generation = self.amll_reconnect_generation;
                let sender = sender.clone();
                gtk::glib::timeout_add_local_once(
                    Duration::from_millis(AMLL_RECONNECT_DELAY_MS),
                    move || {
                        let _ = sender.send(AppMsg::ReconnectAmll { generation });
                    },
                );
            }
        }
        if let Some(window) = &self.control_center {
            window.bootstrap(&self.config);
        }
        if let Some(window) = &self.font_picker {
            window.bootstrap(&self.config);
        }
        if let Some(window) = &self.manual_search_window {
            window.bootstrap(&self.config);
        }
        self.controller.refresh_lyrics_presentation();
        self.render_search_state();
        if reload_lyrics {
            self.controller.reload_lyrics();
        }
    }

    /// Moves the lyrics output to the mode and address in the current config.
    fn switch_output(&mut self, sender: &relm4::Sender<AppMsg>) {
        let view = self
            .output
            .activate(&self.config, &self.i18n, &self.backend, sender);
        self.controller.set_view(view);
        self.output.present();
    }

    fn open_settings(&mut self, page: ControlPage, sender: relm4::Sender<AppMsg>) {
        self.control_center
            .get_or_insert_with(|| {
                ControlCenterView::new(&self.config, Rc::new(about::license_data_json()), sender)
            })
            .show(page);
    }

    fn open_manual_search(&mut self, sender: relm4::Sender<AppMsg>) {
        self.manual_search_window
            .get_or_insert_with(|| ManualSearchView::new(&self.config, sender.clone()))
            .show();
        self.manual_search.prepare_and_search(sender);
        self.render_search_state();
    }

    fn render_config_state(&self, saved: bool, error: Option<&str>) {
        if let Some(window) = &self.control_center {
            window.set_config_state(&self.config, saved, error);
        }
        if let Some(window) = &self.font_picker {
            window.set_config_state(&self.config, saved, error);
        }
    }

    fn render_search_state(&self) {
        if let Some(window) = &self.manual_search_window {
            window.set_search_state(&self.manual_search.snapshot(self.i18n.language()));
        }
    }
}

fn forward_to_running_instance() -> Result<bool> {
    // A remote instance has no Relm4 components to wake the shutdown loop.
    // Let GApplication forward its command line without starting GTK or workers.
    let launcher = gtk::gio::Application::builder()
        .application_id(APP_ID)
        .flags(
            gtk::gio::ApplicationFlags::IS_LAUNCHER
                | gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE,
        )
        .build();
    if launcher.register(None::<&gtk::gio::Cancellable>).is_err() {
        return Ok(false);
    }
    if !launcher.is_remote() {
        return Ok(false);
    }
    let status = launcher.run();
    ensure!(
        status == gtk::glib::ExitCode::SUCCESS,
        "running FloatLyrics instance rejected the command (exit code {})",
        status.get()
    );
    Ok(true)
}

/// Starts the GTK application with resolved `paths` and loaded `config`.
///
/// The function blocks until the application exits.
///
/// # Errors
///
/// Returns an error when the lyrics cache, Tokio runtime, or configuration
/// save worker cannot initialize, or the running instance rejects the command.
pub fn run(paths: AppPaths, config: AppConfig) -> Result<()> {
    if forward_to_running_instance()? {
        return Ok(());
    }
    // Open the cache before GTK starts so initialization errors remain
    // recoverable through the public `Result` API.
    let backend = backend::Backend::new(&paths.database_file)?;
    let config_saver = settings::ConfigSaveService::new(paths.config_file)?;

    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    // The overlay is shown by the model rather than by Relm4, because the run
    // mode can change after startup and only the floating output is a window.
    app.connect_activate(|_| APP_BROKER.send(AppMsg::Activated));
    app.connect_command_line(|app, command_line| {
        app.activate();
        let arguments = command_line.arguments();
        for command in requested_commands(&arguments) {
            APP_BROKER.send(command.message());
        }
        gtk::glib::ExitCode::SUCCESS
    });

    RelmApp::from_app(app)
        .visible_on_activate(false)
        .with_broker(&APP_BROKER)
        .run::<AppModel>(AppInit {
            config,
            backend,
            config_saver,
        });
    Ok(())
}

#[cfg(test)]
#[path = "test/app_test.rs"]
mod tests;
