//! User information lookup tool implementation.

use log::debug;
use poise::serenity_prelude::UserId;
use serde::{Deserialize, Serialize};

use crate::error::{BotError, Result};

use super::executor::ToolContext;

/// Minimum similarity threshold for fuzzy matching
const FUZZY_THRESHOLD: f64 = 0.85;

/// Arguments for the get_user_info tool
#[derive(Debug, Deserialize)]
struct UserInfoArgs {
    username: Option<String>,
    user_id: Option<String>,
}

/// User information returned by the tool
#[derive(Debug, Serialize)]
struct UserInfoResult {
    username: String,
    display_name: Option<String>,
    avatar_url: Option<String>,
    joined_server: Option<String>,
    roles: Vec<String>,
    created_at: String,
}

/// Check if username matches using case-insensitive and fuzzy matching
fn matches_username(name: &str, search: &str) -> bool {
    let name_lower = name.to_lowercase();
    let search_lower = search.to_lowercase();

    // Check for exact substring match first
    if name_lower.contains(&search_lower) {
        return true;
    }

    // Fall back to fuzzy matching
    strsim::jaro_winkler(&name_lower, &search_lower) > FUZZY_THRESHOLD
}

/// Get detailed information about a Discord user in a server
///
/// Looks up a user by their ID (exact match) or username (fuzzy match).
/// Returns user details including roles, join date, and avatar.
pub async fn get_user_info(arguments: &str, tool_ctx: &ToolContext<'_>) -> Result<String> {
    let args: UserInfoArgs = serde_json::from_str(arguments)?;

    debug!(
        "Looking up user: username={:?}, user_id={:?}",
        args.username, args.user_id
    );

    let guild_id = tool_ctx
        .guild_id
        .ok_or_else(|| BotError::ToolExecution("Not in a server (DM context)".to_string()))?;

    let member = match (&args.user_id, &args.username) {
        (Some(id_str), _) => {
            let user_id: UserId = id_str.parse()?;
            guild_id.member(&tool_ctx.ctx.http, user_id).await?
        }
        (None, Some(username)) => {
            let members = guild_id
                .members(&tool_ctx.ctx.http, Some(1000), None)
                .await?;

            debug!("Searching through {} guild members", members.len());

            members
                .into_iter()
                .find(|m| {
                    let name = m.user.global_name.as_ref().unwrap_or(&m.user.name);
                    matches_username(name, username)
                        || m.nick
                            .as_ref()
                            .is_some_and(|n| matches_username(n, username))
                })
                .ok_or_else(|| BotError::ToolExecution(format!("User '{}' not found", username)))?
        }
        (None, None) => {
            return Err(BotError::ToolExecution(
                "Must provide either username or user_id".to_string(),
            ));
        }
    };

    // Get role names from cache
    let role_names: Vec<String> = match tool_ctx.ctx.cache.guild(guild_id) {
        Some(guild) => member
            .roles
            .iter()
            .filter_map(|role_id| guild.roles.get(role_id).map(|r| r.name.clone()))
            .collect(),
        None => vec![],
    };

    let result = UserInfoResult {
        username: member.user.name.clone(),
        display_name: member.user.global_name.clone(),
        avatar_url: member.user.avatar_url(),
        joined_server: member.joined_at.and_then(|t| t.to_rfc3339()),
        roles: role_names,
        created_at: member.user.created_at().to_rfc3339().unwrap_or_default(),
    };

    debug!("Found user: {}", result.username);

    Ok(serde_json::to_string(&result)?)
}
