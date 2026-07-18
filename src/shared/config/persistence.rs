// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Atomic loading and persistence for application configuration.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result};

use super::AppConfig;

impl AppConfig {
    /// Loads `path`, creating and saving defaults when it does not exist.
    ///
    /// # Errors
    /// Returns an error when the file cannot be read, parsed, validated, or initially saved.
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            let config = Self::default();
            config.save(path)?;
            return Ok(config);
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("reading config file {}", path.display()))?;
        let config: Self = toml::from_str(&content)
            .with_context(|| format!("parsing config file {}", path.display()))?;
        config
            .validate()
            .with_context(|| format!("validating config file {}", path.display()))?;
        Ok(config)
    }

    /// Atomically replaces the configuration at `path`.
    ///
    /// # Errors
    /// Returns an error when validation, serialization, directory creation,
    /// writing, or replacement fails. A failed write cleans up its temporary file.
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate().context("validating config")?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating config directory {}", parent.display()))?;
        }

        let content = toml::to_string_pretty(self).context("serializing config")?;
        let temporary = temporary_config_path(path)?;
        if let Err(error) = fs::write(&temporary, content)
            .with_context(|| format!("writing temporary config file {}", temporary.display()))
            .and_then(|()| {
                fs::rename(&temporary, path)
                    .with_context(|| format!("replacing config file {}", path.display()))
            })
        {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }

        Ok(())
    }
}

fn temporary_config_path(path: &Path) -> Result<PathBuf> {
    static NEXT_TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

    let file_name = path
        .file_name()
        .context("config path must point to a file")?
        .to_string_lossy();
    let id = NEXT_TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
    Ok(path.with_file_name(format!(".{file_name}.{}.{}.tmp", std::process::id(), id)))
}
