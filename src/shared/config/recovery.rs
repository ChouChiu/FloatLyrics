// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Field-level recovery and backup for incompatible persisted configuration.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;

use super::{AppConfig, ConfigLimits, WindowPosition, normalized_hex_color, validation::Numeric};

pub(super) fn recover_fields(content: &str) -> AppConfig {
    let value = match toml::from_str::<toml::Value>(content) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "configuration syntax cannot be recovered field by field");
            return AppConfig::default();
        }
    };
    let mut config = AppConfig::default();

    apply(
        &value,
        "general",
        "language",
        &mut config.general.language,
        |_| true,
    );
    apply(&value, "general", "mode", &mut config.general.mode, |_| {
        true
    });

    apply(
        &value,
        "window",
        "anchor",
        &mut config.window.anchor,
        |_| true,
    );
    apply(
        &value,
        "window",
        "remember_position",
        &mut config.window.remember_position,
        |_| true,
    );
    apply(
        &value,
        "window",
        "position",
        &mut config.window.position,
        valid_position,
    );
    apply_ranged(&value, "window", "margin", &mut config.window.margin);
    apply_ranged(&value, "window", "width", &mut config.window.width);
    apply_ranged(&value, "window", "opacity", &mut config.window.opacity);
    apply_ranged(
        &value,
        "window",
        "bottom_panel_height",
        &mut config.window.bottom_panel_height,
    );

    apply(
        &value,
        "lyrics",
        "apple_music_style",
        &mut config.lyrics.apple_music_style,
        |_| true,
    );
    apply_ranged(&value, "lyrics", "offset_ms", &mut config.lyrics.offset_ms);
    apply(
        &value,
        "lyrics",
        "provider_order",
        &mut config.lyrics.provider_order,
        |_| true,
    );
    apply(
        &value,
        "lyrics",
        "show_translation",
        &mut config.lyrics.show_translation,
        |_| true,
    );
    apply(
        &value,
        "lyrics",
        "show_romanization",
        &mut config.lyrics.show_romanization,
        |_| true,
    );
    apply(
        &value,
        "lyrics",
        "chinese_romanization",
        &mut config.lyrics.chinese_romanization,
        |_| true,
    );
    apply(
        &value,
        "lyrics",
        "font_order",
        &mut config.lyrics.font_order,
        |fonts| !fonts.is_empty() && fonts.iter().all(|font| !font.trim().is_empty()),
    );
    apply_ranged(
        &value,
        "lyrics",
        "lyric_font_size",
        &mut config.lyrics.lyric_font_size,
    );
    apply_ranged(
        &value,
        "lyrics",
        "translation_font_size",
        &mut config.lyrics.translation_font_size,
    );
    apply_ranged(
        &value,
        "lyrics",
        "romanization_font_size",
        &mut config.lyrics.romanization_font_size,
    );
    apply_ranged(
        &value,
        "lyrics",
        "transition_ms",
        &mut config.lyrics.transition_ms,
    );
    apply_ranged(
        &value,
        "lyrics",
        "word_fade_width",
        &mut config.lyrics.word_fade_width,
    );
    apply_ranged(
        &value,
        "lyrics",
        "track_offset_step_ms",
        &mut config.lyrics.track_offset_step_ms,
    );
    apply_color(&value, "played_color", &mut config.lyrics.played_color);
    apply_color(&value, "unplayed_color", &mut config.lyrics.unplayed_color);
    apply_color(
        &value,
        "translation_color",
        &mut config.lyrics.translation_color,
    );
    apply_color(
        &value,
        "romanization_color",
        &mut config.lyrics.romanization_color,
    );

    apply(
        &value,
        "player",
        "preferred_players",
        &mut config.player.preferred_players,
        |selectors| valid_player_selectors(selectors),
    );
    apply(
        &value,
        "player",
        "ignored_players",
        &mut config.player.ignored_players,
        |selectors| valid_player_selectors(selectors),
    );
    if let Some(prefix) = value
        .get("spotify")
        .and_then(|section| section.get("mpris_prefix"))
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|prefix| !prefix.is_empty())
    {
        config.player.preferred_players = vec![prefix.to_string()];
    }

    apply(
        &value,
        "amll",
        "address",
        &mut config.amll.address,
        |address| super::validation::validate_listener_address("amll.address", address).is_ok(),
    );
    apply(&value, "tray", "enabled", &mut config.tray.enabled, |_| {
        true
    });

    config
}

pub(super) fn backup(path: &Path, content: &[u8]) -> Result<PathBuf> {
    let file_name = path
        .file_name()
        .context("config path must point to a file")?
        .to_string_lossy();
    for index in 0_u64.. {
        let suffix = if index == 0 {
            ".incompatible".to_string()
        } else {
            format!(".incompatible.{index}")
        };
        let backup = path.with_file_name(format!("{file_name}{suffix}"));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup)
        {
            Ok(mut file) => {
                if let Err(error) = file
                    .write_all(content)
                    .with_context(|| format!("writing config backup {}", backup.display()))
                    .and_then(|()| {
                        file.sync_all()
                            .with_context(|| format!("syncing config backup {}", backup.display()))
                    })
                {
                    let _ = fs::remove_file(&backup);
                    return Err(error);
                }
                return Ok(backup);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("creating config backup {}", backup.display()));
            }
        }
    }
    unreachable!("configuration backup suffix space is inexhaustible")
}

fn apply<T>(
    root: &toml::Value,
    section: &str,
    name: &str,
    target: &mut T,
    validate: impl FnOnce(&T) -> bool,
) where
    T: DeserializeOwned,
{
    let Some(value) = root.get(section).and_then(|section| section.get(name)) else {
        return;
    };
    let field = format!("{section}.{name}");
    match value.clone().try_into::<T>() {
        Ok(candidate) if validate(&candidate) => *target = candidate,
        Ok(_) => tracing::warn!(%field, "invalid config field fell back to its default"),
        Err(error) => {
            tracing::warn!(%field, %error, "incompatible config field fell back to its default")
        }
    }
}

fn apply_ranged<T>(root: &toml::Value, section: &str, name: &str, target: &mut T)
where
    T: DeserializeOwned + Numeric,
{
    let field = format!("{section}.{name}");
    apply(root, section, name, target, |value| {
        ConfigLimits::contains(&field, *value)
    });
}

fn apply_color(root: &toml::Value, name: &str, target: &mut String) {
    apply(root, "lyrics", name, target, |color| {
        normalized_hex_color(color).is_some()
    });
}

fn valid_position(position: &Option<WindowPosition>) -> bool {
    position.is_none_or(|position| {
        position.horizontal.is_finite()
            && (0.0..=1.0).contains(&position.horizontal)
            && position.vertical.is_finite()
            && (0.0..=1.0).contains(&position.vertical)
    })
}

fn valid_player_selectors(selectors: &[String]) -> bool {
    selectors.iter().all(|selector| !selector.trim().is_empty())
}
