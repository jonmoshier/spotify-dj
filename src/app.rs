use crate::config::Config;
use librespot_metadata::audio::AudioItem;
use librespot_playback::player::PlayerEvent;

use crate::spotify::player::primary_artist;

pub enum CrossfadeTick {
    Continue,
    PlayTrack(String), // URI to start playing on the incoming deck
    Complete,
}

pub struct CrossfadeState {
    pub total_ms: u32,
    pub elapsed_ms: u32,
    pub midpoint_fired: bool,
    pub cued_uri: String,
    pub start_volume: u8,  // active deck volume at fade start
    pub target_volume: u8, // incoming deck volume at fade end
}

pub enum WebApiEvent {
    SearchResults(Vec<TrackSummary>),
}

/// Which panel currently has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiFocus {
    DeckA,
    DeckB,
    Library,
    Mixer,
}

impl Default for UiFocus {
    fn default() -> Self {
        Self::Library
    }
}

/// Which deck is the active (playing) one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveDeck {
    A,
    B,
}

#[derive(Debug, Clone, Default)]
pub struct DeckState {
    pub track_uri: Option<String>,
    pub track_title: Option<String>,
    pub track_artist: Option<String>,
    pub duration_ms: u32,
    pub position_ms: u32,
    pub is_playing: bool,
    /// True when a track was loaded via L/R but play_track hasn't been called yet.
    /// False once librespot owns the track (either we started it or it came from the phone).
    pub needs_initial_play: bool,
    pub bpm: Option<f32>,
    pub key: Option<String>,
    pub energy: Option<f32>,
    pub volume: u8, // 0–100
}

#[derive(Debug, Clone, Default)]
pub struct LibraryState {
    pub search_query: String,
    pub results: Vec<TrackSummary>,
    pub selected: usize,
    pub is_searching: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TrackSummary {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub duration_ms: u32,
    pub bpm: Option<f32>,
}

pub struct AppState {
    pub config: Config,
    pub deck_a: DeckState,
    pub deck_b: DeckState,
    pub active_deck: ActiveDeck,
    pub crossfader: f32, // -1.0 = full A, 1.0 = full B
    pub crossfade: Option<CrossfadeState>,
    pub library: LibraryState,
    pub focus: UiFocus,
    pub should_quit: bool,
    pub status_message: Option<String>,
    /// Latest FFT band magnitudes (0..1) from the active audio stream.
    pub fft_bands: Vec<f32>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let volume = config.ui.default_volume;
        let mut deck_a = DeckState::default();
        deck_a.volume = volume;
        let mut deck_b = DeckState::default();
        deck_b.volume = 0;

        Self {
            config,
            deck_a,
            deck_b,
            active_deck: ActiveDeck::A,
            crossfader: -1.0,
            crossfade: None,
            library: LibraryState::default(),
            focus: UiFocus::Library,
            should_quit: false,
            status_message: None,
            fft_bands: Vec::new(),
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some(msg.into());
    }

    pub fn active_deck_mut(&mut self) -> &mut DeckState {
        match self.active_deck {
            ActiveDeck::A => &mut self.deck_a,
            ActiveDeck::B => &mut self.deck_b,
        }
    }

    pub fn active_deck_state(&self) -> &DeckState {
        match self.active_deck {
            ActiveDeck::A => &self.deck_a,
            ActiveDeck::B => &self.deck_b,
        }
    }

    pub fn inactive_deck_mut(&mut self) -> &mut DeckState {
        match self.active_deck {
            ActiveDeck::A => &mut self.deck_b,
            ActiveDeck::B => &mut self.deck_a,
        }
    }

    pub fn inactive_deck_state(&self) -> &DeckState {
        match self.active_deck {
            ActiveDeck::A => &self.deck_b,
            ActiveDeck::B => &self.deck_a,
        }
    }

    /// Load a library track's metadata onto the specified deck (no audio starts).
    pub fn load_to_deck(&mut self, track: &TrackSummary, deck: ActiveDeck) {
        let uri = track.id.clone(); // already a full spotify:track:XXX URI from rspotify
        let d = match deck {
            ActiveDeck::A => &mut self.deck_a,
            ActiveDeck::B => &mut self.deck_b,
        };
        d.track_uri = Some(uri);
        d.track_title = Some(track.title.clone());
        d.track_artist = Some(track.artist.clone());
        d.duration_ms = track.duration_ms;
        d.position_ms = 0;
        d.bpm = track.bpm;
        d.key = None;
        d.energy = None;
        d.is_playing = false;
        d.needs_initial_play = true;
    }

    pub fn swap_active_deck(&mut self) {
        self.active_deck = match self.active_deck {
            ActiveDeck::A => ActiveDeck::B,
            ActiveDeck::B => ActiveDeck::A,
        };
    }

    /// Begin a crossfade to the inactive deck. Returns `None` if the inactive deck has no track.
    pub fn start_crossfade(&mut self, duration_secs: u64) -> bool {
        let Some(uri) = self.inactive_deck_state().track_uri.clone() else {
            return false;
        };
        let start_volume = self.active_deck_state().volume;
        let target_volume = self.config.ui.default_volume;
        self.crossfade = Some(CrossfadeState {
            total_ms: (duration_secs as u32).saturating_mul(1000),
            elapsed_ms: 0,
            midpoint_fired: false,
            cued_uri: uri,
            start_volume,
            target_volume,
        });
        true
    }

    /// Advance the crossfade by `delta_ms`. Called from the 100ms redraw tick.
    pub fn tick_crossfade(&mut self, delta_ms: u32) -> CrossfadeTick {
        let (total_ms, elapsed, midpoint_fired, start_vol, target_vol, cued_uri) =
            match self.crossfade.as_ref() {
                None => return CrossfadeTick::Continue,
                Some(cf) => (
                    cf.total_ms,
                    cf.elapsed_ms,
                    cf.midpoint_fired,
                    cf.start_volume,
                    cf.target_volume,
                    cf.cued_uri.clone(),
                ),
            };

        let new_elapsed = (elapsed + delta_ms).min(total_ms);
        let progress = new_elapsed as f32 / total_ms as f32;

        // Volume ramp: active fades out, incoming fades in
        let active_vol = (start_vol as f32 * (1.0 - progress)).round() as u8;
        let incoming_vol = (target_vol as f32 * progress).round() as u8;
        self.active_deck_mut().volume = active_vol;
        self.inactive_deck_mut().volume = incoming_vol;

        // Crossfader tracks the fade
        self.crossfader = match self.active_deck {
            ActiveDeck::A => -1.0 + 2.0 * progress,
            ActiveDeck::B => 1.0 - 2.0 * progress,
        };

        if let Some(cf) = self.crossfade.as_mut() {
            cf.elapsed_ms = new_elapsed;
        }

        if !midpoint_fired && progress >= 0.5 {
            if let Some(cf) = self.crossfade.as_mut() {
                cf.midpoint_fired = true;
            }
            return CrossfadeTick::PlayTrack(cued_uri);
        }

        if progress >= 1.0 {
            CrossfadeTick::Complete
        } else {
            CrossfadeTick::Continue
        }
    }

    /// Complete the crossfade: swap decks, reset volumes, clear state.
    pub fn finish_crossfade(&mut self) {
        let target_volume = self
            .crossfade
            .as_ref()
            .map(|cf| cf.target_volume)
            .unwrap_or(self.config.ui.default_volume);
        self.crossfade = None;
        self.swap_active_deck();
        self.active_deck_mut().volume = target_volume;
        self.inactive_deck_mut().volume = 0;
        self.crossfader = match self.active_deck {
            ActiveDeck::A => -1.0,
            ActiveDeck::B => 1.0,
        };
    }

    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            UiFocus::DeckA => UiFocus::DeckB,
            UiFocus::DeckB => UiFocus::Mixer,
            UiFocus::Mixer => UiFocus::Library,
            UiFocus::Library => UiFocus::DeckA,
        };
    }

    /// Apply a librespot PlayerEvent to the active deck.
    pub fn apply_player_event(&mut self, event: PlayerEvent) {
        let deck = self.active_deck_mut();
        match event {
            PlayerEvent::Playing { position_ms, .. } => {
                deck.is_playing = true;
                deck.position_ms = position_ms;
            }
            PlayerEvent::Paused { position_ms, .. } => {
                deck.is_playing = false;
                deck.position_ms = position_ms;
            }
            PlayerEvent::Stopped { .. } => {
                deck.is_playing = false;
            }
            PlayerEvent::Seeked { position_ms, .. }
            | PlayerEvent::PositionChanged { position_ms, .. }
            | PlayerEvent::PositionCorrection { position_ms, .. } => {
                deck.position_ms = position_ms;
            }
            PlayerEvent::EndOfTrack { .. } => {
                deck.is_playing = false;
                deck.position_ms = 0;
            }
            PlayerEvent::TrackChanged { audio_item } => {
                self.apply_track_info(*audio_item);
            }
            // Ignore events we don't need to act on.
            _ => {}
        }
    }

    pub fn apply_web_api_event(&mut self, event: WebApiEvent) {
        match event {
            WebApiEvent::SearchResults(results) => {
                self.library.results = results;
                self.library.selected = 0;
            }
        }
    }

    fn apply_track_info(&mut self, item: AudioItem) {
        let deck = self.active_deck_mut();
        deck.track_uri = Some(item.uri.clone());
        deck.track_title = Some(item.name.clone());
        deck.track_artist = Some(primary_artist(&item));
        deck.duration_ms = item.duration_ms;
        deck.position_ms = 0;
        deck.needs_initial_play = false; // librespot owns this track
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use librespot_core::{SpotifyUri, spotify_id::SpotifyId};
    use librespot_playback::player::PlayerEvent;

    fn make_uri() -> SpotifyUri {
        SpotifyUri::Track {
            id: SpotifyId { id: 1 },
        }
    }

    fn default_state() -> AppState {
        AppState::new(Config::default())
    }

    #[test]
    fn new_initializes_defaults() {
        let state = default_state();
        assert_eq!(state.deck_a.volume, Config::default().ui.default_volume);
        assert_eq!(state.deck_b.volume, 0);
        assert_eq!(state.crossfader, -1.0);
        assert!(!state.should_quit);
        assert!(matches!(state.focus, UiFocus::Library));
        assert!(matches!(state.active_deck, ActiveDeck::A));
    }

    #[test]
    fn cycle_focus_visits_all_panels() {
        let mut state = default_state();
        // starts at Library
        state.cycle_focus();
        assert!(matches!(state.focus, UiFocus::DeckA));
        state.cycle_focus();
        assert!(matches!(state.focus, UiFocus::DeckB));
        state.cycle_focus();
        assert!(matches!(state.focus, UiFocus::Mixer));
        state.cycle_focus();
        assert!(matches!(state.focus, UiFocus::Library));
    }

    #[test]
    fn playing_event_sets_is_playing_and_position() {
        let mut state = default_state();
        state.apply_player_event(PlayerEvent::Playing {
            play_request_id: 0,
            track_id: make_uri(),
            position_ms: 12_000,
        });
        assert!(state.deck_a.is_playing);
        assert_eq!(state.deck_a.position_ms, 12_000);
    }

    #[test]
    fn paused_event_clears_is_playing() {
        let mut state = default_state();
        state.deck_a.is_playing = true;
        state.apply_player_event(PlayerEvent::Paused {
            play_request_id: 0,
            track_id: make_uri(),
            position_ms: 5_000,
        });
        assert!(!state.deck_a.is_playing);
        assert_eq!(state.deck_a.position_ms, 5_000);
    }

    #[test]
    fn stopped_event_clears_is_playing() {
        let mut state = default_state();
        state.deck_a.is_playing = true;
        state.apply_player_event(PlayerEvent::Stopped {
            play_request_id: 0,
            track_id: make_uri(),
        });
        assert!(!state.deck_a.is_playing);
    }

    #[test]
    fn end_of_track_resets_position() {
        let mut state = default_state();
        state.deck_a.is_playing = true;
        state.deck_a.position_ms = 60_000;
        state.apply_player_event(PlayerEvent::EndOfTrack {
            play_request_id: 0,
            track_id: make_uri(),
        });
        assert!(!state.deck_a.is_playing);
        assert_eq!(state.deck_a.position_ms, 0);
    }

    #[test]
    fn events_target_active_deck() {
        let mut state = default_state();
        state.active_deck = ActiveDeck::B;
        state.apply_player_event(PlayerEvent::Playing {
            play_request_id: 0,
            track_id: make_uri(),
            position_ms: 1_000,
        });
        assert!(!state.deck_a.is_playing);
        assert!(state.deck_b.is_playing);
    }
}
