//! Auto-response rules and matching utilities.

mod handler;
mod matching;
mod rules;

pub use handler::handle_auto_response;
pub use rules::{AutoResponseRule, hardcoded_auto_responses};
