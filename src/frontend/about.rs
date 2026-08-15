// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Dependency notices supplied to the React about page.

mod license_data;

use license_data::LicenseData;

pub(super) fn license_data_json() -> serde_json::Value {
    serde_json::to_value(LicenseData::embedded())
        .expect("embedded dependency license data is serializable")
}
