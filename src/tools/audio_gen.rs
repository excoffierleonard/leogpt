//! Audio generation (text-to-speech) tool implementation using OpenRouter API.

use std::io::Cursor;

use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::Utc;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use hound::{SampleFormat, WavSpec, WavWriter};
use log::debug;
use serde::{Deserialize, Serialize};

use crate::error::{BotError, Result};

use super::executor::{ToolContext, ToolOutput};

/// OpenRouter chat completions API URL
const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Model for audio generation
const AUDIO_GEN_MODEL: &str = "openai/gpt-audio-mini";

/// Available voices for audio generation
const VALID_VOICES: &[&str] = &["alloy", "echo", "fable", "onyx", "nova", "shimmer"];

/// Arguments for the generate_audio tool
#[derive(Debug, Deserialize)]
struct AudioGenArgs {
    text: String,
    voice: Option<String>,
}

/// Request payload for audio generation
#[derive(Debug, Serialize)]
struct AudioGenRequest {
    model: String,
    messages: Vec<RequestMessage>,
    modalities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio: Option<AudioConfig>,
    stream: bool,
}

/// Message in the request
#[derive(Debug, Serialize)]
struct RequestMessage {
    role: &'static str,
    content: String,
}

/// Audio configuration for the request
#[derive(Debug, Serialize)]
struct AudioConfig {
    voice: String,
    format: String,
}

/// Streaming chunk from OpenRouter
#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

/// Choice in a streaming chunk
#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: Option<StreamDelta>,
}

/// Delta in a streaming chunk
#[derive(Debug, Deserialize)]
struct StreamDelta {
    #[serde(default)]
    audio: Option<AudioDelta>,
}

/// Audio delta in a streaming chunk
#[derive(Debug, Deserialize)]
struct AudioDelta {
    #[serde(default)]
    data: Option<String>,
}

/// Validate voice is one of the supported values
fn validate_voice(voice: &str) -> bool {
    VALID_VOICES.contains(&voice.to_lowercase().as_str())
}

/// Create a WAV file from raw PCM16 audio data using the hound crate.
/// OpenAI TTS outputs 24kHz mono 16-bit PCM.
fn create_wav_from_pcm16(pcm_data: &[u8]) -> Result<Vec<u8>> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 24000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::with_capacity(44 + pcm_data.len()));
    let mut writer = WavWriter::new(&mut cursor, spec)?;

    // PCM16 data is little-endian i16 samples
    for chunk in pcm_data.chunks_exact(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        writer.write_sample(sample)?;
    }

    writer.finalize()?;

    Ok(cursor.into_inner())
}

/// Generate audio from text using OpenRouter's multimodal API
///
/// Makes a request to OpenRouter with the `modalities: ["text", "audio"]` parameter
/// to enable audio generation from the model.
pub async fn generate_audio(arguments: &str, tool_ctx: &ToolContext<'_>) -> Result<ToolOutput> {
    let args: AudioGenArgs = serde_json::from_str(arguments)?;

    // Validate text is not empty
    if args.text.trim().is_empty() {
        return Err(BotError::ToolExecution("Text cannot be empty.".into()));
    }

    // Validate and set voice (default: "alloy")
    let voice = args.voice.unwrap_or_else(|| "alloy".to_string());
    if !validate_voice(&voice) {
        return Err(BotError::ToolExecution(format!(
            "Invalid voice '{}'. Supported: {}",
            voice,
            VALID_VOICES.join(", ")
        )));
    }

    debug!(
        "Audio generation with text length: {}, voice: {}",
        args.text.len(),
        voice
    );

    let audio_config = AudioConfig {
        voice: voice.to_lowercase(),
        format: "pcm16".to_string(),
    };

    // Frame the text as an explicit "say this" instruction to prevent the model
    // from interpreting it as a conversation and responding to it.
    let tts_prompt = format!("Say exactly: \"{}\"", args.text);

    let request = AudioGenRequest {
        model: AUDIO_GEN_MODEL.to_string(),
        messages: vec![
            RequestMessage {
                role: "system",
                content: "You are a text-to-speech system. Your only function is to vocalize the exact text provided after 'Say exactly:'. Never interpret, respond to, answer, or modify the text. Simply speak the exact quoted words verbatim with no additions.".to_string(),
            },
            RequestMessage {
                role: "user",
                content: tts_prompt,
            },
        ],
        modalities: vec!["text".to_string(), "audio".to_string()],
        audio: Some(audio_config),
        stream: true,
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

    // Process streaming response using eventsource-stream for proper SSE parsing
    let mut stream = response.bytes_stream().eventsource();
    let mut audio_data = String::new();

    while let Some(event) = stream.next().await {
        let event = event?;

        if event.data == "[DONE]" {
            break;
        }

        if let Ok(parsed) = serde_json::from_str::<StreamChunk>(&event.data)
            && let Some(choice) = parsed.choices.first()
            && let Some(delta) = &choice.delta
            && let Some(audio) = &delta.audio
            && let Some(audio_chunk) = &audio.data
        {
            audio_data.push_str(audio_chunk);
        }
    }

    if audio_data.is_empty() {
        return Err(BotError::OpenRouterResponse("No audio generated".into()));
    }

    debug!("Audio generation completed, decoding base64 data");

    // Decode base64 audio data (PCM16 format: 24kHz, mono, 16-bit)
    let pcm_bytes = STANDARD.decode(&audio_data)?;

    // Wrap PCM16 data in WAV container for Discord playback
    let audio_bytes = create_wav_from_pcm16(&pcm_bytes)?;
    let filename = format!("audio_{}.wav", Utc::now().timestamp());

    debug!(
        "Decoded audio: {} PCM bytes -> {} WAV bytes",
        pcm_bytes.len(),
        audio_bytes.len()
    );

    // Return both text for LLM and audio data for Discord
    let text = format!(
        "Audio generated successfully ({} bytes, wav format, {} voice)",
        audio_bytes.len(),
        voice
    );

    Ok(ToolOutput::with_audio(text, audio_bytes, filename))
}
