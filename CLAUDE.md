# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                  # compile
cargo build --release        # optimized build
cargo run                    # run (requires client_id in config first)
cargo clippy                 # lint
cargo clippy -- -D warnings  # lint as errors
cargo fmt                    # format
cargo check                  # type-check without producing a binary
```

There are no tests yet. When adding them, run a single test with:
```bash
cargo test test_name
```

Before running, `~/.config/spotify-dj/config.toml` must have a non-empty `client_id`. The app will bail with setup instructions if it is missing.

## Architecture

### Data flow

`main.rs` owns the tokio runtime and runs two sequential phases:

1. **Auth phase** — `SpotifyAuth::authenticate()` either loads and refreshes saved tokens from `~/.config/spotify-dj/tokens.json` or runs a full OAuth2 PKCE browser flow. Once done, the `AuthCodePkceSpotify` client inside `SpotifyAuth` holds a valid token.

2. **TUI phase** — `run_event_loop()` polls for crossterm keyboard events every 100ms, dispatches them through focus-aware handlers in `main.rs` (`handle_library_keys`, `handle_deck_keys`, `handle_mixer_keys`), then redraws via `ui::draw()`.

### State

`AppState` in `src/app.rs` is the single source of truth passed by mutable reference through the entire TUI phase. It contains:
- `deck_a` / `deck_b: DeckState` — per-deck playback info (track, position, BPM, key, energy, volume)
- `active_deck: ActiveDeck` — which deck has the audio stream
- `crossfader: f32` — ranges from `-1.0` (full A) to `1.0` (full B)
- `library: LibraryState` — search query, results, selection cursor
- `focus: UiFocus` — which panel receives keyboard events (cycles with Tab)

### UI

`ui::draw()` in `src/ui/mod.rs` splits the terminal into four panels using Ratatui `Layout` constraints and delegates to submodule draw functions:
- `ui::deck::draw_deck()` — called twice (Deck A and B); renders track info, `LineGauge` progress bar, BPM/key/energy, and a static visualizer placeholder
- `ui::library::draw_library()` — search input + `List` widget with stateful selection
- `ui::mixer::draw_mixer()` — crossfader `LineGauge` and duration hint rows

The focused panel has a cyan border; the active (playing) deck has a green border.

### Phases still to implement

The codebase is at Phase 1. Stubs exist in `main.rs` key handlers that log "Phase N" messages. Upcoming modules referenced in `ARCHITECTURE.md` but not yet created:
- `src/spotify/player.rs` — librespot session + `PlayerEvent` handling (Phase 2)
- `src/spotify/web_api.rs` — rspotify search and audio features (Phase 3)
- `src/audio/sink.rs`, `src/audio/fft.rs` — custom PCM sink + FFT visualizer (Phase 4)
- `src/audio/crossfade.rs` — volume ramp + track transition (Phase 5)
- `src/ui/visualizer.rs` — Ratatui `BarChart` driven by FFT bands (Phase 4)

See `ARCHITECTURE.md` for the full design of each component.

### Key constraints

- Spotify only allows one active audio stream per account. Deck B holds metadata only until a crossfade starts; audio switches at the midpoint of the fade.
- librespot (the planned audio backend) is not yet a dependency — it will be added in Phase 2.
- `rspotify` is already a dependency but the client is not yet wired into the TUI; it is only used in `SpotifyAuth` for token management.
