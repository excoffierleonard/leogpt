//! Tool executor for dispatching tool calls.

use log::{debug, warn};
use poise::serenity_prelude::{ChannelId, Context, GuildId};

use crate::error::{BotError, Result};

use super::search::search_channel_history;
use super::server_info::get_server_info;
use super::user_info::get_user_info;

/// Context needed to execute Discord-native tools
pub struct ToolContext<'a> {
    pub ctx: &'a Context,
    pub channel_id: ChannelId,
    pub guild_id: Option<GuildId>,
}

/// Executor for Discord-native tools
pub struct ToolExecutor;

impl ToolExecutor {
    /// Execute a tool by name with the given JSON arguments
    pub async fn execute(
        name: &str,
        arguments: &str,
        tool_ctx: &ToolContext<'_>,
    ) -> Result<String> {
        debug!("Executing tool '{}' with args: {}", name, arguments);

        match name {
            "search_channel_history" => search_channel_history(arguments, tool_ctx).await,
            "get_user_info" => get_user_info(arguments, tool_ctx).await,
            "get_server_info" => get_server_info(arguments, tool_ctx).await,
            _ => {
                warn!("Unknown tool requested: {}", name);
                Err(BotError::ToolExecution(format!("Unknown tool: {}", name)))
            }
        }
    }
}
