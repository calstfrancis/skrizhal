use std::collections::BTreeMap;

use crate::entry::CvEntry;
use crate::registry;

/// A file-level problem — the kind per-entry validation structurally cannot
/// see, because it only becomes visible by comparing entries against each
/// other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// Two entries that look like the same thing recorded twice.
    NearDuplicate { a: String, b: String },
    /// A tag used exactly once that closely resembles a much more common
    /// one. Almost always a typo — and a silent one, since a mistyped tag
    /// just makes the entry invisible to any profile filtering on the real
    /// tag, with no error raised anywhere in Skrizhal or Zerkalo.
    SuspiciousTag {
        tag: String,
        key: String,
        similar_to: String,
        similar_count: usize,
    },
    /// An entry with no tags at all, invisible to every tag-filtered section.
    Untagged { key: String },
    /// A category outside the registry — fine on purpose (the schema is
    /// open), but worth surfacing in bulk in case it's a typo.
    UnknownCategory { key: String, category: String },
}

impl Finding {
    pub fn message(&self) -> String {
        match self {
            Finding::NearDuplicate { a, b } => {
                format!("\"{a}\" and \"{b}\" look like the same entry recorded twice")
            }
            Finding::SuspiciousTag {
                tag,
                key,
                similar_to,
                similar_count,
            } => format!(
                "Tag \"{tag}\" is used once (on \"{key}\") and closely resembles \
                 \"{similar_to}\", used {similar_count} times — likely a typo"
            ),
            Finding::Untagged { key } => {
                format!("\"{key}\" has no tags, so no tag-filtered section will include it")
            }
            Finding::UnknownCategory { key, category } => {
                format!("\"{key}\" uses an unrecognized category \"{category}\"")
            }
        }
    }

    /// The entry keys a finding refers to, so the UI can offer to jump to them.
    pub fn keys(&self) -> Vec<&str> {
        match self {
            Finding::NearDuplicate { a, b } => vec![a.as_str(), b.as_str()],
            Finding::SuspiciousTag { key, .. } => vec![key.as_str()],
            Finding::Untagged { key } => vec![key.as_str()],
            Finding::UnknownCategory { key, .. } => vec![key.as_str()],
        }
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Identity string for duplicate detection: title plus organization, since a
/// title alone ("Volunteer") repeats legitimately across different orgs.
fn identity(entry: &CvEntry) -> String {
    format!(
        "{} {}",
        normalize(&entry.title),
        normalize(entry.organization.as_deref().unwrap_or(""))
    )
    .trim()
    .to_string()
}

/// Distance below which two identities count as the same entry. Scaled to
/// length so a short title isn't matched to an unrelated short title, and a
/// long one tolerates a word of difference.
fn duplicate_threshold(len: usize) -> usize {
    match len {
        0..=10 => 1,
        11..=25 => 2,
        _ => 3,
    }
}

pub fn analyze(entries: &[CvEntry]) -> Vec<Finding> {
    let mut findings = Vec::new();

    for (i, a) in entries.iter().enumerate() {
        let ia = identity(a);
        if ia.is_empty() {
            continue;
        }
        for b in entries.iter().skip(i + 1) {
            let ib = identity(b);
            if ib.is_empty() {
                continue;
            }
            let threshold = duplicate_threshold(ia.len().max(ib.len()));
            if levenshtein(&ia, &ib) <= threshold {
                findings.push(Finding::NearDuplicate {
                    a: a.key.clone(),
                    b: b.key.clone(),
                });
            }
        }
    }

    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for e in entries {
        for t in &e.tags {
            *counts.entry(t.as_str()).or_default() += 1;
        }
    }
    for (tag, count) in &counts {
        if *count != 1 {
            continue;
        }
        // Only flag against a clearly more established tag — two tags each
        // used once are just two tags, not a typo and its correction.
        let best = counts
            .iter()
            .filter(|(other, other_count)| {
                **other_count > 1 && other != &tag && levenshtein(tag, other) <= 2
            })
            .max_by_key(|(_, c)| **c);
        if let Some((similar_to, similar_count)) = best {
            if let Some(owner) = entries.iter().find(|e| e.tags.iter().any(|t| t == tag)) {
                findings.push(Finding::SuspiciousTag {
                    tag: (*tag).to_string(),
                    key: owner.key.clone(),
                    similar_to: (*similar_to).to_string(),
                    similar_count: *similar_count,
                });
            }
        }
    }

    for e in entries {
        if e.tags.is_empty() {
            findings.push(Finding::Untagged { key: e.key.clone() });
        }
        if !e.category.trim().is_empty() && registry::lookup(&e.category).is_none() {
            findings.push(Finding::UnknownCategory {
                key: e.key.clone(),
                category: e.category.clone(),
            });
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, title: &str, org: Option<&str>, tags: &[&str]) -> CvEntry {
        CvEntry {
            key: key.to_string(),
            category: "Employment".to_string(),
            title: title.to_string(),
            organization: org.map(|o| o.to_string()),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn near_duplicates_are_detected_across_minor_differences() {
        let entries = vec![
            entry("a", "Student Minister", Some("Hope United Church"), &["x"]),
            entry("b", "Student minister", Some("Hope United Church."), &["x"]),
        ];
        let dups = analyze(&entries)
            .into_iter()
            .filter(|f| matches!(f, Finding::NearDuplicate { .. }))
            .count();
        assert_eq!(dups, 1);
    }

    #[test]
    fn same_title_at_different_organizations_is_not_a_duplicate() {
        let entries = vec![
            entry("a", "Volunteer", Some("Example Organization"), &["x"]),
            entry("b", "Volunteer", Some("Totally Different Charity"), &["x"]),
        ];
        assert!(!analyze(&entries)
            .iter()
            .any(|f| matches!(f, Finding::NearDuplicate { .. })));
    }

    #[test]
    fn a_tag_used_once_that_resembles_a_common_one_is_flagged() {
        let entries = vec![
            entry("a", "One", Some("Org A"), &["ministry"]),
            entry("b", "Two", Some("Org B"), &["ministry"]),
            entry("c", "Three", Some("Org C"), &["minstry"]),
        ];
        let found = analyze(&entries);
        let flagged = found.iter().find_map(|f| match f {
            Finding::SuspiciousTag { tag, similar_to, .. } => Some((tag.clone(), similar_to.clone())),
            _ => None,
        });
        assert_eq!(flagged, Some(("minstry".to_string(), "ministry".to_string())));
    }

    #[test]
    fn two_unrelated_one_off_tags_are_not_flagged_against_each_other() {
        let entries = vec![
            entry("a", "One", Some("Org A"), &["ministry"]),
            entry("b", "Two", Some("Org B"), &["teaching"]),
        ];
        assert!(!analyze(&entries)
            .iter()
            .any(|f| matches!(f, Finding::SuspiciousTag { .. })));
    }

    #[test]
    fn untagged_entries_are_reported() {
        let entries = vec![entry("a", "One", Some("Org A"), &[])];
        assert!(analyze(&entries)
            .iter()
            .any(|f| matches!(f, Finding::Untagged { key } if key == "a")));
    }

    #[test]
    fn unknown_categories_are_reported() {
        let mut e = entry("a", "One", Some("Org A"), &["x"]);
        e.category = "Totally Made Up".to_string();
        let found = analyze(&[e]);
        assert!(found
            .iter()
            .any(|f| matches!(f, Finding::UnknownCategory { category, .. } if category == "Totally Made Up")));
    }

    #[test]
    fn a_clean_file_produces_no_findings() {
        let entries = vec![
            entry("a", "Lecturer", Some("Example University"), &["teaching"]),
            entry("b", "Chaplain", Some("Example Hospital"), &["teaching"]),
        ];
        assert!(analyze(&entries).is_empty());
    }

    #[test]
    fn levenshtein_basics() {
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", "abc"), 0);
        assert_eq!(levenshtein("ministry", "minstry"), 1);
    }
}
