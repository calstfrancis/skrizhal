use crate::entry::CvEntry;

/// Filter criteria for browsing entries — all `None`/empty fields match
/// everything. Used by both the future GUI's sidebar filter and the `!`
/// autocomplete popup search in Zerkalo.
#[derive(Clone, Debug, Default)]
pub struct FilterOptions<'a> {
    pub entry_type: Option<&'a str>,
    pub tag: Option<&'a str>,
    /// Case-insensitive substring match against title, organization, and
    /// description text.
    pub query: Option<&'a str>,
}

pub fn filter_entries<'a>(entries: &'a [CvEntry], opts: &FilterOptions) -> Vec<&'a CvEntry> {
    let query_lower = opts.query.map(|q| q.to_lowercase());
    entries
        .iter()
        .filter(|e| {
            if let Some(t) = opts.entry_type {
                if e.entry_type != t {
                    return false;
                }
            }
            if let Some(tag) = opts.tag {
                if !e.tags.iter().any(|x| x == tag) {
                    return false;
                }
            }
            if let Some(q) = &query_lower {
                let haystack = format!(
                    "{} {} {}",
                    e.title,
                    e.organization.as_deref().unwrap_or(""),
                    e.description.join(" ")
                )
                .to_lowercase();
                if !haystack.contains(q.as_str()) {
                    return false;
                }
            }
            true
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::parse_str;

    const SAMPLE: &str = r#"
hope-united-2025:
  type: ministry-position
  title: Student Minister
  organization: Hope United Church
  tags: [ministry, current]

mdiv-2024:
  type: education
  title: Master of Divinity
  organization: Atlantic School of Theology
  tags: [academic, ministry]

ssrhc-award:
  type: award
  title: SSHRC Doctoral Award
  tags: [academic]
"#;

    #[test]
    fn filter_by_type() {
        let entries = parse_str(SAMPLE).unwrap();
        let opts = FilterOptions {
            entry_type: Some("education"),
            ..Default::default()
        };
        let result = filter_entries(&entries, &opts);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].key, "mdiv-2024");
    }

    #[test]
    fn filter_by_tag() {
        let entries = parse_str(SAMPLE).unwrap();
        let opts = FilterOptions {
            tag: Some("ministry"),
            ..Default::default()
        };
        let result = filter_entries(&entries, &opts);
        let mut keys: Vec<&str> = result.iter().map(|e| e.key.as_str()).collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["hope-united-2025", "mdiv-2024"]);
    }

    #[test]
    fn filter_by_query_matches_organization() {
        let entries = parse_str(SAMPLE).unwrap();
        let opts = FilterOptions {
            query: Some("atlantic"),
            ..Default::default()
        };
        let result = filter_entries(&entries, &opts);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].key, "mdiv-2024");
    }

    #[test]
    fn filter_combines_criteria_with_and() {
        let entries = parse_str(SAMPLE).unwrap();
        let opts = FilterOptions {
            tag: Some("academic"),
            entry_type: Some("award"),
            query: None,
        };
        let result = filter_entries(&entries, &opts);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].key, "ssrhc-award");
    }

    #[test]
    fn no_criteria_returns_everything() {
        let entries = parse_str(SAMPLE).unwrap();
        let result = filter_entries(&entries, &FilterOptions::default());
        assert_eq!(result.len(), entries.len());
    }
}
