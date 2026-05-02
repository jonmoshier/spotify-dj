# ADR 001: OAuth2 PKCE for authentication

**Status**: Accepted  
**Date**: 2026-05-01

## Context

spotify-dj is a CLI application with no server component. It needs to authenticate with the Spotify Web API to perform searches, browse playlists, and share OAuth tokens with librespot.

Spotify offers three authorization flows for developer apps:
- **Client Credentials** — app-only, no user context, cannot access user data or enable streaming
- **Authorization Code** — requires a client secret, which would have to be embedded in the binary or stored in config; anyone who obtains it can impersonate the app
- **Authorization Code with PKCE** — no client secret required; uses a one-time code verifier/challenge pair per flow; designed for public clients (CLIs, mobile apps, SPAs)

## Decision

Use **Authorization Code with PKCE** (`AuthCodePkceSpotify` from rspotify).

On first run, the app opens the Spotify authorization URL in the system browser and spins up a one-shot TCP listener on `127.0.0.1:8888` to capture the redirect callback. The resulting tokens are persisted to `~/.config/spotify-dj/tokens.json` with Unix permissions `0600`. On subsequent runs, the saved tokens are loaded and refreshed silently; the full browser flow only re-runs if refresh fails.

## Consequences

- No client secret to protect or embed — the binary can be shared freely
- First run requires a browser, which is unavoidable for any Spotify user-auth flow
- Port 8888 must be available during the auth flow; this is a one-time requirement and fails gracefully
- Token file is owner-readable only on Unix, which is the appropriate protection for a credential file in a home directory
- The same `AuthCodePkceSpotify` client is reused for all Web API calls, so auth and API share one token lifecycle
