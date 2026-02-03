use super::{
    AutoResponseRule, AutoResponseRuleConfig, ContentMatchConfig, MatchMode, ResponseConfig,
};

/// Returns the built-in auto-response rules.
pub fn hardcoded_auto_responses() -> Vec<AutoResponseRule> {
    let rules = vec![AutoResponseRuleConfig {
        name: Some("jai-vu-image".to_string()),
        user_ids: vec![398543560330444813],
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
    }];

    rules
        .into_iter()
        .enumerate()
        .map(|(idx, cfg)| cfg.into_rule(idx))
        .collect()
}
