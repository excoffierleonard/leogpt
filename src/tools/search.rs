//! Channel message search tool implementation.

use log::debug;
use poise::serenity_prelude::GetMessages;
use serde::{Deserialize, Serialize};

use crate::error::Result;

use super::executor::ToolContext;

/// Maximum messages Discord API returns per request
const MAX_MESSAGES: u8 = 100;

/// Minimum similarity threshold for fuzzy matching
const FUZZY_THRESHOLD: f64 = 0.85;

/// Arguments for the search_channel_history tool
#[derive(Debug, Deserialize)]
struct SearchArgs {
    keyword: Option<String>,
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

/// Check if content matches keyword using case-insensitive and fuzzy matching
fn matches_keyword(content: &str, keyword: &str) -> bool {
    let content_lower = content.to_lowercase();
    let keyword_lower = keyword.to_lowercase();

    // Check for exact substring match first
    if content_lower.contains(&keyword_lower) {
        return true;
    }

    // Fall back to fuzzy matching against each word
    for word in content_lower.split_whitespace() {
        if strsim::jaro_winkler(word, &keyword_lower) > FUZZY_THRESHOLD {
            return true;
        }
    }

    false
}

/// Check if username matches using case-insensitive and fuzzy matching
fn matches_username(author_name: &str, search_name: &str) -> bool {
    let author_lower = author_name.to_lowercase();
    let search_lower = search_name.to_lowercase();

    // Check for exact substring match first
    if author_lower.contains(&search_lower) {
        return true;
    }

    // Fall back to fuzzy matching
    strsim::jaro_winkler(&author_lower, &search_lower) > FUZZY_THRESHOLD
}

/// Search recent messages in a Discord channel
///
/// Fetches up to 100 recent messages and filters them by keyword and/or username.
/// Supports case-insensitive and fuzzy matching.
pub async fn search_channel_history(arguments: &str, tool_ctx: &ToolContext<'_>) -> Result<String> {
    let args: SearchArgs = serde_json::from_str(arguments)?;
    let result_limit = args.limit.unwrap_or(20).min(100);

    debug!(
        "Searching channel history: keyword={:?}, username={:?}, limit={}",
        args.keyword, args.username, result_limit
    );

    let messages = tool_ctx
        .channel_id
        .messages(&tool_ctx.ctx.http, GetMessages::new().limit(MAX_MESSAGES))
        .await?;

    debug!("Fetched {} messages from channel", messages.len());

    // Filter and collect matching messages
    let mut results: Vec<MessageResult> = Vec::new();

    for msg in messages {
        // Filter by keyword if provided
        if let Some(ref kw) = args.keyword
            && !matches_keyword(&msg.content, kw)
        {
            continue;
        }

        // Filter by username if provided
        if let Some(ref username) = args.username {
            let author_name = msg.author.global_name.as_ref().unwrap_or(&msg.author.name);
            if !matches_username(author_name, username) {
                continue;
            }
        }

        results.push(MessageResult {
            author: msg
                .author
                .global_name
                .clone()
                .unwrap_or_else(|| msg.author.name.clone()),
            content: msg.content.clone(),
            timestamp: msg.timestamp.to_rfc3339().unwrap_or_default(),
        });

        // Stop once we have enough results
        if results.len() >= result_limit {
            break;
        }
    }

    debug!("Found {} matching messages", results.len());

    Ok(serde_json::to_string(&results)?)
}
