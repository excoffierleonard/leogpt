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
    pub music_s3: Option<S3Config>,
    /// Optional S3 configuration for reaction memes.
    pub meme_s3: Option<S3Config>,
}

/// S3 bucket configuration parsed from a virtual-hosted URL.
#[derive(Debug, Clone)]
pub struct S3Config {
    pub bucket: String,
    pub prefix: String,
    pub endpoint: String,
    pub region: String,
    pub public_base_url: String,
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
        let music_s3 = load_optional_s3("MUSIC_S3_URL", "music playback")?;
        let meme_s3 = load_optional_s3("MEME_S3_URL", "reaction memes")?;

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
            meme_s3,
        })
    }
}

/// Load an optional S3 configuration from an environment variable.
fn load_optional_s3(env_var: &str, label: &str) -> Result<Option<S3Config>> {
    if let Ok(url) = env::var(env_var) {
        let config = parse_s3_url(&url, env_var)?;
        info!(
            "{label} S3 configured: bucket={}, prefix={}, endpoint={}, region={}",
            config.bucket, config.prefix, config.endpoint, config.region
        );
        Ok(Some(config))
    } else {
        warn!("{env_var} not set - {label} disabled");
        Ok(None)
    }
}

/// Parse a virtual-hosted S3 URL into its components.
///
/// Expects format: `https://{bucket}.s3.{region}.example.com/{prefix}/`
fn parse_s3_url(url: &str, var_name: &str) -> Result<S3Config> {
    let url =
        Url::parse(url).map_err(|err| BotError::Config(format!("Invalid {var_name}: {err}")))?;
    let scheme = url.scheme();
    if scheme != "https" && scheme != "http" {
        return Err(BotError::Config(format!(
            "{var_name} must use http or https"
        )));
    }
    let host = url
        .host_str()
        .ok_or_else(|| BotError::Config(format!("{var_name} is missing a host")))?;
    let (bucket, rest) = host.split_once(".s3.").ok_or_else(|| {
        BotError::Config(format!(
            "{var_name} must be virtual-hosted style: https://{{bucket}}.s3.{{region}}.backblazeb2.com/..."
        ))
    })?;
    if bucket.is_empty() {
        return Err(BotError::Config(format!("{var_name} bucket is empty")));
    }
    let region = rest.split('.').next().unwrap_or("");
    if region.is_empty() {
        return Err(BotError::Config(format!("{var_name} region is missing")));
    }
    let endpoint = format!("{scheme}://s3.{rest}");
    let public_base_url = format!("{scheme}://{bucket}.s3.{rest}");
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
    Ok(S3Config {
        bucket: bucket.to_string(),
        prefix,
        endpoint,
        region: region.to_string(),
        public_base_url,
    })
}
