//! Entry point for the leogpt Discord bot.

use log::{error, info};

use leogpt::error::{BotError, Result};
use leogpt::run;

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|err| BotError::Config(format!("Rustls init failed: {err:?}")))?;
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("leogpt=info,serenity=warn"),
    )
    .init();
    info!("Starting leogpt Discord bot");

    match run().await {
        Ok(()) => {
            info!("Bot shut down successfully");
            Ok(())
        }
        Err(e) => {
            error!("Bot encountered an error: {e}");
            Err(e)
        }
    }
}
