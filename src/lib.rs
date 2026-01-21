//! LeoGPT - A Discord bot powered by OpenRouter AI.

mod bot;
pub mod config;
pub mod error;
pub mod media;
pub mod openrouter;
pub mod tools;
pub mod types;

pub use bot::run;
pub use error::Result;
