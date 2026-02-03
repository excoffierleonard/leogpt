//! Music playback module for voice channel audio.

mod commands;
mod fuzzy_search;
mod playback;

pub use commands::music_commands;
pub use fuzzy_search::{find_song, list_songs};
pub use playback::{MusicConfig, play_song, stop_playback};
