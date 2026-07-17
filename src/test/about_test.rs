// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

use super::*;

#[test]
fn merges_rust_and_frontend_license_data_in_name_order() {
    let mut rust = LicenseData {
        dependencies: vec![dependency("zbus", "5.0.0"), dependency("serde", "1.0.0")],
        licenses: vec![dependency_license("zbus"), dependency_license("serde")],
    };
    let frontend = LicenseData {
        dependencies: vec![
            dependency("react", "19.0.0"),
            dependency("@pixi/app", "7.0.0"),
        ],
        licenses: vec![dependency_license("react"), dependency_license("@pixi/app")],
    };

    rust.merge(frontend);

    assert_eq!(
        rust.dependencies
            .iter()
            .map(|dependency| dependency.name.as_str())
            .collect::<Vec<_>>(),
        ["@pixi/app", "react", "serde", "zbus"]
    );
    assert_eq!(
        rust.licenses
            .iter()
            .map(|license| license.name.as_str())
            .collect::<Vec<_>>(),
        ["@pixi/app", "react", "serde", "zbus"]
    );
}

fn dependency(name: &str, version: &str) -> Dependency {
    Dependency {
        name: name.into(),
        version: version.into(),
        license: "MIT".into(),
    }
}

fn dependency_license(name: &str) -> DependencyLicense {
    DependencyLicense {
        name: name.into(),
        id: "MIT".into(),
        text: "MIT license text".into(),
    }
}
