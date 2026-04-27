use crate::config::Config;
use librespot_metadata::audio::AudioItem;
use librespot_playback::player::PlayerEvent;

use crate::spotify::player::primary_artist;

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

    fn apply_track_info(&mut self, item: AudioItem) {
        let deck = self.active_deck_mut();
        deck.track_uri = Some(item.uri.clone());
        deck.track_title = Some(item.name.clone());
        deck.track_artist = Some(primary_artist(&item));
        deck.duration_ms = item.duration_ms;
        deck.position_ms = 0;
    }
}
