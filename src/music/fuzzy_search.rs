//! Fuzzy song matching using `SkimMatcherV2`.

use std::fs;
use std::path::{Path, PathBuf};

use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use log::debug;

use crate::error::Result;

/// Find the best matching song file for a query.
///
/// Returns the full path to the matched file, or `None` if no match found.
///
/// # Errors
///
/// Returns an error if the music directory cannot be read.
pub fn find_song(music_dir: &Path, query: &str) -> Result<Option<PathBuf>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(None);
    }

    let matcher = SkimMatcherV2::default();
    let mut best: Option<(PathBuf, i64)> = None;

    for entry in fs::read_dir(music_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        // Skip hidden files
        if name.starts_with('.') {
            continue;
        }

        if let Some(score) = matcher.fuzzy_match(name, query) {
            let is_better = best
                .as_ref()
                .is_none_or(|(_, best_score)| score > *best_score);

            if is_better {
                debug!("New best match: {name} (score: {score})");
                best = Some((path, score));
            }
        }
    }

    Ok(best.map(|(path, _)| path))
}

/// List available songs in the music directory.
///
/// Returns a list of filenames (without full paths).
///
/// # Errors
///
/// Returns an error if the music directory cannot be read.
pub fn list_songs(music_dir: &Path, limit: usize) -> Result<Vec<String>> {
    let mut songs = Vec::new();

    for entry in fs::read_dir(music_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && !name.starts_with('.')
        {
            songs.push(name.to_string());
        }

        if songs.len() >= limit {
            break;
        }
    }

    songs.sort();
    Ok(songs)
}
