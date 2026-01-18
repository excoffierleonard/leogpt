pub mod config;
pub mod error;
pub mod openrouter;

use std::error::Error as StdError;

use config::Config;
use error::Result;
use log::{debug, info};
use openrouter::OpenRouterClient;
use poise::{
    Framework, FrameworkOptions, builtins,
    serenity_prelude::{ClientBuilder, Context, FullEvent, GatewayIntents},
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

        let user_message = new_message.content.clone();

        match data.openrouter_client.chat(&user_message).await {
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
                let error_msg = format!("Sorry, I encountered an error: {}", e);
                new_message.reply(&ctx.http, error_msg).await?;
                info!("Error processing message: {}", e);
            }
        }
    }
    Ok(())
}
