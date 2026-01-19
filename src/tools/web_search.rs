//! Web search tool implementation using OpenRouter's online search.

use log::debug;
use serde::{Deserialize, Serialize};

use crate::error::{BotError, Result};

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Arguments for the web_search tool
#[derive(Debug, Deserialize)]
struct WebSearchArgs {
    query: String,
}

/// Request payload for web search
#[derive(Debug, Serialize)]
struct WebSearchRequest {
    model: String,
    messages: Vec<RequestMessage>,
    max_tokens: u32,
}

/// Message in the request
#[derive(Debug, Serialize)]
struct RequestMessage {
    role: &'static str,
    content: String,
}

/// Response from OpenRouter
#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    choices: Vec<Choice>,
}

/// Choice in the response
#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

/// Message in the response
#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

/// Perform a web search using OpenRouter's online search capability
///
/// Makes a request to OpenRouter with the `:online` suffix appended to the model,
/// which enables web search for that request.
pub async fn web_search(arguments: &str, api_key: &str, model: &str) -> Result<String> {
    let args: WebSearchArgs = serde_json::from_str(arguments)?;

    debug!("Performing web search for: {}", args.query);

    // Append :online to model if not already present
    let online_model = if model.contains(":online") {
        model.to_string()
    } else {
        format!("{}:online", model)
    };

    let request = WebSearchRequest {
        model: online_model,
        messages: vec![RequestMessage {
            role: "user",
            content: args.query.clone(),
        }],
        max_tokens: 4096,
    };

    let client = reqwest::Client::new();
    let response = client
        .post(OPENROUTER_API_URL)
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let message = response
            .text()
            .await
            .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
        return Err(BotError::OpenRouterApi { status, message });
    }

    let api_response: OpenRouterResponse = response.json().await?;

    let content = api_response
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_else(|| "No results found.".to_string());

    debug!("Web search completed");

    Ok(content)
}
