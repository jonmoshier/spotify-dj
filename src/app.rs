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
    /// When to fire PlayTrack — snapped to nearest bar boundary if BPM is known,
    /// otherwise total_ms / 2.
    pub switch_at_ms: u32,
}

pub enum WebApiEvent {
    SearchResults(Vec<TrackSummary>),
    GenreResults(std::collections::HashMap<String, Vec<String>>), // artist_id → genres
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchFocus {
    #[default]
    None,
    Freetext,
    Genre,
    Year,
    Artist,
    Title,
}

impl SearchFocus {
    pub fn is_active(self) -> bool {
        self != SearchFocus::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagFilter {
    #[default]
    None,
    New,
    Hipster,
}

impl TagFilter {
    pub fn cycle(self) -> Self {
        match self {
            TagFilter::None => TagFilter::New,
            TagFilter::New => TagFilter::Hipster,
            TagFilter::Hipster => TagFilter::None,
        }
    }

    pub fn as_query_str(self) -> Option<&'static str> {
        match self {
            TagFilter::None => None,
            TagFilter::New => Some("tag:new"),
            TagFilter::Hipster => Some("tag:hipster"),
        }
    }

    pub fn label(self) -> Option<&'static str> {
        match self {
            TagFilter::None => None,
            TagFilter::New => Some("new"),
            TagFilter::Hipster => Some("hipster"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    Relevance,
    Popularity,
    Duration,
    Artist,
}

impl SortOrder {
    pub fn cycle(self) -> Self {
        match self {
            SortOrder::Relevance => SortOrder::Popularity,
            SortOrder::Popularity => SortOrder::Duration,
            SortOrder::Duration => SortOrder::Artist,
            SortOrder::Artist => SortOrder::Relevance,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SortOrder::Relevance => "relevance",
            SortOrder::Popularity => "popularity",
            SortOrder::Duration => "duration",
            SortOrder::Artist => "artist",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LibraryState {
    pub search_query: String,
    pub filter_genre: String,
    pub filter_year: String,
    pub filter_artist: String,
    pub filter_title: String,
    pub filter_tag: TagFilter,
    pub sort: SortOrder,
    pub search_focus: SearchFocus,
    pub results: Vec<TrackSummary>,
    results_original: Vec<TrackSummary>,
    pub selected: usize,
}

impl LibraryState {
    pub fn build_query(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if !self.search_query.trim().is_empty() {
            parts.push(self.search_query.trim().to_string());
        }
        if !self.filter_artist.trim().is_empty() {
            parts.push(format!("artist:{}", self.filter_artist.trim()));
        }
        if !self.filter_title.trim().is_empty() {
            parts.push(format!("track:{}", self.filter_title.trim()));
        }
        if !self.filter_genre.trim().is_empty() {
            parts.push(format!("genre:{}", self.filter_genre.trim()));
        }
        if !self.filter_year.trim().is_empty() {
            parts.push(format!("year:{}", self.filter_year.trim()));
        }
        if let Some(tag) = self.filter_tag.as_query_str() {
            parts.push(tag.to_string());
        }
        parts.join(" ")
    }

    pub fn clear_filters(&mut self) {
        self.filter_genre.clear();
        self.filter_year.clear();
        self.filter_artist.clear();
        self.filter_title.clear();
        self.filter_tag = TagFilter::None;
    }

    pub fn has_filters(&self) -> bool {
        !self.filter_genre.is_empty()
            || !self.filter_year.is_empty()
            || !self.filter_artist.is_empty()
            || !self.filter_title.is_empty()
            || self.filter_tag != TagFilter::None
    }

    pub fn apply_sort(&mut self) {
        match self.sort {
            SortOrder::Relevance => self.results = self.results_original.clone(),
            SortOrder::Popularity => self.results.sort_by(|a, b| b.popularity.cmp(&a.popularity)),
            SortOrder::Duration => self
                .results
                .sort_by(|a, b| b.duration_ms.cmp(&a.duration_ms)),
            SortOrder::Artist => self.results.sort_by(|a, b| a.artist.cmp(&b.artist)),
        }
        self.selected = 0;
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrackSummary {
    pub id: String,
    pub artist_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub release_year: Option<u16>,
    pub duration_ms: u32,
    pub popularity: u8,
    pub explicit: bool,
    pub genres: Vec<String>,
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
    /// Latest FFT band magnitudes (0..1), exponentially smoothed.
    pub fft_bands: Vec<f32>,
    /// Per-band peak-hold values that decay after each update.
    pub fft_peaks: Vec<f32>,
    /// When true, automatically start a crossfade as the active track nears its end.
    pub auto_fade: bool,
    /// URI of the last track auto-fade fired on. Prevents re-firing on the same track
    /// (e.g. after the position briefly oscillates near the end).
    pub auto_fade_last_fired_uri: Option<String>,
    pub show_help: bool,
    /// When true, automatically play through the library list after each track ends.
    pub queue_mode: bool,
    /// Library index of the track preloaded to the inactive deck for the next queue step.
    pub queue_next_idx: Option<usize>,
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
            fft_peaks: Vec::new(),
            show_help: false,
            auto_fade: false,
            auto_fade_last_fired_uri: None,
            queue_mode: false,
            queue_next_idx: None,
        }
    }

    pub fn update_fft(&mut self, raw: Vec<f32>) {
        const SMOOTH: f32 = 0.25;
        const PEAK_DECAY: f32 = 0.012;

        if self.fft_bands.len() != raw.len() {
            self.fft_peaks = raw.clone();
            self.fft_bands = raw;
            return;
        }
        for i in 0..raw.len() {
            self.fft_bands[i] = SMOOTH * raw[i] + (1.0 - SMOOTH) * self.fft_bands[i];
            self.fft_peaks[i] = (self.fft_peaks[i] - PEAK_DECAY).max(self.fft_bands[i]);
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

    /// Begin a crossfade to the inactive deck. Returns false if the inactive deck has no track.
    pub fn start_crossfade(&mut self, duration_secs: u64) -> bool {
        let Some(uri) = self.inactive_deck_state().track_uri.clone() else {
            return false;
        };
        let start_volume = self.active_deck_state().volume;
        let target_volume = self.config.ui.default_volume;
        let total_ms = (duration_secs as u32).saturating_mul(1000);
        let switch_at_ms = self.compute_switch_ms(total_ms);
        self.crossfade = Some(CrossfadeState {
            total_ms,
            elapsed_ms: 0,
            midpoint_fired: false,
            cued_uri: uri,
            start_volume,
            target_volume,
            switch_at_ms,
        });
        true
    }

    /// Snap the stream switch to the nearest bar boundary at or after the midpoint.
    /// Falls back to the midpoint if BPM is unavailable.
    fn compute_switch_ms(&self, total_ms: u32) -> u32 {
        let midpoint = total_ms / 2;
        let active = self.active_deck_state();

        let Some(bpm) = active.bpm else {
            return midpoint;
        };
        if bpm <= 0.0 {
            return midpoint;
        }

        let beat_ms = 60_000.0 / bpm;
        let bar_ms = beat_ms * 4.0;

        // Project the track position at the midpoint of the fade.
        let pos_at_mid = active.position_ms as f32 + midpoint as f32;
        let pos_in_bar = pos_at_mid % bar_ms;
        let ms_to_next_bar = bar_ms - pos_in_bar;

        // Snap to that bar boundary, but never past total_ms.
        let candidate = (midpoint as f32 + ms_to_next_bar).round() as u32;
        candidate.min(total_ms)
    }

    /// If auto-fade is enabled, the active track is playing, and its remaining
    /// time is within the configured fade duration, start a crossfade. Returns
    /// `true` if a fade was started this call. Fires at most once per active
    /// track URI to prevent re-firing as `position_ms` oscillates near the end.
    pub fn maybe_auto_fade(&mut self) -> bool {
        if !self.auto_fade || self.crossfade.is_some() {
            return false;
        }
        let active = self.active_deck_state();
        if !active.is_playing || active.duration_ms == 0 {
            return false;
        }
        let Some(active_uri) = active.track_uri.clone() else {
            return false;
        };
        if self.auto_fade_last_fired_uri.as_ref() == Some(&active_uri) {
            return false;
        }
        if self.inactive_deck_state().track_uri.is_none() {
            return false;
        }
        let fade_ms = (self.config.ui.crossfade_duration_secs as u32).saturating_mul(1000);
        if active.duration_ms.saturating_sub(active.position_ms) > fade_ms {
            return false;
        }
        self.auto_fade_last_fired_uri = Some(active_uri);
        self.start_crossfade(self.config.ui.crossfade_duration_secs)
    }

    /// Advance the crossfade by `delta_ms`. Called from the 100ms redraw tick.
    pub fn tick_crossfade(&mut self, delta_ms: u32) -> CrossfadeTick {
        let (total_ms, elapsed, midpoint_fired, start_vol, target_vol, switch_at_ms, cued_uri) =
            match self.crossfade.as_ref() {
                None => return CrossfadeTick::Continue,
                Some(cf) => (
                    cf.total_ms,
                    cf.elapsed_ms,
                    cf.midpoint_fired,
                    cf.start_volume,
                    cf.target_volume,
                    cf.switch_at_ms,
                    cf.cued_uri.clone(),
                ),
            };

        let new_elapsed = (elapsed + delta_ms).min(total_ms);
        let progress = new_elapsed as f32 / total_ms as f32;

        // Equal-power crossfade: cos/sin curves keep perceived loudness constant.
        // Linear ramps create a volume dip at the midpoint; this doesn't.
        let angle = progress * std::f32::consts::FRAC_PI_2;
        let active_vol = (start_vol as f32 * angle.cos()).round() as u8;
        let incoming_vol = (target_vol as f32 * angle.sin()).round() as u8;
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

        if !midpoint_fired && new_elapsed >= switch_at_ms {
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

    /// In queue mode, load the next library track (selected+1) onto the inactive deck
    /// so auto_fade can pick it up. Idempotent: skips if that index is already preloaded.
    pub fn queue_preload_next(&mut self) {
        if !self.queue_mode {
            return;
        }
        let next_idx = self.library.selected + 1;
        if self.queue_next_idx == Some(next_idx) {
            return;
        }
        if let Some(track) = self.library.results.get(next_idx).cloned() {
            let inactive = match self.active_deck {
                ActiveDeck::A => ActiveDeck::B,
                ActiveDeck::B => ActiveDeck::A,
            };
            self.load_to_deck(&track, inactive);
            self.queue_next_idx = Some(next_idx);
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

    pub fn cycle_focus_back(&mut self) {
        self.focus = match self.focus {
            UiFocus::DeckA => UiFocus::Library,
            UiFocus::DeckB => UiFocus::DeckA,
            UiFocus::Mixer => UiFocus::DeckB,
            UiFocus::Library => UiFocus::Mixer,
        };
    }

    /// Apply a librespot PlayerEvent to the appropriate deck.
    ///
    /// Normally events update the active deck. During a crossfade, librespot
    /// will emit events for the *incoming* track (TrackChanged, Playing, etc.)
    /// while the active deck is still the outgoing one. Those events must
    /// target the inactive (incoming) deck — otherwise the new track's
    /// metadata overwrites the outgoing deck and appears duplicated.
    pub fn apply_player_event(&mut self, event: PlayerEvent) {
        let route_to_inactive = match (self.crossfade.as_ref(), event_track_uri(&event)) {
            (Some(cf), Some(uri)) => cf.cued_uri == uri,
            _ => false,
        };
        let deck = if route_to_inactive {
            self.inactive_deck_mut()
        } else {
            self.active_deck_mut()
        };
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
                Self::apply_track_info(deck, &audio_item);
            }
            // Ignore events we don't need to act on.
            _ => {}
        }
    }

    pub fn apply_web_api_event(&mut self, event: WebApiEvent) {
        match event {
            WebApiEvent::SearchResults(results) => {
                self.library.results_original = results.clone();
                self.library.results = results;
                self.library.selected = 0;
                self.library.apply_sort();
            }
            WebApiEvent::GenreResults(genre_map) => {
                for track in &mut self.library.results {
                    if let Some(genres) = genre_map.get(&track.artist_id) {
                        track.genres = genres.clone();
                    }
                }
            }
        }
    }

    fn apply_track_info(deck: &mut DeckState, item: &AudioItem) {
        deck.track_uri = Some(item.uri.clone());
        deck.track_title = Some(item.name.clone());
        deck.track_artist = Some(primary_artist(item));
        deck.duration_ms = item.duration_ms;
        deck.position_ms = 0;
        deck.needs_initial_play = false; // librespot owns this track
    }
}

/// Extract the Spotify URI a PlayerEvent refers to (if any).
fn event_track_uri(event: &PlayerEvent) -> Option<String> {
    match event {
        PlayerEvent::Playing { track_id, .. }
        | PlayerEvent::Paused { track_id, .. }
        | PlayerEvent::Stopped { track_id, .. }
        | PlayerEvent::Seeked { track_id, .. }
        | PlayerEvent::PositionChanged { track_id, .. }
        | PlayerEvent::PositionCorrection { track_id, .. }
        | PlayerEvent::EndOfTrack { track_id, .. }
        | PlayerEvent::Loading { track_id, .. }
        | PlayerEvent::Preloading { track_id, .. }
        | PlayerEvent::Unavailable { track_id, .. }
        | PlayerEvent::TimeToPreloadNextTrack { track_id, .. } => track_id.to_uri().ok(),
        PlayerEvent::TrackChanged { audio_item } => Some(audio_item.uri.clone()),
        _ => None,
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

    /// Set up a state primed for auto-fade: deck A playing near end, deck B cued.
    fn primed_for_auto_fade() -> AppState {
        let mut state = default_state();
        state.auto_fade = true;
        state.config.ui.crossfade_duration_secs = 5;
        state.deck_a.track_uri = Some("spotify:track:A".into());
        state.deck_a.is_playing = true;
        state.deck_a.duration_ms = 200_000;
        state.deck_a.position_ms = 197_000; // 3s remaining < 5s fade
        state.deck_b.track_uri = Some("spotify:track:B".into());
        state
    }

    #[test]
    fn auto_fade_fires_when_track_near_end() {
        let mut state = primed_for_auto_fade();
        assert!(state.maybe_auto_fade());
        assert!(state.crossfade.is_some());
        assert_eq!(
            state.auto_fade_last_fired_uri.as_deref(),
            Some("spotify:track:A")
        );
    }

    #[test]
    fn auto_fade_skips_when_disabled() {
        let mut state = primed_for_auto_fade();
        state.auto_fade = false;
        assert!(!state.maybe_auto_fade());
        assert!(state.crossfade.is_none());
    }

    #[test]
    fn auto_fade_skips_when_far_from_end() {
        let mut state = primed_for_auto_fade();
        state.deck_a.position_ms = 10_000; // 190s remaining
        assert!(!state.maybe_auto_fade());
    }

    #[test]
    fn auto_fade_skips_when_inactive_deck_empty() {
        let mut state = primed_for_auto_fade();
        state.deck_b.track_uri = None;
        assert!(!state.maybe_auto_fade());
    }

    #[test]
    fn auto_fade_skips_when_paused() {
        let mut state = primed_for_auto_fade();
        state.deck_a.is_playing = false;
        assert!(!state.maybe_auto_fade());
    }

    #[test]
    fn auto_fade_fires_at_most_once_per_track() {
        let mut state = primed_for_auto_fade();
        assert!(state.maybe_auto_fade());
        // Cancel the fade and try again with the same active URI — should not refire.
        state.crossfade = None;
        assert!(!state.maybe_auto_fade());
    }

    #[test]
    fn events_for_cued_uri_route_to_inactive_during_crossfade() {
        let mut state = default_state();
        let cued_track_id = make_uri();
        let cued_uri = cued_track_id.to_uri().expect("to_uri");

        // Active deck is mid-track; inactive deck has the cued URI loaded.
        state.deck_a.track_uri = Some("spotify:track:other".into());
        state.deck_a.is_playing = true;
        state.deck_a.duration_ms = 100_000;
        state.deck_a.position_ms = 95_000;
        state.deck_b.track_uri = Some(cued_uri.clone());
        state.config.ui.crossfade_duration_secs = 5;
        assert!(state.start_crossfade(5));

        // Librespot starts streaming the cued track. Active deck (A) must not
        // be touched — it's still fading out the outgoing track.
        state.apply_player_event(PlayerEvent::Playing {
            play_request_id: 0,
            track_id: cued_track_id,
            position_ms: 0,
        });

        assert_eq!(
            state.deck_a.position_ms, 95_000,
            "outgoing deck position should not be reset by incoming track event"
        );
        assert!(
            state.deck_b.is_playing,
            "incoming deck should reflect the new playing state"
        );
        assert_eq!(state.deck_b.position_ms, 0);
    }

    #[test]
    fn events_for_outgoing_uri_still_route_to_active_during_crossfade() {
        let mut state = default_state();
        let outgoing_track_id = make_uri();
        let outgoing_uri = outgoing_track_id.to_uri().expect("to_uri");

        state.deck_a.track_uri = Some(outgoing_uri);
        state.deck_a.is_playing = true;
        state.deck_b.track_uri = Some("spotify:track:incoming".into());
        state.config.ui.crossfade_duration_secs = 5;
        assert!(state.start_crossfade(5));

        // A position update for the still-fading outgoing track should land on
        // the outgoing (active) deck, not the incoming one.
        state.apply_player_event(PlayerEvent::PositionChanged {
            play_request_id: 0,
            track_id: outgoing_track_id,
            position_ms: 50_000,
        });

        assert_eq!(state.deck_a.position_ms, 50_000);
        assert_eq!(state.deck_b.position_ms, 0);
    }

    #[test]
    fn auto_fade_rearms_after_track_change() {
        let mut state = primed_for_auto_fade();
        assert!(state.maybe_auto_fade());
        state.crossfade = None;
        // New track loaded onto active deck.
        state.deck_a.track_uri = Some("spotify:track:C".into());
        assert!(state.maybe_auto_fade());
    }
}
