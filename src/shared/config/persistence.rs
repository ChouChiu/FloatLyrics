// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Atomic loading and persistence for application configuration.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result, anyhow};

use super::{AppConfig, recovery};

impl AppConfig {
    /// Loads `path`, creating defaults when it does not exist and repairing
    /// incompatible persisted data field by field when possible.
    ///
    /// Before replacing incompatible data, the original bytes are preserved
    /// next to the configuration using an `.incompatible` suffix. Unknown
    /// fields are ignored during recovery, while malformed or invalid fields
    /// fall back to their current defaults.
    ///
    /// # Errors
    /// Returns an error when the file cannot be read, backed up, or replaced,
    /// or when a new default configuration cannot be saved.
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            let config = Self::default();
            config.save(path)?;
            return Ok(config);
        }

        let bytes =
            fs::read(path).with_context(|| format!("reading config file {}", path.display()))?;
        let content = match std::str::from_utf8(&bytes) {
            Ok(content) => content,
            Err(error) => {
                return recover_incompatible(
                    path,
                    &bytes,
                    None,
                    anyhow!(error).context("configuration is not valid UTF-8"),
                );
            }
        };
        match toml::from_str::<Self>(content) {
            Ok(config) => match config.validate() {
                Ok(()) => Ok(config),
                Err(error) => recover_incompatible(
                    path,
                    &bytes,
                    Some(content),
                    error.context("validating configuration"),
                ),
            },
            Err(error) => recover_incompatible(
                path,
                &bytes,
                Some(content),
                anyhow!(error).context("parsing configuration"),
            ),
        }
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

fn recover_incompatible(
    path: &Path,
    original: &[u8],
    content: Option<&str>,
    incompatibility: anyhow::Error,
) -> Result<AppConfig> {
    let config = content.map_or_else(AppConfig::default, recovery::recover_fields);
    config
        .validate()
        .context("validating recovered configuration")?;
    let backup = recovery::backup(path, original)?;
    config
        .save(path)
        .with_context(|| format!("saving recovered config file {}", path.display()))?;
    tracing::warn!(
        config_path = %path.display(),
        backup_path = %backup.display(),
        error = %format_args!("{incompatibility:#}"),
        "recovered incompatible configuration with current defaults"
    );
    Ok(config)
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
