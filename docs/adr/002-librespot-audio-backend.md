# ADR 002: librespot as the audio backend

**Status**: Accepted  
**Date**: 2026-05-01

## Context

Streaming Spotify audio to the local system requires either the official Spotify client or a third-party implementation of the Spotify protocol. The official desktop client provides no programmatic audio API — there is no way to intercept the PCM stream from it, which rules out real-time FFT and BPM detection.

Options considered:

- **Official Spotify client + Spotify Connect remote control** — can send play/pause/seek commands via the Web API, but the audio path is opaque; no PCM access, no FFT, no visualizer
- **librespot** — open-source Rust implementation of the Spotify client protocol; provides a custom `AudioSink` trait that exposes the raw PCM stream before it reaches the speakers; used in production by spotify-player, Spotifyd, and others
- **youtube-dl / yt-dlp pipeline** — downloads audio outside of Spotify entirely; legally riskier, loses Spotify Connect integration, not viable

## Decision

Use **librespot** (`librespot-core`, `librespot-playback`, `librespot-connect`, `librespot-metadata` at 0.8.x).

librespot is the only viable path to raw PCM access within the Spotify ecosystem. The custom `AudioSink` interface is the foundation for every audio-dependent feature: the real-time FFT visualizer, BPM detection, waveform caching, and crossfade volume control.

## Consequences

- **Requires Spotify Premium** — librespot is rejected at the protocol level by free accounts; this is a documented requirement
- **ToS gray area** — librespot uses Spotify's internal protocols, not the public Web API. It is not officially sanctioned. This project is personal use only and must not be distributed commercially
- **Registers as a Spotify Connect device** — the app appears in the device picker on other Spotify clients, which is useful but also means it can receive remote commands
- **One audio stream per account** — the Spotify protocol enforces a single active stream; this shapes the entire dual-deck model (see ADR 003)
- librespot's API surface is stable enough for this use case; spotify-player has validated the stack at production scale
