use crate::config::Config;

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
        deck_b.volume = 0; // cued deck starts silent

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
}
