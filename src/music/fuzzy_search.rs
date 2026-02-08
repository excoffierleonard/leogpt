//! Fuzzy song matching using `SkimMatcherV2`.

use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use log::debug;

use super::s3_store::S3Entry;

/// Find the best matching song file for a query.
///
/// Returns the matched entry, or `None` if no match found.
pub fn find_song<'a>(entries: &'a [S3Entry], query: &str) -> Option<&'a S3Entry> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }

    let matcher = SkimMatcherV2::default();
    let mut best: Option<(&S3Entry, i64)> = None;

    for entry in entries {
        let name = entry.name.as_str();
        if let Some(score) = matcher.fuzzy_match(name, query) {
            let is_better = best
                .as_ref()
                .is_none_or(|(_, best_score)| score > *best_score);

            if is_better {
                debug!("New best match: {name} (score: {score})");
                best = Some((entry, score));
            }
        }
    }

    best.map(|(entry, _)| entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<S3Entry> {
        vec![
            S3Entry {
                key: "music/alpha.mp3".to_string(),
                name: "alpha.mp3".to_string(),
            },
            S3Entry {
                key: "music/beta.wav".to_string(),
                name: "beta.wav".to_string(),
            },
            S3Entry {
                key: "music/gamma.flac".to_string(),
                name: "gamma.flac".to_string(),
            },
        ]
    }

    #[test]
    fn finds_best_match() {
        let entries = entries();
        let found = find_song(&entries, "alp").expect("expected match");
        assert_eq!(found.name, "alpha.mp3");
    }

    #[test]
    fn empty_query_returns_none() {
        let entries = entries();
        assert!(find_song(&entries, "  ").is_none());
    }

    #[test]
    fn lists_limited_songs() {
        let entries = entries();
        let listed = list_songs(&entries, 2);
        assert_eq!(listed, vec!["alpha.mp3", "beta.wav"]);
    }
}
