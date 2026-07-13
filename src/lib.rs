// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

#![warn(missing_docs)]

//! FloatLyrics application library.
//!
//! The root crate is split into a GTK/WebKit [`frontend`], a service-oriented
//! [`backend`], and dependency-light data contracts in [`shared`].

/// Playback, lyrics-service, cache-coordination, and MPRIS backend.
pub mod backend;
/// GTK/Relm4 and WebKit application frontend.
pub mod frontend;
/// Configuration and contracts shared across application layers.
pub mod shared;

/// Compatibility re-export for the former MPRIS module path.
pub use backend::mpris;
/// Compatibility alias for the former application module path.
pub use frontend as app;
/// Compatibility re-export for the former configuration module path.
pub use shared::config;

use anyhow::Result;
use clap::Parser;
use floatlyrics_core::paths::AppPaths;
use shared::config::AppConfig;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Cli {
    #[arg(long)]
    debug: bool,

    #[arg(long)]
    config: Option<PathBuf>,

    #[arg(long)]
    reset_window: bool,

    /// Opens the settings window (also suitable for a desktop-shell button).
    #[arg(long)]
    settings: bool,

    /// Opens manual lyrics search for the currently playing track.
    #[arg(long)]
    select_lyrics: bool,
}

/// Runs FloatLyrics using command-line arguments from the current process.
///
/// # Errors
///
/// Returns an error when telemetry, user paths, configuration, the cache, or
/// the asynchronous runtime cannot be initialized.
pub fn run() -> Result<()> {
    configure_default_gtk_renderer();

    let cli = Cli::parse();
    floatlyrics_core::telemetry::init(cli.debug)?;
    floatlyrics_core::i18n::validate_catalogues()?;

    let paths = AppPaths::resolve(cli.config.as_deref())?;
    let mut config = AppConfig::load_or_default(&paths.config_file)?;

    if cli.reset_window {
        config.window = Default::default();
        config.save(&paths.config_file)?;
    }

    frontend::run(paths, config)
}

fn configure_default_gtk_renderer() {
    if std::env::var_os("GSK_RENDERER").is_none() {
        // SAFETY: this runs at process startup, before GTK is initialized and before
        // the app creates worker threads, so no other thread can concurrently read
        // or mutate the process environment from this code path.
        unsafe {
            std::env::set_var("GSK_RENDERER", "gl");
        }
    }
    if std::env::var_os("GTK_A11Y").is_none() {
        // SAFETY: same reasoning — single-threaded process startup before GTK init.
        unsafe {
            std::env::set_var("GTK_A11Y", "none");
        }
    }
}

#[cfg(test)]
#[path = "test/lib_test.rs"]
mod tests;
