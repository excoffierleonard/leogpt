use std::env;

use log::{debug, error, info};

use crate::error::Result;

#[derive(Debug, Clone)]
pub struct Config {
    pub discord_token: String,
    pub openrouter_api_key: String,
    pub openrouter_model: String,
    pub system_prompt: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        debug!("Loading configuration from environment");
        dotenvy::dotenv().ok();

        let discord_token = env::var("DISCORD_TOKEN").map_err(|e| {
            error!("Failed to load DISCORD_TOKEN from environment: {}", e);
            e
        })?;

        let openrouter_api_key = env::var("OPENROUTER_API_KEY").map_err(|e| {
            error!("Failed to load OPENROUTER_API_KEY from environment: {}", e);
            e
        })?;

        let openrouter_model = env::var("OPENROUTER_MODEL").map_err(|e| {
            error!("Failed to load OPENROUTER_MODEL from environment: {}", e);
            e
        })?;

        let system_prompt = env::var("SYSTEM_PROMPT").map_err(|e| {
            error!("Failed to load SYSTEM_PROMPT from environment: {}", e);
            e
        })?;

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
