pub mod config;
pub mod error;
pub mod openrouter;

use std::error::Error as StdError;

use chrono::Utc;
use config::Config;
use error::Result;
use log::{debug, info};
use openrouter::{Message, OpenRouterClient};
use poise::{
    Framework, FrameworkOptions, builtins,
    serenity_prelude::{
        ClientBuilder, Context, FullEvent, GatewayIntents, Message as SerenityMessage, UserId,
    },
};

type EventResult = std::result::Result<(), Box<dyn StdError + Send + Sync>>;

struct Data {
    openrouter_client: OpenRouterClient,
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

    debug!("Building framework");
    let framework = Framework::builder()
        .options(FrameworkOptions {
            event_handler: |ctx, event, _framework, data| Box::pin(event_handler(ctx, event, data)),
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                info!("Bot is ready and connected to Discord");
                debug!("Registering commands globally");
                builtins::register_globally(ctx, &framework.options().commands).await?;
                info!("Commands registered successfully");
                Ok(Data { openrouter_client })
            })
        })
        .build();

    debug!("Creating Discord client");
    let mut client = ClientBuilder::new(config.discord_token, intents)
        .framework(framework)
        .await?;

    info!("Starting Discord client");
    client.start().await?;

    Ok(())
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
            "assistant"
        } else {
            "user"
        };

        history.push(Message {
            role: role.to_string(),
            content: ref_msg.content.clone(),
        });

        // Try to fetch the full message to continue the chain
        match ctx.http.get_message(ref_msg.channel_id, ref_msg.id).await {
            Ok(msg) => {
                current_message = msg;
            }
            Err(_) => break, // Can't fetch more, stop here
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

    let mut context = format!("Current datetime: {}", timestamp);

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

async fn event_handler(ctx: &Context, event: &FullEvent, data: &Data) -> EventResult {
    if let FullEvent::Message { new_message } = event
        && new_message.mentions_user_id(ctx.cache.current_user().id)
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
        conversation_history.push(Message {
            role: "user".to_string(),
            content: new_message.content.clone(),
        });

        debug!(
            "Conversation history has {} messages",
            conversation_history.len()
        );

        // Build dynamic context for the system prompt
        let dynamic_context = build_dynamic_context(new_message);

        match data
            .openrouter_client
            .chat_with_history(conversation_history, Some(dynamic_context))
            .await
        {
            Ok(reply_content) => {
                // Send reply
                new_message.reply(&ctx.http, &reply_content).await?;

                info!(
                    "Replied to {} in channel {}: {}",
                    new_message.author.tag(),
                    new_message.channel_id,
                    reply_content
                );
            }
            Err(e) => {
                // Log the full technical error for debugging
                log::error!(
                    "Error processing message from {}: {}",
                    new_message.author.tag(),
                    e
                );

                // Send a user-friendly error message to Discord
                let user_msg = e.user_message();
                new_message.reply(&ctx.http, user_msg).await?;
            }
        }
    }
    Ok(())
}
