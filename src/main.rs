//! Entry point for the leogpt Discord bot.

use log::{error, info};

#[tokio::main]
async fn main() -> leogpt::error::Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("leogpt=info,serenity=warn"),
    )
    .init();
    info!("Starting leogpt Discord bot");

    match leogpt::run().await {
        Ok(_) => {
            info!("Bot shut down successfully");
            Ok(())
        }
        Err(e) => {
            error!("Bot encountered an error: {}", e);
            Err(e)
        }
    }
}
