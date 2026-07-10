# FloatLyrics

Rust desktop lyrics app MVP for Linux Wayland, focused on Spotify MPRIS and floating lyrics.

## Run

```bash
cargo run -- --debug
```

Supported CLI:

```bash
floatlyrics
floatlyrics --debug
floatlyrics --config <path>
floatlyrics --reset-window
```

Default paths:

- Config: `~/.config/floatlyrics/config.toml`
- Database: `~/.local/share/floatlyrics/floatlyrics.sqlite3`

## Architecture

The repository is a Cargo workspace built around a fat application library and a
one-call binary entry point:

- `src/lib.rs`: CLI, startup, application assembly, and public crate facade.
- `src/app.rs`: GTK floating-window presentation and UI orchestration.
- `src/main.rs`: thin binary delegating to `floatlyrics::run()`.
- `crates/core`: track domain plus lyrics providers, parsing, matching, and timing.
- `crates/support`: SQLite persistence, configuration, MPRIS, application paths,
  and tracing infrastructure.

## MVP Scope

- Spotify-only MPRIS tracking.
- Lyrics provider order: QQ Music, NetEase Cloud Music.
- GTK4 Wayland layer-shell floating lyrics window.
- SQLite cache for tracks, lyrics, manual matches, provider results, and settings.
