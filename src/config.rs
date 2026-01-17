use std::env;

use crate::error::Result;

#[derive(Debug, Clone)]
pub struct Config {
    pub discord_token: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let discord_token = env::var("DISCORD_TOKEN")?;

        Ok(Self { discord_token })
    }
}
