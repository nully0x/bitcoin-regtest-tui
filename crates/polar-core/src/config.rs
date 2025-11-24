//! Configuration management.

use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Data directory for networks.
    pub data_dir: PathBuf,
    /// Docker socket path.
    pub docker_socket: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = ProjectDirs::from("", "", "polar-tui")
            .map(|dirs| dirs.data_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".polar"));

        Self {
            data_dir,
            docker_socket: None,
        }
    }
}

impl Config {
    /// Load configuration from disk or create default.
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Save configuration to disk.
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    /// Get configuration file path.
    fn config_path() -> Result<PathBuf> {
        ProjectDirs::from("", "", "polar-tui")
            .map(|dirs| dirs.config_dir().join("config.json"))
            .ok_or_else(|| Error::Config("could not determine config directory".into()))
    }
}
