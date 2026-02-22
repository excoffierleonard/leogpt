//! Shared fuzzy matching utilities for S3-backed content.

use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use log::debug;

use crate::s3_index::S3Entry;

/// Find an exact (case-insensitive) match by entry name.
#[must_use]
pub fn find_exact<'a>(entries: &'a [S3Entry], query: &str) -> Option<&'a S3Entry> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }

    entries
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(query))
}

/// Find the best fuzzy match for a query.
#[must_use]
pub fn find_best_fuzzy<'a>(entries: &'a [S3Entry], query: &str) -> Option<&'a S3Entry> {
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

/// Find the best matching entries for a query, ordered by score (best first).
#[must_use]
pub fn search_fuzzy<'a>(entries: &'a [S3Entry], query: &str, limit: usize) -> Vec<&'a S3Entry> {
    let query = query.trim();
    if query.is_empty() || limit == 0 {
        return Vec::new();
    }

    let matcher = SkimMatcherV2::default();
    let mut scored_matches: Vec<(&S3Entry, i64)> = entries
        .iter()
        .filter_map(|entry| {
            matcher
                .fuzzy_match(entry.name.as_str(), query)
                .map(|score| (entry, score))
        })
        .collect();

    scored_matches.sort_by(|(left_entry, left_score), (right_entry, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| left_entry.name.cmp(&right_entry.name))
    });

    scored_matches
        .into_iter()
        .take(limit)
        .map(|(entry, _)| entry)
        .collect()
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
    fn exact_match_is_case_insensitive() {
        let entries = entries();
        let found = find_exact(&entries, "ALPHA.MP3").expect("expected match");
        assert_eq!(found.name, "alpha.mp3");
    }

    #[test]
    fn empty_query_returns_none() {
        let entries = entries();
        assert!(find_best_fuzzy(&entries, "  ").is_none());
    }

    #[test]
    fn finds_best_match() -> Result<(), &'static str> {
        let entries = entries();
        let found = find_best_fuzzy(&entries, "alp").ok_or("expected match")?;
        assert_eq!(found.name, "alpha.mp3");
        Ok(())
    }

    #[test]
    fn search_returns_ranked_results() -> Result<(), &'static str> {
        let entries = entries();
        let results = search_fuzzy(&entries, "a", 10);
        assert!(!results.is_empty());

        let matcher = SkimMatcherV2::default();
        let mut last_score = None;
        for entry in results {
            let score = matcher
                .fuzzy_match(entry.name.as_str(), "a")
                .ok_or("expected score")?;
            if let Some(previous) = last_score {
                assert!(score <= previous);
            }
            last_score = Some(score);
        }
        Ok(())
    }

    #[test]
    fn search_limits_results() {
        let entries = entries();
        let results = search_fuzzy(&entries, "a", 2);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_empty_query_returns_empty() {
        let entries = entries();
        assert!(search_fuzzy(&entries, "   ", 10).is_empty());
    }
}
