//! Discord-native tools for the OpenRouter tool calling API

mod definitions;
mod executor;
mod search;
mod user_info;

pub use definitions::get_tool_definitions;
pub use executor::{ToolContext, ToolExecutor};
