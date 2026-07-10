# Repository Guidelines

## Project Structure & Module Organization

FloatLyrics is a single Rust 2024 package for a Linux Wayland desktop app. `src/main.rs` is the entry point, `src/lib.rs` handles startup, and `src/app.rs` is the application composition root. Playback orchestration, presentation state, and GTK rendering live under `src/app/`. Lyrics models, providers, parsing, matching, and timing live under `src/lyrics/`; MPRIS watching and synchronization live under `src/mpris/`. Configuration, paths, SQLite caching, track types, and telemetry are focused top-level modules in `src/`. Tests sit beside their implementations in `#[cfg(test)]` modules; there are no separate `tests/` or asset directories.

## Build, Test, and Development Commands

- `cargo run -- --debug` runs the app with verbose logging; full functionality requires Spotify, MPRIS, Wayland, GTK4, and layer-shell.
- `cargo build` compiles the application package.
- `cargo test` runs all tests; use a module filter such as `cargo test lyrics::` to focus on one area.
- `cargo fmt --all -- --check` verifies formatting without changing files; run `cargo fmt --all` to apply it.
- `cargo clippy --all-targets --all-features -- -D warnings` treats lint findings as errors.

The stable toolchain, `rustfmt`, and Clippy are declared in `rust-toolchain.toml`.

## Coding Style & Naming Conventions

Follow `rustfmt` output (four-space indentation). Use `snake_case` for modules, functions, and variables; `PascalCase` for types and traits; and `SCREAMING_SNAKE_CASE` for constants. Keep `main.rs` minimal, place reusable domain behavior in focused modules such as `lyrics` and `track`, and isolate OS, database, and configuration concerns from GTK presentation.

## Testing Guidelines

Add focused `#[test]` functions to the source module being changed and name them after observable behavior, such as `parses_enhanced_lrc`. Use `tempfile` for filesystem or database isolation. Tests should not require a live Spotify session, D-Bus, or network access. No coverage threshold is configured; protect parsing, timing, persistence, and MPRIS edge cases with regression tests.

## Commit & Pull Request Guidelines

Use Conventional Commits: `<type>(<scope>): <description>`, with a short, imperative, lowercase description. Common types include `feat`, `fix`, `refactor`, `test`, `docs`, and `chore`; useful scopes include `app`, `lyrics`, `mpris`, `infra`, and `ui` (for example, `fix(mpris): handle missing player position`). Mark breaking changes with `!` or a `BREAKING CHANGE:` footer. Keep each commit scoped and explain non-obvious behavior in its body. Pull requests should summarize the change, list verification commands, and link relevant issues. Include screenshots or a short recording for lyrics layout/window changes, and call out new configuration keys, schema changes, or Linux system dependencies.

## Configuration & Generated Files

Do not commit local SQLite files or anything under `target/`. Default user data lives at `~/.config/floatlyrics/config.toml` and `~/.local/share/floatlyrics/floatlyrics.sqlite3`; use `--config <path>` when testing alternate configuration.
