//! Common types used throughout the leogpt bot.

use mime::Mime;
use serde::{Deserialize, Serialize};

/// Role of a message in the conversation.
///
/// Maps to OpenRouter API message roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// Message from the human user
    User,
    /// Message from the AI assistant
    Assistant,
    /// System prompt or instructions
    System,
    /// Result from a tool execution
    Tool,
}

/// Supported media types for attachments.
///
/// Determines how Discord attachments are processed for the OpenRouter API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    /// Image files (PNG, JPEG, GIF, WebP)
    Image,
    /// Video files (MP4, WebM)
    Video,
    /// Audio files (MP3, WAV, OGG, FLAC)
    Audio,
    /// PDF documents
    Pdf,
}

impl MediaType {
    /// Determine media type from a MIME content type string
    pub fn from_content_type(content_type: &str) -> Option<MediaType> {
        let mime: Mime = content_type.parse().ok()?;
        match (mime.type_(), mime.subtype()) {
            (mime::IMAGE, _) => Some(MediaType::Image),
            (mime::VIDEO, _) => Some(MediaType::Video),
            (mime::AUDIO, _) => Some(MediaType::Audio),
            (mime::APPLICATION, subtype) if subtype.as_str() == "pdf" => Some(MediaType::Pdf),
            _ => None,
        }
    }
}

/// Audio format for OpenRouter API
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFormat(String);

impl AudioFormat {
    /// Convert a MIME type to an audio format string
    /// e.g., "audio/mpeg" -> "mp3", "audio/wav" -> "wav"
    pub fn from_mime_type(mime_type: &str) -> Self {
        let format = mime_type
            .parse::<Mime>()
            .ok()
            .and_then(|mime| {
                mime_guess::get_mime_extensions(&mime)
                    .and_then(|extensions| extensions.first().copied())
            })
            .unwrap_or("wav");
        AudioFormat(format.to_string())
    }

    /// Returns the format string for the OpenRouter API
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<AudioFormat> for String {
    fn from(format: AudioFormat) -> Self {
        format.0
    }
}
