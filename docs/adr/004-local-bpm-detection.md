# ADR 004: Local BPM detection over Spotify Audio Features API

**Status**: Accepted  
**Date**: 2026-05-01

## Context

BPM (and ideally key and energy) are essential metadata for a DJ interface — they tell you whether two tracks are compatible before mixing. Spotify's `GET /audio-features/{id}` endpoint provided pre-computed BPM, key, and energy values for any track.

As of November 2024, Spotify restricted the Audio Features endpoint to partner apps only. Personal developer accounts receive a 403 on this endpoint regardless of OAuth scopes.

Options considered:

- **Spotify Audio Features API** — no longer accessible for personal apps
- **Third-party music analysis service (AcousticBrainz, AudD)** — requires either a separate account/API key or sending audio data to an external service; adds a dependency and introduces privacy concerns
- **Local onset detection on live PCM** — librespot exposes raw PCM; BPM can be derived in real-time via onset detection without any external service
- **Static BPM database (bpm-tag, EchoNest corpus)** — incomplete coverage, stale data, another external dependency

## Decision

Detect BPM **locally via onset detection on the live PCM stream** in `audio/bpm.rs`.

The sink intercepts PCM frames and feeds them to a lightweight onset detector. BPM is estimated and updated approximately every 3 seconds after playback starts. Key and energy are not yet implemented but can be derived from the same PCM stream (chromagram analysis for key, RMS amplitude for energy).

## Consequences

- No external API dependency for BPM — works entirely offline after the initial Spotify auth
- BPM is only available for the **active deck** (Deck A) because it requires a live audio stream; Deck B's BPM display will be empty or stale until it becomes active (see ADR 003)
- BPM updates lag the true value by ~3 seconds at the start of a track; this is acceptable for a DJ use case where BPM is most relevant once the track is underway
- Local onset detection may be less accurate than Spotify's pre-computed values for tracks with variable tempo or complex rhythms; acceptable for the personal-use target
- Key and energy remain unimplemented; they are derivable from the same PCM pipeline when prioritized
