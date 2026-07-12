// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Resolution and initialization of user-owned application paths.

use anyhow::{Context, Result};
use directories::BaseDirs;
use std::path::{Path, PathBuf};

/// Files used to persist FloatLyrics user data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    /// TOML configuration file.
    pub config_file: PathBuf,
    /// SQLite lyrics cache.
    pub database_file: PathBuf,
}

impl AppPaths {
    /// Resolves user paths and creates their parent directories.
    ///
    /// `config_override` replaces only the configuration location; the cache
    /// continues to use the platform data directory.
    ///
    /// # Errors
    ///
    /// Returns an error when user directories cannot be determined or a parent
    /// directory cannot be created.
    pub fn resolve(config_override: Option<&Path>) -> Result<Self> {
        let base_dirs = BaseDirs::new().context("could not resolve user directories")?;

        let config_file = config_override.map(Path::to_path_buf).unwrap_or_else(|| {
            base_dirs
                .config_dir()
                .join("floatlyrics")
                .join("config.toml")
        });

        let database_file = base_dirs
            .data_dir()
            .join("floatlyrics")
            .join("floatlyrics.sqlite3");

        if let Some(parent) = config_file.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating config directory {}", parent.display()))?;
        }
        if let Some(parent) = database_file.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating data directory {}", parent.display()))?;
        }

        Ok(Self {
            config_file,
            database_file,
        })
    }
}
