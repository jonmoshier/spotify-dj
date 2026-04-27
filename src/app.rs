use crate::config::Config;
use librespot_metadata::audio::AudioItem;
use librespot_playback::player::PlayerEvent;

use crate::spotify::player::primary_artist;

pub enum WebApiEvent {
    AudioFeatures {
        track_uri: String,
        bpm: f32,
        key: String,
        energy: f32,
    },
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
    pub library: LibraryState,
    pub focus: UiFocus,
    pub should_quit: bool,
    pub status_message: Option<String>,
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
            library: LibraryState::default(),
            focus: UiFocus::Library,
            should_quit: false,
            status_message: None,
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
            WebApiEvent::AudioFeatures { track_uri, bpm, key, energy } => {
                for deck in [&mut self.deck_a, &mut self.deck_b] {
                    if deck.track_uri.as_deref() == Some(&track_uri) {
                        deck.bpm = Some(bpm);
                        deck.key = Some(key.clone());
                        deck.energy = Some(energy);
                    }
                }
            }
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
