use anyhow::{Context, Result};
use rspotify::{
    model::{Modality, SearchResult, SearchType, TrackId},
    prelude::*,
    AuthCodePkceSpotify,
};
use std::sync::Arc;

use crate::app::TrackSummary;

pub struct SpotifyWebApi {
    client: Arc<AuthCodePkceSpotify>,
}

pub struct TrackFeatures {
    pub bpm: f32,
    pub key: String,
    pub energy: f32,
}

impl SpotifyWebApi {
    pub fn new(client: Arc<AuthCodePkceSpotify>) -> Self {
        Self { client }
    }

    pub async fn audio_features(&self, track_uri: &str) -> Result<TrackFeatures> {
        let id = TrackId::from_uri(track_uri)
            .with_context(|| format!("invalid track URI: {track_uri}"))?;

        let features = self
            .client
            .track_features(id)
            .await
            .context("audio_features request failed")?;

        Ok(TrackFeatures {
            bpm: features.tempo,
            key: key_name(features.key, features.mode),
            energy: features.energy,
        })
    }

    pub async fn search_tracks(&self, query: &str) -> Result<Vec<TrackSummary>> {
        let result = self
            .client
            .search(query, SearchType::Track, None, None, Some(10), None)
            .await
            .context("search request failed")?;

        let SearchResult::Tracks(page) = result else {
            return Ok(vec![]);
        };

        let summaries = page
            .items
            .into_iter()
            .filter_map(|t| {
                let id = t.id?.to_string();
                let artist = t.artists.first().map(|a| a.name.clone()).unwrap_or_default();
                Some(TrackSummary {
                    id,
                    title: t.name,
                    artist,
                    duration_ms: t.duration.num_milliseconds().max(0) as u32,
                    bpm: None, // populated later via audio_features if needed
                })
            })
            .collect();

        Ok(summaries)
    }
}

fn key_name(key: i32, mode: Modality) -> String {
    let note = match key {
        0 => "C",
        1 => "C♯",
        2 => "D",
        3 => "D♯",
        4 => "E",
        5 => "F",
        6 => "F♯",
        7 => "G",
        8 => "G♯",
        9 => "A",
        10 => "A♯",
        11 => "B",
        _ => "?",
    };
    let mode_str = match mode {
        Modality::Major => "maj",
        Modality::Minor => "min",
        Modality::NoResult => "",
    };
    if mode_str.is_empty() {
        note.to_string()
    } else {
        format!("{note} {mode_str}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_name_major() {
        assert_eq!(key_name(0, Modality::Major), "C maj");
        assert_eq!(key_name(9, Modality::Major), "A maj");
    }

    #[test]
    fn key_name_minor() {
        assert_eq!(key_name(9, Modality::Minor), "A min");
        assert_eq!(key_name(1, Modality::Minor), "C♯ min");
    }

    #[test]
    fn key_name_no_result() {
        assert_eq!(key_name(0, Modality::NoResult), "C");
    }
}
