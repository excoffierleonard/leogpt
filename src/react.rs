//! Slash command for random reaction images.

use rand::prelude::IndexedRandom;

use crate::{
    bot::Data,
    error::{BotError, Result},
};

/// Context type for reaction command.
type Context<'a> = poise::Context<'a, Data, BotError>;

const REACTION_IMAGES: &[&str] = &[
    "https://vault-public.s3.ca-east-006.backblazeb2.com/media/memes/enhanced/an_iq_too_high.png",
    "https://vault-public.s3.ca-east-006.backblazeb2.com/media/memes/enhanced/does_he_know.png",
    "https://vault-public.s3.ca-east-006.backblazeb2.com/media/memes/enhanced/he_made_a_statement_so_trash_even_his_gang_clowed_him.png",
    "https://vault-public.s3.ca-east-006.backblazeb2.com/media/memes/enhanced/why_is_he_lying.png",
];

/// Send a random reaction image.
#[poise::command(slash_command)]
pub async fn react(ctx: Context<'_>) -> Result<()> {
    let url = REACTION_IMAGES
        .choose(&mut rand::rng())
        .copied()
        .ok_or(BotError::ReactionImagesEmpty)?;

    ctx.say(url).await?;
    Ok(())
}

/// Get available reaction commands.
#[must_use]
pub fn react_commands() -> Vec<poise::Command<Data, BotError>> {
    vec![react()]
}
