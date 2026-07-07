use crate::entry::CvEntry;

/// Distinct tags across all entries with usage counts, sorted by tag name.
pub fn all_tags_with_counts(entries: &[CvEntry]) -> Vec<(String, usize)> {
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for e in entries {
        for tag in &e.tags {
            *counts.entry(tag.as_str()).or_insert(0) += 1;
        }
    }
    counts.into_iter().map(|(t, c)| (t.to_string(), c)).collect()
}

/// Renames `old` to `new` across every entry's tag list, deduplicating so
/// renaming onto an existing tag name merges the two. Returns how many
/// entries were changed.
pub fn rename_tag(entries: &mut [CvEntry], old: &str, new: &str) -> usize {
    if old == new {
        return 0;
    }
    let mut changed = 0;
    for e in entries {
        if !e.tags.iter().any(|t| t == old) {
            continue;
        }
        for t in &mut e.tags {
            if t == old {
                *t = new.to_string();
            }
        }
        let mut seen = std::collections::HashSet::new();
        e.tags.retain(|t| seen.insert(t.clone()));
        changed += 1;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::parse_str;

    const SAMPLE: &str = r#"
a:
  type: award
  title: A
  tags: [ministry, current]
b:
  type: award
  title: B
  tags: [ministry, academic]
c:
  type: award
  title: C
  tags: [academic]
"#;

    #[test]
    fn counts_are_correct() {
        let entries = parse_str(SAMPLE).unwrap();
        let counts = all_tags_with_counts(&entries);
        assert_eq!(
            counts,
            vec![
                ("academic".to_string(), 2),
                ("current".to_string(), 1),
                ("ministry".to_string(), 2),
            ]
        );
    }

    #[test]
    fn rename_updates_all_matching_entries() {
        let mut entries = parse_str(SAMPLE).unwrap();
        let changed = rename_tag(&mut entries, "ministry", "church");
        assert_eq!(changed, 2);
        let a = entries.iter().find(|e| e.key == "a").unwrap();
        assert_eq!(a.tags, vec!["church", "current"]);
    }

    #[test]
    fn rename_onto_existing_tag_merges_and_dedupes() {
        let mut entries = parse_str(SAMPLE).unwrap();
        rename_tag(&mut entries, "ministry", "academic");
        let b = entries.iter().find(|e| e.key == "b").unwrap();
        // was [ministry, academic] -> both become "academic" -> deduped to one
        assert_eq!(b.tags, vec!["academic"]);
    }

    #[test]
    fn rename_nonexistent_tag_changes_nothing() {
        let mut entries = parse_str(SAMPLE).unwrap();
        let changed = rename_tag(&mut entries, "not-a-tag", "whatever");
        assert_eq!(changed, 0);
    }

    #[test]
    fn rename_to_same_name_is_noop() {
        let mut entries = parse_str(SAMPLE).unwrap();
        let changed = rename_tag(&mut entries, "ministry", "ministry");
        assert_eq!(changed, 0);
    }
}
