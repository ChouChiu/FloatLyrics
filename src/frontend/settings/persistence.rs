// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Serialized configuration writes outside the GTK main thread.

use std::{
    path::PathBuf,
    sync::mpsc,
    thread::{self, JoinHandle},
};

use anyhow::{Context, Result};

use crate::shared::config::AppConfig;

type Completion = Box<dyn FnOnce(ConfigSaveResult) + Send + 'static>;

struct SaveRequest {
    config: AppConfig,
    complete: Completion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::frontend) enum ConfigSaveResult {
    Saved,
    Failed(String),
    Superseded,
}

pub(in crate::frontend) struct ConfigSaveService {
    sender: Option<mpsc::Sender<SaveRequest>>,
    worker: Option<JoinHandle<()>>,
}

impl ConfigSaveService {
    pub(in crate::frontend) fn new(config_file: PathBuf) -> Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("floatlyrics-config".to_string())
            .spawn(move || run_worker(config_file, receiver))
            .context("spawning configuration save worker")?;
        Ok(Self {
            sender: Some(sender),
            worker: Some(worker),
        })
    }

    pub(super) fn save(
        &self,
        config: AppConfig,
        complete: impl FnOnce(ConfigSaveResult) + Send + 'static,
    ) {
        let request = SaveRequest {
            config,
            complete: Box::new(complete),
        };
        let Some(sender) = &self.sender else {
            (request.complete)(ConfigSaveResult::Failed(
                "configuration save worker is unavailable".to_string(),
            ));
            return;
        };
        if let Err(error) = sender.send(request) {
            (error.0.complete)(ConfigSaveResult::Failed(
                "configuration save worker stopped".to_string(),
            ));
        }
    }
}

impl Drop for ConfigSaveService {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            tracing::warn!("configuration save worker panicked");
        }
    }
}

fn run_worker(config_file: PathBuf, receiver: mpsc::Receiver<SaveRequest>) {
    while let Ok(mut request) = receiver.recv() {
        while let Ok(newer) = receiver.try_recv() {
            (request.complete)(ConfigSaveResult::Superseded);
            request = newer;
        }

        let result = request.config.save(&config_file).map_or_else(
            |error| ConfigSaveResult::Failed(format!("{error:#}")),
            |()| ConfigSaveResult::Saved,
        );
        (request.complete)(result);
    }
}

#[cfg(test)]
#[path = "../../test/config_save_service_test.rs"]
mod tests;
