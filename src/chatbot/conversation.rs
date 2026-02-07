//! Conversation history building from Discord reply chains.

use log::warn;
use poise::serenity_prelude::{Context, Message as SerenityMessage, UserId};

use crate::{
    media::{has_supported_media, process_attachments},
    openrouter::{ContentPart, Message, MessageContent},
    types::MessageRole,
};

/// Converts a Discord message into an `OpenRouter` Message, including any media attachments.
pub async fn message_to_openrouter_message(
    discord_msg: &SerenityMessage,
    role: MessageRole,
) -> Message {
    let content = if has_supported_media(&discord_msg.attachments) {
        let mut parts = Vec::new();

        // Add text first (OpenRouter recommends text before media)
        if !discord_msg.content.is_empty() {
            parts.push(ContentPart::Text {
                text: discord_msg.content.clone(),
            });
        }

        // Add media attachments
        parts.extend(process_attachments(&discord_msg.attachments).await);

        MessageContent::MultiPart(parts)
    } else {
        MessageContent::Text(discord_msg.content.clone())
    };

    Message {
        role,
        content: Some(content),
        tool_calls: None,
        tool_call_id: None,
    }
}

/// Builds conversation history by walking up the Discord reply chain.
pub async fn build_conversation_history(
    ctx: &Context,
    message: &SerenityMessage,
    bot_user_id: UserId,
) -> Vec<Message> {
    let mut history = Vec::new();
    let mut current_message = message.clone();

    // Walk up the reply chain
    while let Some(ref_msg) = &current_message.referenced_message {
        // Fetch the full message to get attachments (referenced_message is partial)
        let full_msg = match ctx.http.get_message(ref_msg.channel_id, ref_msg.id).await {
            Ok(msg) => msg,
            Err(e) => {
                warn!("Failed to fetch message in reply chain: {e}");
                break;
            }
        };

        let role = if full_msg.author.id == bot_user_id {
            MessageRole::Assistant
        } else {
            MessageRole::User
        };

        history.push(message_to_openrouter_message(&full_msg, role).await);
        current_message = full_msg;
    }

    // Reverse to get chronological order
    history.reverse();
    history
}
