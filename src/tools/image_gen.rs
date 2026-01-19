//! Image generation tool implementation using OpenRouter's multimodal API.

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use chrono::Utc;
use log::debug;
use serde::{Deserialize, Serialize};

use crate::error::{BotError, Result};

use super::executor::{ToolContext, ToolOutput};

/// OpenRouter chat completions API URL
const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Model for image generation
const IMAGE_GEN_MODEL: &str = "google/gemini-2.5-flash-image";

/// Arguments for the generate_image tool
#[derive(Debug, Deserialize)]
struct ImageGenArgs {
    prompt: String,
    aspect_ratio: Option<String>,
    size: Option<String>,
}

/// Request payload for image generation
#[derive(Debug, Serialize)]
struct ImageGenRequest {
    model: String,
    messages: Vec<RequestMessage>,
    modalities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_config: Option<ImageConfig>,
}

/// Message in the request
#[derive(Debug, Serialize)]
struct RequestMessage {
    role: &'static str,
    content: MessageContent,
}

/// Content can be text or multipart (for image editing)
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    MultiPart(Vec<ContentPart>),
}

/// A part of multipart content
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrlInput },
}

/// Image URL input for editing
#[derive(Debug, Serialize)]
struct ImageUrlInput {
    url: String,
}

/// Image configuration for Gemini models
#[derive(Debug, Serialize)]
struct ImageConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    aspect_ratio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_size: Option<String>,
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
    #[serde(default)]
    images: Vec<ImageOutput>,
}

/// Image output in the response
#[derive(Debug, Deserialize)]
struct ImageOutput {
    image_url: ImageUrlOutput,
}

/// Image URL in the output
#[derive(Debug, Deserialize)]
struct ImageUrlOutput {
    url: String,
}

/// Validate aspect ratio is one of the supported values
fn validate_aspect_ratio(ratio: &str) -> bool {
    matches!(
        ratio,
        "1:1" | "16:9" | "9:16" | "2:3" | "3:2" | "3:4" | "4:3" | "4:5" | "5:4" | "21:9"
    )
}

/// Validate image size is one of the supported values
fn validate_image_size(size: &str) -> bool {
    matches!(size, "1K" | "2K" | "4K")
}

/// Parse a data URL and extract the image bytes and format
///
/// Expected format: `data:image/png;base64,<base64-encoded-data>`
fn parse_data_url(data_url: &str) -> Result<(Vec<u8>, String)> {
    // Check for data URL prefix
    let data_url = data_url
        .strip_prefix("data:")
        .ok_or_else(|| BotError::ToolExecution("Invalid data URL format".into()))?;

    // Split mime type and data
    let (mime_and_encoding, base64_data) = data_url
        .split_once(',')
        .ok_or_else(|| BotError::ToolExecution("Invalid data URL: missing data".into()))?;

    // Extract file extension from mime type (e.g., "image/png;base64" -> "png")
    let extension = mime_and_encoding
        .split(';')
        .next()
        .and_then(|mime| mime.strip_prefix("image/"))
        .unwrap_or("png")
        .to_string();

    // Decode base64 data
    let bytes = BASE64.decode(base64_data)?;

    Ok((bytes, extension))
}

/// Generate an image using OpenRouter's multimodal API
///
/// Makes a request to OpenRouter with the `modalities: ["image", "text"]` parameter
/// to enable image generation from the Gemini model.
pub async fn generate_image(arguments: &str, tool_ctx: &ToolContext<'_>) -> Result<ToolOutput> {
    let args: ImageGenArgs = serde_json::from_str(arguments)?;

    debug!(
        "Image generation with prompt: '{}', {} context images available",
        args.prompt,
        tool_ctx.recent_images.len()
    );

    // Build image config if any options are provided
    let image_config = if args.aspect_ratio.is_some() || args.size.is_some() {
        // Validate aspect ratio if provided
        if let Some(ref ratio) = args.aspect_ratio
            && !validate_aspect_ratio(ratio)
        {
            return Err(BotError::ToolExecution(format!(
                "Invalid aspect ratio '{}'. Supported: 1:1, 16:9, 9:16, 2:3, 3:2, 3:4, 4:3, 4:5, 5:4, 21:9",
                ratio
            )));
        }

        // Validate image size if provided
        if let Some(ref size) = args.size
            && !validate_image_size(size)
        {
            return Err(BotError::ToolExecution(format!(
                "Invalid image size '{}'. Supported: 1K, 2K, 4K",
                size
            )));
        }

        Some(ImageConfig {
            aspect_ratio: args.aspect_ratio.clone(),
            image_size: args.size.clone(),
        })
    } else {
        None
    };

    // Build message content - include context images if available
    let content = if tool_ctx.recent_images.is_empty() {
        MessageContent::Text(args.prompt.clone())
    } else {
        // Include recent images as context, then the prompt
        let mut parts: Vec<ContentPart> = tool_ctx
            .recent_images
            .iter()
            .map(|url| ContentPart::ImageUrl {
                image_url: ImageUrlInput { url: url.clone() },
            })
            .collect();
        parts.push(ContentPart::Text {
            text: args.prompt.clone(),
        });
        MessageContent::MultiPart(parts)
    };

    let request = ImageGenRequest {
        model: IMAGE_GEN_MODEL.to_string(),
        messages: vec![RequestMessage {
            role: "user",
            content,
        }],
        modalities: vec!["image".to_string()],
        image_config,
    };

    let client = reqwest::Client::new();
    let response = client
        .post(OPENROUTER_API_URL)
        .bearer_auth(tool_ctx.openrouter_api_key)
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

    let data_url = api_response
        .choices
        .first()
        .and_then(|c| c.message.images.first())
        .map(|img| img.image_url.url.clone())
        .ok_or_else(|| BotError::OpenRouterResponse("No image generated".into()))?;

    debug!("Image generation completed, decoding base64 data");

    // Parse the data URL to get raw bytes and format
    let (image_bytes, extension) = parse_data_url(&data_url)?;
    let filename = format!("generated_{}.{}", Utc::now().timestamp(), extension);

    debug!(
        "Decoded image: {} bytes, format: {}",
        image_bytes.len(),
        extension
    );

    // Return both text for LLM and image data for Discord
    let text = format!(
        "Image generated successfully ({} bytes, {} format)",
        image_bytes.len(),
        extension
    );

    Ok(ToolOutput::with_image(text, image_bytes, filename))
}
