// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use tracing_subscriber::{EnvFilter, fmt};

pub fn init(debug: bool) -> Result<()> {
    let default_level = if debug { "debug" } else { "info" };
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(format!("floatlyrics={default_level},warn")))?;

    fmt()
        .with_env_filter(filter)
        .compact()
        .try_init()
        .map_err(|err| anyhow::anyhow!("initializing tracing subscriber: {err}"))?;
    Ok(())
}
