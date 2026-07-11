// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

pub mod app;
pub mod config;
pub mod mpris;

use anyhow::Result;
use clap::Parser;
use config::AppConfig;
use floatlyrics_core::paths::AppPaths;
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
pub fn run() -> Result<()> {
    configure_default_gtk_renderer();

    let cli = Cli::parse();
    floatlyrics_core::telemetry::init(cli.debug)?;

    let paths = AppPaths::resolve(cli.config.as_deref())?;
    let mut config = AppConfig::load_or_default(&paths.config_file)?;

    if cli.reset_window {
        config.window = Default::default();
        config.save(&paths.config_file)?;
    }

    app::run(paths, config)
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
