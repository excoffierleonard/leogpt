//! Response sending utilities for Discord.

use log::{info, warn};
use poise::serenity_prelude::{
    Context, CreateAttachment, CreateMessage, Message as SerenityMessage,
};

use crate::error::Result;
use crate::tools::{AudioAttachment, ImageAttachment};

/// Result of the tool loop containing text and media attachments.
pub struct ToolLoopResult {
    pub text: Option<String>,
    pub images: Vec<ImageAttachment>,
    pub audio: Vec<AudioAttachment>,
}

/// Send the chatbot response to Discord.
pub async fn send_response(
    ctx: &Context,
    new_message: &SerenityMessage,
    result: ToolLoopResult,
) -> Result<()> {
    let has_media = !result.images.is_empty() || !result.audio.is_empty();
    let mut attachments: Vec<CreateAttachment> = result
        .images
        .into_iter()
        .map(|img| CreateAttachment::bytes(img.data, img.filename))
        .collect();
    attachments.extend(
        result
            .audio
            .into_iter()
            .map(|aud| CreateAttachment::bytes(aud.data, aud.filename)),
    );

    match (result.text, has_media) {
        (Some(text), false) => {
            new_message.reply(&ctx.http, &text).await?;
            info!(
                "Replied to {} in channel {}: {}",
                new_message.author.tag(),
                new_message.channel_id,
                text
            );
        }
        (None, true) => {
            let message = CreateMessage::new()
                .reference_message(new_message)
                .add_files(attachments);
            new_message
                .channel_id
                .send_message(&ctx.http, message)
                .await?;
            info!(
                "Replied to {} in channel {} with media attachment",
                new_message.author.tag(),
                new_message.channel_id
            );
        }
        (Some(text), true) => {
            let message = CreateMessage::new()
                .content(&text)
                .reference_message(new_message)
                .add_files(attachments);
            new_message
                .channel_id
                .send_message(&ctx.http, message)
                .await?;
            info!(
                "Replied to {} in channel {}: {} (with media)",
                new_message.author.tag(),
                new_message.channel_id,
                text
            );
        }
        (None, false) => {
            warn!("No response content generated");
        }
    }

    Ok(())
}
