//! Application composition root.
//!
//! Runtime and infrastructure dependencies are created here, while playback
//! orchestration, presentation state, and GTK rendering live in focused modules.

mod controller;
mod model;
mod view;

use anyhow::{Context, Result};
use gtk::prelude::*;
use std::{rc::Rc, sync::mpsc};

use crate::{cache::Cache, config::AppConfig, mpris::spawn_spotify_watcher, paths::AppPaths};

pub fn run(paths: AppPaths, config: AppConfig) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("floatlyrics-worker")
        .build()
        .context("creating Tokio runtime")?;
    let _runtime_guard = runtime.enter();
    let runtime_handle = runtime.handle().clone();
    let cache = Rc::new(Cache::open(&paths.database_file)?);
    let config = Rc::new(config);

    let app = gtk::Application::builder()
        .application_id("io.github.chouchiu.FloatLyrics")
        .build();

    app.connect_activate(move |app| {
        let overlay = view::build(app, &config);
        let (spotify_sender, spotify_receiver) = mpsc::channel();
        spawn_spotify_watcher(&runtime_handle, spotify_sender);
        controller::attach(
            spotify_receiver,
            runtime_handle.clone(),
            overlay,
            Rc::clone(&cache),
            Rc::clone(&config),
        );
    });

    app.run();
    Ok(())
}
