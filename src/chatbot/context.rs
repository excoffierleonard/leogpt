//! Dynamic context building for system prompts.

use std::fmt::Write;

use chrono::Utc;
use poise::serenity_prelude::Message as SerenityMessage;

/// Builds dynamic context information for the system prompt.
pub fn build_dynamic_context(message: &SerenityMessage) -> String {
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    let user = &message.author;

    let mut context = String::from(
        "You are a Discord bot. Users interact with you by mentioning you in messages.",
    );

    let _ = write!(context, "\nCurrent datetime: {timestamp}");

    let username = user.global_name.as_ref().unwrap_or(&user.name);
    let _ = write!(context, "\nUser: {} (ID: {})", username, user.id);

    if let Some(ref member) = user.member {
        if let Some(ref nick) = member.nick {
            let _ = write!(context, " (Server nick: {nick})");
        }
        if let Some(joined_at) = member.joined_at {
            let join_date = joined_at.format("%Y-%m-%d");
            let _ = write!(context, ", joined {join_date}");
        }
    }

    if let Some(guild_id) = message.guild_id {
        let _ = write!(context, "\nServer ID: {guild_id}");
    }
    let _ = write!(context, "\nChannel ID: {}", message.channel_id);

    if !message.mentions.is_empty() {
        context.push_str("\n\nUsers mentioned in this message:");
        for mentioned in &message.mentions {
            if mentioned.bot {
                continue;
            }
            let display = mentioned.global_name.as_ref().unwrap_or(&mentioned.name);
            let _ = write!(
                context,
                "\n- {} (ID: {}, mention: <@{}>)",
                display, mentioned.id, mentioned.id
            );
        }
    }

    context
}
