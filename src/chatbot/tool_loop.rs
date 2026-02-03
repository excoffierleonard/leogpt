//! Tool execution loop for AI-powered conversations.

use log::{debug, warn};

use crate::error::BotError;
use crate::openrouter::{ChatResult, ContentPart, Message, MessageContent, OpenRouterClient};
use crate::tools::{ToolContext, ToolExecutor, get_tool_definitions};
use crate::types::MessageRole;

use super::response::ToolLoopResult;

const MAX_TOOL_ITERATIONS: usize = 5;

/// Extract image URLs from conversation history (most recent first).
pub fn extract_image_urls(messages: &[Message]) -> Vec<String> {
    let mut urls = Vec::new();
    // Iterate in reverse to get most recent first
    for message in messages.iter().rev() {
        if let Some(MessageContent::MultiPart(parts)) = &message.content {
            for part in parts {
                if let ContentPart::ImageUrl { image_url } = part {
                    urls.push(image_url.url.clone());
                }
            }
        }
    }
    urls
}

/// Run the tool execution loop, handling multiple rounds of tool calls.
pub async fn run_tool_loop(
    client: &OpenRouterClient,
    conversation_history: &mut Vec<Message>,
    dynamic_context: &str,
    tool_ctx: &ToolContext<'_>,
) -> Result<ToolLoopResult, BotError> {
    let tools = Some(get_tool_definitions());
    let mut generated_images = Vec::new();
    let mut generated_audio = Vec::new();

    for _ in 0..MAX_TOOL_ITERATIONS {
        let _ = tool_ctx
            .channel_id
            .broadcast_typing(&tool_ctx.ctx.http)
            .await;

        match client
            .chat_with_history(
                conversation_history.clone(),
                Some(dynamic_context.to_string()),
                tools.clone(),
            )
            .await?
        {
            ChatResult::TextResponse(text) => {
                return Ok(ToolLoopResult {
                    text: Some(text),
                    images: generated_images,
                    audio: generated_audio,
                });
            }
            ChatResult::ToolCalls {
                tool_calls,
                assistant_message,
            } => {
                debug!("Processing {} tool calls", tool_calls.len());
                conversation_history.push(assistant_message);

                for tool_call in tool_calls {
                    let (result_text, maybe_image, maybe_audio) = match ToolExecutor::execute(
                        &tool_call.function.name,
                        &tool_call.function.arguments,
                        tool_ctx,
                    )
                    .await
                    {
                        Ok(output) => (output.text, output.image, output.audio),
                        Err(e) => {
                            warn!("Tool execution failed: {e}");
                            (format!("Error: {e}"), None, None)
                        }
                    };

                    if let Some(image) = maybe_image {
                        generated_images.push(image);
                        return Ok(ToolLoopResult {
                            text: None,
                            images: generated_images,
                            audio: generated_audio,
                        });
                    }
                    if let Some(audio) = maybe_audio {
                        generated_audio.push(audio);
                        return Ok(ToolLoopResult {
                            text: None,
                            images: generated_images,
                            audio: generated_audio,
                        });
                    }

                    conversation_history.push(Message {
                        role: MessageRole::Tool,
                        content: Some(MessageContent::Text(result_text)),
                        tool_calls: None,
                        tool_call_id: Some(tool_call.id.clone()),
                    });
                }
            }
        }
    }

    Err(BotError::ToolLoopLimit)
}
