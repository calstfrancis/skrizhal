use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// A YAML scalar or sequence — Hayagriva-style fields like `description` or
/// `author` accept either shape (`description: foo` or `description: [a, b]`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> OneOrMany<T> {
    fn into_vec(self) -> Vec<T> {
        match self {
            OneOrMany::One(v) => vec![v],
            OneOrMany::Many(v) => v,
        }
    }
}

/// Wire format for a single entry's fields, without its key — the key comes
/// from the enclosing YAML mapping, same as a Hayagriva/BibTeX entry.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
struct RawEntry {
    #[serde(rename = "type")]
    entry_type: String,
    title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    organization: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    date: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<OneOrMany<String>>,
    /// Type-specific fields (degree, doi, amount, ...) not modeled explicitly —
    /// new entry types only need a registry entry, not a Rust struct change.
    #[serde(flatten)]
    extra: BTreeMap<String, serde_yaml_ng::Value>,
}

/// A single CV element: a job, a degree, an award, a publication, etc.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CvEntry {
    pub key: String,
    pub entry_type: String,
    pub title: String,
    pub organization: Option<String>,
    pub location: Option<String>,
    pub date: Option<String>,
    pub tags: Vec<String>,
    pub description: Vec<String>,
    pub extra: BTreeMap<String, serde_yaml_ng::Value>,
}

impl CvEntry {
    fn from_raw(key: String, raw: RawEntry) -> Self {
        CvEntry {
            key,
            entry_type: raw.entry_type,
            title: raw.title,
            organization: raw.organization,
            location: raw.location,
            date: raw.date,
            tags: raw.tags,
            description: raw.description.map(OneOrMany::into_vec).unwrap_or_default(),
            extra: raw.extra,
        }
    }

    fn into_raw(self) -> RawEntry {
        let description = match self.description.len() {
            0 => None,
            1 => Some(OneOrMany::One(self.description.into_iter().next().unwrap())),
            _ => Some(OneOrMany::Many(self.description)),
        };
        RawEntry {
            entry_type: self.entry_type,
            title: self.title,
            organization: self.organization,
            location: self.location,
            date: self.date,
            tags: self.tags,
            description,
            extra: self.extra,
        }
    }
}

impl CvEntry {
    /// Clones this entry under a new key — used by the "Duplicate" action.
    pub fn duplicate_with_key(&self, new_key: String) -> CvEntry {
        let mut copy = self.clone();
        copy.key = new_key;
        copy
    }
}

/// Lowercases and hyphenates `s` for use as a citation-style key
/// (`"Student Minister"` -> `"student-minister"`).
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Returns `base` if it isn't already used as a key in `existing`, otherwise
/// `base-2`, `base-3`, ... until an unused key is found.
pub fn unique_key(base: &str, existing: &[CvEntry]) -> String {
    let base = if base.is_empty() { "entry" } else { base };
    if !existing.iter().any(|e| e.key == base) {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !existing.iter().any(|e| e.key == candidate) {
            return candidate;
        }
        n += 1;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse YAML: {0}")]
    Parse(#[from] serde_yaml_ng::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("failed to write {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to serialize YAML: {0}")]
    Serialize(#[from] serde_yaml_ng::Error),
}

/// Parses a `cv-elements.yaml`-shaped string into entries. Entry order follows
/// the YAML mapping's key order as returned by the parser (not necessarily
/// source order — use `sort_entries_by_date_desc` for chronological order).
pub fn parse_str(yaml: &str) -> Result<Vec<CvEntry>, LoadError> {
    let raw: BTreeMap<String, RawEntry> = serde_yaml_ng::from_str(yaml)?;
    Ok(raw.into_iter().map(|(key, r)| CvEntry::from_raw(key, r)).collect())
}

pub fn load_file(path: &Path) -> Result<Vec<CvEntry>, LoadError> {
    let content = std::fs::read_to_string(path).map_err(|source| LoadError::Io {
        path: path.display().to_string(),
        source,
    })?;
    parse_str(&content)
}

/// Serializes entries back to `cv-elements.yaml` shape, sorted by key for a
/// stable, git-diff-friendly ordering.
pub fn to_yaml_string(entries: &[CvEntry]) -> Result<String, SaveError> {
    let raw: BTreeMap<String, RawEntry> = entries
        .iter()
        .cloned()
        .map(|e| (e.key.clone(), e.into_raw()))
        .collect();
    Ok(serde_yaml_ng::to_string(&raw)?)
}

pub fn save_file(path: &Path, entries: &[CvEntry]) -> Result<(), SaveError> {
    let content = to_yaml_string(entries)?;
    std::fs::write(path, content).map_err(|source| SaveError::Io {
        path: path.display().to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
hope-united-2025:
  type: ministry-position
  title: Student Minister
  organization: Hope United Church
  location: Halifax, NS
  date: 2025-09/2026-04
  tags: [ministry, current]
  description:
    - Preaching and worship leadership on a rotating basis
    - Liturgical preparation for seasonal services

mdiv-2024:
  type: education
  title: Master of Divinity (in progress)
  organization: Atlantic School of Theology
  date: 2023/
  tags: [academic, ministry]
  degree: MDiv
  field-of-study: Divinity
"#;

    #[test]
    fn parse_entry_count() {
        let entries = parse_str(SAMPLE).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn parse_common_fields() {
        let entries = parse_str(SAMPLE).unwrap();
        let e = entries.iter().find(|e| e.key == "hope-united-2025").unwrap();
        assert_eq!(e.entry_type, "ministry-position");
        assert_eq!(e.title, "Student Minister");
        assert_eq!(e.organization.as_deref(), Some("Hope United Church"));
        assert_eq!(e.location.as_deref(), Some("Halifax, NS"));
        assert_eq!(e.date.as_deref(), Some("2025-09/2026-04"));
        assert_eq!(e.tags, vec!["ministry", "current"]);
        assert_eq!(e.description.len(), 2);
    }

    #[test]
    fn parse_extra_type_specific_fields() {
        let entries = parse_str(SAMPLE).unwrap();
        let e = entries.iter().find(|e| e.key == "mdiv-2024").unwrap();
        assert_eq!(
            e.extra.get("degree").and_then(|v| v.as_str()),
            Some("MDiv")
        );
        assert_eq!(
            e.extra.get("field-of-study").and_then(|v| v.as_str()),
            Some("Divinity")
        );
    }

    #[test]
    fn description_single_string_becomes_one_element_vec() {
        let yaml = r#"
solo:
  type: award
  title: Some Award
  description: A single-line description
"#;
        let entries = parse_str(yaml).unwrap();
        assert_eq!(entries[0].description, vec!["A single-line description"]);
    }

    #[test]
    fn round_trip_preserves_fields() {
        let entries = parse_str(SAMPLE).unwrap();
        let yaml = to_yaml_string(&entries).unwrap();
        let reparsed = parse_str(&yaml).unwrap();
        assert_eq!(entries.len(), reparsed.len());
        let orig = entries.iter().find(|e| e.key == "hope-united-2025").unwrap();
        let again = reparsed.iter().find(|e| e.key == "hope-united-2025").unwrap();
        assert_eq!(orig, again);
    }

    #[test]
    fn load_file_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cv-elements.yaml");
        std::fs::write(&path, SAMPLE).unwrap();
        let entries = load_file(&path).unwrap();
        assert_eq!(entries.len(), 2);

        let out_path = dir.path().join("out.yaml");
        save_file(&out_path, &entries).unwrap();
        let reloaded = load_file(&out_path).unwrap();
        assert_eq!(reloaded.len(), 2);
    }

    #[test]
    fn load_file_missing_returns_io_error() {
        let err = load_file(Path::new("/nonexistent/cv-elements.yaml"));
        assert!(matches!(err, Err(LoadError::Io { .. })));
    }

    #[test]
    fn parse_invalid_yaml_returns_parse_error() {
        let err = parse_str("not: valid: yaml: at all: [");
        assert!(matches!(err, Err(LoadError::Parse(_))));
    }

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("Student Minister"), "student-minister");
        assert_eq!(slugify("  Master of Divinity!! "), "master-of-divinity");
    }

    #[test]
    fn duplicate_with_key_copies_fields_under_new_key() {
        let entries = parse_str(SAMPLE).unwrap();
        let orig = entries.iter().find(|e| e.key == "mdiv-2024").unwrap();
        let dup = orig.duplicate_with_key("mdiv-2024-copy".into());
        assert_eq!(dup.key, "mdiv-2024-copy");
        assert_eq!(dup.title, orig.title);
        assert_eq!(dup.entry_type, orig.entry_type);
    }

    #[test]
    fn unique_key_returns_base_when_unused() {
        let entries = parse_str(SAMPLE).unwrap();
        assert_eq!(unique_key("brand-new", &entries), "brand-new");
    }

    #[test]
    fn unique_key_appends_suffix_when_taken() {
        let entries = parse_str(SAMPLE).unwrap();
        assert_eq!(unique_key("mdiv-2024", &entries), "mdiv-2024-2");
    }
}
