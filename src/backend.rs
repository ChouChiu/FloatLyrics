// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Playback, lyrics orchestration, caching coordination, and MPRIS adapters.
//!
//! This layer depends on [`crate::shared`] and contains no GTK or WebKit code.

use std::{path::Path, rc::Rc, sync::mpsc};

use crate::shared::runtime::LyricsRuntimeConfig;
use anyhow::{Context, Result};

mod cache;
mod controller;
mod manual_search;
mod model;
pub mod mpris;

pub(crate) use controller::{Controller, ControllerHandle, LyricsView};
pub(crate) use manual_search::ManualSearchService;

/// Owns backend runtime and persistence services for one frontend instance.
pub(crate) struct Backend {
    runtime: tokio::runtime::Runtime,
    cache: cache::CacheWorker,
}

impl Backend {
    pub(crate) fn new(database_file: &Path) -> Result<Self> {
        let cache = cache::CacheWorker::new(database_file)?;
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
        config: LyricsRuntimeConfig,
    ) -> Controller {
        Controller::new(
            receiver,
            self.runtime.handle().clone(),
            floating,
            self.cache.service(),
            config,
        )
    }

    pub(crate) fn manual_search(&self) -> ManualSearchService {
        ManualSearchService::new(self.runtime.handle().clone(), self.cache.service())
    }
}
