//! Text-to-speech tool implementation using `OpenRouter`'s dedicated speech API.

use chrono::Utc;
use log::debug;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString, VariantNames};

use crate::{
    config::SPEECH_GEN_MODEL,
    error::{BotError, Result, ensure_success},
};

use super::executor::{ToolContext, ToolOutput};

/// `OpenRouter` text-to-speech endpoint
const OPENROUTER_SPEECH_URL: &str = "https://openrouter.ai/api/v1/audio/speech";

#[derive(Debug, Clone, Copy, EnumString, VariantNames, Display)]
#[strum(ascii_case_insensitive)]
enum SpeechVoice {
    Zephyr,
    Puck,
    Charon,
    Kore,
    Fenrir,
    Leda,
    Orus,
    Aoede,
    Callirrhoe,
    Autonoe,
    Enceladus,
    Iapetus,
    Umbriel,
    Algieba,
    Despina,
    Erinome,
    Algenib,
    Rasalgethi,
    Laomedeia,
    Achernar,
    Alnilam,
    Schedar,
    Gacrux,
    Pulcherrima,
    Achird,
    Zubenelgenubi,
    Vindemiatrix,
    Sadachbia,
    Sadaltager,
    Sulafat,
}

/// Arguments for the `generate_speech` tool
#[derive(Debug, Deserialize)]
struct SpeechGenArgs {
    text: String,
    voice: Option<String>,
}

/// Request payload for the `/audio/speech` endpoint
#[derive(Debug, Serialize)]
struct SpeechGenRequest {
    model: String,
    input: String,
    voice: String,
    response_format: &'static str,
}

/// Convert text to speech using `OpenRouter`'s dedicated text-to-speech API.
pub async fn generate_speech(arguments: &str, tool_ctx: &ToolContext<'_>) -> Result<ToolOutput> {
    let args: SpeechGenArgs = serde_json::from_str(arguments)?;

    if args.text.trim().is_empty() {
        return Err(BotError::ToolExecution("Text cannot be empty.".into()));
    }

    let voice = match args.voice.as_deref() {
        Some(raw) => raw.parse::<SpeechVoice>().map_err(|_| {
            BotError::ToolExecution(format!(
                "Invalid voice '{}'. Supported: {}",
                raw,
                SpeechVoice::VARIANTS.join(", ")
            ))
        })?,
        None => SpeechVoice::Zephyr,
    };

    debug!(
        "Speech generation with text length: {}, voice: {}",
        args.text.len(),
        voice
    );

    let request = SpeechGenRequest {
        model: SPEECH_GEN_MODEL.to_string(),
        input: args.text,
        voice: voice.to_string(),
        response_format: "mp3",
    };

    let response = tool_ctx
        .client
        .post(OPENROUTER_SPEECH_URL)
        .bearer_auth(tool_ctx.openrouter_api_key)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;

    let audio_bytes = ensure_success(response).await?.bytes().await?.to_vec();
    if audio_bytes.is_empty() {
        return Err(BotError::OpenRouterResponse("No audio generated".into()));
    }
    let filename = format!("speech_{}.mp3", Utc::now().timestamp());

    debug!("Speech generation completed: {} bytes", audio_bytes.len());

    let text = format!(
        "Speech generated successfully ({} bytes, mp3 format, {} voice)",
        audio_bytes.len(),
        voice
    );
    Ok(ToolOutput::with_audio(text, audio_bytes, filename))
}
