//! `LeoGPT` - A Discord bot powered by `OpenRouter` AI.

mod auto_response;
mod bot;
mod chatbot;
pub mod config;
pub mod error;
mod fuzzy_search;
pub mod media;
pub mod music;
pub mod openrouter;
mod react;
mod s3_index;
pub mod tools;
pub mod types;

pub use bot::run;
pub use error::Result;
