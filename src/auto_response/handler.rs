//! Auto-response handler for pre-configured match rules.

use log::{debug, info};
use poise::serenity_prelude::{Context, CreateMessage, Message as SerenityMessage};

use crate::error::Result;

use super::{
    matching::select_auto_response,
    rules::{AutoResponsePayload, AutoResponseRule},
};

/// Handle auto-responses for pre-configured match rules.
///
/// Returns `true` if an auto-response was sent, `false` otherwise.
pub async fn handle_auto_response(
    ctx: &Context,
    new_message: &SerenityMessage,
    rules: &[AutoResponseRule],
) -> Result<bool> {
    if rules.is_empty() {
        return Ok(false);
    }

    debug!(
        "Auto response check: msg from {} in channel {}: {}",
        new_message.author.tag(),
        new_message.channel_id,
        new_message.content
    );

    let Some(action) = select_auto_response(rules, new_message.author.id, &new_message.content)
    else {
        return Ok(false);
    };

    let AutoResponsePayload::ImageUrl(content) = action.payload;

    let message = CreateMessage::new()
        .content(content)
        .reference_message(new_message);

    new_message
        .channel_id
        .send_message(&ctx.http, message)
        .await?;

    info!(
        "Sent auto response '{}' to {} in channel {}",
        action.rule_name,
        new_message.author.tag(),
        new_message.channel_id
    );

    Ok(true)
}
