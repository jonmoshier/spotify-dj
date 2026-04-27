use anyhow::{Context, Result};
use rspotify::{
    AuthCodePkceSpotify,
    model::{PlayableId, SearchResult, SearchType, TrackId},
    prelude::*,
};
use std::sync::Arc;

use crate::app::TrackSummary;

pub struct SpotifyWebApi {
    client: Arc<AuthCodePkceSpotify>,
}

impl SpotifyWebApi {
    pub fn new(client: Arc<AuthCodePkceSpotify>) -> Self {
        Self { client }
    }

    pub async fn play_track(&self, track_uri: &str, device_id: &str) -> Result<()> {
        let id = TrackId::from_uri(track_uri)
            .with_context(|| format!("invalid track URI: {track_uri}"))?;

        self.client
            .start_uris_playback([PlayableId::Track(id)], Some(device_id), None, None)
            .await
            .context("play_track request failed")?;

        Ok(())
    }

    pub async fn search_tracks(&self, query: &str) -> Result<Vec<TrackSummary>> {
        let result = self
            .client
            .search(query, SearchType::Track, None, None, Some(50), None)
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
                let artist = t
                    .artists
                    .first()
                    .map(|a| a.name.clone())
                    .unwrap_or_default();
                Some(TrackSummary {
                    id,
                    title: t.name,
                    artist,
                    duration_ms: t.duration.num_milliseconds().max(0) as u32,
                    bpm: None,
                })
            })
            .collect();

        Ok(summaries)
    }
}
