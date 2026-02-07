//! Configuration management for the leogpt bot.

use log::{debug, info, warn};
use std::env;

use crate::error::{BotError, Result};

/// Bot configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    pub discord_token: String,
    pub openrouter_api_key: String,
    /// Optional S3 configuration for music playback.
    pub music_s3: Option<MusicS3Config>,
}

/// S3 configuration for music playback.
#[derive(Debug, Clone)]
pub struct MusicS3Config {
    pub bucket: String,
    pub prefix: String,
    pub endpoint: String,
    pub region: String,
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

        // Optional S3 music configuration
        let music_s3 = if let Ok(bucket) = env::var("MUSIC_S3_BUCKET") {
            let endpoint = env::var("MUSIC_S3_ENDPOINT").map_err(|_| {
                BotError::Config(
                    "MUSIC_S3_ENDPOINT is required when MUSIC_S3_BUCKET is set".to_string(),
                )
            })?;
            let region = env::var("MUSIC_S3_REGION").map_err(|_| {
                BotError::Config(
                    "MUSIC_S3_REGION is required when MUSIC_S3_BUCKET is set".to_string(),
                )
            })?;
            let prefix = env::var("MUSIC_S3_PREFIX").unwrap_or_else(|_| "music/".to_string());
            info!(
                "Music S3 configured: bucket={bucket}, prefix={prefix}, endpoint={endpoint}, region={region}"
            );
            Some(MusicS3Config {
                bucket,
                prefix,
                endpoint,
                region,
            })
        } else {
            warn!("MUSIC_S3_BUCKET not set - music playback disabled");
            None
        };

        info!("Configuration loaded successfully");
        debug!("Discord token length: {} characters", discord_token.len());
        debug!(
            "OpenRouter API key length: {} characters",
            openrouter_api_key.len()
        );
        Ok(Self {
            discord_token,
            openrouter_api_key,
            music_s3,
        })
    }
}
