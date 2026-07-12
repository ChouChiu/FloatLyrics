// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Relm4 application composition root.
//!
//! Runtime and infrastructure dependencies are created here.  Relm4 owns the
//! top-level window and routes UI actions through [`AppMsg`], while the
//! playback controller remains independent from the concrete widget tree.

mod about;
mod controller;
mod localization;
mod manual_search;
mod model;
mod settings;
mod view;

use anyhow::{Context, Result};
use gtk::prelude::*;
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, MessageBroker,
    RelmApp, SimpleComponent,
};
use std::{cell::RefCell, ffi::OsStr, rc::Rc, sync::mpsc};

use floatlyrics_core::{i18n::I18n, paths::AppPaths};
use floatlyrics_lyrics::cache::{Cache, LyricsCache};

use crate::{config::AppConfig, mpris::spawn_spotify_watcher_with_prefix};

static APP_BROKER: MessageBroker<AppMsg> = MessageBroker::new();

struct AppInit {
    paths: AppPaths,
    config: AppConfig,
    cache: Rc<dyn LyricsCache>,
    runtime: tokio::runtime::Runtime,
}

struct AppModel {
    _runtime: tokio::runtime::Runtime,
    config: Rc<RefCell<AppConfig>>,
    i18n: I18n,
    overlay: view::OverlayView,
    settings: Controller<settings::SettingsModel>,
    manual_search: Controller<manual_search::ManualSearchModel>,
    about: Controller<about::AboutModel>,
    controller: controller::Controller,
}

#[derive(Debug)]
pub(super) enum AppMsg {
    Tick,
    OpenSettings,
    OpenManualSearch,
    OpenAbout,
    ConfigChanged(AppConfig),
    Quit,
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
            paths,
            config,
            cache,
            runtime,
        } = init;
        let runtime_handle = runtime.handle().clone();
        let config = Rc::new(RefCell::new(config));
        let i18n = I18n::new(config.borrow().general.language);
        let overlay = view::build(
            &root,
            &config.borrow(),
            i18n.clone(),
            sender.input_sender().clone(),
        );
        let (spotify_sender, spotify_receiver) = mpsc::channel();
        spawn_spotify_watcher_with_prefix(
            &runtime_handle,
            spotify_sender,
            config.borrow().spotify.mpris_prefix.clone(),
        );
        let controller = controller::Controller::new(
            spotify_receiver,
            runtime_handle.clone(),
            overlay.clone(),
            Rc::clone(&cache),
            Rc::clone(&config),
        );

        let manual_search = manual_search::ManualSearchModel::builder()
            .launch(manual_search::ManualSearchInit {
                runtime: runtime_handle,
                cache: Rc::clone(&cache),
                controller: controller.handle(),
                i18n: i18n.clone(),
            })
            .detach();
        let about = about::AboutModel::builder().launch(i18n.clone()).detach();
        let settings = settings::SettingsModel::builder()
            .launch(settings::SettingsInit {
                initial: config.borrow().clone(),
                config_file: paths.config_file,
                i18n: i18n.clone(),
            })
            .forward(sender.input_sender(), |output| match output {
                settings::SettingsOutput::Saved(config) => AppMsg::ConfigChanged(config),
                settings::SettingsOutput::OpenAbout => AppMsg::OpenAbout,
            });

        {
            let input = sender.input_sender().clone();
            overlay.tick_widget().add_tick_callback(move |_, _| {
                let _ = input.send(AppMsg::Tick);
                gtk::glib::ControlFlow::Continue
            });
        }

        let model = Self {
            _runtime: runtime,
            config,
            i18n,
            overlay,
            settings,
            manual_search,
            about,
            controller,
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            AppMsg::Tick => self.controller.tick(),
            AppMsg::OpenSettings => {
                let _ = self.settings.sender().send(settings::SettingsMsg::Show);
            }
            AppMsg::OpenManualSearch => {
                let _ = self
                    .manual_search
                    .sender()
                    .send(manual_search::ManualSearchMsg::Show);
            }
            AppMsg::OpenAbout => {
                let _ = self.about.sender().send(about::AboutMsg::Show);
            }
            AppMsg::ConfigChanged(next_config) => {
                let reload_lyrics = {
                    let current = self.config.borrow();
                    current.lyrics.provider_order != next_config.lyrics.provider_order
                        || (!current.lyrics.show_translation && next_config.lyrics.show_translation)
                };
                self.overlay.apply_config(&next_config);
                self.i18n.set_language(next_config.general.language);
                *self.config.borrow_mut() = next_config;
                if reload_lyrics {
                    self.controller.handle().reload_lyrics();
                }
            }
            AppMsg::Quit => relm4::main_application().quit(),
        }
    }
}

pub fn run(paths: AppPaths, config: AppConfig) -> Result<()> {
    // Open the cache before GTK starts so initialization errors remain
    // recoverable through the public `Result` API.
    let cache: Rc<dyn LyricsCache> = Rc::new(Cache::open(&paths.database_file)?);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("floatlyrics-worker")
        .build()
        .context("creating Tokio runtime")?;

    let app = gtk::Application::builder()
        .application_id("io.github.chouchiu.floatlyrics")
        .flags(gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.connect_command_line(|app, command_line| {
        app.activate();
        let arguments = command_line.arguments();
        if command_requests_settings(&arguments) {
            APP_BROKER.send(AppMsg::OpenSettings);
        }
        if command_requests_manual_search(&arguments) {
            APP_BROKER.send(AppMsg::OpenManualSearch);
        }
        gtk::glib::ExitCode::SUCCESS
    });

    RelmApp::from_app(app)
        .with_broker(&APP_BROKER)
        .run::<AppModel>(AppInit {
            paths,
            config,
            cache,
            runtime,
        });
    Ok(())
}

fn command_requests_settings(arguments: &[std::ffi::OsString]) -> bool {
    arguments
        .iter()
        .any(|argument| argument == OsStr::new("--settings"))
}

fn command_requests_manual_search(arguments: &[std::ffi::OsString]) -> bool {
    arguments
        .iter()
        .any(|argument| argument == OsStr::new("--select-lyrics"))
}

#[cfg(test)]
#[path = "test/app_test.rs"]
mod tests;
