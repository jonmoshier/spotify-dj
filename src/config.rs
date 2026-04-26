use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub auth: AuthConfig,
    pub playback: PlaybackConfig,
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub client_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackConfig {
    pub device_name: String,
    pub bitrate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub crossfade_duration_secs: u64,
    pub default_volume: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auth: AuthConfig {
                client_id: String::new(),
            },
            playback: PlaybackConfig {
                device_name: "spotify-dj".to_string(),
                bitrate: 320,
            },
            ui: UiConfig {
                crossfade_duration_secs: 10,
                default_volume: 80,
            },
        }
    }
}

impl Config {
    pub fn config_path() -> Result<PathBuf> {
        let proj = ProjectDirs::from("", "", "spotify-dj")
            .context("could not determine config directory")?;
        Ok(proj.config_dir().join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            let config = Config::default();
            config.save()?;
            return Ok(config);
        }

        let text = fs::read_to_string(&path)
            .with_context(|| format!("could not read config at {}", path.display()))?;

        toml::from_str(&text)
            .with_context(|| format!("invalid config at {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("could not create config dir {}", parent.display()))?;
        }

        let text = toml::to_string_pretty(self).context("could not serialize config")?;
        fs::write(&path, text)
            .with_context(|| format!("could not write config to {}", path.display()))
    }

    pub fn config_dir() -> Result<PathBuf> {
        let proj = ProjectDirs::from("", "", "spotify-dj")
            .context("could not determine config directory")?;
        Ok(proj.config_dir().to_path_buf())
    }
}
