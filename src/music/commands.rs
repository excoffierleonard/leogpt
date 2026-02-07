//! Poise slash commands for music playback.

use poise::serenity_prelude::GuildId;

use crate::bot::Data;
use crate::error::{BotError, Result};

use super::playback::{MusicConfig, play_song, stop_playback};

/// Context type for music commands.
type Context<'a> = poise::Context<'a, Data, BotError>;

fn get_guild_id(ctx: Context<'_>) -> Result<GuildId> {
    ctx.guild_id().ok_or(BotError::NotInServer)
}

fn get_music_config(ctx: Context<'_>) -> Result<MusicConfig> {
    ctx.data()
        .music_store
        .as_ref()
        .map(|store| MusicConfig {
            store: store.clone(),
        })
        .ok_or(BotError::MusicNotConfigured)
}

/// Play a song in your voice channel.
#[poise::command(slash_command, guild_only)]
pub async fn play(
    ctx: Context<'_>,
    #[description = "Song name to search for"] song: String,
) -> Result<()> {
    let guild_id = get_guild_id(ctx)?;
    let config = get_music_config(ctx)?;

    ctx.defer().await?;

    let song_name = play_song(
        ctx.serenity_context(),
        guild_id,
        ctx.author().id,
        &song,
        &config,
    )
    .await?;

    ctx.say(format!("Now playing: **{song_name}**")).await?;
    Ok(())
}

/// Stop music and leave the voice channel.
#[poise::command(slash_command, guild_only)]
pub async fn stop(ctx: Context<'_>) -> Result<()> {
    let guild_id = get_guild_id(ctx)?;

    stop_playback(ctx.serenity_context(), guild_id).await?;
    ctx.say("Stopped playback and left the voice channel.")
        .await?;
    Ok(())
}

/// Get available music commands.
#[must_use]
pub fn music_commands() -> Vec<poise::Command<Data, BotError>> {
    vec![play(), stop()]
}
