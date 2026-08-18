//! Tool definitions for the `OpenRouter` tool calling API.

use serde_json::json;

use crate::openrouter::{FunctionDefinition, Tool};

fn tool(name: &str, description: &str, parameters: serde_json::Value) -> Tool {
    Tool {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: name.to_string(),
            description: description.to_string(),
            parameters,
        },
    }
}

fn search_channel_history_tool() -> Tool {
    tool(
        "search_channel_history",
        "Search recent messages in the current Discord channel using semantic search. \
         Understands meaning, not just keywords - 'food discussion' finds messages about pizza, dinner, etc. \
         Searches up to 100 recent messages.",
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "What to search for (semantic search - understands meaning)" },
                "username": { "type": "string", "description": "Filter messages by author name (fuzzy match)" },
                "limit": { "type": "integer", "description": "Maximum number of results to return (default: 20, max: 100)" }
            },
            "required": []
        }),
    )
}

fn get_user_info_tool() -> Tool {
    tool(
        "get_user_info",
        "Get detailed information about a Discord user in the current server. \
         Returns user ID, mention string (use this directly in your response to tag/ping the user), \
         roles, join date, and avatar. To mention the user, include the 'mention' field value \
         exactly as returned (e.g., <@123456789>) in your response text.",
        json!({
            "type": "object",
            "properties": {
                "username": { "type": "string", "description": "Username or display name to search for (fuzzy match)" },
                "user_id": { "type": "string", "description": "Discord user ID (exact match)" }
            },
            "required": []
        }),
    )
}

fn get_server_info_tool() -> Tool {
    tool(
        "get_server_info",
        "Get detailed information about the current Discord server, including member count, boost level, channels, and roles.",
        json!({ "type": "object", "properties": {}, "required": [] }),
    )
}

fn web_search_tool() -> Tool {
    tool(
        "web_search",
        "Search the web for current information, news, or facts. \
         Use when the user asks about recent events or topics that may have changed since your knowledge cutoff.",
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "The search query" }
            },
            "required": ["query"]
        }),
    )
}

fn generate_image_tool() -> Tool {
    tool(
        "generate_image",
        "Generate or edit images using AI. Creates new images from text descriptions, \
         or edits images from the conversation if the prompt requests modifications. \
         The model automatically sees recent images from the conversation.",
        json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string", "description": "Description of image to generate, or editing instructions (e.g., 'make it purple', 'add a hat')" },
                "aspect_ratio": { "type": "string", "description": "Aspect ratio (1:1, 16:9, 9:16, 2:3, 3:2, 3:4, 4:3, 4:5, 5:4, 21:9). Default: 1:1" },
                "size": { "type": "string", "description": "Image resolution (1K, 2K, 4K). Default: 1K" }
            },
            "required": ["prompt"]
        }),
    )
}

fn generate_audio_tool() -> Tool {
    tool(
        "generate_audio",
        "Generate a short piece of music from a text description - can include vocals, \
         timed lyrics, and full instrumental arrangements. Not for literal text narration; \
         use generate_speech instead when the goal is to read exact text aloud.",
        json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string", "description": "Description of the music to generate (e.g., 'an upbeat synthwave track', 'a sad piano ballad', 'a pop song with lyrics about summer')" }
            },
            "required": ["prompt"]
        }),
    )
}

fn generate_speech_tool() -> Tool {
    tool(
        "generate_speech",
        "Convert exact text into spoken audio (text-to-speech). Reads the given text aloud \
         verbatim, without interpreting or responding to it. Useful for voice responses, \
         narration, or reading text aloud.",
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "The exact text to speak aloud" },
                "voice": { "type": "string", "description": "Voice to use, e.g. Zephyr, Puck, Kore, Charon, Fenrir, Leda, Orus, Aoede. Default: Zephyr" }
            },
            "required": ["text"]
        }),
    )
}

fn generate_video_tool() -> Tool {
    tool(
        "generate_video",
        "Generate a short AI video from a text description. If a recent image is present \
         in the conversation, it is used as the starting frame to animate (image-to-video). \
         Video generation takes one to several minutes to complete.",
        json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string", "description": "Description of the video to generate, or motion/action instructions if animating a recent image" },
                "aspect_ratio": { "type": "string", "description": "Aspect ratio (16:9 or 9:16). Default: 16:9" },
                "duration": { "type": "integer", "description": "Video length in seconds (4-8). Default: model default" }
            },
            "required": ["prompt"]
        }),
    )
}

/// Returns the tool definitions for the `OpenRouter` API.
#[must_use]
pub fn get_tool_definitions() -> Vec<Tool> {
    vec![
        search_channel_history_tool(),
        get_user_info_tool(),
        get_server_info_tool(),
        web_search_tool(),
        generate_image_tool(),
        generate_audio_tool(),
        generate_speech_tool(),
        generate_video_tool(),
    ]
}
