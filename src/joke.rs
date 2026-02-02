//! One-off joke responder utilities.

use poise::serenity_prelude::UserId;

/// Hard toggle for the temporary joke feature.
pub const JOKE_ENABLED: bool = true;

/// Only respond for this specific user (set via config/env).
pub const JOKE_USER_ID: Option<UserId> = Some(UserId::new(398543560330444813));

/// Image link used as the joke response.
pub const JOKE_IMAGE_URL: &str =
    "https://vault-public.s3.ca-east-006.backblazeb2.com/guillaume_a_vu_4k.png";

const FUZZY_THRESHOLD: f64 = 0.75;
const TARGET_COMPACT: [&str; 2] = ["jaivu", "jlaivu"];

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

fn contains_target_compact(haystack: &str) -> bool {
    TARGET_COMPACT
        .iter()
        .any(|target| haystack.contains(target))
}

fn levenshtein(a: &str, b: &str) -> usize {
    if a == b {
        return 0;
    }
    if a.is_empty() {
        return b.chars().count();
    }
    if b.is_empty() {
        return a.chars().count();
    }

    let b_len = b.chars().count();
    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0; b_len + 1];

    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        prev.copy_from_slice(&curr);
    }

    prev[b_len]
}

fn similarity(a: &str, b: &str) -> f64 {
    let b_len = b.chars().count();
    let max_len = b_len.max(a.chars().count());
    if max_len == 0 {
        return 1.0;
    }
    let dist = levenshtein(a, b) as f64;
    1.0 - (dist / max_len as f64)
}

/// Returns true if the content looks like "J'ai vu" or a close variant.
pub fn matches_jai_vu(content: &str) -> bool {
    let normalized = normalize(content);
    if normalized.is_empty() {
        log::info!("Joke match: empty content after normalize");
        return false;
    }

    let compacted = compact(&normalized);
    if contains_target_compact(&compacted) {
        log::info!(
            "Joke match: compact substring hit (compact='{}')",
            compacted
        );
        return true;
    }

    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    for i in 0..tokens.len() {
        let mut window = String::new();
        for token in tokens.iter().skip(i).take(3) {
            window.push_str(token);
            let window_compact = compact(&window);
            if contains_target_compact(&window_compact) {
                log::info!("Joke match: token window hit (window='{}')", window_compact);
                return true;
            }
        }
    }

    let sim = similarity(&normalized, "j ai vu");
    log::info!(
        "Joke match: similarity={:.3} normalized='{}'",
        sim,
        normalized
    );
    sim >= FUZZY_THRESHOLD
}

/// Gate the joke response behind a toggle and user ID.
pub fn should_trigger_joke(user_id: UserId, content: &str) -> bool {
    if !JOKE_ENABLED {
        log::info!("Joke check: disabled");
        return false;
    }
    if let Some(allowed) = JOKE_USER_ID {
        if user_id != allowed {
            log::info!(
                "Joke check: user mismatch (got={}, expected={})",
                user_id,
                allowed
            );
            return false;
        }
    } else {
        log::info!("Joke check: missing user id");
        return false;
    }

    matches_jai_vu(content)
}
