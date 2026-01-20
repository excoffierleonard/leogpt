//! Shared utility functions for tools.

/// Minimum similarity threshold for fuzzy username matching.
pub const FUZZY_THRESHOLD: f64 = 0.85;

/// Check if username matches using case-insensitive and fuzzy matching.
///
/// First attempts a case-insensitive substring match, then falls back
/// to Jaro-Winkler similarity for fuzzy matching.
pub fn matches_username(name: &str, search: &str) -> bool {
    let name_lower = name.to_lowercase();
    let search_lower = search.to_lowercase();

    // Check for exact substring match first
    if name_lower.contains(&search_lower) {
        return true;
    }

    // Fall back to fuzzy matching
    strsim::jaro_winkler(&name_lower, &search_lower) > FUZZY_THRESHOLD
}
