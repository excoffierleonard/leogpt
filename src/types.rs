use serde::{Deserialize, Serialize};

/// Role of a message in the conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Supported media types for attachments
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Image,
    Video,
    Audio,
    Pdf,
}

impl MediaType {
    /// Determine media type from a MIME content type string
    pub fn from_content_type(content_type: &str) -> Option<MediaType> {
        if content_type.starts_with("image/") {
            Some(MediaType::Image)
        } else if content_type.starts_with("video/") {
            Some(MediaType::Video)
        } else if content_type.starts_with("audio/") {
            Some(MediaType::Audio)
        } else if content_type == "application/pdf" {
            Some(MediaType::Pdf)
        } else {
            None
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
            .trim_start_matches("audio/")
            .trim_start_matches("x-")
            .replace("mpeg", "mp3");
        AudioFormat(format)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<AudioFormat> for String {
    fn from(format: AudioFormat) -> Self {
        format.0
    }
}
