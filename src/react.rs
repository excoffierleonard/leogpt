//! Slash command for random reaction images.

mod s3_store;

use rand::prelude::IndexedRandom;

use crate::{
    bot::Data,
    error::{BotError, Result},
    fuzzy_search::{find_best_fuzzy, find_exact, search_fuzzy},
};

pub use s3_store::S3MemeStore;

/// Context type for reaction command.
type Context<'a> = poise::Context<'a, Data, BotError>;

const AUTOCOMPLETE_LIMIT: usize = 25;

async fn autocomplete_reaction(ctx: Context<'_>, partial: &str) -> Vec<String> {
    let Some(store) = ctx.data().meme_store.as_ref() else {
        return Vec::new();
    };
    let cache = store.cache().read().await;
    let partial = partial.trim();

    if partial.is_empty() {
        return cache
            .entries
            .iter()
            .take(AUTOCOMPLETE_LIMIT)
            .map(|entry| entry.name.clone())
            .collect();
    }

    search_fuzzy(&cache.entries, partial, AUTOCOMPLETE_LIMIT)
        .into_iter()
        .map(|entry| entry.name.clone())
        .collect()
}

/// Send a random reaction image.
#[poise::command(slash_command)]
pub async fn react(
    ctx: Context<'_>,
    #[description = "Reaction image name (optional)"]
    #[autocomplete = "autocomplete_reaction"]
    image: Option<String>,
) -> Result<()> {
    let store = ctx
        .data()
        .meme_store
        .as_ref()
        .ok_or(BotError::MemeNotConfigured)?;
    let cache = store.cache().read().await;
    let entry = if let Some(image) = image {
        let query = image.trim();
        if query.is_empty() {
            return Err(BotError::SearchQueryEmpty);
        }

        match (
            find_exact(&cache.entries, query),
            find_best_fuzzy(&cache.entries, query),
        ) {
            (Some(entry), _) | (None, Some(entry)) => entry,
            (None, None) => return Err(BotError::SearchNoMatches(query.to_string())),
        }
    } else {
        cache
            .entries
            .choose(&mut rand::rng())
            .ok_or(BotError::ReactionImagesEmpty)?
    };
    let url = store.public_url(&entry.key);

    ctx.say(url).await?;
    Ok(())
}

/// Get available reaction commands.
#[must_use]
pub fn react_commands() -> Vec<poise::Command<Data, BotError>> {
    vec![react()]
}
