use reqwest::StatusCode;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BotError {
    #[error("Serenity error: {0}")]
    Serenity(Box<poise::serenity_prelude::Error>),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Environment variable error: {0}")]
    EnvVar(#[from] std::env::VarError),

    #[error("OpenRouter API error ({status}): {message}")]
    OpenRouterApi {
        status: reqwest::StatusCode,
        message: String,
    },

    #[error("OpenRouter response error: {0}")]
    OpenRouterResponse(String),

    #[error("HTTP request error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Tool execution error: {0}")]
    ToolExecution(String),

    #[error("Tool loop limit exceeded")]
    ToolLoopLimit,
}

impl From<poise::serenity_prelude::Error> for BotError {
    fn from(err: poise::serenity_prelude::Error) -> Self {
        BotError::Serenity(Box::new(err))
    }
}

impl BotError {
    /// Returns a user-friendly error message suitable for displaying in Discord
    pub fn user_message(&self) -> String {
        match self {
            BotError::Serenity(_) => {
                "Sorry, I'm having trouble communicating with Discord right now. Please try again later.".to_string()
            }
            BotError::Config(_) => {
                "Sorry, there's a configuration issue on my end. Please contact the bot administrator.".to_string()
            }
            BotError::EnvVar(_) => {
                "Sorry, there's a configuration issue on my end. Please contact the bot administrator.".to_string()
            }
            BotError::OpenRouterApi { status, .. } => {
                match *status {
                    StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                        "Sorry, I'm having authentication issues with my AI service. Please contact the bot administrator.".to_string()
                    }
                    StatusCode::TOO_MANY_REQUESTS => {
                        "Sorry, I've hit my rate limit. Please try again in a few moments.".to_string()
                    }
                    status if status.is_server_error() => {
                        "Sorry, the AI service is experiencing issues right now. Please try again later.".to_string()
                    }
                    status if status.is_client_error() => {
                        "Sorry, there was an issue with my request to the AI service. Please try again or contact the bot administrator.".to_string()
                    }
                    _ => {
                        "Sorry, I'm having trouble connecting to my AI service. Please try again later.".to_string()
                    }
                }
            }
            BotError::OpenRouterResponse(_) => {
                "Sorry, I received an unexpected response from my AI service. Please try again.".to_string()
            }
            BotError::Reqwest(_) => {
                "Sorry, I'm having network issues. Please try again in a moment.".to_string()
            }
            BotError::ToolExecution(_) => {
                "Sorry, I encountered an error while trying to look up information. Please try again.".to_string()
            }
            BotError::ToolLoopLimit => {
                "Sorry, I got stuck in a loop. Please try rephrasing your request.".to_string()
            }
        }
    }
}

pub type Result<T> = std::result::Result<T, BotError>;
