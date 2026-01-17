use thiserror::Error;

#[derive(Error, Debug)]
pub enum BotError {
    #[error("Serenity error: {0}")]
    Serenity(Box<poise::serenity_prelude::Error>),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Environment variable error: {0}")]
    EnvVar(#[from] std::env::VarError),
}

impl From<poise::serenity_prelude::Error> for BotError {
    fn from(err: poise::serenity_prelude::Error) -> Self {
        BotError::Serenity(Box::new(err))
    }
}

pub type Result<T> = std::result::Result<T, BotError>;
