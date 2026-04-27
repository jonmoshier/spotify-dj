use anyhow::{Context, Result};
use librespot_connect::{ConnectConfig, Spirc};
use librespot_core::{
    Session,
    authentication::Credentials,
    config::{DeviceType, SessionConfig},
};
use librespot_metadata::audio::{AudioItem, UniqueFields};
use librespot_playback::{
    audio_backend,
    config::{AudioFormat, Bitrate, PlayerConfig},
    mixer::{Mixer, MixerConfig, softmixer::SoftMixer},
    player::{Player, PlayerEventChannel},
};
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;

use crate::audio::{bpm::BpmDetector, sink::TeeSink};
use crate::config::Config;

pub struct SpotifyPlayer {
    pub spirc: Spirc,
    pub player: Arc<Player>,
    pub session: Session,
    pub bpm_rx: watch::Receiver<Option<f32>>,
    pub device_id: String,
}

impl SpotifyPlayer {
    pub async fn new(config: &Config, access_token: String) -> Result<Self> {
        let device_id = format!("spotify-dj-{}", &config.playback.device_name);
        let session_config = SessionConfig {
            device_id: device_id.clone(),
            ..Default::default()
        };

        let credentials = Credentials::with_access_token(&access_token);
        let session = Session::new(session_config, None);

        let mixer =
            Arc::new(SoftMixer::open(MixerConfig::default()).context("failed to create mixer")?);

        let player_config = PlayerConfig {
            bitrate: Bitrate::Bitrate320,
            position_update_interval: Some(Duration::from_secs(1)),
            ..Default::default()
        };

        let audio_format = AudioFormat::default();
        let volume_getter = mixer.get_soft_volume();

        // PCM channel: audio thread → BPM detector thread
        let (pcm_tx, pcm_rx) = std::sync::mpsc::sync_channel::<Vec<f64>>(128);
        // BPM channel: detector thread → event loop (watch = always reads latest value)
        let (bpm_tx, bpm_rx) = watch::channel::<Option<f32>>(None);

        let sink_builder = audio_backend::find(None).expect("no audio backend found");
        let player = Player::new(player_config, session.clone(), volume_getter, move || {
            let real_sink = sink_builder(None, audio_format);
            Box::new(TeeSink::new(real_sink, pcm_tx))
        });

        // BPM detector runs in a dedicated OS thread (blocking recv loop).
        let detector = BpmDetector::new(pcm_rx, bpm_tx);
        std::thread::spawn(move || detector.run());

        let connect_config = ConnectConfig {
            name: config.playback.device_name.clone(),
            device_type: DeviceType::Computer,
            initial_volume: (config.ui.default_volume as u16) * 655,
            ..Default::default()
        };

        let (spirc, spirc_task) = Spirc::new(
            connect_config,
            session.clone(),
            credentials,
            player.clone(),
            mixer,
        )
        .await
        .context("failed to create Spirc")?;

        tokio::spawn(spirc_task);

        Ok(Self {
            spirc,
            player,
            session,
            bpm_rx,
            device_id,
        })
    }

    pub fn event_channel(&self) -> PlayerEventChannel {
        self.player.get_player_event_channel()
    }

    pub fn play_pause(&self) -> Result<()> {
        self.spirc.play_pause().context("play_pause failed")
    }

    pub fn seek(&self, position_ms: u32) -> Result<()> {
        self.spirc
            .set_position_ms(position_ms)
            .context("seek failed")
    }

    pub fn set_volume(&self, volume_pct: u8) -> Result<()> {
        let vol = (volume_pct as u16).saturating_mul(655);
        self.spirc.set_volume(vol).context("set_volume failed")
    }
}

/// Pull the primary artist name out of an AudioItem.
pub fn primary_artist(item: &AudioItem) -> String {
    if let UniqueFields::Track { artists, .. } = &item.unique_fields {
        if let Some(first) = artists.first() {
            return first.name.clone();
        }
    }
    String::new()
}
