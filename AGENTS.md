# AGENTS.md — FloatLyrics

This is the repository-wide operating guide for coding agents. It applies to
the whole tree unless a more specific `AGENTS.md` exists below the file being
edited. Keep changes small, preserve existing user work, and prefer repository
code, tests, and CI configuration over assumptions when documentation disagrees.

## Start every task here

Before editing:

1. Run `git status --short` and identify pre-existing changes. They belong to
   the user; do not overwrite, reformat, stage, or revert them.
2. Check for a nearer `AGENTS.md`, then inspect the owning module, its callers,
   adjacent tests, and relevant configuration or generated files.
3. Put the change in the lowest crate that can own the behavior while preserving
   the dependency direction below.
4. Identify compatibility surfaces up front: persisted TOML, SQLite data,
   localized text keys, public library APIs, CLI flags, and packaging metadata.
5. Prefer a focused failing test or baseline check before implementation when
   practical. Finish with validation proportional to the final diff.

Do not run destructive Git commands, bulk-format unrelated files, upgrade
dependencies, or regenerate artifacts unless the task requires it. Do not
commit, push, open a pull request, publish packages, or change a release unless
the user explicitly asks.

## Workspace and dependency direction

FloatLyrics is a Cargo workspace using Rust 2024 with a declared MSRV of Rust
1.93:

```text
floatlyrics (src/)              binary + library: CLI, app assembly, GTK/Relm4, MPRIS
  └─ floatlyrics-lyrics/        lyrics model, LRC/QRC parsing, search, SQLite cache
       └─ floatlyrics-core/     paths, i18n, telemetry, track fingerprinting
```

Dependencies flow only from top to bottom. A lower crate must never import an
application-layer concern from a higher crate.

| Area | Owner | Boundary |
|---|---|---|
| Stable metadata, paths, i18n, fingerprints, telemetry | `floatlyrics-core` | No lyrics, GTK, D-Bus, provider, or SQLite concerns |
| Lyrics models, parsing, filtering, romanization, timeline, provider search, cache | `floatlyrics-lyrics` | No application UI, configuration, or MPRIS concerns |
| CLI, startup, persisted configuration, MPRIS, GTK/Relm4 UI | root crate | May compose both lower crates |

Keep domain decisions outside GTK widgets, D-Bus adapters, provider-specific
HTTP code, and SQLite statements where practical. Provider payloads should be
converted to provider-neutral lyrics types at the adapter boundary. SQL details
must remain behind `LyricsCache`. Keep `src/main.rs` minimal; testable startup
behavior belongs in `src/lib.rs` or a focused module.

## Important paths

| Path | Responsibility |
|---|---|
| `src/lib.rs` | CLI arguments, environment defaults, and application startup |
| `src/app.rs`, `src/app/` | Relm4 composition, controllers, presentation models, views, settings |
| `src/mpris.rs`, `src/mpris/` | player discovery, events, and playback-position synchronization |
| `src/config.rs` | persisted TOML model and atomic writes |
| `floatlyrics-lyrics/src/lyrics/` | provider-neutral models, parsing, filtering, romanization, timeline, search |
| `floatlyrics-lyrics/src/cache.rs`, `cache/` | cache boundary, SQLite access, and schema |
| `floatlyrics-core/src/i18n.rs` | locale selection, typed text keys, catalogue validation |
| `data/locale/` | runtime catalogues for every supported locale |
| `data/licenses/` | cargo-about template and generated dependency notices |
| `.github/workflows/` | CI and release automation; use it as the source of truth for CI commands |
| `packaging/` | install scripts, AUR metadata, and packaging automation |

When adding a module, follow the existing facade-plus-submodule layout rather
than creating a parallel architecture. First-party Rust files and other files
with an established convention should retain the repository's SPDX header.

## Change discipline

- Make the smallest coherent change; avoid drive-by refactors, public API
  expansion, dependency churn, and generated-file noise.
- Preserve existing error and abstraction boundaries. Add context at I/O,
  database, network, process, and D-Bus boundaries; libraries return errors and
  must not choose how the UI displays them.
- Keep behavior changes and their tests in the same change. A bug fix should
  include a regression test that fails without the fix.
- Do not silently weaken validation, discard errors, add broad lint allowances,
  or introduce fallback behavior that hides invalid persisted data.
- If the requested behavior conflicts with an invariant in this guide, surface
  the conflict instead of working around it invisibly.

## Toolchain and commands

`rust-toolchain.toml` selects stable Rust and the `rustfmt`, `clippy`, `rust-src`,
and `rust-analyzer` components. CI runs in an Arch Linux container with GTK4,
gtk4-layer-shell, OpenSSL, and packaging tools. Compiling the root crate locally
requires the corresponding system development libraries.

Use `--locked` on Cargo commands that resolve dependencies. Do not add it to
`cargo fmt`, which does not accept it. Never regenerate `Cargo.lock`
accidentally.

Start with the narrowest useful checks:

```bash
cargo test --locked -p floatlyrics-core
cargo test --locked -p floatlyrics-core i18n
cargo test --locked -p floatlyrics-lyrics
cargo test --locked -p floatlyrics-lyrics parsing::
cargo test --locked -p floatlyrics-lyrics timeline::
cargo test --locked -p floatlyrics-lyrics cache::
cargo test --locked -p floatlyrics controller::
cargo test --locked -p floatlyrics manual_search::
cargo test --locked -p floatlyrics mpris::
cargo test --locked -p floatlyrics config::
cargo clippy --locked -p <package> --all-targets --all-features -- -D warnings
```

Full pre-merge verification:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release
cargo docs
```

The first four commands mirror the Rust quality steps in CI. `cargo docs` and
`cargo docs-open` are repository aliases in `.cargo/config.toml`; they already
use `--locked`, cover the workspace, and deny rustdoc warnings. Documentation
validation is an additional repository requirement.

Choose focused validation according to the diff:

| Change | Minimum focused validation before full checks |
|---|---|
| Rust source | affected tests, `cargo fmt --all -- --check`, affected-package Clippy |
| i18n catalogue or text key | `cargo test --locked -p floatlyrics-core i18n` |
| configuration | root config tests, including load/save and old on-disk representations |
| parser, timeline, romanization, search | corresponding `floatlyrics-lyrics` test module |
| cache or schema | cache tests covering migration, stored representation, and read-back |
| MPRIS, controller, or UI model | corresponding root-crate test module |
| dependency or feature | full Clippy/test/build plus license regeneration and freshness diff |
| Rust API documentation | `cargo docs` |
| Markdown or metadata only | inspect rendered structure, links, commands, and the exact diff |
| packaging or workflow | inspect the script/workflow and run its safe validation path when available |

Do not claim checks that were not run. If system libraries, network, Wayland, or
another environment constraint prevents a check, report the exact skipped
command and reason.

Run the application only in a suitable Wayland session:

```bash
cargo run --locked -- --debug
```

`--debug` enables verbose tracing; it does not select Cargo's debug profile.

## Rust and public API conventions

- Use default `rustfmt` and standard Rust naming conventions.
- Prefer explicit domain types and small pure functions over UI-, transport-,
  or persistence-coupled helpers.
- Use checked conversions for timestamps, durations, database integers, and
  external values where truncation or sign changes are possible.
- Public items in `floatlyrics-core` and `floatlyrics-lyrics` require useful
  rustdoc; both crates enable `missing_docs` warnings. Document error behavior
  and invariants, not just signatures.
- Do not add `unsafe` unless the task requires it and the safety invariant is
  documented next to the block.
- Fix Clippy findings at their cause. Use only a narrow, locally justified lint
  allowance when no clearer implementation exists.

## Localization invariant

Every user-visible string must go through the localization layer. A new or
renamed string requires one atomic change containing all of:

1. the key in `data/locale/en.json`;
2. the key in `data/locale/zh-CN.json`;
3. the key in `data/locale/zh-TW.json`;
4. the key in `define_text_keys!` in `floatlyrics-core/src/i18n.rs`;
5. updated tests when lookup, interpolation, or locale selection changes.

Keep catalogue key sets identical. Do not hard-code fallback UI text in GTK
views or business logic, and do not use translated display text as a stable
identifier. Startup calls `i18n::validate_catalogues()` and treats missing or
invalid catalogues as an error. `FLOATLYRICS_LOCALE_DIR` overrides catalogue
discovery and is the preferred way to isolate catalogue tests.

## Configuration and persistence

- Config structs use `#[serde(deny_unknown_fields)]`; misspelled or obsolete
  keys fail startup rather than being ignored.
- Treat field renames/removals, default changes, type changes, and serialized
  enum value changes as compatibility changes. Add deliberate migration or
  compatibility behavior and test an existing TOML representation.
- Preserve the temporary-file-plus-rename atomic write path in `src/config.rs`,
  including cleanup on failure.
- Validate user-controlled numeric ranges at the existing configuration/domain
  boundary; do not rely on GTK widgets as the sole validation layer.
- Filesystem and database tests must use `tempfile` or in-memory SQLite; never
  use developer-local paths or shared user state.
- Schema changes must be safe for an existing database. Test both a pre-change
  representation/migration path and post-migration reads and writes. Do not make
  cache internals leak into lyrics-domain or application APIs.

## MPRIS, async, network, and UI boundaries

- Unit tests must not require Spotify, a live D-Bus session, network access,
  Wayland, or a running compositor. Put external interactions behind the
  existing boundaries and test state transitions with deterministic inputs.
- Keep playback-position and lyrics-timeline calculations deterministic. When
  changing them, cover boundary timestamps, offsets, paused state, seeks,
  track changes, and missing metadata as applicable.
- Tag asynchronous lyrics/search/romanization results with track identity or a
  generation token. Ignore stale results after a track or query change.
- Keep blocking filesystem, SQLite, CPU-heavy romanization, and network work off
  the GTK update path. Do not hold `RefCell` borrows or UI state across an
  `.await` point.
- Provider failures remain recoverable unless the existing API documents
  otherwise. Preserve configured provider order, result deduplication, and
  manual-selection precedence when changing search or cache behavior.
- Relm4 initializes GTK; do not call `gtk::init()`.
- The application is a Wayland layer-shell overlay and is not expected to run on
  X11 or without compositor layer-shell support.
- Before GTK initialization, `src/lib.rs` supplies default `GSK_RENDERER=gl` and
  `GTK_A11Y=none` values when unset. Preserve caller-provided values.

## Tests

Tests live in each owning crate's `src/test/` directory and are dispatched from
the owning module with:

```rust
#[cfg(test)]
#[path = "test/foo_test.rs"]
mod tests;
```

Nested modules use the appropriate relative path, as existing modules show.

- Name tests after observable behavior, for example `parses_enhanced_lrc`.
- Prefer one clear arrange/act/assert path and deterministic inputs.
- Avoid timing-sensitive sleeps, real wall-clock assumptions, execution-order
  dependencies, global mutable state, ambient locale, and shared filesystem
  state.
- Test error and boundary paths as well as the happy path when changing parsing,
  persistence, external metadata, or asynchronous state handling.
- Prefer the lowest owning layer. UI tests should verify presentation state and
  wiring rather than repeat domain algorithms.

## Dependencies and generated license data

Do not add a crate when the standard library or an existing dependency is
adequate. Keep shared versions in `[workspace.dependencies]` when they are used
across crates, and do not enable broader features than needed.

If `Cargo.toml` or `Cargo.lock` changes, regenerate the embedded license data
using the version used by CI:

```bash
cargo install --locked --features cli --version 0.9.1 cargo-about
cargo about generate --locked --all-features data/licenses/about.hbs \
  --output-file data/licenses/dependencies.json
git diff -- data/licenses/dependencies.json
```

Review and include the generated diff. CI regenerates this file from a clean
checkout and runs `git diff --exit-code` as its freshness check. Never hand-edit
`data/licenses/dependencies.json`; commit it together with `Cargo.lock` when it
changes.

## Generated and release-sensitive files

- Do not edit build output under `target/`.
- Keep AUR metadata under `packaging/aur/<package>/`; never add `PKGBUILD` or
  `.SRCINFO` at the repository root.
- Treat `.SRCINFO` as generated from its matching `PKGBUILD`; keep the pair in
  sync and use repository packaging scripts rather than hand-copying metadata.
- Do not change package metadata, AUR files, workflow action versions, release
  workflows, or version numbers as a side effect of unrelated work.
- When user-visible packaging assets change, check `Cargo.toml` package asset
  lists, desktop metadata, metainfo, install scripts, and AUR files for the same
  path and version assumptions.
- Follow `CONTRIBUTING.md` for maintainer-only AUR and release procedures. Never
  run a publishing path without explicit authorization.

## Commit and handoff

When asked to commit, use Conventional Commits:

```text
<type>(<scope>): <lowercase imperative description>
```

Common types are `feat`, `fix`, `refactor`, `test`, `docs`, and `chore`; common
scopes are `app`, `lyrics`, `mpris`, `infra`, and `ui`. Stage only files in the
approved task scope and review the staged diff before committing.

The final handoff must:

- summarize observable behavior changed, not merely list edited files;
- list validation commands actually run and their results;
- call out skipped checks, compatibility considerations, and remaining risks;
- state whether a UI change was exercised in a real layer-shell Wayland session;
- avoid implying that pre-existing user changes were part of the work.
