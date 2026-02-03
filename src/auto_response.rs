//! Auto-response rules and matching utilities.

mod rules;

use log::debug;
use poise::serenity_prelude::UserId;
use strsim::normalized_levenshtein;

pub use rules::hardcoded_auto_responses;

#[derive(Debug, Clone)]
/// Matching strategy for auto-response patterns.
pub enum MatchMode {
    Fuzzy,
}

#[derive(Debug, Clone)]
/// Content matching configuration for a rule.
pub struct ContentMatchConfig {
    pub patterns: Vec<String>,
    pub mode: MatchMode,
    pub compact: bool,
    pub fuzzy_threshold: f64,
    pub max_token_window: usize,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
/// Response configuration before resolving into sendable payloads.
pub enum ResponseConfig {
    ImageUrl { url: String },
}

#[derive(Debug, Clone)]
/// Raw rule configuration before resolving user IDs.
pub struct AutoResponseRuleConfig {
    pub name: Option<String>,
    pub user_ids: Vec<u64>,
    pub content: ContentMatchConfig,
    pub response: ResponseConfig,
}

#[derive(Debug, Clone)]
/// Response payload ready to be sent.
pub enum AutoResponsePayload {
    ImageUrl(String),
}

#[derive(Debug, Clone)]
/// Fully-resolved auto-response rule.
pub struct AutoResponseRule {
    pub name: String,
    pub user_ids: Vec<UserId>,
    pub content: ContentMatchConfig,
    pub response: AutoResponsePayload,
}

#[derive(Debug, Clone)]
/// Matched auto-response action.
pub struct AutoResponseAction {
    pub rule_name: String,
    pub payload: AutoResponsePayload,
}

/// Returns the first matching auto-response action, if any.
pub fn select_auto_response(
    rules: &[AutoResponseRule],
    user_id: UserId,
    content: &str,
) -> Option<AutoResponseAction> {
    for rule in rules {
        if !rule.user_ids.is_empty() && !rule.user_ids.contains(&user_id) {
            continue;
        }
        if rule.content.matches(content) {
            return Some(AutoResponseAction {
                rule_name: rule.name.clone(),
                payload: rule.response.clone(),
            });
        }
    }
    None
}

impl AutoResponseRuleConfig {
    /// Convert a config entry into a resolved rule.
    pub fn into_rule(self, index: usize) -> AutoResponseRule {
        let name = self.name.unwrap_or_else(|| format!("rule-{}", index + 1));

        let response = match self.response {
            ResponseConfig::ImageUrl { url } => AutoResponsePayload::ImageUrl(url),
        };

        let user_ids = self
            .user_ids
            .into_iter()
            .map(UserId::new)
            .collect::<Vec<_>>();

        AutoResponseRule {
            name,
            user_ids,
            content: self.content,
            response,
        }
    }
}

impl ContentMatchConfig {
    /// Returns true when content matches this config.
    pub fn matches(&self, content: &str) -> bool {
        let normalized = normalize(content);
        if normalized.is_empty() {
            debug!("Auto response match: empty content after normalize");
            return false;
        }

        let compacted = if self.compact {
            Some(compact(&normalized))
        } else {
            None
        };

        let tokens: Vec<&str> = normalized.split_whitespace().collect();

        for pattern in &self.patterns {
            let pattern_norm = normalize(pattern);
            if pattern_norm.is_empty() {
                continue;
            }

            let pattern_compact = if self.compact {
                Some(compact(&pattern_norm))
            } else {
                None
            };

            match self.mode {
                MatchMode::Fuzzy => {
                    if fuzzy_match(
                        &tokens,
                        &pattern_norm,
                        self.fuzzy_threshold,
                        self.max_token_window,
                    ) {
                        return true;
                    }
                    if let (Some(compacted), Some(pattern_compact)) =
                        (compacted.as_ref(), pattern_compact.as_ref())
                        && compacted.contains(pattern_compact)
                    {
                        return true;
                    }
                }
            }
        }

        false
    }
}

fn normalize(input: &str) -> String {
    input
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn compact(input: &str) -> String {
    input.chars().filter(|c| !c.is_whitespace()).collect()
}

fn fuzzy_match(tokens: &[&str], pattern: &str, threshold: f64, max_window: usize) -> bool {
    if tokens.is_empty() {
        return false;
    }

    let pattern_tokens: Vec<&str> = pattern.split_whitespace().collect();
    let max_window = max_window.max(pattern_tokens.len()).max(1);

    for start in 0..tokens.len() {
        for window in 1..=max_window {
            if start + window > tokens.len() {
                break;
            }
            let candidate = tokens[start..start + window].join(" ");
            let sim = normalized_levenshtein(&candidate, pattern);
            if sim >= threshold {
                debug!(
                    "Auto response match: fuzzy hit (candidate='{}', pattern='{}', sim={:.3})",
                    candidate, pattern, sim
                );
                return true;
            }
        }
    }

    false
}
