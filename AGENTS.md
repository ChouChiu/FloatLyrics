# AGENTS.md

## Workspace

Cargo workspace with three crates. Dependency direction:
`floatlyrics` (root) -> `floatlyrics-lyrics` -> `floatlyrics-core`.

- `floatlyrics-core` — paths, i18n, telemetry, track fingerprinting. No GTK or DBus.
- `floatlyrics-lyrics` — lyrics models, parsing, search, timeline, SQLite cache (rusqlite `bundled`).
- `floatlyrics` — CLI, GTK4 layer-shell UI, MPRIS/DBus watcher, app composition root.

Rust edition 2024, MSRV 1.93 (stable, see `rust-toolchain.toml`).

## Commands

All commands use `--locked`:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release
```

Filter tests by module: `cargo test lyrics::`, `cargo test mpris::`, `cargo test cache::`, etc.

`cargo test --locked` (without `--all-targets` / `--all-features`) also works for focused iterations; CI uses the full flags.

## Crate alias note

The `gtk` crate in `Cargo.toml` is aliased from `gtk4` (`package = "gtk4"`). Code uses `use gtk::...` but it resolves to the GTK4 Rust bindings (v0.11, requiring GTK 4.12+ system library). Relm4 v0.11 drives the UI.

## App ID

`io.github.chouchiu.floatlyrics` — used in `gtk::Application`, desktop file, metainfo, and packaging scripts.

## GTK runtime expectations

`src/lib.rs:54-67` sets `GSK_RENDERER=gl` and `GTK_A11Y=none` at process startup (before GTK init). The app requires a Wayland compositor with layer-shell support and a running D-Bus session bus.

## i18n

Translations are loaded at runtime from JSON files in `data/locale/` and shipped to
`/usr/share/floatlyrics/locale/`. Three languages: English, Simplified Chinese,
Traditional Chinese. `FLOATLYRICS_LOCALE_DIR` overrides the resource directory.
`Language::text(key)` uses a process-wide lazy cache, and the `I18n` subscriber
pattern updates GTK widgets when the active language changes.

When adding a new user-visible string:
1. Add a variant to the `define_text_keys!` invocation.
2. Add the same key to `data/locale/en.json`, `zh-CN.json`, and `zh-TW.json`.

## SPDX headers

Every source file must start with:
```rust
// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later
```
`.toml` files use `#` instead of `//`.

## Build script

`build.rs` generates `dep_list.rs` in `OUT_DIR` from `Cargo.toml` + `Cargo.lock` + `cargo metadata`. Used by the about dialog to list open-source dependencies. Re-runs when `Cargo.toml`, `Cargo.lock`, or `build.rs` change.

## Testing rules

- Tests are `#[cfg(test)]` modules pointing to `test/` subdirectories via `#[path = "test/xxx_test.rs"]`.
- Unit tests **must not** depend on a running Spotify instance, D-Bus session, or network.
- Use `tempfile` for filesystem/database isolation.

## Config

`src/config.rs` — written atomically (temp file + rename). Config path: `~/.config/floatlyrics/config.toml`. Database: `~/.local/share/floatlyrics/floatlyrics.sqlite3`.

## Commits

Conventional Commits: `<type>(<scope>): <description>`. Common scopes: `app`, `lyrics`, `mpris`, `infra`, `ui`.

## CI

- `build.yml` runs on every push/PR in `archlinux:latest`. Builds .deb and .rpm artifacts.
- `release.yml` triggers on tags `v*.*.*`, runs on `ubuntu-24.04`, validates tag matches Cargo.toml version.
- Both workflows cache `~/.cargo/registry`, `~/.cargo/git`, and `target`.
