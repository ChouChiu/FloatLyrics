// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Runtime JSON catalogue discovery, validation, and cached lookup.

use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use super::{Language, Text};

const LOCALE_DIR_ENV: &str = "FLOATLYRICS_LOCALE_DIR";
const XDG_DATA_DIRS_ENV: &str = "XDG_DATA_DIRS";

#[derive(Debug)]
struct Catalog {
    entries: HashMap<String, String>,
}

impl Catalog {
    fn load(language: Language) -> Self {
        locale_directories()
            .into_iter()
            .find_map(|directory| Self::load_file(&directory, language))
            .unwrap_or_else(|| panic!("{} locale catalogue was not validated", language.code()))
    }

    fn load_file(directory: &Path, language: Language) -> Option<Self> {
        let content =
            fs::read_to_string(directory.join(format!("{}.json", language.code()))).ok()?;
        let entries: HashMap<String, String> = serde_json::from_str(&content).ok()?;
        Text::ALL
            .iter()
            .all(|key| entries.contains_key(key.key()))
            .then_some(Self { entries })
    }

    fn text(&self, key: Text) -> &str {
        self.entries
            .get(key.key())
            .expect("validated locale catalogue is missing a declared key")
    }
}

pub(super) fn text(language: Language, key: Text) -> &'static str {
    static ENGLISH: OnceLock<Catalog> = OnceLock::new();
    static SIMPLIFIED_CHINESE: OnceLock<Catalog> = OnceLock::new();
    static TRADITIONAL_CHINESE: OnceLock<Catalog> = OnceLock::new();

    let catalog = match language {
        Language::English => ENGLISH.get_or_init(|| Catalog::load(Language::English)),
        Language::SimplifiedChinese => {
            SIMPLIFIED_CHINESE.get_or_init(|| Catalog::load(Language::SimplifiedChinese))
        }
        Language::TraditionalChinese => {
            TRADITIONAL_CHINESE.get_or_init(|| Catalog::load(Language::TraditionalChinese))
        }
    };
    catalog.text(key)
}

fn locale_directories() -> Vec<PathBuf> {
    let mut directories = env::var_os(LOCALE_DIR_ENV)
        .map(PathBuf::from)
        .into_iter()
        .collect::<Vec<_>>();
    directories.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/locale"));

    let data_dirs = env::var_os(XDG_DATA_DIRS_ENV)
        .filter(|value| !value.is_empty())
        .map(|value| env::split_paths(&value).collect::<Vec<_>>())
        .unwrap_or_else(|| {
            vec![
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share"),
            ]
        });
    directories.extend(
        data_dirs
            .into_iter()
            .map(|directory| directory.join("floatlyrics/locale")),
    );
    directories
}

pub(super) fn validate_catalogues() -> anyhow::Result<()> {
    let directories = locale_directories();
    for language in Language::ALL {
        if !directories
            .iter()
            .any(|directory| Catalog::load_file(directory, language).is_some())
        {
            let searched = directories
                .iter()
                .map(|directory| directory.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            anyhow::bail!(
                "could not load complete {} locale catalogue; searched: {searched}",
                language.code()
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../test/catalogue_test.rs"]
mod tests;
