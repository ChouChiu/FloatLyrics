// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{env, path::Path, process::Command};

fn main() {
    for path in [
        "package.json",
        "bun.lock",
        "tsconfig.json",
        "src/frontend/view/lyrics",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("Cargo provides CARGO_MANIFEST_DIR");
    run_bun(
        &manifest_dir,
        &["install", "--frozen-lockfile"],
        "install locked frontend dependencies",
    );

    let out_dir = env::var("OUT_DIR").expect("Cargo provides OUT_DIR");
    let status = Command::new("bun")
        .args(["run", "build:lyrics"])
        .current_dir(&manifest_dir)
        .env("FLOATLYRICS_LYRICS_OUT_DIR", &out_dir)
        .status()
        .unwrap_or_else(|error| {
            panic!(
                "failed to start Bun while building the React lyrics view: {error}; install Bun 1.3.14"
            )
        });
    assert!(
        status.success(),
        "Bun failed to build the React lyrics view"
    );

    for file in ["lyrics.html", "frontend-dependencies.json"] {
        assert!(
            Path::new(&out_dir).join(file).is_file(),
            "Bun did not generate {file} in Cargo OUT_DIR"
        );
    }
}

fn run_bun(manifest_dir: &str, args: &[&str], action: &str) {
    let status = Command::new("bun")
        .args(args)
        .current_dir(manifest_dir)
        .status()
        .unwrap_or_else(|error| {
            panic!("failed to start Bun to {action}: {error}; install Bun 1.3.14")
        });
    assert!(status.success(), "Bun failed to {action}");
}
