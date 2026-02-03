//! AI chatbot module - handles bot mentions and conversations.

mod context;
mod conversation;
mod handler;
mod response;
mod tool_loop;

pub use handler::handle_bot_mention;
