pub mod config;
pub mod error;

use std::error::Error as StdError;

use config::Config;
use error::Result;
use poise::{
    Framework, FrameworkOptions, builtins,
    serenity_prelude::{ClientBuilder, Context, FullEvent, GatewayIntents},
};

type EventResult = std::result::Result<(), Box<dyn StdError + Send + Sync>>;

struct Data {}

pub async fn run() -> Result<()> {
    let config = Config::from_env()?;

    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;

    let framework = Framework::builder()
        .options(FrameworkOptions {
            event_handler: |ctx, event, _framework, _data| Box::pin(event_handler(ctx, event)),
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                println!("Bot is ready!");
                builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .build();

    let mut client = ClientBuilder::new(config.discord_token, intents)
        .framework(framework)
        .await?;

    client.start().await?;

    Ok(())
}

async fn event_handler(ctx: &Context, event: &FullEvent) -> EventResult {
    if let FullEvent::Message { new_message } = event
        && new_message.mentions_user_id(ctx.cache.current_user().id)
    {
        new_message.reply(&ctx.http, "Hello.").await?;
    }
    Ok(())
}
