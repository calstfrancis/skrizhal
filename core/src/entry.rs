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
    #[serde(alias = "type")]
    category: String,
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
    /// Category-specific fields (degree, doi, amount, ...) not modeled explicitly —
    /// new categories only need a registry entry, not a Rust struct change.
    #[serde(flatten)]
    extra: BTreeMap<String, serde_yaml_ng::Value>,
}

/// A single CV element: a job, a degree, an award, a publication, etc.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CvEntry {
    pub key: String,
    pub category: String,
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
            category: raw.category,
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
            category: self.category,
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

/// Result of parsing a `cv-elements.yaml`-shaped string. Each top-level key
/// is parsed independently, so one entry with a schema problem (missing
/// `category`/`title`, wrong field type, etc.) doesn't block every other
/// entry in the file from loading.
#[derive(Debug, Default)]
pub struct ParseOutcome {
    /// Entries that parsed successfully, in the YAML mapping's key order
    /// (not necessarily source order — use `sort_entries_by_date_desc` for
    /// chronological order).
    pub entries: Vec<CvEntry>,
    /// `(key, error message)` for entries that failed to parse.
    pub failed: Vec<(String, String)>,
    /// Raw YAML for the failed entries, keyed the same as `failed` — carried
    /// through unchanged by `to_yaml_string`/`save_file` so a save never
    /// silently deletes an entry Skrizhal couldn't understand.
    pub raw_failed: BTreeMap<String, serde_yaml_ng::Value>,
}

/// Parses a `cv-elements.yaml`-shaped string. Only fails outright if the text
/// isn't valid YAML, or isn't a mapping at the top level — a well-formed
/// mapping with individually-malformed entries instead reports those via
/// `ParseOutcome::failed` and continues loading everything else.
pub fn parse_str(yaml: &str) -> Result<ParseOutcome, LoadError> {
    let raw: BTreeMap<String, serde_yaml_ng::Value> = serde_yaml_ng::from_str(yaml)?;
    let mut outcome = ParseOutcome::default();
    for (key, value) in raw {
        // Re-serialize each entry's Value back to YAML text and re-parse it
        // rather than using `from_value` directly: `from_value` is strict
        // about scalar types (an unquoted `date: 2020` is a YAML integer,
        // and `from_value` rejects that for an `Option<String>` field),
        // while `from_str` on the original text coerces it to a string like
        // everyone expects. Round-tripping through text keeps that lenience.
        let per_entry_yaml = match serde_yaml_ng::to_string(&value) {
            Ok(s) => s,
            Err(err) => {
                outcome.failed.push((key.clone(), err.to_string()));
                outcome.raw_failed.insert(key, value);
                continue;
            }
        };
        match serde_yaml_ng::from_str::<RawEntry>(&per_entry_yaml) {
            Ok(r) => outcome.entries.push(CvEntry::from_raw(key, r)),
            Err(err) => {
                outcome.failed.push((key.clone(), err.to_string()));
                outcome.raw_failed.insert(key, value);
            }
        }
    }
    Ok(outcome)
}

pub fn load_file(path: &Path) -> Result<ParseOutcome, LoadError> {
    let content = std::fs::read_to_string(path).map_err(|source| LoadError::Io {
        path: path.display().to_string(),
        source,
    })?;
    parse_str(&content)
}

/// Serializes entries back to `cv-elements.yaml` shape, sorted by key for a
/// stable, git-diff-friendly ordering. `raw_failed` (from a prior
/// `ParseOutcome`, or empty for a brand new file) is merged in unchanged so
/// entries Skrizhal couldn't parse are preserved on save rather than dropped.
pub fn to_yaml_string(
    entries: &[CvEntry],
    raw_failed: &BTreeMap<String, serde_yaml_ng::Value>,
) -> Result<String, SaveError> {
    let mut raw: BTreeMap<String, serde_yaml_ng::Value> = raw_failed.clone();
    for e in entries.iter().cloned() {
        raw.insert(e.key.clone(), serde_yaml_ng::to_value(e.into_raw())?);
    }
    Ok(serde_yaml_ng::to_string(&raw)?)
}

pub fn save_file(
    path: &Path,
    entries: &[CvEntry],
    raw_failed: &BTreeMap<String, serde_yaml_ng::Value>,
) -> Result<(), SaveError> {
    let content = to_yaml_string(entries, raw_failed)?;
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
  category: Ministry Position
  title: Student Minister
  organization: Hope United Church
  location: Halifax, NS
  date: 2025-09/2026-04
  tags: [ministry, current]
  description:
    - Preaching and worship leadership on a rotating basis
    - Liturgical preparation for seasonal services

mdiv-2024:
  category: Education
  title: Master of Divinity (in progress)
  organization: Atlantic School of Theology
  date: 2023/
  tags: [academic, ministry]
  degree: MDiv
  field-of-study: Divinity
"#;

    #[test]
    fn parse_entry_count() {
        let outcome = parse_str(SAMPLE).unwrap();
        assert_eq!(outcome.entries.len(), 2);
        assert!(outcome.failed.is_empty());
    }

    #[test]
    fn parse_common_fields() {
        let outcome = parse_str(SAMPLE).unwrap();
        let e = outcome.entries.iter().find(|e| e.key == "hope-united-2025").unwrap();
        assert_eq!(e.category, "Ministry Position");
        assert_eq!(e.title, "Student Minister");
        assert_eq!(e.organization.as_deref(), Some("Hope United Church"));
        assert_eq!(e.location.as_deref(), Some("Halifax, NS"));
        assert_eq!(e.date.as_deref(), Some("2025-09/2026-04"));
        assert_eq!(e.tags, vec!["ministry", "current"]);
        assert_eq!(e.description.len(), 2);
    }

    #[test]
    fn parse_extra_category_specific_fields() {
        let outcome = parse_str(SAMPLE).unwrap();
        let e = outcome.entries.iter().find(|e| e.key == "mdiv-2024").unwrap();
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
  category: Award
  title: Some Award
  description: A single-line description
"#;
        let outcome = parse_str(yaml).unwrap();
        assert_eq!(outcome.entries[0].description, vec!["A single-line description"]);
    }

    #[test]
    fn unquoted_numeric_date_still_parses_as_string() {
        // Regression test: an early version of the per-entry parsing here
        // used `from_value` on the already-parsed generic Value, which is
        // strict about scalar types — an unquoted `date: 2020` is a YAML
        // integer, not a string, and got silently rejected (the entry ended
        // up in `failed` instead of `entries`). Parsing straight from text
        // coerces it to a string like everyone expects; this must keep doing
        // that even though entries are now parsed one at a time.
        let yaml = r#"
dated:
  category: Award
  title: Dated
  date: 2020
"#;
        let outcome = parse_str(yaml).unwrap();
        assert!(outcome.failed.is_empty(), "should not have failed: {:?}", outcome.failed);
        assert_eq!(outcome.entries[0].date.as_deref(), Some("2020"));
    }

    #[test]
    fn legacy_type_field_is_read_as_category() {
        let yaml = r#"
guelphEconomics:
  type: Education
  title: Economics
  organization: University of Guelph
"#;
        let outcome = parse_str(yaml).unwrap();
        assert!(outcome.failed.is_empty());
        assert_eq!(outcome.entries[0].category, "Education");
    }

    #[test]
    fn entry_missing_required_field_is_skipped_not_fatal() {
        let yaml = r#"
good-entry:
  category: Award
  title: Some Award

bad-entry:
  organization: Missing category and title
"#;
        let outcome = parse_str(yaml).unwrap();
        assert_eq!(outcome.entries.len(), 1);
        assert_eq!(outcome.entries[0].key, "good-entry");
        assert_eq!(outcome.failed.len(), 1);
        assert_eq!(outcome.failed[0].0, "bad-entry");
        assert!(outcome.raw_failed.contains_key("bad-entry"));
    }

    #[test]
    fn saving_preserves_unparseable_entries_untouched() {
        let yaml = r#"
good-entry:
  category: Award
  title: Some Award

bad-entry:
  organization: Missing category and title
"#;
        let outcome = parse_str(yaml).unwrap();
        let saved = to_yaml_string(&outcome.entries, &outcome.raw_failed).unwrap();
        let reparsed = parse_str(&saved).unwrap();
        assert_eq!(reparsed.entries.len(), 1);
        assert_eq!(reparsed.failed.len(), 1);
        assert_eq!(reparsed.failed[0].0, "bad-entry");
    }

    #[test]
    fn round_trip_preserves_fields() {
        let outcome = parse_str(SAMPLE).unwrap();
        let yaml = to_yaml_string(&outcome.entries, &outcome.raw_failed).unwrap();
        let reparsed = parse_str(&yaml).unwrap();
        assert_eq!(outcome.entries.len(), reparsed.entries.len());
        let orig = outcome.entries.iter().find(|e| e.key == "hope-united-2025").unwrap();
        let again = reparsed.entries.iter().find(|e| e.key == "hope-united-2025").unwrap();
        assert_eq!(orig, again);
    }

    #[test]
    fn load_file_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cv-elements.yaml");
        std::fs::write(&path, SAMPLE).unwrap();
        let outcome = load_file(&path).unwrap();
        assert_eq!(outcome.entries.len(), 2);

        let out_path = dir.path().join("out.yaml");
        save_file(&out_path, &outcome.entries, &outcome.raw_failed).unwrap();
        let reloaded = load_file(&out_path).unwrap();
        assert_eq!(reloaded.entries.len(), 2);
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
        let outcome = parse_str(SAMPLE).unwrap();
        let orig = outcome.entries.iter().find(|e| e.key == "mdiv-2024").unwrap();
        let dup = orig.duplicate_with_key("mdiv-2024-copy".into());
        assert_eq!(dup.key, "mdiv-2024-copy");
        assert_eq!(dup.title, orig.title);
        assert_eq!(dup.category, orig.category);
    }

    #[test]
    fn unique_key_returns_base_when_unused() {
        let outcome = parse_str(SAMPLE).unwrap();
        assert_eq!(unique_key("brand-new", &outcome.entries), "brand-new");
    }

    #[test]
    fn unique_key_appends_suffix_when_taken() {
        let outcome = parse_str(SAMPLE).unwrap();
        assert_eq!(unique_key("mdiv-2024", &outcome.entries), "mdiv-2024-2");
    }
}
