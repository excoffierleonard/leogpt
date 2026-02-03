//! Configuration management for the leogpt bot.

use std::env;
use std::path::PathBuf;

use log::{debug, info, warn};

use crate::error::Result;

/// Bot configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    pub discord_token: String,
    pub openrouter_api_key: String,
    /// Optional directory containing music files for voice playback.
    pub music_dir: Option<PathBuf>,
}

impl Config {
    /// Load configuration from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error if required environment variables are missing.
    pub fn from_env() -> Result<Self> {
        debug!("Loading configuration from environment");
        dotenvy::dotenv().ok();

        let discord_token = env::var("DISCORD_TOKEN")?;
        let openrouter_api_key = env::var("OPENROUTER_API_KEY")?;

        // Optional music directory
        let music_dir = env::var("MUSIC_DIR").ok().map(PathBuf::from);
        if let Some(ref dir) = music_dir {
            info!("Music directory configured: {}", dir.display());
        } else {
            warn!("MUSIC_DIR not set - music playback disabled");
        }

        info!("Configuration loaded successfully");
        debug!("Discord token length: {} characters", discord_token.len());
        debug!(
            "OpenRouter API key length: {} characters",
            openrouter_api_key.len()
        );
        Ok(Self {
            discord_token,
            openrouter_api_key,
            music_dir,
        })
    }
}
