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
    #[serde(default)]
    pub search_presets: Vec<SearchPreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchPreset {
    pub name: String,
    #[serde(default)]
    pub genre: String,
    #[serde(default)]
    pub year: String,
    #[serde(default)]
    pub tag: String,
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
                search_presets: Vec::new(),
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

        toml::from_str(&text).with_context(|| format!("invalid config at {}", path.display()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let config = Config::default();
        assert!(config.auth.client_id.is_empty());
        assert_eq!(config.playback.device_name, "spotify-dj");
        assert_eq!(config.playback.bitrate, 320);
        assert_eq!(config.ui.default_volume, 80);
        assert_eq!(config.ui.crossfade_duration_secs, 10);
    }

    #[test]
    fn toml_round_trip() {
        let original = Config {
            auth: AuthConfig {
                client_id: "abc123".to_string(),
            },
            playback: PlaybackConfig {
                device_name: "my-dj".to_string(),
                bitrate: 160,
            },
            ui: UiConfig {
                crossfade_duration_secs: 5,
                default_volume: 60,
            },
        };
        let serialized = toml::to_string_pretty(&original).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.auth.client_id, "abc123");
        assert_eq!(deserialized.playback.device_name, "my-dj");
        assert_eq!(deserialized.playback.bitrate, 160);
        assert_eq!(deserialized.ui.crossfade_duration_secs, 5);
        assert_eq!(deserialized.ui.default_volume, 60);
    }
}
