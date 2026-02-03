//! Server information lookup tool implementation.

use log::debug;
use serde::{Deserialize, Serialize};

use crate::error::{BotError, Result};

use super::executor::ToolContext;

/// Arguments for the `get_server_info` tool
#[derive(Debug, Deserialize)]
struct ServerInfoArgs {
    // No arguments needed - uses current server context
}

/// Server information returned by the tool
#[derive(Debug, Serialize)]
struct ServerInfoResult {
    name: String,
    description: Option<String>,
    owner: String,
    member_count: u64,
    created_at: String,
    boost_level: u8,
    boost_count: u64,
    channel_count: usize,
    role_count: usize,
    emoji_count: usize,
    icon_url: Option<String>,
}

/// Get detailed information about a Discord server
///
/// Returns server details including member count, boost level, channels, and roles.
pub async fn get_server_info(arguments: &str, tool_ctx: &ToolContext<'_>) -> Result<String> {
    let _args: ServerInfoArgs = serde_json::from_str(arguments)?;

    debug!("Looking up server info");

    let guild_id = tool_ctx.guild_id.ok_or(BotError::NotInServer)?;

    // Get guild from cache for most data
    // Extract all values before any .await to avoid Send issues with CacheRef
    let (
        name,
        description,
        owner_id,
        member_count,
        premium_tier,
        premium_subscription_count,
        channel_count,
        role_count,
        emoji_count,
        icon_url,
    ) = {
        let guild = tool_ctx
            .ctx
            .cache
            .guild(guild_id)
            .ok_or_else(|| BotError::ToolExecution("Server not found in cache".into()))?;

        (
            guild.name.clone(),
            guild.description.clone(),
            guild.owner_id,
            guild.member_count,
            guild.premium_tier,
            guild.premium_subscription_count,
            guild.channels.len(),
            guild.roles.len(),
            guild.emojis.len(),
            guild.icon_url(),
        )
    };

    // Get owner info (now safe to await since CacheRef is dropped)
    let owner = guild_id.member(&tool_ctx.ctx.http, owner_id).await?;
    let owner_name = owner
        .user
        .global_name
        .clone()
        .unwrap_or(owner.user.name.clone());

    let result = ServerInfoResult {
        name,
        description,
        owner: owner_name,
        member_count,
        created_at: guild_id.created_at().to_rfc3339().unwrap_or_default(),
        boost_level: u8::from(premium_tier),
        boost_count: premium_subscription_count.unwrap_or(0),
        channel_count,
        role_count,
        emoji_count,
        icon_url,
    };

    debug!("Found server: {}", result.name);

    Ok(serde_json::to_string(&result)?)
}
