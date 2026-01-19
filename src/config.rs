//! Configuration management for the leogpt bot.

use std::env;

use log::{debug, info};

use crate::error::Result;

/// Bot configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    pub discord_token: String,
    pub openrouter_api_key: String,
    pub openrouter_model: String,
    pub system_prompt: String,
}

impl Config {
    /// Load configuration from environment variables.
    pub fn from_env() -> Result<Self> {
        debug!("Loading configuration from environment");
        dotenvy::dotenv().ok();

        let discord_token = env::var("DISCORD_TOKEN")?;
        let openrouter_api_key = env::var("OPENROUTER_API_KEY")?;
        let openrouter_model = env::var("OPENROUTER_MODEL")?;
        let system_prompt = env::var("SYSTEM_PROMPT")?;

        info!("Configuration loaded successfully");
        debug!("Discord token length: {} characters", discord_token.len());
        debug!(
            "OpenRouter API key length: {} characters",
            openrouter_api_key.len()
        );
        debug!("OpenRouter model: {}", openrouter_model);
        debug!("System prompt length: {} characters", system_prompt.len());

        Ok(Self {
            discord_token,
            openrouter_api_key,
            openrouter_model,
            system_prompt,
        })
    }
}
