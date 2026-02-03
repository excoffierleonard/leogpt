//! Voice channel playback using Songbird.

use std::path::PathBuf;
use std::sync::Arc;

use log::info;
use poise::serenity_prelude::{ChannelId, Context, GuildId, UserId};
use songbird::input::File as AudioFile;
use songbird::{Songbird, driver::Bitrate};

use crate::error::{BotError, Result};

use super::fuzzy_search::find_song;

/// Configuration for music playback.
pub struct MusicConfig {
    pub music_dir: PathBuf,
}

/// Get the Songbird voice manager from context.
///
/// # Errors
///
/// Returns `MissingVoiceManager` if Songbird is not registered.
pub async fn get_manager(ctx: &Context) -> Result<Arc<Songbird>> {
    songbird::get(ctx)
        .await
        .ok_or(BotError::MissingVoiceManager)
}

/// Get the voice channel the user is currently in.
#[must_use]
pub fn get_user_voice_channel(
    ctx: &Context,
    guild_id: GuildId,
    user_id: UserId,
) -> Option<ChannelId> {
    ctx.cache.guild(guild_id).and_then(|guild| {
        guild
            .voice_states
            .get(&user_id)
            .and_then(|vs| vs.channel_id)
    })
}

/// Play a song in the user's voice channel.
///
/// Returns the name of the song being played.
///
/// # Errors
///
/// Returns an error if:
/// - The song cannot be found
/// - The user is not in a voice channel
/// - The bot cannot join the voice channel
/// - The audio file cannot be read
pub async fn play_song(
    ctx: &Context,
    guild_id: GuildId,
    user_id: UserId,
    query: &str,
    config: &MusicConfig,
) -> Result<String> {
    // Find the song
    let song_path = find_song(&config.music_dir, query)?
        .ok_or_else(|| BotError::AudioFileNotFound(query.to_string()))?;

    let song_name = song_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Get user's voice channel
    let channel_id =
        get_user_voice_channel(ctx, guild_id, user_id).ok_or(BotError::NotInVoiceChannel)?;

    // Get Songbird manager
    let manager = get_manager(ctx).await?;

    // Join the channel
    let handler_lock = manager.join(guild_id, channel_id).await?;
    let mut handler = handler_lock.lock().await;

    // Stop any current playback and set max bitrate
    handler.stop();
    handler.set_bitrate(Bitrate::Max);

    // Play the file
    let abs_path = std::fs::canonicalize(&song_path)?;
    info!("Playing file: {}", abs_path.display());

    let source = AudioFile::new(abs_path);
    let track_handle = handler.play_input(source.into());
    info!("Track started: {:?}", track_handle.uuid());

    Ok(song_name)
}

/// Stop playback and leave the voice channel.
///
/// # Errors
///
/// Returns an error if the voice manager is unavailable or leaving fails.
pub async fn stop_playback(ctx: &Context, guild_id: GuildId) -> Result<()> {
    let manager = get_manager(ctx).await?;
    manager.leave(guild_id).await?;
    info!("Left voice channel in guild {guild_id}");
    Ok(())
}
