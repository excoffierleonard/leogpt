//! Image generation tool implementation using `OpenRouter`'s multimodal API.

use chrono::Utc;
use data_url::DataUrl;
use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString, VariantNames};

use crate::error::{BotError, Result};

use super::executor::{ToolContext, ToolOutput};

/// `OpenRouter` chat completions API URL
const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Model for image generation
const IMAGE_GEN_MODEL: &str = "google/gemini-2.5-flash-image";

#[derive(Debug, Clone, Copy, EnumString, VariantNames, Display)]
#[strum(ascii_case_insensitive)]
enum AspectRatio {
    #[strum(serialize = "1:1")]
    OneToOne,
    #[strum(serialize = "16:9")]
    SixteenToNine,
    #[strum(serialize = "9:16")]
    NineToSixteen,
    #[strum(serialize = "2:3")]
    TwoToThree,
    #[strum(serialize = "3:2")]
    ThreeToTwo,
    #[strum(serialize = "3:4")]
    ThreeToFour,
    #[strum(serialize = "4:3")]
    FourToThree,
    #[strum(serialize = "4:5")]
    FourToFive,
    #[strum(serialize = "5:4")]
    FiveToFour,
    #[strum(serialize = "21:9")]
    TwentyOneToNine,
}

#[derive(Debug, Clone, Copy, EnumString, VariantNames, Display)]
#[strum(ascii_case_insensitive)]
enum ImageSize {
    #[strum(serialize = "1K")]
    OneK,
    #[strum(serialize = "2K")]
    TwoK,
    #[strum(serialize = "4K")]
    FourK,
}

/// Arguments for the `generate_image` tool
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
    /// Text content (present when model returns text instead of image)
    #[serde(default)]
    content: Option<String>,
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

/// Parse a data URL and extract the image bytes and format using RFC 2397 compliant parsing.
///
/// Expected format: `data:image/png;base64,<base64-encoded-data>`
fn parse_data_url(url: &str) -> Result<(Vec<u8>, String)> {
    let data_url = DataUrl::process(url)?;
    let (body, _fragment) = data_url.decode_to_vec()?;

    // Extract file extension from MIME type
    // Default to PNG if not an image type - safe fallback since
    // most image generation models output PNG and Discord accepts any format
    let mime = data_url.mime_type();
    let extension = if mime.type_ == "image" {
        mime.subtype.clone()
    } else {
        "png".to_string()
    };

    Ok((body, extension))
}

fn build_image_config(args: &ImageGenArgs) -> Result<Option<ImageConfig>> {
    if args.aspect_ratio.is_none() && args.size.is_none() {
        return Ok(None);
    }

    let aspect_ratio = match args.aspect_ratio.as_deref() {
        Some(raw) => Some(raw.parse::<AspectRatio>().map_err(|_| {
            BotError::ToolExecution(format!(
                "Invalid aspect ratio '{}'. Supported: {}",
                raw,
                AspectRatio::VARIANTS.join(", ")
            ))
        })?),
        None => None,
    };

    let image_size = match args.size.as_deref() {
        Some(raw) => Some(raw.parse::<ImageSize>().map_err(|_| {
            BotError::ToolExecution(format!(
                "Invalid image size '{}'. Supported: {}",
                raw,
                ImageSize::VARIANTS.join(", ")
            ))
        })?),
        None => None,
    };

    Ok(Some(ImageConfig {
        aspect_ratio: aspect_ratio.map(|r| r.to_string()),
        image_size: image_size.map(|s| s.to_string()),
    }))
}

fn build_message_content(prompt: &str, recent_images: &[String]) -> MessageContent {
    if recent_images.is_empty() {
        return MessageContent::Text(prompt.to_string());
    }

    let mut parts: Vec<ContentPart> = recent_images
        .iter()
        .map(|url| ContentPart::ImageUrl {
            image_url: ImageUrlInput { url: url.clone() },
        })
        .collect();
    parts.push(ContentPart::Text {
        text: prompt.to_string(),
    });
    MessageContent::MultiPart(parts)
}

fn extract_image_from_response(response: &OpenRouterResponse) -> Result<String> {
    let choice = response
        .choices
        .first()
        .ok_or_else(|| BotError::OpenRouterResponse("No response from image model".into()))?;

    if choice.message.images.is_empty() {
        if let Some(ref text) = choice.message.content {
            return Err(BotError::OpenRouterResponse(format!(
                "Model returned text instead of image: {}",
                text.chars().take(200).collect::<String>()
            )));
        }
        return Err(BotError::OpenRouterResponse("No image generated".into()));
    }

    choice
        .message
        .images
        .first()
        .map(|img| img.image_url.url.clone())
        .ok_or_else(|| BotError::OpenRouterResponse("No image generated".into()))
}

/// Generate an image using `OpenRouter`'s multimodal API.
pub async fn generate_image(arguments: &str, tool_ctx: &ToolContext<'_>) -> Result<ToolOutput> {
    let args: ImageGenArgs = serde_json::from_str(arguments)?;
    debug!(
        "Image generation with prompt: '{}', {} context images available",
        args.prompt,
        tool_ctx.recent_images.len()
    );

    let request = ImageGenRequest {
        model: IMAGE_GEN_MODEL.to_string(),
        messages: vec![RequestMessage {
            role: "user",
            content: build_message_content(&args.prompt, &tool_ctx.recent_images),
        }],
        modalities: vec!["image".to_string()],
        image_config: build_image_config(&args)?,
    };

    let response = Client::new()
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
    let data_url = extract_image_from_response(&api_response)?;

    debug!("Image generation completed, decoding base64 data");
    let (image_bytes, extension) = parse_data_url(&data_url)?;
    let filename = format!("generated_{}.{}", Utc::now().timestamp(), extension);
    debug!(
        "Decoded image: {} bytes, format: {}",
        image_bytes.len(),
        extension
    );

    let text = format!(
        "Image generated successfully ({} bytes, {} format)",
        image_bytes.len(),
        extension
    );
    Ok(ToolOutput::with_image(text, image_bytes, filename))
}
