// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Playback, lyrics orchestration, caching coordination, and MPRIS adapters.
//!
//! This layer depends on [`crate::shared`] and contains no GTK or WebKit code.

use std::{cell::RefCell, path::Path, rc::Rc, sync::mpsc};

use anyhow::{Context, Result};
use floatlyrics_lyrics::cache::{Cache, LyricsCache};

use crate::shared::config::AppConfig;

mod controller;
mod manual_search;
mod model;
pub mod mpris;

pub(crate) use controller::{Controller, ControllerHandle, LyricsView};
pub(crate) use manual_search::ManualSearchService;

/// Owns backend runtime and persistence services for one frontend instance.
pub(crate) struct Backend {
    runtime: tokio::runtime::Runtime,
    cache: Rc<dyn LyricsCache>,
}

impl Backend {
    pub(crate) fn new(database_file: &Path) -> Result<Self> {
        let cache: Rc<dyn LyricsCache> = Rc::new(Cache::open(database_file)?);
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("floatlyrics-worker")
            .build()
            .context("creating Tokio runtime")?;
        Ok(Self { runtime, cache })
    }

    pub(crate) fn spawn_spotify_watcher(
        &self,
        sender: mpsc::Sender<mpris::SpotifyWatcherEvent>,
        prefix: String,
    ) {
        mpris::spawn_spotify_watcher_with_prefix(self.runtime.handle(), sender, prefix);
    }

    pub(crate) fn controller(
        &self,
        receiver: mpsc::Receiver<mpris::SpotifyWatcherEvent>,
        floating: Rc<dyn LyricsView>,
        config: Rc<RefCell<AppConfig>>,
    ) -> Controller {
        Controller::new(
            receiver,
            self.runtime.handle().clone(),
            floating,
            Rc::clone(&self.cache),
            config,
        )
    }

    pub(crate) fn manual_search(&self) -> ManualSearchService {
        ManualSearchService::new(self.runtime.handle().clone(), Rc::clone(&self.cache))
    }
}
