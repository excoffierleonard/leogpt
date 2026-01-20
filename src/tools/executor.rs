//! Tool executor for dispatching tool calls.

use log::{debug, warn};
use poise::serenity_prelude::{ChannelId, Context, GuildId};

use crate::error::{BotError, Result};

use super::audio_gen::generate_audio;
use super::image_gen::generate_image;
use super::search::search_channel_history;
use super::server_info::get_server_info;
use super::user_info::get_user_info;
use super::web_search::web_search;

/// Context needed to execute tools
pub struct ToolContext<'a> {
    pub ctx: &'a Context,
    pub channel_id: ChannelId,
    pub guild_id: Option<GuildId>,
    pub openrouter_api_key: &'a str,
    /// Image URLs from the conversation history (most recent first)
    pub recent_images: Vec<String>,
}

/// Image attachment data to be sent to Discord
pub struct ImageAttachment {
    /// Raw image bytes (decoded from base64)
    pub data: Vec<u8>,
    /// Filename for the attachment
    pub filename: String,
}

/// Audio attachment data to be sent to Discord
pub struct AudioAttachment {
    /// Raw audio bytes (decoded from base64)
    pub data: Vec<u8>,
    /// Filename for the attachment
    pub filename: String,
}

/// Output from a tool execution
pub struct ToolOutput {
    /// Text result for the LLM conversation
    pub text: String,
    /// Optional image to send as Discord attachment
    pub image: Option<ImageAttachment>,
    /// Optional audio to send as Discord attachment
    pub audio: Option<AudioAttachment>,
}

impl ToolOutput {
    /// Create a text-only output
    pub fn text(text: String) -> Self {
        Self {
            text,
            image: None,
            audio: None,
        }
    }

    /// Create an output with both text and an image
    pub fn with_image(text: String, data: Vec<u8>, filename: String) -> Self {
        Self {
            text,
            image: Some(ImageAttachment { data, filename }),
            audio: None,
        }
    }

    /// Create an output with both text and audio
    pub fn with_audio(text: String, data: Vec<u8>, filename: String) -> Self {
        Self {
            text,
            image: None,
            audio: Some(AudioAttachment { data, filename }),
        }
    }
}

/// Executor for Discord-native tools
pub struct ToolExecutor;

impl ToolExecutor {
    /// Execute a tool by name with the given JSON arguments
    pub async fn execute(
        name: &str,
        arguments: &str,
        tool_ctx: &ToolContext<'_>,
    ) -> Result<ToolOutput> {
        debug!("Executing tool '{}' with args: {}", name, arguments);

        match name {
            "search_channel_history" => search_channel_history(arguments, tool_ctx)
                .await
                .map(ToolOutput::text),
            "get_user_info" => get_user_info(arguments, tool_ctx)
                .await
                .map(ToolOutput::text),
            "get_server_info" => get_server_info(arguments, tool_ctx)
                .await
                .map(ToolOutput::text),
            "web_search" => web_search(arguments, tool_ctx.openrouter_api_key)
                .await
                .map(ToolOutput::text),
            "generate_image" => generate_image(arguments, tool_ctx).await,
            "generate_audio" => generate_audio(arguments, tool_ctx).await,
            _ => {
                warn!("Unknown tool requested: {}", name);
                Err(BotError::ToolExecution(format!("Unknown tool: {}", name)))
            }
        }
    }
}
