# ADR 005: PCM-derived waveform cache over Spotify Audio Analysis API

**Status**: Accepted  
**Date**: 2026-05-01

## Context

A waveform display gives the DJ a visual overview of a track's energy and structure before and during playback. Spotify's `GET /audio-analysis/{id}` endpoint provided per-segment loudness data across the full track, which could be rendered as a waveform immediately on track load.

As of November 2024, the Audio Analysis endpoint was deprecated and removed for non-partner apps.

Options considered:

- **Spotify Audio Analysis API** — unavailable; returns 403 for personal developer accounts
- **Decode audio file locally** — would require full audio file access outside the streaming protocol; not feasible
- **No waveform** — fall back to the real-time FFT spectrum analyzer only; acceptable but loses the full-track overview
- **PCM RMS cache** — collect RMS amplitude values per ~50ms window from the librespot PCM stream during first playback; serialize to a local file keyed by Spotify track ID; on subsequent loads, display the full waveform immediately

## Decision

Use a **PCM RMS cache** stored at `~/.cache/spotify-dj/{track_id}.waveform`.

During first playback, the audio sink computes a downsampled RMS value per ~50ms window and appends it to an in-memory buffer. On track completion, the buffer is serialized to disk. On subsequent loads, the cache file is checked first; a cache hit provides the full waveform before playback begins.

The existing `Visualizer` widget in `src/ui/visualizer.rs` already maps a `&[f32]` slice to terminal width at render time, so no widget changes are required beyond passing waveform data instead of FFT band data and adding a playhead parameter.

## Consequences

- **First play**: the waveform builds in real-time left-to-right as the track plays; no full overview until the track has been played once. This is a known limitation and common pattern (SoundCloud used this approach for years)
- **Subsequent plays**: full waveform is available immediately; the playhead overlaid on the static waveform gives the expected DJ overview
- Cache size is negligible: ~1.2 KB per track at one value per 50ms for a 3-minute song
- Waveform values are true PCM amplitudes rather than Spotify's loudness approximation — more accurate
- Only the active deck accumulates waveform data (PCM only flows for the active stream); Deck B displays a cached waveform or nothing on first load (consistent with ADR 003 and ADR 004)
- Cache is keyed by Spotify track ID, so the same track played on different devices or sessions reuses the cache correctly
