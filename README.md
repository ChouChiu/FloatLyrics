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

## MVP Scope

- Spotify-only MPRIS tracking.
- Lyrics provider order: QQ Music, NetEase Cloud Music.
- GTK4 Wayland layer-shell floating lyrics window.
- SQLite cache for tracks, lyrics, manual matches, provider results, and settings.
