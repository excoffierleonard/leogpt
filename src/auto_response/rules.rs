//! Auto-response rule types and configuration.

use poise::serenity_prelude::UserId;

use super::matching::ContentMatchConfig;

#[derive(Debug, Clone)]
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

/// Returns the built-in auto-response rules.
pub fn hardcoded_auto_responses() -> Vec<AutoResponseRule> {
    use super::matching::MatchMode;

    let rules = vec![
        AutoResponseRuleConfig {
            name: Some("jai-vu-image".to_string()),
            user_ids: vec![398_543_560_330_444_813],
            content: ContentMatchConfig {
                patterns: vec![
                    "j ai vu".to_string(),
                    "jaivu".to_string(),
                    "jlaivu".to_string(),
                ],
                mode: MatchMode::Fuzzy,
                compact: true,
                fuzzy_threshold: 0.75,
                max_token_window: 3,
            },
            response: ResponseConfig::ImageUrl {
                url: "https://vault-public.s3.ca-east-006.backblazeb2.com/guillaume_a_vu_4k.png"
                    .to_string(),
            },
        },
        AutoResponseRuleConfig {
            name: Some("laugh-emoji-image".to_string()),
            user_ids: vec![398_620_783_498_493_964],
            content: ContentMatchConfig {
                patterns: vec!["😂".to_string()],
                mode: MatchMode::Fuzzy,
                compact: false,
                fuzzy_threshold: 0.75,
                max_token_window: 1,
            },
            response: ResponseConfig::ImageUrl {
                url: "https://vault-public.s3.ca-east-006.backblazeb2.com/amir_drole_4k.png"
                    .to_string(),
            },
        },
    ];

    rules
        .into_iter()
        .enumerate()
        .map(|(idx, cfg)| cfg.into_rule(idx))
        .collect()
}
