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

The repository is a single Cargo package with internal modules separated by
responsibility:

- `src/lib.rs`: CLI, startup, and the public module facade.
- `src/app.rs`: application composition root and dependency wiring.
- `src/app/`: playback controller, GTK-independent presentation model, and GTK view.
- `src/main.rs`: thin binary delegating to `floatlyrics::run()`.
- `src/lyrics/`: lyrics models, parsing, provider search, and timeline calculations.
- `src/mpris/`: D-Bus watcher, player models, and position synchronization.
- `src/cache.rs`, `src/config.rs`, `src/paths.rs`, `src/telemetry.rs`: local
  infrastructure concerns.

## MVP Scope

- Spotify-only MPRIS tracking.
- Lyrics provider order: QQ Music, NetEase Cloud Music.
- GTK4 Wayland layer-shell floating lyrics window.
- SQLite cache for tracks, lyrics, manual matches, provider results, and settings.
