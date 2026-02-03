//! Main handler for bot mentions.

use log::{debug, error, info};
use poise::serenity_prelude::{Context, Message as SerenityMessage, UserId};

use crate::bot::Data;
use crate::error::Result;
use crate::tools::ToolContext;
use crate::types::MessageRole;

use super::context::build_dynamic_context;
use super::conversation::{build_conversation_history, message_to_openrouter_message};
use super::response::send_response;
use super::tool_loop::{extract_image_urls, run_tool_loop};

/// Main handler for messages that mention the bot.
pub async fn handle_bot_mention(
    ctx: &Context,
    new_message: &SerenityMessage,
    data: &Data,
    bot_user_id: UserId,
) -> Result<()> {
    if !new_message.mentions_user_id(bot_user_id) {
        return Ok(());
    }

    info!(
        "Received message from {} in channel {}: {}",
        new_message.author.tag(),
        new_message.channel_id,
        new_message.content
    );

    if let Err(e) = new_message.channel_id.broadcast_typing(&ctx.http).await {
        debug!("Failed to broadcast typing indicator: {e}");
    }

    let mut conversation_history = build_conversation_history(ctx, new_message, bot_user_id).await;
    conversation_history.push(message_to_openrouter_message(new_message, MessageRole::User).await);
    debug!(
        "Conversation history has {} messages",
        conversation_history.len()
    );

    let dynamic_context = build_dynamic_context(new_message);
    let recent_images = extract_image_urls(&conversation_history);
    debug!(
        "Found {} images in conversation history",
        recent_images.len()
    );

    let tool_ctx = ToolContext {
        ctx,
        channel_id: new_message.channel_id,
        guild_id: new_message.guild_id,
        openrouter_api_key: data.openrouter_api_key(),
        recent_images,
    };

    match run_tool_loop(
        data.openrouter_client(),
        &mut conversation_history,
        &dynamic_context,
        &tool_ctx,
    )
    .await
    {
        Ok(result) => send_response(ctx, new_message, result).await?,
        Err(e) => {
            error!(
                "Error processing message from {}: {}",
                new_message.author.tag(),
                e
            );
            new_message.reply(&ctx.http, e.user_message()).await?;
        }
    }

    Ok(())
}
