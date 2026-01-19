//! LeoGPT - A Discord bot powered by OpenRouter AI.

pub mod config;
pub mod error;
pub mod media;
pub mod openrouter;
pub mod tools;
pub mod types;

use std::error::Error as StdError;

use chrono::Utc;
use config::Config;
use error::{BotError, Result};
use log::{debug, info, warn};
use media::{has_supported_media, process_attachments};
use openrouter::{ChatResult, ContentPart, Message, MessageContent, OpenRouterClient};
use poise::{
    Framework, FrameworkOptions, builtins,
    serenity_prelude::{
        ClientBuilder, Context, FullEvent, GatewayIntents, Message as SerenityMessage, UserId,
    },
};
use tools::{ToolContext, ToolExecutor, get_tool_definitions};
use types::MessageRole;

type EventResult = std::result::Result<(), Box<dyn StdError + Send + Sync>>;

struct Data {
    openrouter_client: OpenRouterClient,
    openrouter_api_key: String,
    openrouter_model: String,
}

pub async fn run() -> Result<()> {
    info!("Initializing bot");
    let config = Config::from_env()?;

    debug!("Initializing OpenRouter client");
    let openrouter_client = OpenRouterClient::new(
        config.openrouter_api_key.clone(),
        config.openrouter_model.clone(),
        config.system_prompt.clone(),
    );

    debug!("Setting up gateway intents");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;

    // Extract values before moving config into closure
    let discord_token = config.discord_token.clone();
    let api_key = config.openrouter_api_key.clone();
    let model = config.openrouter_model.clone();

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
                    openrouter_model: model,
                })
            })
        })
        .build();

    debug!("Creating Discord client");
    let mut client = ClientBuilder::new(discord_token, intents)
        .framework(framework)
        .await?;

    info!("Starting Discord client");
    client.start().await?;

    Ok(())
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
        // Add the referenced message to history (we'll reverse later)
        let role = if ref_msg.author.id == bot_user_id {
            MessageRole::Assistant
        } else {
            MessageRole::User
        };

        history.push(message_to_openrouter_message(ref_msg, role).await);

        // Try to fetch the full message to continue the chain
        match ctx.http.get_message(ref_msg.channel_id, ref_msg.id).await {
            Ok(msg) => {
                current_message = msg;
            }
            Err(e) => {
                warn!("Failed to fetch message in reply chain: {}", e);
                break;
            }
        }
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
    context.push_str(&format!("\nUser: {}", username));

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

    context
}

const MAX_TOOL_ITERATIONS: usize = 5;

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

        // Get tool definitions (conditional based on model)
        let tools = Some(get_tool_definitions(&data.openrouter_model));

        // Tool execution context
        let tool_ctx = ToolContext {
            ctx,
            channel_id: new_message.channel_id,
            guild_id: new_message.guild_id,
            openrouter_api_key: &data.openrouter_api_key,
            openrouter_model: &data.openrouter_model,
        };

        // Tool loop
        let mut iterations = 0;
        let result: std::result::Result<String, BotError> = loop {
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
                Ok(ChatResult::TextResponse(text)) => break Ok(text),
                Ok(ChatResult::ToolCalls {
                    tool_calls,
                    assistant_message,
                }) => {
                    debug!("Processing {} tool calls", tool_calls.len());

                    // Add assistant's tool call message to history
                    conversation_history.push(assistant_message);

                    // Execute each tool and add results
                    for tool_call in tool_calls {
                        let result = match ToolExecutor::execute(
                            &tool_call.function.name,
                            &tool_call.function.arguments,
                            &tool_ctx,
                        )
                        .await
                        {
                            Ok(result) => result,
                            Err(e) => {
                                warn!("Tool execution failed: {}", e);
                                format!("Error: {}", e)
                            }
                        };

                        // Add tool result to history
                        conversation_history.push(Message {
                            role: MessageRole::Tool,
                            content: Some(MessageContent::Text(result)),
                            tool_calls: None,
                            tool_call_id: Some(tool_call.id.clone()),
                        });
                    }
                }
                Err(e) => break Err(e),
            }
        };

        match result {
            Ok(reply_content) => {
                new_message.reply(&ctx.http, &reply_content).await?;

                info!(
                    "Replied to {} in channel {}: {}",
                    new_message.author.tag(),
                    new_message.channel_id,
                    reply_content
                );
            }
            Err(e) => {
                log::error!(
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
