//! Audio transcription tool implementation using `OpenRouter`'s speech-to-text API.

use base64::{Engine, engine::general_purpose::STANDARD};
use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{
    config::TRANSCRIBE_MODEL,
    error::{BotError, Result},
};

use super::executor::{ToolContext, ToolOutput};

/// `OpenRouter` audio transcription endpoint
const OPENROUTER_TRANSCRIPTIONS_URL: &str = "https://openrouter.ai/api/v1/audio/transcriptions";

/// Audio formats accepted by the `input_audio.format` field.
const SUPPORTED_FORMATS: &[&str] = &["wav", "mp3", "flac", "m4a", "ogg", "webm", "aac"];

/// Default format used when it cannot be determined from the source.
const DEFAULT_FORMAT: &str = "mp3";

/// Arguments for the `transcribe_audio` tool
#[derive(Debug, Deserialize)]
struct TranscribeArgs {
    url: String,
    language: Option<String>,
}

/// Request payload for the `/audio/transcriptions` endpoint
#[derive(Debug, Serialize)]
struct TranscribeRequest {
    model: String,
    input_audio: InputAudio,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
}

/// Base64-encoded audio payload
#[derive(Debug, Serialize)]
struct InputAudio {
    data: String,
    format: String,
}

/// Response from the `/audio/transcriptions` endpoint
#[derive(Debug, Deserialize)]
struct TranscribeResponse {
    text: String,
}

/// Guess the audio format from a response's Content-Type header, falling back to the
/// file extension in the URL, and finally to `DEFAULT_FORMAT`.
fn guess_audio_format(response: &reqwest::Response, url: &str) -> String {
    let from_content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<mime::Mime>().ok())
        .map(|mime| mime.subtype().to_string());

    let from_extension = || {
        url.rsplit('.')
            .next()
            .map(str::to_lowercase)
            .filter(|ext| !ext.is_empty())
    };

    from_content_type
        .into_iter()
        .chain(from_extension())
        .find(|format| SUPPORTED_FORMATS.contains(&format.as_str()))
        .unwrap_or_else(|| DEFAULT_FORMAT.to_string())
}

/// Transcribe audio to text using `OpenRouter`'s speech-to-text API.
///
/// Downloads the audio from the given URL, base64-encodes it, and sends it to
/// `OpenRouter`'s dedicated transcription endpoint.
pub async fn transcribe_audio(arguments: &str, tool_ctx: &ToolContext<'_>) -> Result<ToolOutput> {
    let args: TranscribeArgs = serde_json::from_str(arguments)?;

    if args.url.trim().is_empty() {
        return Err(BotError::ToolExecution("Audio URL cannot be empty.".into()));
    }

    debug!("Transcribing audio from URL: {}", args.url);

    let client = Client::new();
    let audio_response = client.get(&args.url).send().await?;
    if !audio_response.status().is_success() {
        let status = audio_response.status();
        let message = audio_response.text().await?;
        return Err(BotError::OpenRouterApi { status, message });
    }
    let format = guess_audio_format(&audio_response, &args.url);
    let audio_bytes = audio_response.bytes().await?;
    if audio_bytes.is_empty() {
        return Err(BotError::ToolExecution("Downloaded audio is empty.".into()));
    }

    let request = TranscribeRequest {
        model: TRANSCRIBE_MODEL.to_string(),
        input_audio: InputAudio {
            data: STANDARD.encode(&audio_bytes),
            format,
        },
        language: args.language,
    };

    let response = client
        .post(OPENROUTER_TRANSCRIPTIONS_URL)
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

    let transcription: TranscribeResponse = response.json().await?;
    if transcription.text.trim().is_empty() {
        return Err(BotError::OpenRouterResponse(
            "No transcription text returned".into(),
        ));
    }

    debug!(
        "Transcription completed: {} characters",
        transcription.text.len()
    );

    Ok(ToolOutput::text(transcription.text))
}
