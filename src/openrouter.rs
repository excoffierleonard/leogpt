use log::debug;
use serde::{Deserialize, Serialize};

use crate::error::{BotError, Result};

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

// Discord's message limit is 2000 characters (standard users)
// Roughly 1 token ≈ 4 characters, so 2000 chars ≈ 500 tokens
// Using 512 tokens to be safe
const MAX_TOKENS: u32 = 512;

#[derive(Debug, Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

pub struct OpenRouterClient {
    api_key: String,
    client: reqwest::Client,
    model: String,
    system_prompt: String,
}

impl OpenRouterClient {
    pub fn new(api_key: String, model: String, system_prompt: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
            model,
            system_prompt,
        }
    }

    pub async fn chat_with_history(
        &self,
        mut messages: Vec<Message>,
        dynamic_context: Option<String>,
    ) -> Result<String> {
        debug!(
            "Sending request to OpenRouter API with {} messages",
            messages.len()
        );

        // Build the full system prompt with dynamic context
        let full_system_prompt = if let Some(context) = dynamic_context {
            format!("{}\n\n{}", context, self.system_prompt)
        } else {
            self.system_prompt.clone()
        };

        // Ensure system prompt is at the beginning
        if messages.is_empty() || messages[0].role != "system" {
            messages.insert(
                0,
                Message {
                    role: "system".to_string(),
                    content: full_system_prompt,
                },
            );
        }

        let request = OpenRouterRequest {
            model: self.model.clone(),
            messages,
            max_tokens: MAX_TOKENS,
        };

        let response = self
            .client
            .post(OPENROUTER_API_URL)
            .bearer_auth(&self.api_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(BotError::OpenRouterApi { status, message });
        }

        let api_response: OpenRouterResponse = response.json().await?;

        let reply = api_response
            .choices
            .first()
            .map(|choice| choice.message.content.clone())
            .ok_or_else(|| BotError::OpenRouterResponse("No choices in response".to_string()))?;

        debug!("Received response from OpenRouter API");
        Ok(reply)
    }
}
