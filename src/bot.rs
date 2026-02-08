//! Discord bot core - initialization and event routing.

use std::sync::Arc;

use log::{debug, error, info};
use poise::{
    Framework, FrameworkError, FrameworkOptions, builtins,
    serenity_prelude::{ClientBuilder, Context, FullEvent, GatewayIntents},
};
use rustls::crypto::aws_lc_rs::default_provider;
use songbird::SerenityInit;
use tokio::signal::ctrl_c;

use crate::{
    auto_response::{AutoResponseRule, handle_auto_response, hardcoded_auto_responses},
    chatbot::handle_bot_mention,
    config::Config,
    error::{BotError, Result},
    music::{S3MusicStore, music_commands},
    openrouter::OpenRouterClient,
};

type EventResult = Result<()>;

/// Shared data accessible to all commands and event handlers.
pub struct Data {
    openrouter_client: OpenRouterClient,
    openrouter_api_key: String,
    auto_responses: Vec<AutoResponseRule>,
    /// Optional S3 music store for voice playback.
    pub music_store: Option<Arc<S3MusicStore>>,
}

impl Data {
    /// Get a reference to the `OpenRouter` client.
    pub fn openrouter_client(&self) -> &OpenRouterClient {
        &self.openrouter_client
    }

    /// Get a reference to the `OpenRouter` API key.
    pub fn openrouter_api_key(&self) -> &str {
        &self.openrouter_api_key
    }
}

/// Handle command errors by logging and responding to the user.
async fn on_error(error: FrameworkError<'_, Data, BotError>) {
    match error {
        FrameworkError::Command { error, ctx, .. } => {
            error!("Command error: {error}");
            let _ = ctx.say(error.user_message()).await;
        }
        FrameworkError::Setup { error, .. } => {
            error!("Setup error: {error}");
        }
        other => {
            if let Err(e) = builtins::on_error(other).await {
                error!("Error handling error: {e}");
            }
        }
    }
}

/// Run the Discord bot.
///
/// # Errors
///
/// Returns an error if configuration loading, Discord client creation, or connection fails.
pub async fn run() -> Result<()> {
    // Ensure AWS SDK uses rustls for TLS, which is compatible with our S3 endpoint.
    default_provider()
        .install_default()
        .expect("install rustls crypto provider");

    info!("Initializing bot");
    let config = Config::from_env()?;

    debug!("Initializing OpenRouter client");
    let openrouter_client = OpenRouterClient::new(config.openrouter_api_key.clone());

    debug!("Setting up gateway intents");
    let intents = GatewayIntents::non_privileged()
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::GUILD_VOICE_STATES;

    // Extract values before moving config into closure
    let discord_token = config.discord_token.clone();
    let api_key = config.openrouter_api_key.clone();
    let music_s3 = config.music_s3.clone();
    let auto_responses = hardcoded_auto_responses();

    debug!("Building framework");
    let framework = Framework::builder()
        .options(FrameworkOptions {
            commands: music_commands(),
            event_handler: |ctx, event, _framework, data| Box::pin(event_handler(ctx, event, data)),
            on_error: |error| Box::pin(on_error(error)),
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
                    music_store: match music_s3 {
                        Some(config) => {
                            let store = S3MusicStore::from_config(&config).await?;
                            store.load_cache().await?;
                            Some(Arc::new(store))
                        }
                        None => None,
                    },
                })
            })
        })
        .build();

    debug!("Creating Discord client");
    let mut client = ClientBuilder::new(discord_token, intents)
        .framework(framework)
        .register_songbird()
        .await?;

    info!("Starting Discord client");

    tokio::select! {
        result = client.start() => {
            result?;
        }
        _ = ctrl_c() => {
            info!("Shutdown signal received, shutting down...");
        }
    }

    Ok(())
}

/// Main event router - delegates to feature handlers.
async fn event_handler(ctx: &Context, event: &FullEvent, data: &Data) -> EventResult {
    if let FullEvent::Message { new_message } = event {
        let bot_user_id = ctx.cache.current_user().id;

        // Ignore own messages
        if new_message.author.id == bot_user_id {
            return Ok(());
        }

        // Priority 1: Auto-responses (fast pattern matching)
        if handle_auto_response(ctx, new_message, &data.auto_responses).await? {
            return Ok(());
        }

        // Priority 2: AI chatbot (when mentioned)
        handle_bot_mention(ctx, new_message, data, bot_user_id).await?;
    }

    Ok(())
}
