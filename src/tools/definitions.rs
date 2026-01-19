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
    ]
}
