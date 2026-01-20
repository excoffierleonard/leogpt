//! Audio generation (text-to-speech) tool implementation using OpenRouter API.

use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::Utc;
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

/// Create a WAV file from raw PCM16 audio data
/// OpenAI TTS outputs 24kHz mono 16-bit PCM
fn create_wav_from_pcm16(pcm_data: &[u8]) -> Vec<u8> {
    const SAMPLE_RATE: u32 = 24000;
    const NUM_CHANNELS: u16 = 1;
    const BITS_PER_SAMPLE: u16 = 16;
    const BYTE_RATE: u32 = SAMPLE_RATE * NUM_CHANNELS as u32 * BITS_PER_SAMPLE as u32 / 8;
    const BLOCK_ALIGN: u16 = NUM_CHANNELS * BITS_PER_SAMPLE / 8;

    let data_size = pcm_data.len() as u32;
    let file_size = 36 + data_size;

    let mut wav = Vec::with_capacity(44 + pcm_data.len());

    // RIFF header
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // fmt subchunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size (16 for PCM)
    wav.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat (1 = PCM)
    wav.extend_from_slice(&NUM_CHANNELS.to_le_bytes());
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&BYTE_RATE.to_le_bytes());
    wav.extend_from_slice(&BLOCK_ALIGN.to_le_bytes());
    wav.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());

    // data subchunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    wav.extend_from_slice(pcm_data);

    wav
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

    let request = AudioGenRequest {
        model: AUDIO_GEN_MODEL.to_string(),
        messages: vec![RequestMessage {
            role: "user",
            content: args.text.clone(),
        }],
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

    // Process streaming response - read full body and parse SSE events
    let response_text = response.text().await?;
    let mut audio_data = String::new();

    // Parse SSE lines
    for line in response_text.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            if data == "[DONE]" {
                continue;
            }

            if let Ok(parsed) = serde_json::from_str::<StreamChunk>(data)
                && let Some(choice) = parsed.choices.first()
                && let Some(delta) = &choice.delta
                && let Some(audio) = &delta.audio
                && let Some(audio_chunk) = &audio.data
            {
                audio_data.push_str(audio_chunk);
            }
        }
    }

    if audio_data.is_empty() {
        return Err(BotError::OpenRouterResponse("No audio generated".into()));
    }

    debug!("Audio generation completed, decoding base64 data");

    // Decode base64 audio data (PCM16 format: 24kHz, mono, 16-bit)
    let pcm_bytes = STANDARD.decode(&audio_data)?;

    // Wrap PCM16 data in WAV container for Discord playback
    let audio_bytes = create_wav_from_pcm16(&pcm_bytes);
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
