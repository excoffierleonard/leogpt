//! Configuration management for the leogpt bot.

use std::env;

use log::{debug, info, warn};
use url::Url;

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
        let music_s3 = if let Ok(url) = env::var("MUSIC_S3_URL") {
            let config = parse_music_s3_url(&url)?;
            info!(
                "Music S3 configured: bucket={}, prefix={}, endpoint={}, region={}",
                config.bucket, config.prefix, config.endpoint, config.region
            );
            Some(config)
        } else {
            warn!("MUSIC_S3_URL not set - music playback disabled");
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

fn parse_music_s3_url(url: &str) -> Result<MusicS3Config> {
    let url =
        Url::parse(url).map_err(|err| BotError::Config(format!("Invalid MUSIC_S3_URL: {err}")))?;
    let scheme = url.scheme();
    if scheme != "https" && scheme != "http" {
        return Err(BotError::Config(
            "MUSIC_S3_URL must use http or https".to_string(),
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| BotError::Config("MUSIC_S3_URL is missing a host".to_string()))?;
    let (bucket, rest) = host.split_once(".s3.").ok_or_else(|| {
        BotError::Config(
            "MUSIC_S3_URL must be virtual-hosted style: https://{bucket}.s3.{region}.backblazeb2.com/..."
                .to_string(),
        )
    })?;
    if bucket.is_empty() {
        return Err(BotError::Config("MUSIC_S3_URL bucket is empty".to_string()));
    }
    let region = rest.split('.').next().unwrap_or("");
    if region.is_empty() {
        return Err(BotError::Config(
            "MUSIC_S3_URL region is missing".to_string(),
        ));
    }
    let endpoint = format!("{scheme}://s3.{rest}");
    let raw_path = url.path().trim_start_matches('/');
    let prefix = if raw_path.is_empty() {
        String::new()
    } else if raw_path.ends_with('/') {
        raw_path.to_string()
    } else if let Some((parent, _)) = raw_path.rsplit_once('/') {
        format!("{parent}/")
    } else {
        String::new()
    };
    Ok(MusicS3Config {
        bucket: bucket.to_string(),
        prefix,
        endpoint,
        region: region.to_string(),
    })
}
