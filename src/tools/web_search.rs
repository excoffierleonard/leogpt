//! Web search tool implementation using `OpenRouter`'s online search.

use log::debug;
use serde::{Deserialize, Serialize};

use crate::{
    config::TEXT_MODEL,
    error::{BotError, Result, ensure_success},
};

use super::executor::ToolContext;

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Arguments for the `web_search` tool
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

/// Response from `OpenRouter`
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

/// Perform a web search using `OpenRouter`'s online search capability
///
/// Makes a request to `OpenRouter` with the `:online` suffix appended to the model,
/// which enables web search for that request.
pub async fn web_search(arguments: &str, tool_ctx: &ToolContext<'_>) -> Result<String> {
    let args: WebSearchArgs = serde_json::from_str(arguments)?;

    debug!("Performing web search for: {}", args.query);

    let online_model = format!("{TEXT_MODEL}:online");

    let request = WebSearchRequest {
        model: online_model,
        messages: vec![RequestMessage {
            role: "user",
            content: args.query.clone(),
        }],
        max_tokens: 4096,
    };

    let response = tool_ctx
        .client
        .post(OPENROUTER_API_URL)
        .bearer_auth(tool_ctx.openrouter_api_key)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;

    let api_response: OpenRouterResponse = ensure_success(response).await?.json().await?;

    let content = api_response
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .ok_or_else(|| BotError::OpenRouterResponse("No search results".into()))?;

    debug!("Web search completed");

    Ok(content)
}
