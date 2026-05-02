# ADR 003: Single-stream dual-deck model

**Status**: Accepted  
**Date**: 2026-05-01

## Context

A DJ interface conventionally has two decks: one playing to the audience, one being prepared in headphones. This implies two simultaneous audio streams. Spotify's protocol only permits one active audio stream per account — starting a second stream silently kills the first.

Options considered:

- **Two Spotify accounts** — requires the user to own two Premium subscriptions; unreasonable for a personal tool
- **One deck only** — loses the core DJ workflow; not viable
- **True dual stream via audio file access** — would require downloading and decoding the audio files outside the streaming protocol; not feasible without violating Spotify's terms more significantly
- **Metadata-only cued deck** — Deck B displays track metadata (BPM, key, energy, waveform) while Deck A holds the audio stream; audio switches to Deck B at the crossfade midpoint

## Decision

Use a **metadata-only cued deck** model.

- **Deck A (active)**: holds the librespot audio stream; FFT visualizer is live
- **Deck B (cued)**: track is loaded from the library; BPM/key/energy/waveform are displayed; no audio plays until crossfade
- **Crossfade**: a volume ramp gradually reduces Deck A's volume to zero. At the 50% midpoint, librespot issues a play command for Deck B's track URI. When Deck A reaches zero, the decks swap roles

## Consequences

- This matches real hardware DJ behavior — a DJ prepares the next track on the cued deck and previews it in headphones before fading it in; the constraint is invisible to the user
- BPM and waveform data for Deck B must come from somewhere other than live PCM (BPM detection requires playback). See ADR 004 and ADR 005
- The crossfade is not a true audio blend — it is a volume fade-out on A followed by a play command for B. There is a brief moment at the midpoint where neither deck's audio is at full volume; this is audible but acceptable for a personal tool
- Deck role swap at crossfade completion keeps the mental model consistent: the deck you were preparing becomes the active deck
