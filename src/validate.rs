use crate::entry::{parse_str, CvEntry, LoadError};
use crate::registry;

/// Soft-validation findings — never hard errors, since the YAML schema is
/// deliberately open (new types/fields don't require code changes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    UnknownType { key: String, entry_type: String },
    MissingRecommendedField { key: String, field: &'static str },
    DuplicateKey { key: String },
}

fn field_present(entry: &CvEntry, field: &str) -> bool {
    match field {
        "organization" => entry.organization.as_deref().is_some_and(|s| !s.trim().is_empty()),
        "location" => entry.location.as_deref().is_some_and(|s| !s.trim().is_empty()),
        "date" => entry.date.as_deref().is_some_and(|s| !s.trim().is_empty()),
        "description" => !entry.description.is_empty(),
        "tags" => !entry.tags.is_empty(),
        other => entry.extra.get(other).is_some_and(|v| !v.is_null()),
    }
}

/// Checks unknown `type` values and missing type-recommended fields. Never
/// flags a type that isn't in the registry as anything but `UnknownType` —
/// unregistered types get no recommended-field checks since there's nothing
/// to check them against.
pub fn validate_entries(entries: &[CvEntry]) -> Vec<Warning> {
    let mut warnings = Vec::new();
    for e in entries {
        match registry::lookup(&e.entry_type) {
            None => warnings.push(Warning::UnknownType {
                key: e.key.clone(),
                entry_type: e.entry_type.clone(),
            }),
            Some(spec) => {
                for field in spec.recommended_fields {
                    if !field_present(e, field) {
                        warnings.push(Warning::MissingRecommendedField {
                            key: e.key.clone(),
                            field,
                        });
                    }
                }
            }
        }
    }
    warnings
}

/// Scans raw YAML text for duplicate top-level (unindented) keys. Needed
/// because a `BTreeMap`-based parse silently keeps only the last occurrence,
/// losing the fact that a key was duplicated in source — this looks at the
/// text directly instead. Only recognizes block-style entries (`key:` with
/// nested indented fields), which is the only shape this schema uses.
pub fn find_duplicate_top_level_keys(yaml: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut reported = std::collections::HashSet::new();
    let mut dups = Vec::new();
    for line in yaml.lines() {
        if line.starts_with(char::is_whitespace) || line.trim().is_empty() {
            continue;
        }
        let trimmed = line.trim_end();
        if trimmed.starts_with('#') {
            continue;
        }
        let Some(colon_idx) = trimmed.find(':') else { continue };
        let key_part = trimmed[..colon_idx].trim();
        let is_valid_key = !key_part.is_empty()
            && key_part.chars().next().is_some_and(|c| c.is_alphanumeric())
            && key_part.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-');
        if !is_valid_key {
            continue;
        }
        if !seen.insert(key_part.to_string()) && reported.insert(key_part.to_string()) {
            dups.push(key_part.to_string());
        }
    }
    dups
}

pub fn validate_yaml_text(yaml: &str) -> Vec<Warning> {
    find_duplicate_top_level_keys(yaml)
        .into_iter()
        .map(|key| Warning::DuplicateKey { key })
        .collect()
}

/// Convenience: parses `yaml` and runs both the text-level (duplicate key)
/// and entry-level (unknown type, missing fields) checks in one call.
pub fn validate_all(yaml: &str) -> Result<Vec<Warning>, LoadError> {
    let mut warnings = validate_yaml_text(yaml);
    let entries = parse_str(yaml)?;
    warnings.extend(validate_entries(&entries));
    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_type_is_flagged() {
        let yaml = r#"
mystery:
  type: not-a-real-type
  title: Something
"#;
        let entries = parse_str(yaml).unwrap();
        let warnings = validate_entries(&entries);
        assert!(warnings.contains(&Warning::UnknownType {
            key: "mystery".into(),
            entry_type: "not-a-real-type".into(),
        }));
    }

    #[test]
    fn missing_recommended_field_is_flagged() {
        let yaml = r#"
bare-job:
  type: employment
  title: Some Job
"#;
        let entries = parse_str(yaml).unwrap();
        let warnings = validate_entries(&entries);
        assert!(warnings.contains(&Warning::MissingRecommendedField {
            key: "bare-job".into(),
            field: "organization",
        }));
        assert!(warnings.contains(&Warning::MissingRecommendedField {
            key: "bare-job".into(),
            field: "date",
        }));
    }

    #[test]
    fn fully_specified_entry_has_no_warnings() {
        let yaml = r#"
complete-job:
  type: employment
  title: Some Job
  organization: Some Org
  location: Somewhere
  date: 2020-01/2021-01
  description: Did things
"#;
        let entries = parse_str(yaml).unwrap();
        assert!(validate_entries(&entries).is_empty());
    }

    #[test]
    fn type_specific_extra_field_satisfies_recommendation() {
        let yaml = r#"
mdiv-2024:
  type: education
  title: Master of Divinity
  organization: Atlantic School of Theology
  date: 2023/
  degree: MDiv
"#;
        let entries = parse_str(yaml).unwrap();
        let warnings = validate_entries(&entries);
        assert!(!warnings.iter().any(|w| matches!(
            w,
            Warning::MissingRecommendedField { field: "degree", .. }
        )));
    }

    #[test]
    fn detects_duplicate_top_level_key() {
        let yaml = r#"
same-key:
  type: award
  title: First

same-key:
  type: award
  title: Second
"#;
        let dups = find_duplicate_top_level_keys(yaml);
        assert_eq!(dups, vec!["same-key".to_string()]);
    }

    #[test]
    fn duplicate_key_reported_once_even_if_repeated_thrice() {
        let yaml = "a:\n  type: award\nb:\n  type: award\na:\n  type: award\na:\n  type: award\n";
        let dups = find_duplicate_top_level_keys(yaml);
        assert_eq!(dups, vec!["a".to_string()]);
    }

    #[test]
    fn no_duplicates_in_well_formed_file() {
        let yaml = r#"
one:
  type: award
  title: First
two:
  type: award
  title: Second
"#;
        assert!(find_duplicate_top_level_keys(yaml).is_empty());
    }

    #[test]
    fn nested_indented_keys_are_not_mistaken_for_top_level_duplicates() {
        let yaml = r#"
one:
  type: award
  title: First
two:
  type: award
  title: First
"#;
        // "title" appears twice but always indented — must not be flagged.
        assert!(find_duplicate_top_level_keys(yaml).is_empty());
    }

    #[test]
    fn validate_all_combines_text_and_entry_warnings() {
        let yaml = r#"
dup:
  type: bogus-type
  title: A

dup:
  type: bogus-type
  title: B
"#;
        let warnings = validate_all(yaml).unwrap();
        assert!(warnings
            .iter()
            .any(|w| matches!(w, Warning::DuplicateKey { key } if key == "dup")));
        assert!(warnings
            .iter()
            .any(|w| matches!(w, Warning::UnknownType { .. })));
    }
}
