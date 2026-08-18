//! Video generation tool implementation using `OpenRouter`'s async video API.

use std::time::{Duration, Instant};

use chrono::Utc;
use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString, VariantNames};

use crate::{
    config::VIDEO_GEN_MODEL,
    error::{BotError, Result},
};

use super::executor::{ToolContext, ToolOutput};

/// `OpenRouter` video generation submit endpoint
const OPENROUTER_VIDEOS_URL: &str = "https://openrouter.ai/api/v1/videos";

/// Interval between poll attempts. Also drives the Discord typing-indicator
/// keepalive tick, so this must stay under Discord's ~10s expiry.
const POLL_INTERVAL: Duration = Duration::from_secs(8);

/// Max total time to wait for a video job before giving up.
const MAX_POLL_DURATION: Duration = Duration::from_mins(8);

/// Supported clip length range for `VIDEO_GEN_MODEL` (in seconds).
const MIN_DURATION: u32 = 4;
const MAX_DURATION: u32 = 8;

/// Default file extension when content-type sniffing fails.
const DEFAULT_VIDEO_EXTENSION: &str = "mp4";

#[derive(Debug, Clone, Copy, EnumString, VariantNames, Display)]
#[strum(ascii_case_insensitive)]
enum AspectRatio {
    #[strum(serialize = "16:9")]
    SixteenToNine,
    #[strum(serialize = "9:16")]
    NineToSixteen,
}

/// Arguments for the `generate_video` tool
#[derive(Debug, Deserialize)]
struct VideoGenArgs {
    prompt: String,
    aspect_ratio: Option<String>,
    duration: Option<u32>,
}

/// Request payload for video generation
#[derive(Debug, Serialize)]
struct VideoGenRequest {
    model: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    aspect_ratio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frame_images: Option<Vec<FrameImage>>,
}

/// A frame image reference for image-to-video generation
#[derive(Debug, Serialize)]
struct FrameImage {
    #[serde(rename = "type")]
    kind: &'static str,
    image_url: FrameImageUrl,
    frame_type: &'static str,
}

/// URL wrapper for a frame image
#[derive(Debug, Serialize)]
struct FrameImageUrl {
    url: String,
}

/// Response from submitting a video generation job (202 Accepted)
#[derive(Debug, Deserialize)]
struct VideoSubmitResponse {
    id: String,
    polling_url: String,
}

/// Response from polling a video generation job
#[derive(Debug, Deserialize)]
struct VideoPollResponse {
    status: String,
    #[serde(default)]
    error: Option<serde_json::Value>,
}

/// Validate and normalize the aspect ratio argument.
fn parse_aspect_ratio(raw: Option<&str>) -> Result<Option<String>> {
    match raw {
        Some(raw) => {
            let ratio = raw.parse::<AspectRatio>().map_err(|_| {
                BotError::ToolExecution(format!(
                    "Invalid aspect ratio '{}'. Supported: {}",
                    raw,
                    AspectRatio::VARIANTS.join(", ")
                ))
            })?;
            Ok(Some(ratio.to_string()))
        }
        None => Ok(None),
    }
}

/// Build the `frame_images` array from the most recent image in the conversation, if any.
fn build_frame_images(recent_images: &[String]) -> Option<Vec<FrameImage>> {
    recent_images.first().map(|url| {
        vec![FrameImage {
            kind: "image_url",
            image_url: FrameImageUrl { url: url.clone() },
            frame_type: "first_frame",
        }]
    })
}

/// Permissively extract a human-readable message from an arbitrary `error` JSON value.
fn extract_error_message(error: &serde_json::Value) -> String {
    error
        .as_str()
        .map(str::to_string)
        .or_else(|| {
            error
                .get("message")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| error.to_string())
}

/// Guess a file extension from a response's Content-Type header, falling back to mp4.
fn extension_from_content_type(response: &reqwest::Response) -> String {
    response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<mime::Mime>().ok())
        .and_then(|mime| {
            mime_guess::get_mime_extensions(&mime).and_then(|exts| exts.first().copied())
        })
        .unwrap_or(DEFAULT_VIDEO_EXTENSION)
        .to_string()
}

/// Submit a video generation job to `OpenRouter`.
async fn submit_job(
    client: &Client,
    request: &VideoGenRequest,
    tool_ctx: &ToolContext<'_>,
) -> Result<VideoSubmitResponse> {
    let response = client
        .post(OPENROUTER_VIDEOS_URL)
        .bearer_auth(tool_ctx.openrouter_api_key)
        .header("Content-Type", "application/json")
        .json(request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let message = response.text().await?;
        return Err(BotError::OpenRouterApi { status, message });
    }

    Ok(response.json().await?)
}

/// Poll until the job completes, fails, or exceeds `MAX_POLL_DURATION`.
///
/// Re-broadcasts the Discord typing indicator on every tick so the channel
/// shows "bot is typing" for the whole generation duration.
async fn poll_until_complete(
    client: &Client,
    polling_url: &str,
    tool_ctx: &ToolContext<'_>,
) -> Result<()> {
    let start = Instant::now();

    loop {
        if start.elapsed() > MAX_POLL_DURATION {
            return Err(BotError::VideoGenerationTimeout);
        }

        let _ = tool_ctx
            .channel_id
            .broadcast_typing(&tool_ctx.ctx.http)
            .await;
        tokio::time::sleep(POLL_INTERVAL).await;

        let response = client
            .get(polling_url)
            .bearer_auth(tool_ctx.openrouter_api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let message = response.text().await?;
            return Err(BotError::OpenRouterApi { status, message });
        }

        let poll: VideoPollResponse = response.json().await?;
        debug!("Video job status: {}", poll.status);

        match poll.status.as_str() {
            "completed" => return Ok(()),
            "failed" => {
                let message = poll.error.as_ref().map_or_else(
                    || "Video generation failed with no error detail".to_string(),
                    extract_error_message,
                );
                return Err(BotError::VideoGenerationFailed(message));
            }
            _ => {}
        }
    }
}

/// Generate a video using `OpenRouter`'s async video generation API.
pub async fn generate_video(arguments: &str, tool_ctx: &ToolContext<'_>) -> Result<ToolOutput> {
    let args: VideoGenArgs = serde_json::from_str(arguments)?;
    debug!(
        "Video generation with prompt: '{}', {} context images available",
        args.prompt,
        tool_ctx.recent_images.len()
    );

    let aspect_ratio = parse_aspect_ratio(args.aspect_ratio.as_deref())?;
    let duration = args.duration.map(|d| d.clamp(MIN_DURATION, MAX_DURATION));

    let request = VideoGenRequest {
        model: VIDEO_GEN_MODEL.to_string(),
        prompt: args.prompt,
        aspect_ratio,
        duration,
        frame_images: build_frame_images(&tool_ctx.recent_images),
    };

    let client = Client::new();
    let submitted = submit_job(&client, &request, tool_ctx).await?;
    debug!("Video job submitted, polling at {}", submitted.polling_url);

    poll_until_complete(&client, &submitted.polling_url, tool_ctx).await?;

    let content_url = format!("{OPENROUTER_VIDEOS_URL}/{}/content?index=0", submitted.id);
    let response = client
        .get(&content_url)
        .bearer_auth(tool_ctx.openrouter_api_key)
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let message = response.text().await?;
        return Err(BotError::OpenRouterApi { status, message });
    }
    let extension = extension_from_content_type(&response);
    let video_bytes = response.bytes().await?.to_vec();
    let filename = format!("generated_{}.{}", Utc::now().timestamp(), extension);

    debug!(
        "Downloaded video: {} bytes, format: {}",
        video_bytes.len(),
        extension
    );

    let text = format!(
        "Video generated successfully ({} bytes, {} format)",
        video_bytes.len(),
        extension
    );
    Ok(ToolOutput::with_video(text, video_bytes, filename))
}
