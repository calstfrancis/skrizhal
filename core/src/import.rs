use std::collections::BTreeMap;

use crate::entry::{slugify, unique_key, CvEntry};

/// Parsed BibTeX record before it's mapped onto a `CvEntry` — the raw entry
/// type, citation key, and field map exactly as they appeared.
#[derive(Debug, Clone, PartialEq, Eq)]
struct BibRecord {
    kind: String,
    key: String,
    fields: BTreeMap<String, String>,
}

/// Maps a BibTeX entry type onto a Skrizhal category.
///
/// Note the irony worth stating plainly: this plan rejected the `hayagriva`
/// crate for CV *storage* because its `EntryType` enum is closed and would
/// refuse a category like "Ministry Position". For *importing publications*
/// that same closed vocabulary is exactly right — a `@inproceedings` really
/// is a presentation, and there's no open-ended case to accommodate.
fn category_for(kind: &str) -> &'static str {
    match kind.to_lowercase().as_str() {
        "inproceedings" | "conference" => "Presentation",
        "phdthesis" | "mastersthesis" => "Education",
        _ => "Publication",
    }
}

/// Strips one layer of `{}`/`""` wrapping and collapses whitespace. BibTeX
/// values routinely wrap words in extra braces to protect capitalization
/// (`{DNA}`), which should not survive into a human-readable YAML field.
fn clean_value(raw: &str) -> String {
    let mut out = String::new();
    let mut last_was_space = false;
    for ch in raw.chars() {
        match ch {
            '{' | '}' => continue,
            c if c.is_whitespace() => {
                if !last_was_space && !out.is_empty() {
                    out.push(' ');
                }
                last_was_space = true;
            }
            c => {
                out.push(c);
                last_was_space = false;
            }
        }
    }
    out.trim().to_string()
}

/// Reads the balanced-brace or quoted value starting at `chars[i]`, plus any
/// bare (unbraced) value such as a year. Returns the value and the index just
/// past it.
fn read_value(chars: &[char], mut i: usize) -> (String, usize) {
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    if i >= chars.len() {
        return (String::new(), i);
    }
    match chars[i] {
        '{' => {
            let mut depth = 0usize;
            let start = i;
            while i < chars.len() {
                match chars[i] {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            i += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            (chars[start..i].iter().collect(), i)
        }
        '"' => {
            let start = i + 1;
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            let value: String = chars[start..i.min(chars.len())].iter().collect();
            (value, (i + 1).min(chars.len()))
        }
        _ => {
            let start = i;
            while i < chars.len() && chars[i] != ',' && chars[i] != '}' {
                i += 1;
            }
            (chars[start..i].iter().collect(), i)
        }
    }
}

fn parse_records(text: &str) -> Vec<BibRecord> {
    let chars: Vec<char> = text.chars().collect();
    let mut records = Vec::new();
    let mut i = 0usize;

    while i < chars.len() {
        if chars[i] != '@' {
            i += 1;
            continue;
        }
        i += 1;
        let kind_start = i;
        while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
            i += 1;
        }
        let kind: String = chars[kind_start..i].iter().collect::<String>().to_lowercase();
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() || chars[i] != '{' {
            continue;
        }
        // @string/@preamble/@comment carry no citation key and no fields worth
        // importing; skipping them here keeps the field loop below simple.
        if matches!(kind.as_str(), "string" | "preamble" | "comment") {
            let (_, next) = read_value(&chars, i);
            i = next;
            continue;
        }
        i += 1;

        let key_start = i;
        while i < chars.len() && chars[i] != ',' && chars[i] != '}' {
            i += 1;
        }
        let key: String = chars[key_start..i].iter().collect::<String>().trim().to_string();
        let mut fields = BTreeMap::new();

        while i < chars.len() && chars[i] != '}' {
            i += 1; // skip ',' or whitespace
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            let name_start = i;
            while i < chars.len() && chars[i] != '=' && chars[i] != '}' && chars[i] != ',' {
                i += 1;
            }
            if i >= chars.len() || chars[i] != '=' {
                break;
            }
            let name: String = chars[name_start..i]
                .iter()
                .collect::<String>()
                .trim()
                .to_lowercase();
            i += 1;
            let (value, next) = read_value(&chars, i);
            i = next;
            if !name.is_empty() {
                fields.insert(name, clean_value(&value));
            }
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
        }

        if !key.is_empty() {
            records.push(BibRecord { kind, key, fields });
        }
        i += 1;
    }

    records
}

fn record_to_entry(record: &BibRecord, taken: &[CvEntry]) -> CvEntry {
    let field = |name: &str| record.fields.get(name).cloned().filter(|v| !v.is_empty());

    let mut extra: BTreeMap<String, serde_yaml_ng::Value> = BTreeMap::new();
    // `venue` is what the category registry recommends for Publication and
    // Presentation; BibTeX spreads the same idea across three field names.
    if let Some(venue) = field("journal").or_else(|| field("booktitle")).or_else(|| field("series"))
    {
        extra.insert("venue".into(), serde_yaml_ng::Value::String(venue));
    }
    for (bib_field, target) in [
        ("author", "author"),
        ("doi", "doi"),
        ("url", "url"),
        ("volume", "volume"),
        ("pages", "pages"),
        ("editor", "editor"),
    ] {
        if let Some(value) = field(bib_field) {
            extra.insert(target.into(), serde_yaml_ng::Value::String(value));
        }
    }
    if record.kind == "phdthesis" || record.kind == "mastersthesis" {
        extra.insert(
            "degree".into(),
            serde_yaml_ng::Value::String(if record.kind == "phdthesis" {
                "PhD".into()
            } else {
                "Master's".into()
            }),
        );
    }

    let title = field("title").unwrap_or_else(|| record.key.clone());
    // Prefer the BibTeX citation key — it's already a citation-style slug and
    // often the same key the user cites elsewhere — but slugify it so it
    // matches Skrizhal's own key convention.
    let base = {
        let slug = slugify(&record.key);
        if slug.is_empty() {
            slugify(&title)
        } else {
            slug
        }
    };

    CvEntry {
        key: unique_key(&base, taken),
        category: category_for(&record.kind).to_string(),
        title,
        organization: field("publisher").or_else(|| field("school")).or_else(|| field("institution")),
        location: field("address"),
        date: field("year"),
        order: None,
        tags: Vec::new(),
        description: Vec::new(),
        extra,
    }
}

/// Parses BibTeX source into entries, assigning each a key unique against
/// `existing` (and against the rest of the import). Import is always
/// additive — nothing here modifies or replaces an existing entry.
pub fn parse_bibtex(text: &str, existing: &[CvEntry]) -> Vec<CvEntry> {
    let mut taken: Vec<CvEntry> = existing.to_vec();
    let mut imported = Vec::new();
    for record in parse_records(text) {
        let entry = record_to_entry(&record, &taken);
        taken.push(entry.clone());
        imported.push(entry);
    }
    imported
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
@article{smith2020faith,
  title   = {Faith and {Practice} in Modern Ministry},
  author  = {Smith, Jane and Doe, John},
  journal = {Journal of Practical Theology},
  year    = 2020,
  doi     = {10.1000/example},
  volume  = {12},
  pages   = {45--67}
}

@inproceedings{smith2021liturgy,
  title     = "Liturgy in Practice",
  author    = "Smith, Jane",
  booktitle = "Proceedings of the Example Conference",
  year      = {2021},
  address   = {Halifax, NS}
}

@phdthesis{smith2019thesis,
  title  = {A Very Long Dissertation},
  school = {Example University},
  year   = {2019}
}
"#;

    #[test]
    fn parses_every_record() {
        let entries = parse_bibtex(SAMPLE, &[]);
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn maps_entry_types_to_categories() {
        let entries = parse_bibtex(SAMPLE, &[]);
        assert_eq!(entries[0].category, "Publication");
        assert_eq!(entries[1].category, "Presentation");
        assert_eq!(entries[2].category, "Education");
    }

    /// Protective braces exist to preserve capitalization for BibTeX's own
    /// styling, and must not leak into a human-readable title.
    #[test]
    fn strips_protective_braces_from_values() {
        let entries = parse_bibtex(SAMPLE, &[]);
        assert_eq!(entries[0].title, "Faith and Practice in Modern Ministry");
    }

    #[test]
    fn handles_quoted_and_bare_values() {
        let entries = parse_bibtex(SAMPLE, &[]);
        assert_eq!(entries[1].title, "Liturgy in Practice");
        assert_eq!(entries[1].location.as_deref(), Some("Halifax, NS"));
        // `year = 2020` is unbraced and unquoted.
        assert_eq!(entries[0].date.as_deref(), Some("2020"));
    }

    #[test]
    fn maps_journal_and_booktitle_onto_venue() {
        let entries = parse_bibtex(SAMPLE, &[]);
        let venue = |e: &CvEntry| match e.extra.get("venue") {
            Some(serde_yaml_ng::Value::String(s)) => s.clone(),
            _ => String::new(),
        };
        assert_eq!(venue(&entries[0]), "Journal of Practical Theology");
        assert_eq!(venue(&entries[1]), "Proceedings of the Example Conference");
    }

    #[test]
    fn thesis_school_becomes_organization() {
        let entries = parse_bibtex(SAMPLE, &[]);
        assert_eq!(entries[2].organization.as_deref(), Some("Example University"));
    }

    #[test]
    fn keys_derive_from_citation_keys() {
        let entries = parse_bibtex(SAMPLE, &[]);
        assert_eq!(entries[0].key, "smith2020faith");
    }

    #[test]
    fn keys_do_not_collide_with_existing_entries() {
        let existing = vec![CvEntry {
            key: "smith2020faith".to_string(),
            ..Default::default()
        }];
        let entries = parse_bibtex(SAMPLE, &existing);
        assert_ne!(entries[0].key, "smith2020faith");
        assert!(entries[0].key.starts_with("smith2020faith"));
    }

    #[test]
    fn keys_do_not_collide_within_one_import() {
        let text = "@article{dup, title={A}}\n@article{dup, title={B}}";
        let entries = parse_bibtex(text, &[]);
        assert_eq!(entries.len(), 2);
        assert_ne!(entries[0].key, entries[1].key);
    }

    #[test]
    fn string_and_comment_blocks_are_skipped() {
        let text = r#"
@comment{ this is ignored }
@string{jpt = "Journal of Practical Theology"}
@article{real2020, title = {A Real Entry}, year = {2020}}
"#;
        let entries = parse_bibtex(text, &[]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "A Real Entry");
    }

    #[test]
    fn empty_or_garbage_input_yields_nothing() {
        assert!(parse_bibtex("", &[]).is_empty());
        assert!(parse_bibtex("not bibtex at all", &[]).is_empty());
    }
}
