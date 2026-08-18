//! Audio generation tool implementation using `OpenRouter`'s multimodal API.
//!
//! For general creative audio (sound effects, music, expressive voice performances).
//! For literal text-to-speech narration, see `speech_gen`.

use std::io::Cursor;

use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::Utc;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use hound::{SampleFormat, WavSpec, WavWriter};
use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{
    config::AUDIO_GEN_MODEL,
    error::{BotError, Result},
};

use super::executor::{ToolContext, ToolOutput};

/// `OpenRouter` chat completions API URL
const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Arguments for the `generate_audio` tool
#[derive(Debug, Deserialize)]
struct AudioGenArgs {
    prompt: String,
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
    format: String,
}

/// Streaming chunk from `OpenRouter`
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

/// Create a WAV file from raw PCM16 audio data using the hound crate.
/// `AUDIO_GEN_MODEL` (Lyria) outputs 48kHz stereo 16-bit PCM.
fn create_wav_from_pcm16(pcm_data: &[u8]) -> Result<Vec<u8>> {
    let spec = WavSpec {
        channels: 2,
        sample_rate: 48000,
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

/// Generate audio from a creative prompt using `OpenRouter`'s multimodal API
///
/// Makes a request to `OpenRouter` with the `modalities: ["text", "audio"]` parameter
/// to enable audio generation from the model.
pub async fn generate_audio(arguments: &str, tool_ctx: &ToolContext<'_>) -> Result<ToolOutput> {
    let args: AudioGenArgs = serde_json::from_str(arguments)?;

    if args.prompt.trim().is_empty() {
        return Err(BotError::ToolExecution("Prompt cannot be empty.".into()));
    }

    debug!("Audio generation with prompt length: {}", args.prompt.len());

    let audio_config = AudioConfig {
        format: "pcm16".to_string(),
    };

    let request = AudioGenRequest {
        model: AUDIO_GEN_MODEL.to_string(),
        messages: vec![RequestMessage {
            role: "user",
            content: args.prompt,
        }],
        modalities: vec!["text".to_string(), "audio".to_string()],
        audio: Some(audio_config),
        stream: true,
    };

    let client = Client::new();
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

    // Decode base64 audio data (PCM16 format: 48kHz, stereo, 16-bit)
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
        "Audio generated successfully ({} bytes, wav format)",
        audio_bytes.len()
    );

    Ok(ToolOutput::with_audio(text, audio_bytes, filename))
}
