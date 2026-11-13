//! `skrizhal --search-json QUERY…`: search the CV entries and print the hits as one line of
//! JSON, for the Porog launcher (`flatpak run --command=skrizhal-search …`). No window, no
//! keyring, no network and no writes: it reads the configured file and prints.
//!
//! Failures are printed as `{"schema":1,"error":"…"}` with exit code 0, because "no CV file
//! yet" is an answer, not a crash.

use skrizhal_core::{filter_entries, load_file, CvEntry, FilterOptions};

use crate::config::Config;

/// Version of the JSON this prints. Bumped only for a change an older Porog cannot ignore.
const SCHEMA: u32 = 1;
/// Most hits one answer carries; Porog shows a handful.
const LIMIT: usize = 8;

pub fn run(query: &[String]) {
    println!("{}", answer(query));
}

fn answer(query: &[String]) -> String {
    match hits(query) {
        Ok(hits) => serde_json::json!({ "schema": SCHEMA, "hits": hits }).to_string(),
        Err(message) => serde_json::json!({ "schema": SCHEMA, "error": message }).to_string(),
    }
}

fn hits(query: &[String]) -> Result<Vec<serde_json::Value>, String> {
    let path = Config::load().data_path;
    if !path.exists() {
        return Err("Skrizhal has no CV file yet.".to_string());
    }
    let outcome = load_file(&path).map_err(|_| "Skrizhal could not read its CV file.".to_string())?;
    Ok(matching(outcome.entries, query)
        .into_iter()
        .take(LIMIT)
        .map(|e| {
            let detail = [Some(e.category.as_str()), e.organization.as_deref()]
                .into_iter()
                .flatten()
                .filter(|part| !part.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            serde_json::json!({ "key": e.key, "title": e.title, "detail": detail })
        })
        .collect())
}

/// The entries every word of `query` matches (title, organization or description).
fn matching(entries: Vec<CvEntry>, query: &[String]) -> Vec<CvEntry> {
    query.iter().fold(entries, |pool, word| {
        let opts = FilterOptions {
            query: Some(word.as_str()),
            ..FilterOptions::default()
        };
        filter_entries(&pool, &opts).into_iter().cloned().collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, title: &str, org: &str) -> CvEntry {
        CvEntry {
            key: key.into(),
            category: "Education".into(),
            title: title.into(),
            organization: Some(org.into()),
            ..CvEntry::default()
        }
    }

    #[test]
    fn every_word_has_to_match() {
        let entries = vec![
            entry("a", "Master of Divinity", "Example University"),
            entry("b", "Bachelor of Arts", "Example University"),
        ];
        let words = |s: &str| s.split_whitespace().map(String::from).collect::<Vec<_>>();
        assert_eq!(matching(entries.clone(), &words("example")).len(), 2);
        let hit = matching(entries.clone(), &words("divinity example"));
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].key, "a");
        assert!(matching(entries, &words("divinity nothing")).is_empty());
    }
}
