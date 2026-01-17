use poise::serenity_prelude::{ClientBuilder, Context, FullEvent, GatewayIntents};

struct Data {}
type Error = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() {
    let token = std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN environment variable");

    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            event_handler: |ctx, event, _framework, _data| Box::pin(event_handler(ctx, event)),
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                println!("Bot is ready!");
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .build();

    let client = ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    client.unwrap().start().await.unwrap();
}

async fn event_handler(ctx: &Context, event: &FullEvent) -> Result<(), Error> {
    if let FullEvent::Message { new_message } = event
        && new_message.mentions_user_id(ctx.cache.current_user().id)
    {
        new_message.reply(&ctx.http, "Hey").await?;
    }
    Ok(())
}
