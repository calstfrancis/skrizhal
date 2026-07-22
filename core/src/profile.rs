use serde::{Deserialize, Serialize};

use crate::date::sort_key;
use crate::entry::CvEntry;

/// The reserved top-level key profiles live under in `cv-elements.yaml`.
/// Underscore-prefixed so it can never collide with an entry key, which is
/// always a citation-style slug.
pub const PROFILES_KEY: &str = "_profiles";

/// One section of a CV: a heading plus the rules deciding which entries land
/// under it.
///
/// `categories`/`tags` are the filter half — an empty list means "don't
/// filter on this axis" (so a section with neither matches everything).
/// `include`/`exclude` are the explicit half, and they're the whole reason
/// profiles exist rather than just saved filters: they express the one-off
/// exception ("this CV only, drop that job") without inventing a
/// single-use tag for it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSection {
    #[serde(default)]
    pub heading: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Keys pulled in regardless of the filters above.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    /// Keys kept out even if the filters (or `include`) would take them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
}

/// Wire format for a profile — everything except its name, which comes from
/// the enclosing mapping key, the same way an entry's key does.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawProfile {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(default)]
    pub sections: Vec<ProfileSection>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    pub label: String,
    pub sections: Vec<ProfileSection>,
}

impl Profile {
    pub fn from_raw(name: String, raw: RawProfile) -> Self {
        Profile {
            name,
            label: raw.label,
            sections: raw.sections,
        }
    }

    pub fn into_raw(self) -> RawProfile {
        RawProfile {
            label: self.label,
            sections: self.sections,
        }
    }

    /// What to show in a list — the human label if set, else the raw name.
    pub fn display_name(&self) -> &str {
        if self.label.trim().is_empty() {
            &self.name
        } else {
            &self.label
        }
    }
}

fn matches_filters(entry: &CvEntry, section: &ProfileSection) -> bool {
    let category_ok = section.categories.is_empty()
        || section
            .categories
            .iter()
            .any(|c| c.eq_ignore_ascii_case(&entry.category));
    let tags_ok = section.tags.is_empty()
        || section
            .tags
            .iter()
            .any(|t| entry.tags.iter().any(|et| et.eq_ignore_ascii_case(t)));
    category_ok && tags_ok
}

/// Orders entries within a section: anything with an explicit `order` leads,
/// ascending, and everything else follows in the usual most-recent-first
/// order. Real CVs lead with what's relevant rather than what's newest, but
/// only for the handful of entries the author actually cares to pin — so
/// `order` is an opt-in override, not a field anyone has to maintain.
fn section_sort(entries: &mut [CvEntry]) {
    entries.sort_by(|a, b| {
        let a_order = a.order.unwrap_or(i64::MAX);
        let b_order = b.order.unwrap_or(i64::MAX);
        a_order
            .cmp(&b_order)
            .then_with(|| sort_key(b.date.as_deref()).cmp(&sort_key(a.date.as_deref())))
            .then_with(|| a.key.cmp(&b.key))
    });
}

/// Resolves one section against the full entry set, applying filters, then
/// explicit includes, then explicit excludes — in that order, so `exclude`
/// always wins over both.
pub fn resolve_section(entries: &[CvEntry], section: &ProfileSection) -> Vec<CvEntry> {
    let mut out: Vec<CvEntry> = entries
        .iter()
        .filter(|e| matches_filters(e, section) || section.include.contains(&e.key))
        .filter(|e| !section.exclude.contains(&e.key))
        .cloned()
        .collect();
    section_sort(&mut out);
    out
}

/// Resolves every section of a profile, returning `(heading, entries)` pairs
/// in the profile's own section order.
pub fn resolve_profile(entries: &[CvEntry], profile: &Profile) -> Vec<(String, Vec<CvEntry>)> {
    profile
        .sections
        .iter()
        .map(|s| (s.heading.clone(), resolve_section(entries, s)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, category: &str, tags: &[&str], date: Option<&str>) -> CvEntry {
        CvEntry {
            key: key.to_string(),
            category: category.to_string(),
            title: key.to_string(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            date: date.map(|d| d.to_string()),
            ..Default::default()
        }
    }

    fn sample() -> Vec<CvEntry> {
        vec![
            entry("job-old", "Employment", &["teaching"], Some("2015/2018")),
            entry("job-new", "Employment", &["ministry"], Some("2022/2024")),
            entry("degree", "Education", &["academic"], Some("2019/2022")),
            entry("award", "Award", &["academic"], Some("2021")),
        ]
    }

    fn keys(entries: &[CvEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.key.as_str()).collect()
    }

    #[test]
    fn empty_section_matches_everything() {
        let got = resolve_section(&sample(), &ProfileSection::default());
        assert_eq!(got.len(), 4);
    }

    #[test]
    fn category_filter_selects_matching_entries() {
        let section = ProfileSection {
            categories: vec!["Employment".into()],
            ..Default::default()
        };
        assert_eq!(keys(&resolve_section(&sample(), &section)), vec!["job-new", "job-old"]);
    }

    #[test]
    fn category_filter_is_case_insensitive() {
        let section = ProfileSection {
            categories: vec!["employment".into()],
            ..Default::default()
        };
        assert_eq!(resolve_section(&sample(), &section).len(), 2);
    }

    #[test]
    fn tag_filter_matches_any_listed_tag() {
        let section = ProfileSection {
            tags: vec!["academic".into()],
            ..Default::default()
        };
        assert_eq!(keys(&resolve_section(&sample(), &section)), vec!["degree", "award"]);
    }

    #[test]
    fn category_and_tag_filters_are_combined_with_and() {
        let section = ProfileSection {
            categories: vec!["Employment".into()],
            tags: vec!["academic".into()],
            ..Default::default()
        };
        assert!(resolve_section(&sample(), &section).is_empty());
    }

    #[test]
    fn include_pulls_in_an_entry_the_filters_reject() {
        let section = ProfileSection {
            categories: vec!["Education".into()],
            include: vec!["award".into()],
            ..Default::default()
        };
        assert_eq!(keys(&resolve_section(&sample(), &section)), vec!["degree", "award"]);
    }

    #[test]
    fn exclude_removes_an_entry_the_filters_accept() {
        let section = ProfileSection {
            categories: vec!["Employment".into()],
            exclude: vec!["job-old".into()],
            ..Default::default()
        };
        assert_eq!(keys(&resolve_section(&sample(), &section)), vec!["job-new"]);
    }

    /// The one-off exception is the whole point of explicit lists, so the
    /// precedence between them has to be unambiguous: exclude wins.
    #[test]
    fn exclude_beats_include_for_the_same_key() {
        let section = ProfileSection {
            include: vec!["award".into()],
            exclude: vec!["award".into()],
            ..Default::default()
        };
        assert!(!keys(&resolve_section(&sample(), &section)).contains(&"award"));
    }

    #[test]
    fn explicit_order_leads_and_the_rest_stay_chronological() {
        let mut entries = sample();
        entries
            .iter_mut()
            .find(|e| e.key == "job-old")
            .unwrap()
            .order = Some(1);
        let got = resolve_section(&entries, &ProfileSection::default());
        assert_eq!(got[0].key, "job-old");
        assert_eq!(keys(&got)[1..], ["job-new", "degree", "award"]);
    }

    #[test]
    fn resolve_profile_keeps_section_order_and_headings() {
        let profile = Profile {
            name: "academic".into(),
            label: "Academic CV".into(),
            sections: vec![
                ProfileSection {
                    heading: "Education".into(),
                    categories: vec!["Education".into()],
                    ..Default::default()
                },
                ProfileSection {
                    heading: "Work".into(),
                    categories: vec!["Employment".into()],
                    ..Default::default()
                },
            ],
        };
        let got = resolve_profile(&sample(), &profile);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "Education");
        assert_eq!(keys(&got[0].1), vec!["degree"]);
        assert_eq!(got[1].0, "Work");
        assert_eq!(keys(&got[1].1), vec!["job-new", "job-old"]);
    }

    #[test]
    fn display_name_falls_back_to_the_raw_name() {
        let mut p = Profile {
            name: "academic-2026".into(),
            label: String::new(),
            sections: vec![],
        };
        assert_eq!(p.display_name(), "academic-2026");
        p.label = "Academic CV".into();
        assert_eq!(p.display_name(), "Academic CV");
    }
}
