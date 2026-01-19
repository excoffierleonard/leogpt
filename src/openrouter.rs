use log::debug;
use serde::{Deserialize, Serialize};

use crate::error::{BotError, Result};
use crate::types::MessageRole;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Tool>>,
}

// Tool calling structures
#[derive(Debug, Clone, Serialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    MultiPart(Vec<ContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
    VideoUrl { video_url: VideoUrl },
    File { file: File },
    InputAudio { input_audio: AudioData },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoUrl {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub filename: String,
    pub file_data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioData {
    pub data: String,
    pub format: String,
}

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
        tools: Option<Vec<Tool>>,
    ) -> Result<ChatResult> {
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
            model: self.model.clone(),
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
            let message = response
                .text()
                .await
                .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
            return Err(BotError::OpenRouterApi { status, message });
        }

        let api_response: OpenRouterResponse = response.json().await?;

        let message = api_response
            .choices
            .first()
            .ok_or_else(|| BotError::OpenRouterResponse("No choices in response".to_string()))?
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
