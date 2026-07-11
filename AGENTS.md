# AGENTS.md

## Workspace

Cargo workspace with three crates. Dependency direction:
`floatlyrics` (root) -> `floatlyrics-lyrics` -> `floatlyrics-core`.

- `floatlyrics-core` — paths, i18n, telemetry, track fingerprinting. No GTK or DBus.
- `floatlyrics-lyrics` — lyrics models, parsing, search, timeline, SQLite cache (rusqlite `bundled`).
- `floatlyrics` — CLI, GTK4 layer-shell UI, MPRIS/DBus watcher, app composition root.

Rust edition 2024, MSRV 1.92 (stable, see `rust-toolchain.toml`).

## Commands

All commands use `--locked`:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release
```

Filter tests by module: `cargo test lyrics::`, `cargo test mpris::`, etc.

`make validate-data` is mentioned in README but **does not exist** — no Makefile in repo.

## App ID

`io.github.chouchiu.floatlyrics` — used in `gtk::Application`, desktop file, metainfo, and packaging scripts.

## GTK runtime expectations

`src/lib.rs:54-67` sets `GSK_RENDERER=gl` and `GTK_A11Y=none` at process startup (before GTK init). The app requires a Wayland compositor with layer-shell support and a running D-Bus session bus.

## i18n

Translations are **compiled in** (`floatlyrics-core/src/i18n.rs`), not loaded from external files. Three languages: English, Simplified Chinese, Traditional Chinese. Supported via a const function dispatch (`language.text(key)`) and an `I18n` subscriber pattern for GTK widgets. The `data/locale/` directory exists only for potential future use — it is not currently shipped.

When adding a new user-visible string:
1. Add a variant to `Text` enum.
2. Add entries to `english()`, `simplified_chinese()`, and `traditional_chinese()`.

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
