//! Media attachment processing for Discord messages.

use base64::{Engine, engine::general_purpose::STANDARD};
use log::{debug, warn};
use poise::serenity_prelude::Attachment;
use reqwest::get;

use crate::openrouter::{AudioData, ContentPart, File, ImageUrl, VideoUrl};
use crate::types::{AudioFormat, MediaType};

/// Check if an attachment is a supported media type
#[must_use]
pub fn is_supported_attachment(attachment: &Attachment) -> bool {
    attachment
        .content_type
        .as_ref()
        .and_then(|ct| MediaType::from_content_type(ct))
        .is_some()
}

/// Check if any attachments contain supported media
pub fn has_supported_media(attachments: &[Attachment]) -> bool {
    attachments.iter().any(is_supported_attachment)
}

/// Process a single attachment into a `ContentPart`
pub async fn process_attachment(attachment: &Attachment) -> Option<ContentPart> {
    let content_type = attachment.content_type.as_ref()?;
    let media_type = MediaType::from_content_type(content_type)?;

    match media_type {
        MediaType::Image => {
            debug!("Adding image attachment: {}", attachment.filename);
            Some(ContentPart::ImageUrl {
                image_url: ImageUrl {
                    url: attachment.url.clone(),
                },
            })
        }
        MediaType::Video => {
            debug!("Adding video attachment: {}", attachment.filename);
            Some(ContentPart::VideoUrl {
                video_url: VideoUrl {
                    url: attachment.url.clone(),
                },
            })
        }
        MediaType::Audio => {
            debug!("Fetching audio attachment: {}", attachment.filename);
            if let Some((audio_base64, bytes_len)) = fetch_audio_as_base64(&attachment.url).await {
                debug!("Adding audio attachment ({bytes_len} bytes)");
                let format = AudioFormat::from_mime_type(content_type);
                debug!("Audio format: {} -> {}", content_type, format.as_str());
                Some(ContentPart::InputAudio {
                    input_audio: AudioData {
                        data: audio_base64,
                        format: format.into(),
                    },
                })
            } else {
                warn!("Failed to fetch audio attachment: {}", attachment.filename);
                None
            }
        }
        MediaType::Pdf => {
            debug!("Adding PDF attachment: {}", attachment.filename);
            Some(ContentPart::File {
                file: File {
                    filename: attachment.filename.clone(),
                    file_data: attachment.url.clone(),
                },
            })
        }
    }
}

/// Fetch audio data from URL and encode as base64
async fn fetch_audio_as_base64(url: &str) -> Option<(String, usize)> {
    let response = get(url).await.ok()?;
    let audio_bytes = response.bytes().await.ok()?;
    let len = audio_bytes.len();
    let audio_base64 = STANDARD.encode(&audio_bytes);
    Some((audio_base64, len))
}

/// Process all attachments and return `ContentParts` for supported media
pub async fn process_attachments(attachments: &[Attachment]) -> Vec<ContentPart> {
    let mut parts = Vec::new();
    for attachment in attachments {
        if let Some(part) = process_attachment(attachment).await {
            parts.push(part);
        }
    }
    parts
}
