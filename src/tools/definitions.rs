//! Tool definitions for the OpenRouter tool calling API.

use serde_json::json;

use crate::openrouter::{FunctionDefinition, Tool};

/// Returns the tool definitions for the OpenRouter API
///
/// These definitions describe the available Discord-native tools
/// that the LLM can invoke during a conversation.
pub fn get_tool_definitions() -> Vec<Tool> {
    vec![
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "search_channel_history".to_string(),
                description: "Search recent messages in the current Discord channel using \
                    semantic search. Understands meaning, not just keywords - 'food discussion' \
                    finds messages about pizza, dinner, etc. Searches up to 100 recent messages."
                    .to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "What to search for (semantic search - understands meaning)"
                        },
                        "username": {
                            "type": "string",
                            "description": "Filter messages by author name (fuzzy match)"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of results to return (default: 20, max: 100)"
                        }
                    },
                    "required": []
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "get_user_info".to_string(),
                description:
                    "Get detailed information about a Discord user in the current server, \
                    including their user ID, mention string, roles, join date, and avatar."
                        .to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "username": {
                            "type": "string",
                            "description": "Username or display name to search for (fuzzy match)"
                        },
                        "user_id": {
                            "type": "string",
                            "description": "Discord user ID (exact match)"
                        }
                    },
                    "required": []
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "get_server_info".to_string(),
                description: "Get detailed information about the current Discord server, \
                    including member count, boost level, channels, and roles."
                    .to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "web_search".to_string(),
                description: "Search the web for current information, news, or facts. \
                    Use when the user asks about recent events or topics that may have \
                    changed since your knowledge cutoff."
                    .to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The search query"
                        }
                    },
                    "required": ["query"]
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "generate_image".to_string(),
                description: "Generate or edit images using AI. Creates new images from text \
                    descriptions, or edits images from the conversation if the prompt requests \
                    modifications. The model automatically sees recent images from the conversation."
                    .to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "Description of image to generate, or editing instructions (e.g., 'make it purple', 'add a hat')"
                        },
                        "aspect_ratio": {
                            "type": "string",
                            "description": "Aspect ratio (1:1, 16:9, 9:16, 2:3, 3:2, 3:4, 4:3, 4:5, 5:4, 21:9). Default: 1:1"
                        },
                        "size": {
                            "type": "string",
                            "description": "Image resolution (1K, 2K, 4K). Default: 1K"
                        }
                    },
                    "required": ["prompt"]
                }),
            },
        },
    ]
}
