//! Channel message search tool implementation.

use log::debug;
use poise::serenity_prelude::{GetMessages, Message as DiscordMessage};
use serde::{Deserialize, Serialize};

use crate::error::Result;

use super::{executor::ToolContext, utils::matches_username};

/// Maximum messages Discord API returns per request
const MAX_MESSAGES: u8 = 100;

/// Arguments for the `search_channel_history` tool
#[derive(Debug, Deserialize)]
struct SearchArgs {
    query: Option<String>,
    username: Option<String>,
    limit: Option<usize>,
}

/// A single message result returned by the search
#[derive(Debug, Serialize)]
struct MessageResult {
    author: String,
    content: String,
    timestamp: String,
}

/// Check if message author matches username filter
fn author_matches(msg: &DiscordMessage, username: &str) -> bool {
    let nick = msg.member.as_ref().and_then(|m| m.nick.as_deref());
    let global_name = msg.author.global_name.as_deref();
    let name = &msg.author.name;

    nick.is_some_and(|n| matches_username(n, username))
        || global_name.is_some_and(|g| matches_username(g, username))
        || matches_username(name, username)
}

impl From<&DiscordMessage> for MessageResult {
    fn from(msg: &DiscordMessage) -> Self {
        Self {
            author: msg
                .author
                .global_name
                .clone()
                .unwrap_or(msg.author.name.clone()),
            content: msg.content.clone(),
            timestamp: msg.timestamp.to_rfc3339().unwrap_or_default(),
        }
    }
}

/// Search recent messages in a Discord channel
///
/// Returns the most recent messages in the channel (up to 100, Discord's per-request cap),
/// optionally filtered by author and/or a case-insensitive keyword match. The model reads
/// the returned messages directly, so no separate relevance ranking is performed.
pub async fn search_channel_history(arguments: &str, tool_ctx: &ToolContext<'_>) -> Result<String> {
    let args: SearchArgs = serde_json::from_str(arguments)?;
    let result_limit = args.limit.unwrap_or(20).min(100);

    debug!(
        "Searching channel history: query={:?}, username={:?}, limit={}",
        args.query, args.username, result_limit
    );

    let messages = tool_ctx
        .channel_id
        .messages(&tool_ctx.ctx.http, GetMessages::new().limit(MAX_MESSAGES))
        .await?;

    debug!("Fetched {} messages from channel", messages.len());

    let query_lower = args.query.as_ref().map(|q| q.to_lowercase());

    let results: Vec<MessageResult> = messages
        .iter()
        .filter(|msg| !msg.content.is_empty()) // Skip empty messages
        .filter(|msg| {
            args.username
                .as_ref()
                .is_none_or(|u| author_matches(msg, u))
        })
        .filter(|msg| {
            query_lower
                .as_ref()
                .is_none_or(|q| msg.content.to_lowercase().contains(q.as_str()))
        })
        .take(result_limit)
        .map(MessageResult::from)
        .collect();

    debug!("Returning {} messages", results.len());

    Ok(serde_json::to_string(&results)?)
}
