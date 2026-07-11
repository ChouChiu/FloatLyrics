// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Application composition root.
//!
//! Runtime and infrastructure dependencies are created here, while playback
//! orchestration, presentation state, and GTK rendering live in focused modules.

mod about;
mod controller;
mod localization;
mod manual_search;
mod model;
mod settings;
mod view;

use anyhow::{Context, Result};
use gtk::prelude::*;
use std::{cell::RefCell, ffi::OsStr, rc::Rc, sync::mpsc};

use crate::{
    cache::{Cache, LyricsCache},
    config::AppConfig,
    i18n::I18n,
    mpris::spawn_spotify_watcher_with_prefix,
    paths::AppPaths,
};

use view::LyricsView;

struct ApplicationUi {
    _overlay: view::OverlayView,
    settings: settings::SettingsWindow,
    manual_search: manual_search::ManualSearchWindow,
    _about: about::AboutWindow,
    _controller: controller::ControllerHandle,
}

pub fn run(paths: AppPaths, config: AppConfig) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("floatlyrics-worker")
        .build()
        .context("creating Tokio runtime")?;
    let _runtime_guard = runtime.enter();
    let runtime_handle = runtime.handle().clone();
    let cache: Rc<dyn LyricsCache> = Rc::new(Cache::open(&paths.database_file)?);
    let config = Rc::new(RefCell::new(config));
    let i18n = I18n::new(config.borrow().general.language);

    let app = gtk::Application::builder()
        .application_id("io.github.chouchiu.floatlyrics")
        .flags(gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    let ui = Rc::new(RefCell::new(None::<ApplicationUi>));
    {
        let ui = Rc::clone(&ui);
        let config = Rc::clone(&config);
        let i18n = i18n.clone();
        let config_file = paths.config_file.clone();
        app.connect_activate(move |app| {
            if ui.borrow().is_some() {
                return;
            }

            let overlay = view::build(app, &config.borrow(), i18n.clone());
            let overlay_for_settings = overlay.clone();
            let config_for_settings = Rc::clone(&config);
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
            let controller_for_settings = controller.handle();
            let manual_search = manual_search::ManualSearchWindow::new(
                app,
                runtime_handle.clone(),
                Rc::clone(&cache),
                controller.handle(),
                i18n.clone(),
            );
            {
                let manual_search = manual_search.clone();
                overlay.connect_manual_search(move || manual_search.present());
            }
            let about = about::AboutWindow::new(app, i18n.clone());
            let i18n_for_settings = i18n.clone();
            let settings = settings::SettingsWindow::new(
                app,
                config.borrow().clone(),
                config_file.clone(),
                i18n.clone(),
                {
                    let about = about.clone();
                    move || about.present()
                },
                move |next_config| {
                    let reload_lyrics = {
                        let current = config_for_settings.borrow();
                        current.lyrics.provider_order != next_config.lyrics.provider_order
                            || (!current.lyrics.show_translation
                                && next_config.lyrics.show_translation)
                    };
                    overlay_for_settings.apply_config(&next_config);
                    i18n_for_settings.set_language(next_config.general.language);
                    *config_for_settings.borrow_mut() = next_config;
                    if reload_lyrics {
                        controller_for_settings.reload_lyrics();
                    }
                },
            );
            {
                let settings = settings.clone();
                overlay.connect_settings(move || settings.present());
            }
            {
                let app = app.clone();
                overlay.connect_close(move || app.quit());
            }

            // Drive the controller from the GTK tick loop.
            let tick_widget = overlay.tick_widget();
            let controller_rc = Rc::new(RefCell::new(controller));
            let handle = controller_rc.borrow().handle();
            tick_widget.add_tick_callback(move |_, _| {
                controller_rc.borrow().tick();
                gtk::glib::ControlFlow::Continue
            });

            *ui.borrow_mut() = Some(ApplicationUi {
                _overlay: overlay,
                settings,
                manual_search,
                _about: about,
                _controller: handle,
            });
        });
    }

    {
        let ui = Rc::clone(&ui);
        app.connect_command_line(move |app, command_line| {
            app.activate();
            if command_requests_settings(&command_line.arguments())
                && let Some(ui) = ui.borrow().as_ref()
            {
                ui.settings.present();
            }
            if command_requests_manual_search(&command_line.arguments())
                && let Some(ui) = ui.borrow().as_ref()
            {
                ui.manual_search.present();
            }
            gtk::glib::ExitCode::SUCCESS
        });
    }

    app.run();
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
