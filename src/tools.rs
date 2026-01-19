//! Discord-native tools for the OpenRouter tool calling API

mod definitions;
mod executor;
mod image_gen;
mod search;
mod server_info;
mod user_info;
mod web_search;

pub use definitions::get_tool_definitions;
pub use executor::{ImageAttachment, ToolContext, ToolExecutor, ToolOutput};
