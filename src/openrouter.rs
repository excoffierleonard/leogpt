use log::debug;
use serde::{Deserialize, Serialize};

use crate::error::{BotError, Result};

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

#[derive(Debug, Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<Message>,
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
    pub fn new(api_key: String, system_prompt: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
            model: "google/gemini-3-flash-preview".to_string(),
            system_prompt,
        }
    }

    pub async fn chat(&self, user_message: &str) -> Result<String> {
        debug!("Sending request to OpenRouter API");

        let request = OpenRouterRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: self.system_prompt.clone(),
                },
                Message {
                    role: "user".to_string(),
                    content: user_message.to_string(),
                },
            ],
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
