//! Channel message search tool implementation.

use std::cmp::Ordering;

use log::debug;
use poise::serenity_prelude::{GetMessages, Message as DiscordMessage};
use serde::{Deserialize, Serialize};

use crate::error::{BotError, Result};

use super::executor::ToolContext;
use super::utils::matches_username;

/// Maximum messages Discord API returns per request
const MAX_MESSAGES: u8 = 100;

/// `OpenRouter` embeddings API URL
const EMBEDDINGS_URL: &str = "https://openrouter.ai/api/v1/embeddings";

/// Embedding model to use for semantic search
const EMBEDDING_MODEL: &str = "google/gemini-embedding-001";

/// Arguments for the `search_channel_history` tool
#[derive(Debug, Deserialize)]
struct SearchArgs {
    query: Option<String>,
    username: Option<String>,
    limit: Option<usize>,
}

/// A single message result returned by the search
#[derive(Debug, Serialize)]
struct MessageResult {
    author: String,
    content: String,
    timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    similarity: Option<f32>,
}

/// Request payload for the `OpenRouter` embeddings API
#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    input: Vec<String>,
}

/// Response from the `OpenRouter` embeddings API
#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

/// A single embedding result
#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
    index: usize,
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Get embeddings from `OpenRouter` API
async fn get_embeddings(texts: &[String], api_key: &str) -> Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(vec![]);
    }

    let client = reqwest::Client::new();
    let request = EmbeddingRequest {
        model: EMBEDDING_MODEL.to_string(),
        input: texts.to_vec(),
    };

    let response = client
        .post(EMBEDDINGS_URL)
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let message = response.text().await?;
        return Err(BotError::OpenRouterApi { status, message });
    }

    let api_response: EmbeddingResponse = response.json().await?;

    // Sort by index to ensure correct order
    let mut embeddings: Vec<_> = api_response.data.into_iter().collect();
    embeddings.sort_by_key(|e| e.index);

    Ok(embeddings.into_iter().map(|e| e.embedding).collect())
}

/// Check if message author matches username filter
fn author_matches(msg: &DiscordMessage, username: &str) -> bool {
    let nick = msg.member.as_ref().and_then(|m| m.nick.as_deref());
    let global_name = msg.author.global_name.as_deref();
    let name = &msg.author.name;

    nick.is_some_and(|n| matches_username(n, username))
        || global_name.is_some_and(|g| matches_username(g, username))
        || matches_username(name, username)
}

/// Search recent messages in a Discord channel
///
/// Supports semantic search using embeddings when a query is provided.
/// Falls back to returning recent messages when no query is given.
pub async fn search_channel_history(arguments: &str, tool_ctx: &ToolContext<'_>) -> Result<String> {
    let args: SearchArgs = serde_json::from_str(arguments)?;
    let result_limit = args.limit.unwrap_or(20).min(100);

    debug!(
        "Searching channel history: query={:?}, username={:?}, limit={}",
        args.query, args.username, result_limit
    );

    let messages = tool_ctx
        .channel_id
        .messages(&tool_ctx.ctx.http, GetMessages::new().limit(MAX_MESSAGES))
        .await?;

    debug!("Fetched {} messages from channel", messages.len());

    // Filter by username if provided
    let filtered_messages: Vec<_> = messages
        .into_iter()
        .filter(|msg| {
            args.username
                .as_ref()
                .is_none_or(|u| author_matches(msg, u))
        })
        .filter(|msg| !msg.content.is_empty()) // Skip empty messages
        .collect();

    debug!("{} messages after username filter", filtered_messages.len());

    // If no query provided, return recent messages
    let results = if let Some(ref query) = args.query {
        semantic_search(
            query,
            filtered_messages,
            result_limit,
            tool_ctx.openrouter_api_key,
        )
        .await?
    } else {
        // Return most recent messages without semantic ranking
        filtered_messages
            .into_iter()
            .take(result_limit)
            .map(|msg| MessageResult {
                author: msg
                    .author
                    .global_name
                    .clone()
                    .unwrap_or(msg.author.name.clone()),
                content: msg.content.clone(),
                timestamp: msg.timestamp.to_rfc3339().unwrap_or_default(),
                similarity: None,
            })
            .collect()
    };

    debug!("Returning {} messages", results.len());

    Ok(serde_json::to_string(&results)?)
}

/// Perform semantic search using embeddings
async fn semantic_search(
    query: &str,
    messages: Vec<DiscordMessage>,
    limit: usize,
    api_key: &str,
) -> Result<Vec<MessageResult>> {
    if messages.is_empty() {
        return Ok(vec![]);
    }

    // Prepare texts for embedding: query first, then all message contents
    let mut texts: Vec<String> = vec![query.to_string()];
    texts.extend(messages.iter().map(|m| m.content.clone()));

    debug!("Getting embeddings for {} texts", texts.len());

    let embeddings = get_embeddings(&texts, api_key).await?;

    if embeddings.len() != texts.len() {
        return Err(BotError::OpenRouterResponse(
            "Embedding count mismatch".to_string(),
        ));
    }

    let query_embedding = &embeddings[0];
    let message_embeddings = &embeddings[1..];

    // Compute similarities and pair with messages
    let mut scored: Vec<_> = messages
        .into_iter()
        .zip(message_embeddings.iter())
        .map(|(msg, emb)| {
            let similarity = cosine_similarity(query_embedding, emb);
            (msg, similarity)
        })
        .collect();

    // Sort by similarity (highest first)
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

    // Take top results
    Ok(scored
        .into_iter()
        .take(limit)
        .map(|(msg, similarity)| MessageResult {
            author: msg
                .author
                .global_name
                .clone()
                .unwrap_or(msg.author.name.clone()),
            content: msg.content.clone(),
            timestamp: msg.timestamp.to_rfc3339().unwrap_or_default(),
            similarity: Some(similarity),
        })
        .collect())
}
