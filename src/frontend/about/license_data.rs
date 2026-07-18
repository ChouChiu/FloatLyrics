// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Embedded dependency-license parsing and deterministic merging.

use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct LicenseData {
    pub(super) dependencies: Vec<Dependency>,
    pub(super) licenses: Vec<DependencyLicense>,
}

#[derive(Deserialize)]
pub(super) struct Dependency {
    pub(super) name: String,
    pub(super) version: String,
    pub(super) license: String,
}

#[derive(Deserialize)]
pub(super) struct DependencyLicense {
    pub(super) name: String,
    pub(super) id: String,
    pub(super) text: String,
}

impl LicenseData {
    pub(super) fn embedded() -> Self {
        let mut license_data: Self = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/licenses/dependencies.json"
        )))
        .expect("cargo-about generated valid dependency license data");
        let frontend_license_data: Self = serde_json::from_str(include_str!(concat!(
            env!("OUT_DIR"),
            "/frontend-dependencies.json"
        )))
        .expect("Bun generated valid frontend dependency license data");
        license_data.merge(frontend_license_data);
        license_data
    }

    fn merge(&mut self, other: Self) {
        self.dependencies.extend(other.dependencies);
        self.dependencies.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.version.cmp(&right.version))
        });

        self.licenses.extend(other.licenses);
        self.licenses.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.id.cmp(&right.id))
        });
    }
}

#[cfg(test)]
#[path = "../../test/about_test.rs"]
mod tests;
