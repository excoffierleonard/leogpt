//! Error types and result aliases for the leogpt bot.

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
    OpenRouterApi { status: StatusCode, message: String },

    #[error("OpenRouter response error: {0}")]
    OpenRouterResponse(String),

    #[error("HTTP request error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Parse error: {0}")]
    Parse(#[from] std::num::ParseIntError),

    #[error("Base64 decode error: {0}")]
    Base64Decode(#[from] base64::DecodeError),

    #[error("WAV encoding error: {0}")]
    Wav(#[from] hound::Error),

    #[error("Data URL error: {0}")]
    DataUrl(#[from] data_url::DataUrlError),

    #[error("Data URL base64 decode error: {0}")]
    DataUrlBase64(#[from] data_url::forgiving_base64::InvalidBase64),

    #[error("SSE stream error: {0}")]
    EventSource(#[from] eventsource_stream::EventStreamError<reqwest::Error>),

    #[error("Tool execution error: {0}")]
    ToolExecution(String),

    #[error("Tool loop limit exceeded")]
    ToolLoopLimit,

    #[error("Not in a server (DM context)")]
    NotInServer,

    #[error("User not in voice channel")]
    NotInVoiceChannel,

    #[error("Voice manager not available")]
    MissingVoiceManager,

    #[error("Failed to join voice channel: {0}")]
    VoiceJoin(Box<songbird::error::JoinError>),

    #[error("Audio file not found: {0}")]
    AudioFileNotFound(String),

    #[error("Music storage not configured")]
    MusicNotConfigured,

    #[error("S3 error: {0}")]
    S3(String),

    #[error("S3 error: {0}")]
    S3Sdk(Box<aws_sdk_s3::Error>),

    #[error("S3 presign config error: {0}")]
    S3PresignConfig(#[from] aws_sdk_s3::presigning::PresigningConfigError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<poise::serenity_prelude::Error> for BotError {
    fn from(err: poise::serenity_prelude::Error) -> Self {
        BotError::Serenity(Box::new(err))
    }
}

impl From<songbird::error::JoinError> for BotError {
    fn from(err: songbird::error::JoinError) -> Self {
        BotError::VoiceJoin(Box::new(err))
    }
}

impl From<aws_sdk_s3::Error> for BotError {
    fn from(err: aws_sdk_s3::Error) -> Self {
        BotError::S3Sdk(Box::new(err))
    }
}

impl<E, R> From<aws_sdk_s3::error::SdkError<E, R>> for BotError
where
    aws_sdk_s3::Error: From<aws_sdk_s3::error::SdkError<E, R>>,
{
    fn from(err: aws_sdk_s3::error::SdkError<E, R>) -> Self {
        BotError::S3Sdk(Box::new(err.into()))
    }
}

impl BotError {
    /// Returns a user-friendly error message suitable for displaying in Discord
    #[must_use]
    pub fn user_message(&self) -> String {
        match self {
            BotError::Serenity(_) => {
                "Sorry, I'm having trouble communicating with Discord right now. Please try again later.".to_string()
            }
            BotError::Config(_) | BotError::EnvVar(_) => {
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
            BotError::Json(_) | BotError::Parse(_) => {
                "Sorry, I encountered a data processing error. Please try again.".to_string()
            }
            BotError::ToolExecution(_) => {
                "Sorry, I encountered an error while trying to look up information. Please try again.".to_string()
            }
            BotError::ToolLoopLimit => {
                "Sorry, I got stuck in a loop. Please try rephrasing your request.".to_string()
            }
            BotError::Base64Decode(_) | BotError::DataUrl(_) | BotError::DataUrlBase64(_) => {
                "Sorry, I encountered an error processing image data. Please try again.".to_string()
            }
            BotError::Wav(_) => {
                "Sorry, I encountered an error creating audio data. Please try again.".to_string()
            }
            BotError::EventSource(_) => {
                "Sorry, I encountered an error streaming audio data. Please try again.".to_string()
            }
            BotError::NotInServer => {
                "Sorry, this command only works in a server, not in DMs.".to_string()
            }
            BotError::NotInVoiceChannel => {
                "You need to be in a voice channel to use this command.".to_string()
            }
            BotError::MissingVoiceManager => {
                "Voice features are not available. Please try again later.".to_string()
            }
            BotError::VoiceJoin(_) => {
                "Failed to join the voice channel. Please check my permissions.".to_string()
            }
            BotError::AudioFileNotFound(name) => {
                format!("Couldn't find a song matching \"{name}\".")
            }
            BotError::MusicNotConfigured => {
                "Music playback is not configured on this bot.".to_string()
            }
            BotError::S3(_) | BotError::S3Sdk(_) | BotError::S3PresignConfig(_) => {
                "Sorry, I encountered a problem fetching music from storage.".to_string()
            }
            BotError::Io(_) => {
                "An error occurred reading audio files.".to_string()
            }
        }
    }
}

pub type Result<T> = std::result::Result<T, BotError>;
