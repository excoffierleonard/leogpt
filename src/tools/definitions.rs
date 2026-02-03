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
        "Generate spoken audio from text using text-to-speech. Converts written text into natural-sounding speech. \
         Useful for voice responses, narration, or reading text aloud.",
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "The text to convert to speech (max 4096 characters)" },
                "voice": { "type": "string", "description": "Voice to use: alloy (neutral), echo (male), fable (British), onyx (deep male), nova (female), shimmer (soft female). Default: alloy" }
            },
            "required": ["text"]
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
    ]
}
