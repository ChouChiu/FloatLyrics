//! Application composition root.
//!
//! Runtime and infrastructure dependencies are created here, while playback
//! orchestration, presentation state, and GTK rendering live in focused modules.

mod controller;
mod model;
mod settings;
mod view;

use anyhow::{Context, Result};
use gtk::prelude::*;
use std::{cell::RefCell, ffi::OsStr, rc::Rc, sync::mpsc};

use crate::{cache::Cache, config::AppConfig, mpris::spawn_spotify_watcher, paths::AppPaths};

struct ApplicationUi {
    _overlay: view::OverlayView,
    settings: settings::SettingsWindow,
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
    let cache = Rc::new(Cache::open(&paths.database_file)?);
    let config = Rc::new(RefCell::new(config));

    let app = gtk::Application::builder()
        .application_id("io.github.chouchiu.FloatLyrics")
        .flags(gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    let ui = Rc::new(RefCell::new(None::<ApplicationUi>));
    {
        let ui = Rc::clone(&ui);
        let config = Rc::clone(&config);
        let config_file = paths.config_file.clone();
        app.connect_activate(move |app| {
            if ui.borrow().is_some() {
                return;
            }

            let overlay = view::build(app, &config.borrow());
            let overlay_for_settings = overlay.clone();
            let config_for_settings = Rc::clone(&config);
            let (spotify_sender, spotify_receiver) = mpsc::channel();
            spawn_spotify_watcher(&runtime_handle, spotify_sender);
            let controller = controller::attach(
                spotify_receiver,
                runtime_handle.clone(),
                overlay.clone(),
                Rc::clone(&cache),
                Rc::clone(&config),
            );
            let controller_for_settings = controller.clone();
            let settings = settings::SettingsWindow::new(
                app,
                config.borrow().clone(),
                config_file.clone(),
                move |next_config| {
                    let reload_lyrics = {
                        let current = config_for_settings.borrow();
                        current.lyrics.provider_order != next_config.lyrics.provider_order
                            || (!current.lyrics.show_translation
                                && next_config.lyrics.show_translation)
                    };
                    overlay_for_settings.apply_config(&next_config);
                    *config_for_settings.borrow_mut() = next_config;
                    if reload_lyrics {
                        controller_for_settings.reload_lyrics();
                    }
                },
            );

            *ui.borrow_mut() = Some(ApplicationUi {
                _overlay: overlay,
                settings,
                _controller: controller,
            });
        });
    }

    {
        let ui = Rc::clone(&ui);
        app.connect_command_line(move |app, command_line| {
            app.activate();
            if command_requests_settings(&command_line.arguments()) {
                if let Some(ui) = ui.borrow().as_ref() {
                    ui.settings.present();
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_settings_command_without_matching_substrings() {
        assert!(command_requests_settings(&[
            "floatlyrics".into(),
            "--settings".into(),
        ]));
        assert!(!command_requests_settings(&[
            "floatlyrics".into(),
            "--settings-file".into(),
        ]));
    }
}
