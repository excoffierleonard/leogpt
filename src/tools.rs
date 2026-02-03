//! Discord-native tools for the `OpenRouter` tool calling API

mod audio_gen;
mod definitions;
mod executor;
mod image_gen;
mod search;
mod server_info;
mod user_info;
mod utils;
mod web_search;

pub use definitions::get_tool_definitions;
pub use executor::{AudioAttachment, ImageAttachment, ToolContext, ToolExecutor, ToolOutput};
