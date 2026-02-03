//! Discord bot core logic and event handling.

use std::error::Error as StdError;
use std::fmt::Write;

use chrono::Utc;
use log::{debug, error, info, warn};
use poise::{
    Framework, FrameworkOptions, builtins,
    serenity_prelude::{
        ClientBuilder, Context, CreateAttachment, CreateMessage, FullEvent, GatewayIntents,
        Message as SerenityMessage, UserId,
    },
};

use crate::auto_response::{
    AutoResponsePayload, AutoResponseRule, hardcoded_auto_responses, select_auto_response,
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
type AutoResponseResult = std::result::Result<bool, Box<dyn StdError + Send + Sync>>;

const MAX_TOOL_ITERATIONS: usize = 5;

struct Data {
    openrouter_client: OpenRouterClient,
    openrouter_api_key: String,
    auto_responses: Vec<AutoResponseRule>,
}

/// Run the Discord bot.
///
/// # Errors
///
/// Returns an error if configuration loading, Discord client creation, or connection fails.
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
    let auto_responses = hardcoded_auto_responses();

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
                    auto_responses,
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

/// Converts a Discord message into an `OpenRouter` Message, including any media attachments
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

/// Builds dynamic context information for the system prompt
fn build_dynamic_context(message: &SerenityMessage) -> String {
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

async fn event_handler(ctx: &Context, event: &FullEvent, data: &Data) -> EventResult {
    if let FullEvent::Message { new_message } = event {
        let bot_user_id = ctx.cache.current_user().id;
        if new_message.author.id == bot_user_id {
            return Ok(());
        }

        if handle_auto_response(ctx, new_message, &data.auto_responses).await? {
            return Ok(());
        }

        handle_bot_mention(ctx, new_message, data, bot_user_id).await?;
    }
    Ok(())
}

struct ToolLoopResult {
    text: Option<String>,
    images: Vec<ImageAttachment>,
    audio: Vec<AudioAttachment>,
}

async fn run_tool_loop(
    client: &OpenRouterClient,
    conversation_history: &mut Vec<Message>,
    dynamic_context: &str,
    tool_ctx: &ToolContext<'_>,
) -> std::result::Result<ToolLoopResult, BotError> {
    let tools = Some(get_tool_definitions());
    let mut generated_images = Vec::new();
    let mut generated_audio = Vec::new();

    for _ in 0..MAX_TOOL_ITERATIONS {
        let _ = tool_ctx
            .channel_id
            .broadcast_typing(&tool_ctx.ctx.http)
            .await;

        match client
            .chat_with_history(
                conversation_history.clone(),
                Some(dynamic_context.to_string()),
                tools.clone(),
            )
            .await?
        {
            ChatResult::TextResponse(text) => {
                return Ok(ToolLoopResult {
                    text: Some(text),
                    images: generated_images,
                    audio: generated_audio,
                });
            }
            ChatResult::ToolCalls {
                tool_calls,
                assistant_message,
            } => {
                debug!("Processing {} tool calls", tool_calls.len());
                conversation_history.push(assistant_message);

                for tool_call in tool_calls {
                    let (result_text, maybe_image, maybe_audio) = match ToolExecutor::execute(
                        &tool_call.function.name,
                        &tool_call.function.arguments,
                        tool_ctx,
                    )
                    .await
                    {
                        Ok(output) => (output.text, output.image, output.audio),
                        Err(e) => {
                            warn!("Tool execution failed: {e}");
                            (format!("Error: {e}"), None, None)
                        }
                    };

                    if let Some(image) = maybe_image {
                        generated_images.push(image);
                        return Ok(ToolLoopResult {
                            text: None,
                            images: generated_images,
                            audio: generated_audio,
                        });
                    }
                    if let Some(audio) = maybe_audio {
                        generated_audio.push(audio);
                        return Ok(ToolLoopResult {
                            text: None,
                            images: generated_images,
                            audio: generated_audio,
                        });
                    }

                    conversation_history.push(Message {
                        role: MessageRole::Tool,
                        content: Some(MessageContent::Text(result_text)),
                        tool_calls: None,
                        tool_call_id: Some(tool_call.id.clone()),
                    });
                }
            }
        }
    }

    Err(BotError::ToolLoopLimit)
}

async fn send_response(
    ctx: &Context,
    new_message: &SerenityMessage,
    result: ToolLoopResult,
) -> EventResult {
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

/// Main handler for messages that mention the bot.
async fn handle_bot_mention(
    ctx: &Context,
    new_message: &SerenityMessage,
    data: &Data,
    bot_user_id: UserId,
) -> EventResult {
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
        openrouter_api_key: &data.openrouter_api_key,
        recent_images,
    };

    match run_tool_loop(
        &data.openrouter_client,
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

/// Auto-response handler for pre-configured match rules.
async fn handle_auto_response(
    ctx: &Context,
    new_message: &SerenityMessage,
    rules: &[AutoResponseRule],
) -> AutoResponseResult {
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
