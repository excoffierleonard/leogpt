//! Slash command for random reaction images.

mod s3_store;

use rand::prelude::IndexedRandom;

use crate::{
    bot::Data,
    error::{BotError, Result},
};

pub use s3_store::S3MemeStore;

/// Context type for reaction command.
type Context<'a> = poise::Context<'a, Data, BotError>;

/// Send a random reaction image.
#[poise::command(slash_command)]
pub async fn react(ctx: Context<'_>) -> Result<()> {
    let store = ctx
        .data()
        .meme_store
        .as_ref()
        .ok_or(BotError::MemeNotConfigured)?;
    let cache = store.cache().read().await;
    let entry = cache
        .entries
        .choose(&mut rand::rng())
        .ok_or(BotError::ReactionImagesEmpty)?;
    let url = store.public_url(&entry.key);

    ctx.say(url).await?;
    Ok(())
}

/// Get available reaction commands.
#[must_use]
pub fn react_commands() -> Vec<poise::Command<Data, BotError>> {
    vec![react()]
}
