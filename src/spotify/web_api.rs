use anyhow::{Context, Result};
use rspotify::{
    AuthCodePkceSpotify,
    model::{ArtistId, PlayableId, SearchResult, SearchType, TrackId},
    prelude::*,
};
use std::collections::HashMap;
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
                let first_artist = t.artists.into_iter().next()?;
                let artist_id = first_artist
                    .id
                    .as_ref()
                    .map(|a| a.id().to_string())
                    .unwrap_or_default();
                let artist = first_artist.name;
                let album = t.album.name;
                let release_year = t.album.release_date.as_deref().and_then(parse_year);
                let duration_ms = t.duration.num_milliseconds().max(0) as u32;
                #[allow(deprecated)]
                let popularity = (t.popularity as u8).min(100);

                Some(TrackSummary {
                    id,
                    artist_id,
                    title: t.name,
                    artist,
                    album,
                    release_year,
                    duration_ms,
                    popularity,
                    explicit: t.explicit,
                    genres: Vec::new(), // filled in by fetch_artist_genres
                    bpm: None,
                })
            })
            .collect();

        Ok(summaries)
    }

    /// Batch-fetch genres for a list of artist IDs (max 50 per call).
    pub async fn fetch_artist_genres(
        &self,
        artist_ids: &[String],
    ) -> Result<HashMap<String, Vec<String>>> {
        let ids: Vec<ArtistId<'_>> = artist_ids
            .iter()
            .filter(|id| !id.is_empty())
            .filter_map(|id| ArtistId::from_id(id.as_str()).ok())
            .collect();

        if ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut map = HashMap::new();
        for id in ids {
            if let Ok(artist) = self.client.artist(id).await {
                // genres field is deprecated by Spotify but remains the only available source
                #[allow(deprecated)]
                map.insert(artist.id.id().to_string(), artist.genres);
            }
        }

        Ok(map)
    }
}

fn parse_year(date: &str) -> Option<u16> {
    date.get(..4)?.parse().ok()
}
