//! Discord bot core logic and event handling.

use std::error::Error as StdError;

use chrono::Utc;
use log::{debug, error, info, warn};
use poise::{
    Framework, FrameworkOptions, builtins,
    serenity_prelude::{
        ClientBuilder, Context, CreateAttachment, CreateMessage, FullEvent, GatewayIntents,
        Message as SerenityMessage, UserId,
    },
};

use crate::config::Config;
use crate::error::{BotError, Result};
use crate::media::{has_supported_media, process_attachments};
use crate::openrouter::{ChatResult, ContentPart, Message, MessageContent, OpenRouterClient};
use crate::tools::{
    AudioAttachment, ImageAttachment, ToolContext, ToolExecutor, get_tool_definitions,
};
use crate::types::MessageRole;

type EventResult = std::result::Result<(), Box<dyn StdError + Send + Sync>>;

const MAX_TOOL_ITERATIONS: usize = 5;

struct Data {
    openrouter_client: OpenRouterClient,
    openrouter_api_key: String,
}

/// Run the Discord bot.
pub async fn run() -> Result<()> {
    info!("Initializing bot");
    let config = Config::from_env()?;

    debug!("Initializing OpenRouter client");
    let openrouter_client = OpenRouterClient::new(config.openrouter_api_key.clone());

    debug!("Setting up gateway intents");
    let intents = GatewayIntents::non_privileged()
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_MEMBERS;

    // Extract values before moving config into closure
    let discord_token = config.discord_token.clone();
    let api_key = config.openrouter_api_key.clone();

    debug!("Building framework");
    let framework = Framework::builder()
        .options(FrameworkOptions {
            event_handler: |ctx, event, _framework, data| Box::pin(event_handler(ctx, event, data)),
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                info!("Bot is ready and connected to Discord");
                debug!("Registering commands globally");
                builtins::register_globally(ctx, &framework.options().commands).await?;
                info!("Commands registered successfully");
                Ok(Data {
                    openrouter_client,
                    openrouter_api_key: api_key,
                })
            })
        })
        .build();

    debug!("Creating Discord client");
    let mut client = ClientBuilder::new(discord_token, intents)
        .framework(framework)
        .await?;

    info!("Starting Discord client");

    tokio::select! {
        result = client.start() => {
            result?;
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Shutdown signal received, shutting down...");
        }
    }

    Ok(())
}

/// Extract image URLs from conversation history (most recent first)
fn extract_image_urls(messages: &[Message]) -> Vec<String> {
    let mut urls = Vec::new();
    // Iterate in reverse to get most recent first
    for message in messages.iter().rev() {
        if let Some(MessageContent::MultiPart(parts)) = &message.content {
            for part in parts {
                if let ContentPart::ImageUrl { image_url } = part {
                    urls.push(image_url.url.clone());
                }
            }
        }
    }
    urls
}

/// Converts a Discord message into an OpenRouter Message, including any media attachments
async fn message_to_openrouter_message(
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

/// Builds conversation history by walking up the Discord reply chain
async fn build_conversation_history(
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
                warn!("Failed to fetch message in reply chain: {}", e);
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

/// Builds dynamic context information for the system prompt
fn build_dynamic_context(message: &SerenityMessage) -> String {
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    let user = &message.author;

    // Base context for the system prompt
    let mut context = String::from(
        "You are a Discord bot. Users interact with you by mentioning you in messages.",
    );

    // Current datetime
    context.push_str(&format!("\nCurrent datetime: {}", timestamp));

    // User identification
    let username = user.global_name.as_ref().unwrap_or(&user.name);
    context.push_str(&format!("\nUser: {} (ID: {})", username, user.id));

    // Add guild-specific info if available
    if let Some(ref member) = user.member {
        if let Some(ref nick) = member.nick {
            context.push_str(&format!(" (Server nick: {})", nick));
        }
        if let Some(joined_at) = member.joined_at {
            let join_date = joined_at.format("%Y-%m-%d");
            context.push_str(&format!(", joined {}", join_date));
        }
    }

    // Location context
    if let Some(guild_id) = message.guild_id {
        context.push_str(&format!("\nServer ID: {}", guild_id));
    }
    context.push_str(&format!("\nChannel ID: {}", message.channel_id));

    // Include mentioned users (so the model can use these IDs directly)
    if !message.mentions.is_empty() {
        context.push_str("\n\nUsers mentioned in this message:");
        for mentioned in &message.mentions {
            if mentioned.bot {
                continue; // Skip bot mentions (including self)
            }
            let display = mentioned.global_name.as_ref().unwrap_or(&mentioned.name);
            context.push_str(&format!(
                "\n- {} (ID: {}, mention: <@{}>)",
                display, mentioned.id, mentioned.id
            ));
        }
    }

    context
}

async fn event_handler(ctx: &Context, event: &FullEvent, data: &Data) -> EventResult {
    if let FullEvent::Message { new_message } = event
        && new_message.mentions_user_id(ctx.cache.current_user().id)
        && new_message.author.id != ctx.cache.current_user().id
    {
        info!(
            "Received message from {} in channel {}: {}",
            new_message.author.tag(),
            new_message.channel_id,
            new_message.content
        );

        // Show typing indicator while processing
        if let Err(e) = new_message.channel_id.broadcast_typing(&ctx.http).await {
            debug!("Failed to broadcast typing indicator: {}", e);
        }

        let bot_user_id = ctx.cache.current_user().id;

        // Build conversation history from reply chain
        let mut conversation_history =
            build_conversation_history(ctx, new_message, bot_user_id).await;

        // Add current user message
        conversation_history
            .push(message_to_openrouter_message(new_message, MessageRole::User).await);

        debug!(
            "Conversation history has {} messages",
            conversation_history.len()
        );

        // Build dynamic context for the system prompt
        let dynamic_context = build_dynamic_context(new_message);

        // Get tool definitions
        let tools = Some(get_tool_definitions());

        // Extract image URLs from conversation for tool context
        let recent_images = extract_image_urls(&conversation_history);
        debug!(
            "Found {} images in conversation history",
            recent_images.len()
        );

        // Tool execution context
        let tool_ctx = ToolContext {
            ctx,
            channel_id: new_message.channel_id,
            guild_id: new_message.guild_id,
            openrouter_api_key: &data.openrouter_api_key,
            recent_images,
        };

        // Collect media generated during tool execution
        let mut generated_images: Vec<ImageAttachment> = Vec::new();
        let mut generated_audio: Vec<AudioAttachment> = Vec::new();

        // Tool loop - returns Some(text) for text response, None for media-only response
        let mut iterations = 0;
        let result: std::result::Result<Option<String>, BotError> = loop {
            iterations += 1;
            if iterations > MAX_TOOL_ITERATIONS {
                break Err(BotError::ToolLoopLimit);
            }

            // Refresh typing indicator
            let _ = new_message.channel_id.broadcast_typing(&ctx.http).await;

            match data
                .openrouter_client
                .chat_with_history(
                    conversation_history.clone(),
                    Some(dynamic_context.clone()),
                    tools.clone(),
                )
                .await
            {
                Ok(ChatResult::TextResponse(text)) => break Ok(Some(text)),
                Ok(ChatResult::ToolCalls {
                    tool_calls,
                    assistant_message,
                }) => {
                    debug!("Processing {} tool calls", tool_calls.len());

                    // Add assistant's tool call message to history
                    conversation_history.push(assistant_message);

                    // Execute each tool and add results
                    for tool_call in tool_calls {
                        let (result_text, maybe_image, maybe_audio) = match ToolExecutor::execute(
                            &tool_call.function.name,
                            &tool_call.function.arguments,
                            &tool_ctx,
                        )
                        .await
                        {
                            Ok(output) => (output.text, output.image, output.audio),
                            Err(e) => {
                                warn!("Tool execution failed: {}", e);
                                (format!("Error: {}", e), None, None)
                            }
                        };

                        // If media was generated, collect it and exit the loop
                        // to send just the media without further LLM processing
                        if let Some(image) = maybe_image {
                            generated_images.push(image);
                            break;
                        }
                        if let Some(audio) = maybe_audio {
                            generated_audio.push(audio);
                            break;
                        }

                        // Add tool result to history
                        conversation_history.push(Message {
                            role: MessageRole::Tool,
                            content: Some(MessageContent::Text(result_text)),
                            tool_calls: None,
                            tool_call_id: Some(tool_call.id.clone()),
                        });
                    }

                    // If we have media, exit the tool loop entirely
                    if !generated_images.is_empty() || !generated_audio.is_empty() {
                        break Ok(None);
                    }
                }
                Err(e) => break Err(e),
            }
        };

        match result {
            Ok(maybe_text) => {
                // Combine all media attachments (images and audio)
                let has_media = !generated_images.is_empty() || !generated_audio.is_empty();
                let mut attachments: Vec<CreateAttachment> = generated_images
                    .into_iter()
                    .map(|img| CreateAttachment::bytes(img.data, img.filename))
                    .collect();
                attachments.extend(
                    generated_audio
                        .into_iter()
                        .map(|aud| CreateAttachment::bytes(aud.data, aud.filename)),
                );

                // Send reply: text only, media only, or both
                match (maybe_text, has_media) {
                    (Some(text), false) => {
                        // Text only
                        new_message.reply(&ctx.http, &text).await?;
                        info!(
                            "Replied to {} in channel {}: {}",
                            new_message.author.tag(),
                            new_message.channel_id,
                            text
                        );
                    }
                    (None, true) => {
                        // Media only
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
                        // Text + media
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
                        // No text and no media - shouldn't happen, but handle gracefully
                        warn!("No response content generated");
                    }
                }
            }
            Err(e) => {
                error!(
                    "Error processing message from {}: {}",
                    new_message.author.tag(),
                    e
                );

                let user_msg = e.user_message();
                new_message.reply(&ctx.http, user_msg).await?;
            }
        }
    }
    Ok(())
}
