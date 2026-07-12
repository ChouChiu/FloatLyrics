# AGENTS.md — FloatLyrics

## Workspace

Cargo workspace of three crates (edition 2024, MSRV 1.93):

```
floatlyrics (src/)          ← binary + lib: CLI, Relm4/GTK4 layer-shell UI, MPRIS
  └─ floatlyrics-lyrics     ← lyrics model, LRC/QRC parsing, search, SQLite cache
       └─ floatlyrics-core  ← paths, i18n, telemetry, track fingerprinting
```

Dependency direction is top→bottom. Domain logic goes above the GTK/D-Bus/DB boundary.

## Commands (always use `--locked`)

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release
```

Filter tests: `cargo test lyrics::`, `cargo test mpris::`, etc.

Docs (warnings are errors): `cargo docs` / `cargo docs-open` (aliases in `.cargo/config.toml`).

Run locally: `cargo run -- --debug`

## i18n — three-locale invariant

Every user-visible string must exist in all three `data/locale/{en,zh-CN,zh-TW}.json` files. When adding a new key, also add it to the `define_text_keys!` macro in `floatlyrics-core/src/i18n.rs`. Never bypass the localization layer.

At startup, `i18n::validate_catalogues()` checks that all keys are present in all locales. Tests for i18n live in `floatlyrics-core/src/test/i18n_test.rs`.

## Tests

- Tests are `#[cfg(test)]` modules dispatched via `#[path = "test/foo_test.rs"]` to `src/test/` (binary crate) or `src/test/` (sub-crates).
- Unit tests must not depend on running Spotify, D-Bus, network, or developer-local paths.
- File system / database tests use `tempfile` for isolation.
- Test names describe observable behavior (e.g. `parses_enhanced_lrc`).

## Commit style

Conventional Commits: `<type>(<scope>): <description>` — lowercase imperative English.

Common types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`.
Common scopes: `app`, `lyrics`, `mpris`, `infra`, `ui`.

## build.rs

Generates `dep_list.rs` into `OUT_DIR` by running `cargo metadata --locked`. Consumed by About/Acknowledgements page. Touching `Cargo.toml`, `Cargo.lock`, or `build.rs` triggers re-run.

## Toolchain

`rust-toolchain.toml` pins stable with `rustfmt`, `clippy`, `rust-src`, `rust-analyzer`. CI runs in an Arch Linux container (GTK4, gtk4-layer-shell, OpenSSL system deps required).

## Quirks

- `--debug` CLI flag enables verbose tracing (not just debug builds).
- Config writes use atomic temp-file + rename in `src/config.rs`.
- `gtk::init()` is NOT called — Relm4 manages GTK init internally.
- The binary is a Wayland layer-shell overlay; it cannot run under X11 or without a compositor supporting `layer-shell`.
