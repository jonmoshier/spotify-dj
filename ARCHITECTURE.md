# spotify-dj Architecture & Design

A Traktor-style terminal DJ application for Spotify, built in Rust.

---

## Table of Contents

1. [Project Overview](#project-overview)
2. [Feasibility](#feasibility)
3. [Technology Stack](#technology-stack)
4. [Architecture](#architecture)
5. [Dual-Deck Model](#dual-deck-model)
6. [UI Layout](#ui-layout)
7. [Key Components](#key-components)
   - [Authentication](#authentication)
   - [Audio Playback (librespot)](#audio-playback-librespot)
   - [Web API (rspotify)](#web-api-rspotify)
   - [Audio Sink & FFT Visualizer](#audio-sink--fft-visualizer)
   - [Crossfade](#crossfade)
   - [Ratatui Layout](#ratatui-layout)
8. [Project Structure](#project-structure)
9. [Configuration](#configuration)
10. [Phased Roadmap](#phased-roadmap)
11. [Limitations & Workarounds](#limitations--workarounds)

---

## Project Overview

spotify-dj is a keyboard-driven terminal application that presents a Traktor-style DJ interface on top of Spotify. It streams audio directly from Spotify (via librespot), displays two decks with BPM/key/energy metadata, and supports crossfading between tracks.

**Target user**: Spotify Premium subscriber who wants a fast, keyboard-first DJ workflow in the terminal.

---

## Feasibility

| Capability | Status | Notes |
|---|---|---|
| Direct audio streaming | ✅ Feasible | Via librespot — reverse-engineered Spotify protocols |
| Spotify Connect device | ✅ Feasible | App appears in the Spotify device picker on other devices |
| BPM, key, energy metadata | ✅ Available | Spotify Audio Features API |
| Real-time frequency visualizer | ✅ Feasible | FFT computed on raw PCM from librespot |
| Dual deck + crossfade | ✅ Feasible | One stream at a time; Deck B holds metadata until crossfade |
| True simultaneous dual playback | ❌ Not possible | Spotify only allows one active stream per account |
| Waveform data from API | ❌ Deprecated | Audio Analysis endpoint was removed November 2024 |
| Free Spotify accounts | ❌ Not possible | librespot requires Premium |

### ToS Note

librespot uses Spotify's internal protocols rather than the public Web API. It is not officially sanctioned but is widely used in personal projects — [spotify-player](https://github.com/aome510/spotify-player), Spotifyd, and others. Safe for personal use; do not distribute commercially.

---

## Technology Stack

| Crate | Version | Role |
|---|---|---|
| `ratatui` | 0.30 | TUI framework |
| `crossterm` | 0.29 | Terminal backend, keyboard events |
| `librespot-core` | 0.6 | Spotify session & authentication |
| `librespot-playback` | 0.6 | Audio decoding, custom sink |
| `librespot-connect` | 0.6 | Spotify Connect device registration |
| `librespot-metadata` | 0.6 | Track metadata resolution |
| `rspotify` | 0.16 | Spotify Web API (search, audio features) |
| `tokio` | 1 | Async runtime |
| `rustfft` | 6 | Real-time FFT for the visualizer |
| `serde` / `serde_json` | 1 | Serialization |
| `toml` | 1 | Config file parsing |
| `chrono` | 0.4 | Token expiry datetime handling |
| `directories` | 6 | XDG config/data paths |
| `anyhow` / `thiserror` | latest | Error handling |
| `webbrowser` | 1 | Open OAuth URL in system browser |
| `url` | 2 | Parse OAuth callback query string |

---

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│                     spotify-dj                           │
├──────────────────────────────────────────────────────────┤
│  TUI Layer                  (Ratatui + Crossterm)        │
│  ┌────────────┐ ┌─────────────────┐ ┌────────────────┐  │
│  │ DeckWidget │ │  MixerWidget    │ │ LibraryWidget  │  │
│  │ (x2)       │ │  crossfader     │ │ search/browse  │  │
│  └────────────┘ └─────────────────┘ └────────────────┘  │
│  ┌────────────────────────────────────────────────────┐  │
│  │  VisualizerWidget  (real-time FFT bar chart)       │  │
│  └────────────────────────────────────────────────────┘  │
├──────────────────────────────────────────────────────────┤
│  App State + Event Loop        (tokio + mpsc channels)   │
│  AppState { deck_a, deck_b, mixer, library, ui_focus }   │
├──────────────────────────────────────────────────────────┤
│  Spotify Integration                                     │
│  ┌───────────────────────┐  ┌───────────────────────┐   │
│  │  librespot Player     │  │  rspotify Web API      │   │
│  │  - audio stream/PCM   │  │  - search tracks       │   │
│  │  - volume control     │  │  - audio features      │   │
│  │  - Connect device     │  │  - playlists/library   │   │
│  └───────────────────────┘  └───────────────────────┘   │
├──────────────────────────────────────────────────────────┤
│  Audio Processing             (rustfft)                  │
│  - PCM sink → FFT → frequency bands → visualizer data   │
│  - Crossfade: volume ramp + play next track command      │
└──────────────────────────────────────────────────────────┘
```

The app runs a single tokio runtime. The TUI redraws at 100ms intervals driven by the event loop. Background tasks (librespot session, FFT worker) communicate with the main thread via `tokio::sync` channels.

---

## Dual-Deck Model

Spotify only allows one active audio stream per account, so the two-deck model mirrors how hardware DJ setups work: **one deck plays, the other is prepared silently in headphones**.

- **Deck A (active)**: librespot streams audio to the system output; the FFT visualizer is live.
- **Deck B (cued)**: a track is loaded from the library and its BPM/key/energy metadata is displayed, but no audio plays yet.
- **Crossfade**: a volume ramp gradually reduces Deck A's volume to zero while Deck B's "volume" counter increases in the UI. At the midpoint, librespot issues a play command for Deck B's track. Once Deck A reaches zero, the decks swap roles.

This is indistinguishable from real DJ hardware behavior — a DJ prepares the next track on the cued deck and listens on headphones before fading it in.

---

## UI Layout

```
╔══════════════════════════════════════════════════════════════════╗
║  DECK A [active]                  ║  DECK B [cued]               ║
║  Artist — Track Title             ║  Artist — Track Title        ║
║  ▶ 02:14 ───────────▪─────────── 03:45  ║  ⏸ 00:00 ────────── 04:12  ║
║  BPM: 128.0  Key: C maj           ║  BPM: 126.5  Key: A min      ║
║  Vol: ████████░░  Energy: ████░░  ║  Vol: ░░░░░░░░  Energy: ███░ ║
║  ┌──────────────────────────────┐ ║  ┌──────────────────────────┐ ║
║  │ ▁▂▃▄▅▄▃▂▁ ▂▃▅▆▅▃▂ ▁▂▄▅▄▂▁ │ ║  │ (load a track to preview)│ ║
║  └──────────────────────────────┘ ║  └──────────────────────────┘ ║
╠═══════════════════╦══════════════════════════════════════════════╣
║  LIBRARY          ║   ←──────────────▪──────────────→            ║
║  [/] Search...    ║            CROSSFADER                        ║
║  ▶ Playlists      ║  [←] Fade to A       [→] Fade to B           ║
║  ▶ Saved Tracks   ║  Auto-fade: [5] 5s  [0] 10s  [3] 30s         ║
║  ▶ Recently...    ╠══════════════════════════════════════════════╣
║                   ║  [SPC] Play/Pause  [←→] Seek  [TAB] Focus   ║
║  Track 1 128bpm   ║  [L] →DeckA  [R] →DeckB  [X] Crossfade      ║
║  Track 2 124bpm   ║  [Q] Quit  [/] Search  [1/2] Switch deck     ║
╚═══════════════════╩══════════════════════════════════════════════╝
```

**Panel focus** cycles with `Tab`. The focused panel is highlighted in cyan; the active (playing) deck border is green.

---

## Key Components

### Authentication

**File**: `src/spotify/auth.rs`

OAuth2 PKCE flow (no client secret required):

1. On first run, build the Spotify authorization URL with the required scopes and open it in the system browser.
2. Spin up a one-shot `TcpListener` on `127.0.0.1:8888` to capture the redirect callback.
3. Parse the `?code=` parameter from the callback URL.
4. Exchange the code for `access_token` + `refresh_token` via rspotify.
5. Persist tokens as JSON to `~/.config/spotify-dj/tokens.json` (Unix mode `0600`).
6. On subsequent runs, load the saved tokens and call `refresh_token()`. If refresh fails, re-run the full flow.

Required OAuth scopes:
```
streaming
user-read-playback-state
user-modify-playback-state
user-read-currently-playing
user-library-read
playlist-read-private
playlist-read-collaborative
```

---

### Audio Playback (librespot)

**Files**: `src/spotify/player.rs` (Phase 2), `src/audio/sink.rs` (Phase 4)

librespot is used as the audio engine. It:
- Creates a Spotify session using the same credentials as the Web API
- Registers the app as a named Spotify Connect device (visible in the Spotify app's device picker)
- Decodes Vorbis audio and sends PCM frames to a custom `AudioSink`
- Emits `PlayerEvent` values for track changes, play/pause, seek, and end-of-track

High-level setup:
```
librespot::Session::connect(session_config, credentials)
  → librespot::Player::new(player_config, session, audio_sink)
  → librespot::spirc::Spirc::new(connect_config, session, player)
      (registers as Connect device, handles remote commands)
```

On each `PlayerEvent::Playing { track_id, .. }`, the Web API client fetches audio features for the track and populates the active deck's BPM/key/energy fields.

---

### Web API (rspotify)

**File**: `src/spotify/web_api.rs` (Phase 3)

rspotify wraps the Spotify Web API. It shares the same OAuth tokens as the auth flow (both use `AuthCodePkceSpotify`).

Key endpoints used:

| Endpoint | rspotify method | Purpose |
|---|---|---|
| `GET /search` | `search()` | Library search |
| `GET /audio-features/{id}` | `track_features()` | BPM, key, energy |
| `GET /me/playlists` | `current_user_playlists()` | Playlist browser |
| `GET /playlists/{id}/tracks` | `playlist_items()` | Tracks in a playlist |
| `GET /me/tracks` | `current_user_saved_tracks()` | Liked songs |

**Audio features displayed:**

| Spotify field | Display |
|---|---|
| `tempo` | `128.0 BPM` |
| `key` + `mode` | `C maj` / `A min` (pitch class integer → note name) |
| `energy` | Progress gauge 0–100% |
| `loudness` | dB value |
| `danceability` | Optional secondary display |

---

### Audio Sink & FFT Visualizer

**Files**: `src/audio/sink.rs`, `src/audio/fft.rs` (Phase 4)

A custom struct implementing librespot's `AudioSink` trait intercepts the raw PCM stream. It:

1. Forwards PCM frames to the system audio output (via `rodio` or `cpal`)
2. Sends a copy of each frame over an `mpsc` channel to a dedicated FFT worker task

The FFT worker runs at ~30 fps:
- Accumulates PCM samples until it has ~33ms of audio (1,470 frames at 44,100 Hz)
- Runs `rustfft` on the window
- Buckets the output into 20 logarithmically-spaced frequency bands (bass → treble)
- Sends the band magnitudes over a `tokio::sync::watch` channel to the main UI thread

The `VisualizerWidget` reads the latest band magnitudes from the watch channel and renders them as a Ratatui `BarChart`. This gives a live spectrum analyzer that animates in sync with the music.

Because Spotify's Audio Analysis endpoint was deprecated in November 2024, there is no static waveform data available from the API. The real-time FFT from the live audio stream is a better alternative anyway.

---

### Crossfade

**File**: `src/audio/crossfade.rs` (Phase 5)

When the user triggers a crossfade (`X` key, or by dragging the crossfader to a threshold):

1. Determine the fade duration (5 / 10 / 30 seconds, configured via `5` / `0` / `3` keys).
2. Spawn a `tokio::spawn` task that ticks every 100ms.
3. Each tick: reduce the active deck's volume by one step, increase the cued deck's counter.
4. At the 50% point (crossfader center): issue a librespot play command for Deck B's track URI.
5. When the active deck's volume reaches 0: swap `active_deck` — Deck B is now active, Deck A is cued.

Since Spotify only supports one audio stream, "volume" on the fading-out deck is librespot's `set_volume()` (0–65535 internally). Deck B's increasing volume counter is UI-only until it actually starts playing.

---

### Ratatui Layout

**File**: `src/ui/mod.rs`

Layout is built with Ratatui's `Layout` constraint system:

```
Vertical split (100% height):
  ├── Top (60%): Horizontal split 50/50
  │     ├── Deck A panel
  │     └── Deck B panel
  └── Bottom (40%): Horizontal split 30/70
        ├── Library panel
        └── Right side: Vertical split
              ├── Mixer panel (crossfader + duration controls)
              └── Status/keybindings bar (3 rows)

Each Deck panel (vertical split):
  ├── Track title + artist (2 rows)
  ├── Progress bar LineGauge (1 row)
  ├── Position / duration (1 row)
  ├── BPM / Key / Energy (1 row)
  └── Remaining: FFT BarChart + Volume Gauge
```

Focus is tracked in `AppState::focus: UiFocus`. The focused panel renders with a cyan border; the active deck renders with a green border.

---

## Project Structure

```
spotify-dj/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── ARCHITECTURE.md         ← this file
├── .gitignore
└── src/
    ├── main.rs              # tokio entry point, terminal setup, event loop
    ├── app.rs               # AppState, DeckState, LibraryState, UiFocus
    ├── config.rs            # Config struct, TOML load/save, XDG paths
    ├── error.rs             # AppError type
    │
    ├── spotify/
    │   ├── mod.rs
    │   ├── auth.rs          # OAuth2 PKCE flow, token file persistence  [Phase 1 ✓]
    │   ├── player.rs        # librespot session + PlayerEvent handling  [Phase 2]
    │   ├── web_api.rs       # rspotify: search, audio features          [Phase 3]
    │   └── connect.rs       # Spotify Connect device registration       [Phase 2]
    │
    ├── audio/
    │   ├── mod.rs
    │   ├── sink.rs          # Custom AudioSink: PCM → speakers + FFT   [Phase 4]
    │   ├── fft.rs           # rustfft wrapper, 20-band bucketing        [Phase 4]
    │   └── crossfade.rs     # Volume ramp + track transition logic      [Phase 5]
    │
    └── ui/
        ├── mod.rs           # Root draw() function, layout split        [Phase 1 ✓]
        ├── deck.rs          # DeckWidget: progress, BPM, key, gauge     [Phase 1 ✓]
        ├── mixer.rs         # MixerWidget: crossfader                   [Phase 1 ✓]
        ├── library.rs       # LibraryWidget: search, track list         [Phase 1 ✓]
        ├── visualizer.rs    # VisualizerWidget: FFT BarChart             [Phase 4]
        └── keybindings.rs   # Input event → AppAction mapping           [Phase 5]
```

---

## Configuration

**Path**: `~/.config/spotify-dj/config.toml`

Created automatically on first run with defaults. Edit before running:

```toml
[auth]
# From developer.spotify.com/dashboard — create an app and set
# the Redirect URI to http://127.0.0.1:8888/callback
client_id = "your_client_id_here"

[playback]
device_name = "spotify-dj"    # name shown in Spotify Connect device list
bitrate = 320                  # kbps: 96, 160, 320

[ui]
crossfade_duration_secs = 10   # default auto-fade duration
default_volume = 80            # 0–100
```

**Token file**: `~/.config/spotify-dj/tokens.json` (Unix mode `0600`, owner read/write only).

---

## Phased Roadmap

### Phase 1 — Skeleton + Auth ✅
- [x] `cargo init`, full dependency stack
- [x] OAuth2 PKCE flow with browser open + local callback server
- [x] Token persistence to config dir (mode 0600)
- [x] Config load/save with TOML
- [x] Ratatui TUI: dual deck layout, library panel, mixer panel, status bar
- [x] Keyboard routing scaffold (stubs note which phase connects each action)

### Phase 2 — Playback Core
- [ ] librespot `Session` setup using saved credentials
- [ ] `SpircTask` for Spotify Connect device registration
- [ ] `Player` with basic playback: play URI, pause, seek, set volume
- [ ] `PlayerEvent` stream → `AppState` updates (track info, position polling)
- [ ] Single-deck UI connected to real playback state

### Phase 3 — Web API Integration
- [ ] rspotify client sharing same OAuth tokens
- [ ] Track search → Library widget results
- [ ] `GET /audio-features/{id}` → BPM / key / energy display on decks
- [ ] Playlist and saved-tracks browsing

### Phase 4 — FFT Visualizer
- [ ] Custom `AudioSink` that tees PCM to an `mpsc` channel
- [ ] FFT worker task (rustfft, ~30 fps)
- [ ] 20-band logarithmic frequency bucketing
- [ ] `VisualizerWidget` using Ratatui `BarChart`

### Phase 5 — Dual Deck + Crossfade
- [ ] Load track to Deck A (`L`) or Deck B (`R`)
- [ ] Crossfader UI widget with real-time position
- [ ] `crossfade.rs`: volume ramp task + play-next-track command at midpoint
- [ ] Deck swap on crossfade completion
- [ ] BPM compatibility hint (semitone distance between deck keys)

### Phase 6 — Polish
- [ ] `?` help overlay
- [ ] Reconnect logic on librespot session drop
- [ ] Token auto-refresh in background
- [ ] Dark color theme (Traktor-inspired: black background, green/cyan accents)
- [ ] Configurable keybindings

---

## Limitations & Workarounds

| Limitation | Workaround |
|---|---|
| No simultaneous dual audio streams | One stream at a time; Deck B is visual-only until crossfade midpoint |
| No waveform data from Spotify API | Real-time FFT on live PCM gives a spectrum analyzer (better than a static waveform) |
| Audio Analysis API deprecated (Nov 2024) | BPM/key from Audio Features API; beat-grid visualization not planned |
| librespot ToS gray area | Personal use only; not for public/commercial distribution |
| Spotify Premium required | Documented in README; librespot rejects free accounts at the protocol level |

---

## Reference

- [spotify-player](https://github.com/aome510/spotify-player) — production-viable proof that the librespot + rspotify + Ratatui + FFT stack works. The DJ-specific additions (dual deck, crossfader, BPM/key UI) are the novel parts of this project.
- [librespot](https://github.com/librespot-org/librespot)
- [rspotify docs](https://docs.rs/rspotify)
- [Ratatui](https://ratatui.rs)
- [Spotify Audio Features reference](https://developer.spotify.com/documentation/web-api/reference/get-audio-features)
