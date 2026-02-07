//! `OpenRouter` API client for AI chat completions.

use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{
    error::{BotError, Result},
    types::MessageRole,
};

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

// Discord's message limit is 2000 characters (standard users)
// Roughly 1 token ≈ 4 characters, so 2000 chars ≈ 500 tokens
// Using 512 tokens to be safe
const MAX_TOKENS: u32 = 512;

/// Model for chat completions.
const COMPLETION_MODEL: &str = "google/gemini-3-flash-preview";

/// The system prompt for the assistant.
const SYSTEM_PROMPT: &str = "You are a helpful assistant.";

/// Request payload for the `OpenRouter` API.
#[derive(Debug, Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Tool>>,
}

/// A tool definition for the `OpenRouter` API.
#[derive(Debug, Clone, Serialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

/// Function definition within a tool.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// A tool call requested by the model.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

/// Function call details within a tool call.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// Result of a chat completion
pub enum ChatResult {
    /// Model produced a text response
    TextResponse(String),
    /// Model wants to call tools
    ToolCalls {
        tool_calls: Vec<ToolCall>,
        assistant_message: Message,
    },
}

/// Message content that can be text or multi-part.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    MultiPart(Vec<ContentPart>),
}

/// A part of a multi-part message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
    VideoUrl { video_url: VideoUrl },
    File { file: File },
    InputAudio { input_audio: AudioData },
}

/// Image URL for image content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
}

/// Video URL for video content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoUrl {
    pub url: String,
}

/// File data for document content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub filename: String,
    pub file_data: String,
}

/// Audio data for audio input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioData {
    pub data: String,
    pub format: String,
}

/// A message in the chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

/// Client for interacting with the `OpenRouter` API.
pub struct OpenRouterClient {
    api_key: String,
    client: Client,
}

impl OpenRouterClient {
    /// Create a new `OpenRouter` client.
    #[must_use]
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }

    /// Send a chat request with conversation history.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an invalid response.
    pub async fn chat_with_history(
        &self,
        mut messages: Vec<Message>,
        dynamic_context: Option<String>,
        tools: Option<Vec<Tool>>,
    ) -> Result<ChatResult> {
        debug!(
            "Sending request to OpenRouter API with {} messages",
            messages.len()
        );

        // Build the full system prompt with dynamic context
        let full_system_prompt = if let Some(context) = dynamic_context {
            format!("{context}\n\n{SYSTEM_PROMPT}")
        } else {
            SYSTEM_PROMPT.to_string()
        };

        // Ensure system prompt is at the beginning
        if messages.is_empty() || messages[0].role != MessageRole::System {
            messages.insert(
                0,
                Message {
                    role: MessageRole::System,
                    content: Some(MessageContent::Text(full_system_prompt)),
                    tool_calls: None,
                    tool_call_id: None,
                },
            );
        }

        let request = OpenRouterRequest {
            model: COMPLETION_MODEL.to_string(),
            messages,
            max_tokens: MAX_TOKENS,
            tools,
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
            let message = response.text().await?;
            return Err(BotError::OpenRouterApi { status, message });
        }

        let api_response: OpenRouterResponse = response.json().await?;

        let message = api_response
            .choices
            .first()
            .ok_or_else(|| BotError::OpenRouterResponse("No choices in response".into()))?
            .message
            .clone();

        // Check if response contains tool calls
        if let Some(ref tool_calls) = message.tool_calls
            && !tool_calls.is_empty()
        {
            debug!(
                "Received {} tool calls from OpenRouter API",
                tool_calls.len()
            );
            return Ok(ChatResult::ToolCalls {
                tool_calls: tool_calls.clone(),
                assistant_message: message,
            });
        }

        // Extract text from the response
        let reply = match message.content {
            Some(MessageContent::Text(text)) => text,
            Some(MessageContent::MultiPart(parts)) => parts
                .iter()
                .filter_map(|part| match part {
                    ContentPart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
            None => String::new(),
        };

        debug!("Received response from OpenRouter API");
        Ok(ChatResult::TextResponse(reply))
    }
}
