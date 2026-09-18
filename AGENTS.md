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
floatlyrics (src/)              binary + library: CLI and application layers
  ├─ frontend/                  GTK/Relm4/WebKit views and UI adapters
  ├─ backend/                   playback, lyrics orchestration, cache, and MPRIS
  ├─ shared/                    configuration and cross-layer data contracts
  └─ floatlyrics-lyrics/        lyrics model, LRC/QRC parsing, search, SQLite cache
       └─ floatlyrics-core/     paths, i18n, telemetry, track fingerprinting
```

Dependencies flow only from top to bottom. A lower crate must never import an
application-layer concern from a higher crate.

| Area | Owner | Boundary |
|---|---|---|
| Stable metadata, paths, i18n, fingerprints, telemetry | `floatlyrics-core` | No lyrics, GTK, D-Bus, provider, or SQLite concerns |
|Lyrics models, parsing, filtering, romanization, karaoke word segmentation, timeline, provider search, cache|`floatlyrics-lyrics`|No application UI, configuration, or MPRIS concerns|
| CLI, startup, persisted configuration, MPRIS, GTK/Relm4 UI | root crate | May compose both lower crates |

`floatlyrics-lyrics` depends on the external [`lyrics-helper`]
crate and re-exports its `LineInfo`, `LyricsData`, and `LyricsTypes` types as
part of the public API. Application code should import these through
`floatlyrics_lyrics::lyrics` rather than depending on `lyrics-helper` directly.

Inside the root crate, dependencies flow from `frontend` to `backend` to
`shared`; `frontend` may also consume `shared` contracts directly. Backend
modules must not import GTK, Relm4, WebKit, or frontend messages. Shared modules
must not import either application layer.

Keep domain decisions outside GTK widgets, D-Bus adapters, provider-specific
HTTP code, and SQLite statements where practical. Provider payloads should be
converted to provider-neutral lyrics types at the adapter boundary. SQL details
must remain behind `LyricsCache`. Keep `src/main.rs` minimal; testable startup
behavior belongs in `src/lib.rs` or a focused module.

## Important paths

| Path | Responsibility |
|---|---|
| `src/lib.rs` | CLI arguments, environment defaults, and application startup |
| `src/frontend.rs`, `src/frontend/` | Relm4 composition, GTK/WebKit views, settings, and UI adapters |
| `src/backend.rs`, `src/backend/` | playback controller, lyrics/search services, cache coordination, and MPRIS |
| `src/backend/amll.rs`, `src/backend/amll/` | AMLL WebSocket protocol sender (wire types, connection task, inbound control commands, the `LyricsView` implementation used in sender mode) |
| `src/backend/mpris/control.rs` | outbound MPRIS control: command-to-operation resolution, volume and playback mode properties |
| `src/frontend/tray.rs` | StatusNotifierItem tray icon and its language-aware menu |
| `src/shared.rs`, `src/shared/` | persisted TOML model and cross-layer presentation contracts |
| `src/shared/config/` | config persistence, recovery, and validation submodules |
| `floatlyrics-lyrics/src/lyrics.rs` | lyrics domain facade; re-exports lyrics-helper types (`LineInfo`, `LyricsData`, `LyricsTypes`) |
|`floatlyrics-lyrics/src/lyrics/`|provider-neutral models, parsing, filtering, romanization, karaoke word segmentation, timeline, search|
| `floatlyrics-lyrics/src/cache.rs`, `src/cache/` | cache boundary, SQLite access, and schema |
| `floatlyrics-core/src/i18n.rs` | locale selection, typed text keys, catalogue validation |
| `src/frontend/view/lyrics/` | React + PixiJS embedded lyrics view (TypeScript, built by Bun) |
| `data/locale/` | runtime catalogues for every supported locale |
| `data/licenses/` | cargo-about template, generated Rust dependency notices, and frontend license data |
| `.github/workflows/` | CI and release automation; use it as the source of truth for CI commands |
| `packaging/` | install scripts, AUR metadata, and packaging automation |

When adding a module, follow the existing facade-plus-submodule layout rather
than creating a parallel architecture. First-party Rust files and other files
with an established convention should retain the repository's SPDX header.

## Frontend architecture

The lyrics display is a WebKit `WebView` hosting a single-page React application
in `src/frontend/view/lyrics/`:

- Bridge (`bridge.ts`) — installs `window.floatLyrics` with a `dispatch`
  method. Rust calls `web_view.evaluate_script()` to push serialized
  `LyricsCommand` objects. Commands sent before JS loads are buffered in
  `window.floatLyricsPendingCommands` and replayed on bridge install.
- Command types (`types.ts`) — three commands: `configure` (style +
  Apple Music mode), `document` (full lyrics timeline), `frame` (per-frame
  playback snapshot with position and slot-switching signals).
- Store (`store.ts`) — `LyricsViewState` manages a dual-slot ping-pong
  renderer. On track change, `key` changes and `activeSlot` toggles (0↔1),
  triggering a CSS cross-fade between the outgoing and incoming lyric lines.
- Renderer (`app.tsx`, `amll.ts`) — React component subscribes to the
  store via `useSyncExternalStore`, converts `LyricsDocument` to AMLL line
  format, and drives `@applemusic-like-lyrics/react`'s `LyricPlayer` (backed
  by PixiJS). Karaoke word-fill is handled in `karaoke.ts`.

When modifying the frontend, run `bun run typecheck` and `bun test` before
committing. The bridge API (`LyricsCommand`, `LyricsFrame`) is the contract
between Rust and JS — changing it requires coordinated updates on both sides.

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
gtk4-layer-shell, OpenSSL, and packaging tools. The React lyrics frontend uses
Bun 1.3.14, TypeScript, and Biome. Compiling the root crate locally requires Bun
and the corresponding system development libraries; Cargo installs the locked
frontend dependencies and generates the embedded page through `build.rs`.
Use `bun run format` to format the React frontend and apply Biome's safe fixes;
CI uses the non-mutating `bun run check` command.

Use `--locked` on Cargo commands that resolve dependencies. Do not add it to
`cargo fmt`, which does not accept it. Never regenerate `Cargo.lock`
accidentally.

Start with the narrowest useful checks:

```bash
bun install --frozen-lockfile
bun run check
bun run typecheck
bun test
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
bun install --frozen-lockfile
bun run check
bun run typecheck
bun test
bun run build:lyrics
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release
cargo docs
```

The first four commands (Bun install, check, typecheck, test) match the
frontend quality steps in CI. `bun run build:lyrics` generates the embedded
lyrics view and is verified by the later `cargo build` step. `cargo docs` and
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

## Build pipeline

`build.rs` runs automatically before every Cargo compilation:

1. `bun install --frozen-lockfile` — installs locked frontend dependencies.
2. `bun run build:lyrics` — bundles the React lyrics view from
   `src/frontend/view/lyrics/lyrics.html` into a single self-contained HTML
   file in Cargo's `OUT_DIR`, and generates `frontend-dependencies.json` for
   license attribution.
3. Rust embeds the bundled HTML at compile time via
   `include_str!(concat!(env!("OUT_DIR"), "/lyrics.html"))` in
   `src/frontend/view/web_lyrics.rs`.

When changing the React frontend, `bun run build:lyrics` can be run standalone
without a full Cargo rebuild. `build.rs` monitors `package.json`, `bun.lock`,
`tsconfig.json`, `src/frontend/view/lyrics/`, and `data/licenses/frontend/` for
changes via `cargo:rerun-if-changed`.

`floatlyrics-lyrics` embeds two morphological dictionaries for local CJK
readings: IPADIC through `lindera-ipadic` for Japanese and CC-CEDICT through
`lindera-cc-cedict` for Mandarin, plus the JmdictFurigana data through
`jmdict-furigana` for the kana of each Japanese character. The lindera crates'
`build.rs` downloads a pinned archive from `lindera.dev` and compiles it into a
dictionary that is linked into the binary, so a first build needs network access
and the three together add roughly 85 MB to the release binary. To build without that download, point
Loading the furigana data parses the whole JmdictFurigana archive, which costs
about 85 MB of resident memory once a Japanese word needs it; the dictionaries
themselves are memory-mapped and only cost what is read from them.

`LINDERA_BUILD_DICTIONARY_CACHE_DIR` at a directory holding the archives under
`<crate version>-fmt<dictionary format version>` — the version numbers are
pinned by `Cargo.lock`, and the AUR `PKGBUILD` seeds exactly that directory from
its `source` array. A cache that does not match makes the build download again
rather than fail.

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
- Preserve the temporary-file-plus-rename atomic write path in
  `src/shared/config.rs`,
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

### MPRIS module layout

The `mpris` module in `src/backend/mpris.rs` has four submodules:

| Submodule | File | Responsibility |
|---|---|---|
| `compat` | `src/backend/mpris/compat.rs` | Known-player metadata hints for exact provider song IDs |
| `model` | `src/backend/mpris/model.rs` | `PlaybackStatus`, `MprisMetadata`, `PlayerState`, `PlayerWatcherEvent`, D-Bus → domain conversion |
| `position` | `src/backend/mpris/position.rs` | playback position synchronization and timing |
| `watcher` | `src/backend/mpris/watcher.rs` | D-Bus discovery, active-player selection, per-player observation, and compatibility wrappers |

The watcher spawns an async Tokio task that discovers standard MPRIS D-Bus
names, follows the highest-ranked active player, and pushes `PlayerWatcherEvent`
variants into the Controller's MPSC receiver. A separate internal channel
carries track-fingerprint-tagged provider hints so player-specific metadata
cannot expand the public playback-state API or apply after a track change.

## Controller and async topology

The `Controller` in `src/backend/controller.rs` is the central orchestrator
between MPRIS events, lyrics fetching, caching, and presentation:

- MPRIS events arrive via `mpsc::Receiver<PlayerWatcherEvent>` and are
  processed in `tick()`, which the GTK main loop calls repeatedly.
- Exact provider hints inferred from known MPRIS metadata arrive on a separate
  receiver and are ignored unless their bus name and track fingerprint still
  match the active player.
- Three MPSC channels offload blocking/async work to the Tokio runtime:
  `lyrics_sender` (HTTP search), `cache_sender` (SQLite read/write),
  `romanization_sender` (CPU-heavy CJK romanization). Results feed back
  through `Receiver` channels and are re-integrated in `tick()`.
- Generation tokens (`lyrics_generation: u64`) are incremented on every
  lyrics reload or track change. Async results carry a generation; stale
  results (from a previous track) are silently discarded.
- `ControllerHandle` is a cloneable handle exposed to the frontend. It
  exposes `reload_lyrics()` and `current_track()` (via `PlaybackProjection`,
  an `Rc<RefCell<Option<TrackMetadata>>>`) without locking the controller.
- Presentation (`presentation.rs`, `loading.rs`) converts internal state
  into `LyricsFrame` structs pushed into the `LyricsView` trait (implemented
  by `WebLyricsView`) which serializes them as `LyricsCommand` JS calls.

The `CacheWorker` in `src/backend/cache.rs` wraps `floatlyrics_lyrics::cache::Cache`
behind an `mpsc` channel to keep blocking SQLite operations off the GTK thread.

Playback control is AMLL-only: the control commands an AMLL client sends queue a
`MediaCommand` on the watcher's channel, because only the watcher task owns the
D-Bus connection and knows which player is active. Neither the floating overlay
nor the tray renders playback controls. Reads happen the other way round: the
watcher publishes the volume, playback modes, and capabilities it observes, the
controller reports them through `LyricsView::set_player_control`, and the AMLL
sender republishes them as protocol updates.

Every lyrics document enters the application through
`loading/cache.rs::lyrics_state_from_cached`, which parses the cached raw payload
and then runs the karaoke word segmentation from
`floatlyrics_lyrics::lyrics::segment_lines_into_words` before the background
romanization worker starts, so readings are aligned to the tokens the views
render. New presentation behavior that depends on word timing belongs in that
order, not in the parsers.

Neither QRC nor plain LRC has a field for a duet part, a background vocal, or a
sentence the transcriber broke across rows, so all three are read from the
conventions the transcriber wrote the lyrics with, in
`floatlyrics-lyrics/src/lyrics/parsing/conventions.rs`, and nothing beyond them:

- **A speaker label must name an artist the provider listed.** A row such as
  `Doja Cat:` or `The Weeknd：` sets `TimedLine::voice`; a sung line containing a
  colon is left alone. A label may credit several performers at once, and the
  first name in it that the provider also lists decides the side, because joint
  credits are written lead-first — one match is enough, so a featured performer
  the provider omits from its artist list rides along on the name beside them.
  The list matched against is the one the lyrics were resolved with, which is why
  `timed_lines_from_raw` takes it as an argument.
- **A background vocal is bracketed.** NetEase writes it as a bracketed tail on
  the line it answers, with sung text in front of it, and that tail becomes
  `TimedLine::background`. QQ Music writes it as rows of its own, wholly
  bracketed, which are folded into the line before them: the phrase may open on
  one row and close several rows later, and it answers after whatever rest the
  line leaves rather than always at its parent's last word. Both forms need a
  letter or digit inside the brackets, so `(...)` is left alone, and a line-timed
  source is left alone entirely: there is no timing to split off with. Folding
  runs after translations are paired, because the echo
  is translated where it is sung rather than where the line it answers is: it
  keeps its own timing, its own words, and its own translation in
  `BackgroundVocal`: the AMLL sender writes them as the spans inside the `x-bg`
  span, so the listener fills the words it hears as it hears them instead of one
  span crawling over the whole echo, and draws the translation the phrase was
  sung with. The brackets that marked the phrase are stripped from the words and
  from the translation, and are written back on the words at the edges of the
  span, which is the shape a listener strips them from again.
- **A sentence broken across rows is one line.** A transcriber who runs out of
  room writes the rest of a sentence on the row after the one that begins it —
  Saddle Up writes `Put your money` then `where your mouth is`, and `But I had
  enough,` then `so I move onto the next thing` — and the rows are joined by
  `merge_continued_lines` into the line the renderer draws, their words and their
  translations with it, once each row has collected its own translation and after
  a bracketed echo has been folded away. What decides the join is the row after: a
  lowercase word carries the sentence on, a capitalized one begins a new line, and
  the English pronoun, which is written uppercase while it carries the rest of a
  sentence, counts as a continuation only when the row before it left its
  punctuation open. The row before has to be able to say so: a row the punctuation
  closed keeps its own line, and a row ending in a script without letter case — or
  in a digit — says nothing either way, because there the transcriber broke where
  the line ran out rather than where a sentence did.

Do not widen this past what the transcription states: overlapping timings and an
unmatched label are not evidence of a second voice. The resulting voice is
written to the AMLL listener as the `ttm:agent` of the line, which is what tells
it to alternate the two sides; the floating overlay renders `background` only.

A provider that times every word may also write the transcription of the rows
separately, and the two documents of NetEase state the same track in ways that do
not agree: the word-timed one drops the separators between its words, reads the
brackets of an aside as the brackets of a tag, and censors words its row-timed
document spells out. The row-timed transcription is the text a listener reads, so
a payload carries both — the fetch writes `combine_word_timing`, the parse splits
the section again — and `parsing/word_timing.rs` reads the times of the words onto
the row-timed text, one row at a time. A row whose words cannot be spelled from
that text (a masked word, an aside the word-timed document never carried) keeps
its row timing and carries no words, and the words of a row always spell exactly
what the row displays, because the views map a word onto its row by character
offset.

A resolved lookup also carries the artists the provider credits as
`LyricsDisplayState::credited_artists`. When they name a performer the player's
own metadata omits — Spotify reports "Problem" as Ariana Grande alone — the
controller restates the track through `LyricsView::set_track_metadata`, which
only the AMLL sender renders, and never through `set_song_info`. That is a
mid-track track-info message, so the clock has to follow it: `AmllSender`'s
client restates playback once after every music change, because the listener
zeroes its position on one and a paused track would otherwise leave the lyrics at
the top of the song.

The same document carries the kana of a Japanese word: `TimedSyllable::furigana`
holds what the furigana dictionary gives each character, and the AMLL sender
(`src/backend/amll/ttml.rs`) writes it as the `tts:ruby` spans of a TTML document
— the format in which a listener draws a reading above the characters it belongs
to. Per-word readings go into that document's metadata, because that is where the
player matches them to the words by time.

The readings themselves come from `floatlyrics-lyrics/src/lyrics/romanization/`:
Japanese and Mandarin segment their text with the embedded lindera dictionaries
and read each word in context — Japanese then splits a word's reading across its
characters with the JmdictFurigana data — Cantonese annotates through
`rust-canto`, and
Korean applies the Revised Romanization pronunciation rules to a run of Hangul
syllables. All of them emit one segment per character, which is the shape
`assign_syllable_readings` aligns with the karaoke tokens, and Japanese and
Korean decline to read text their rules cannot cover rather than guessing.

## Run modes and the tray

`general.mode` selects exactly one lyrics output, and the choice is made once,
in `AppModel::init`:

- `floating` (default) — the Relm4 root window is initialized as the
  layer-shell overlay and `view::OverlaySender` is the `LyricsView`.
- `amll` — no layer-shell surface and no WebKit lyrics view are created
  (`RelmApp::visible_on_activate(false)` keeps the root window hidden);
  `backend::amll::AmllSender` becomes the `LyricsView` and streams the AMLL
  WebSocket protocol to `amll.address`.

Everything else — MPRIS watching, lyrics search, caching, romanization,
configuration, and the settings windows — is shared by both modes.

- The overlay supplies the controller's frame clock through
  `OverlayView::tick_widget`; without it, `AppModel::init` installs a
  `glib::timeout_add_local` pump instead. Do not remove one without keeping the
  other, or `Controller::tick()` stops running in that mode.
- Mode, `amll.address`, and `tray.enabled` are read at startup, so changing them
  requires a restart; the settings UI says so.
- `src/frontend/tray.rs` publishes a StatusNotifierItem whose callbacks only
  forward `UiAction` values through `APP_BROKER`. Tray callbacks run on the
  D-Bus service loop: never touch GTK state from them. Registration failure is
  logged and ignored, so the app still starts without a tray host.
- Adding a `LyricsView` implementation means implementing every trait method,
  including `set_track_metadata`, which carries the structured track metadata
  the token-based text boundary does not, and `set_playback`, which publishes
  the playback clock for every frame of a playing track — independently of the
  lyrics, so a listener with no lyrics still gets progress.

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

Add JavaScript dependencies with `bun add` or `bun add --dev`, never by typing
versions into `package.json`; commit `bun.lock` with the manifest change. The
lyrics build derives runtime npm license notices from Bun's bundle metafile, so
verify `target/lyrics-web/frontend-dependencies.json` after changing production
frontend dependencies without committing that generated file.

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
