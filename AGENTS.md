# AGENTS.md — FloatLyrics

This file is the repository-wide operating guide for coding agents. Keep changes
small, preserve existing user work, and follow more specific instructions if a
nested `AGENTS.md` is added later.

## Project shape

FloatLyrics is a Cargo workspace using Rust 2024 with MSRV 1.93:

```text
floatlyrics (src/)              binary + library: CLI, app assembly, GTK/Relm4, MPRIS
  └─ floatlyrics-lyrics/        lyrics model, LRC/QRC parsing, search, SQLite cache
       └─ floatlyrics-core/     paths, i18n, telemetry, track fingerprinting
```

Dependencies flow only from top to bottom. Put logic in the lowest crate that can
own it without importing a higher-level boundary:

| Area | Owner | Must not depend on |
|---|---|---|
| Stable metadata, paths, i18n, fingerprints | `floatlyrics-core` | lyrics, GTK, D-Bus |
| Lyrics parsing, timing, search, cache | `floatlyrics-lyrics` | application UI, MPRIS |
| CLI, configuration, MPRIS, GTK/Relm4 UI | root crate | — |

Keep domain decisions outside GTK widgets, D-Bus adapters, HTTP providers, and
SQLite implementations where practical. Keep `src/main.rs` minimal; testable
startup behavior belongs in `src/lib.rs` or a focused module.

## Important paths

| Path | Responsibility |
|---|---|
| `src/app.rs`, `src/app/` | Relm4 application, controllers, view models, views, settings |
| `src/mpris.rs`, `src/mpris/` | player discovery, events, playback-position synchronization |
| `src/config.rs` | persisted TOML model and atomic writes |
| `floatlyrics-lyrics/src/lyrics/` | provider-neutral models, parsing, filtering, timeline, search |
| `floatlyrics-lyrics/src/cache.rs`, `cache/` | cache boundary and SQLite schema |
| `floatlyrics-core/src/i18n.rs` | locale selection, typed text keys, catalogue validation |
| `data/locale/` | runtime catalogues for all supported locales |
| `data/licenses/` | cargo-about template and generated dependency notices |
| `packaging/` | packaging and AUR release automation |

## Working rules

1. Inspect the relevant module, its tests, and the current working tree before
   editing. Existing unrelated changes belong to the user; do not overwrite,
   reformat, or revert them.
2. Make the smallest coherent change. Avoid drive-by refactors, dependency
   upgrades, generated-file churn, and public API expansion unless required.
3. Preserve the workspace dependency direction and existing module layout.
4. Add or update tests for behavior changes and bug fixes in the crate that owns
   the behavior.
5. Run the narrowest useful checks while iterating, then the checks appropriate
   to the final diff. Report commands that were not run and why.

Use `cargo` with `--locked` whenever the subcommand accepts it. Never regenerate
`Cargo.lock` accidentally. Formatting is the exception because `cargo fmt` does
not resolve dependencies.

## Toolchain and required commands

`rust-toolchain.toml` selects stable Rust and the `rustfmt`, `clippy`, `rust-src`,
and `rust-analyzer` components. CI builds in an Arch Linux container with GTK4,
gtk4-layer-shell, OpenSSL, and packaging tools installed. Compiling the root crate
locally requires the corresponding system development libraries.

Full pre-merge verification:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release
cargo docs
```

The first four commands mirror the Rust quality steps in CI. Documentation
validation is an additional repository requirement.

`cargo docs` and `cargo docs-open` are repository aliases defined in
`.cargo/config.toml`; they already use `--locked`, cover the workspace, and deny
rustdoc warnings.

During development, prefer targeted commands first:

```bash
cargo test --locked -p floatlyrics-core
cargo test --locked -p floatlyrics-lyrics
cargo test --locked -p floatlyrics lyrics::
cargo test --locked -p floatlyrics mpris::
cargo clippy --locked -p <package> --all-targets --all-features -- -D warnings
```

Choose validation according to the diff:

| Change | Minimum focused validation before full checks |
|---|---|
| Rust source | affected tests, `cargo fmt`, affected-package Clippy |
| i18n catalogue or text key | `cargo test --locked -p floatlyrics-core i18n` |
| configuration | root config tests; include load/save and compatibility cases |
| parser/timeline/cache | corresponding `floatlyrics-lyrics` test module |
| MPRIS/controller/UI model | corresponding root-crate test module |
| dependency or feature | full Clippy/test/build plus license regeneration |
| documentation only | link/code-block review; run docs for Rust API docs |

Run the application only in a suitable Wayland session:

```bash
cargo run --locked -- --debug
```

`--debug` enables verbose tracing; it does not select Cargo's debug profile.

## Rust and API conventions

- Use default `rustfmt`; follow standard Rust naming conventions.
- Prefer explicit domain types and small pure functions over UI- or transport-
  coupled helpers.
- Add context at I/O and adapter boundaries. Libraries should return errors and
  must not decide how the UI presents them.
- Public items in `floatlyrics-core` and `floatlyrics-lyrics` require useful
  rustdoc; both crates enable `missing_docs` warnings.
- Do not add `unsafe` unless the task requires it and the safety invariant is
  documented next to the block.
- Do not suppress Clippy warnings broadly. Fix the cause or use the narrowest
  justified allowance.

## Localization invariant

Every user-visible string must go through the localization layer. A new or
renamed string requires one atomic change containing all of:

1. the key in `data/locale/en.json`;
2. the key in `data/locale/zh-CN.json`;
3. the key in `data/locale/zh-TW.json`;
4. the key in `define_text_keys!` in `floatlyrics-core/src/i18n.rs`;
5. updated tests when lookup or interpolation behavior changes.

Do not hard-code fallback UI text in GTK views or business logic. Keep catalogue
key sets identical; startup calls `i18n::validate_catalogues()` and treats missing
keys as an error. `FLOATLYRICS_LOCALE_DIR` overrides catalogue discovery and is
the preferred way to isolate catalogue tests.

## Configuration and persistence

- Config structs use `#[serde(deny_unknown_fields)]`; misspelled or obsolete keys
  fail startup instead of being ignored.
- Treat renaming/removing fields or changing serialized enum values as a
  compatibility change. Add migration or compatibility behavior deliberately
  and test an existing on-disk representation.
- Preserve the atomic temporary-file-plus-rename write path in `src/config.rs`.
- Filesystem and database tests must use `tempfile`; never use developer-local
  paths or shared user state.
- Schema/cache changes require tests for both stored representation and read-back
  behavior. Do not make cache internals leak into lyrics-domain APIs.

## MPRIS, async, and UI boundaries

- Unit tests must not require Spotify, a live D-Bus session, network access,
  Wayland, or a running compositor. Put external interactions behind the existing
  boundaries and test state transitions with deterministic inputs.
- Keep playback-position and lyrics-timeline calculations deterministic. Test
  boundary timestamps, offsets, paused state, seeks, and missing metadata when
  those behaviors change.
- Relm4 initializes GTK. Do not call `gtk::init()`.
- The application is a Wayland layer-shell overlay and is not expected to run on
  X11 or without compositor layer-shell support.
- Before GTK initialization, `src/lib.rs` supplies default `GSK_RENDERER=gl` and
  `GTK_A11Y=none` values when unset. Preserve caller-provided environment values.
- Keep blocking filesystem, database, and network work off the GTK update path.

## Tests

Tests live in each crate's `src/test/` directory and are dispatched from the
owning module with:

```rust
#[cfg(test)]
#[path = "test/foo_test.rs"]
mod tests;
```

- Name tests after observable behavior, for example `parses_enhanced_lrc`.
- A bug fix should include a regression test that fails without the fix.
- Avoid timing-sensitive sleeps, execution-order assumptions, and ambient state.
- Prefer testing the lowest owning layer; UI tests should focus on presentation
  state and wiring rather than re-testing domain algorithms.

## Dependencies and third-party licenses

Do not add a crate when the standard library or an existing dependency is
adequate. If `Cargo.toml` or `Cargo.lock` changes, regenerate the embedded license
data using the version used by CI:

```bash
cargo install --locked --features cli --version 0.9.1 cargo-about
cargo about generate --locked --all-features data/licenses/about.hbs \
  --output-file data/licenses/dependencies.json
git diff -- data/licenses/dependencies.json
```

Review and include the generated diff. CI regenerates this file from a clean
checkout and applies `git diff --exit-code` as its freshness check. Do not
hand-edit `data/licenses/dependencies.json`; commit it together with `Cargo.lock`.

## Generated and release-sensitive files

- Do not edit build output under `target/`.
- Do not change package metadata, AUR files, release workflows, or version numbers
  as a side effect of unrelated work.
- When user-visible packaging assets change, check the corresponding entries in
  `Cargo.toml`, desktop metadata, and packaging scripts for consistency.
- Follow `CONTRIBUTING.md` for maintainer-only AUR and release procedures.

## Commit and handoff

Use Conventional Commits when asked to commit:

```text
<type>(<scope>): <lowercase imperative description>
```

Common types are `feat`, `fix`, `refactor`, `test`, `docs`, and `chore`; common
scopes are `app`, `lyrics`, `mpris`, `infra`, and `ui`. Do not create commits,
push branches, or modify releases unless explicitly requested.

In the final handoff, summarize the behavior changed, list validation actually
run, and call out remaining risks or environment-dependent checks. For UI changes,
state whether they were exercised in a real layer-shell Wayland session.
