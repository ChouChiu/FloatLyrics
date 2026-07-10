pub mod app;
pub mod cache;
pub mod config;
pub mod lyrics;
pub mod mpris;
pub mod paths;
pub mod telemetry;
pub mod track;

use anyhow::Result;
use clap::Parser;
use config::AppConfig;
use paths::AppPaths;
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
}

/// Runs FloatLyrics using command-line arguments from the current process.
pub fn run() -> Result<()> {
    configure_default_gtk_renderer();

    let cli = Cli::parse();
    telemetry::init(cli.debug)?;

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
}
