use serde::{Deserialize, Serialize};

use crate::date::sort_key;
use crate::entry::CvEntry;

/// How a list of entries is ordered for display. `DateDesc` is the default
/// because a CV database is read chronologically far more often than
/// alphabetically — but Title stays available, since hunting for a known
/// entry by name in a long list is the one case where it wins.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SortMode {
    #[default]
    DateDesc,
    Title,
    Category,
}

impl SortMode {
    pub const ALL: &'static [SortMode] = &[SortMode::DateDesc, SortMode::Title, SortMode::Category];

    pub fn label(self) -> &'static str {
        match self {
            SortMode::DateDesc => "Newest First",
            SortMode::Title => "Title",
            SortMode::Category => "Category",
        }
    }

    pub fn index(self) -> u32 {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0) as u32
    }

    pub fn from_index(index: u32) -> SortMode {
        Self::ALL.get(index as usize).copied().unwrap_or_default()
    }
}

fn title_key(entry: &CvEntry) -> (String, String) {
    (entry.title.to_lowercase(), entry.key.to_lowercase())
}

/// Sorts in place. Every mode is a total order with the entry key as the
/// final tiebreak, so a refresh never reshuffles rows that compare equal.
pub fn sort_entries(entries: &mut [CvEntry], mode: SortMode) {
    match mode {
        SortMode::DateDesc => entries.sort_by(|a, b| {
            sort_key(b.date.as_deref())
                .cmp(&sort_key(a.date.as_deref()))
                .then_with(|| title_key(a).cmp(&title_key(b)))
        }),
        SortMode::Title => entries.sort_by_key(title_key),
        SortMode::Category => entries.sort_by(|a, b| {
            a.category
                .to_lowercase()
                .cmp(&b.category.to_lowercase())
                .then_with(|| sort_key(b.date.as_deref()).cmp(&sort_key(a.date.as_deref())))
                .then_with(|| title_key(a).cmp(&title_key(b)))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, category: &str, title: &str, date: Option<&str>) -> CvEntry {
        CvEntry {
            key: key.to_string(),
            category: category.to_string(),
            title: title.to_string(),
            date: date.map(|d| d.to_string()),
            ..Default::default()
        }
    }

    fn keys(entries: &[CvEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.key.as_str()).collect()
    }

    fn sample() -> Vec<CvEntry> {
        vec![
            entry("mdiv", "Education", "Master of Divinity", Some("2023/")),
            entry("bath", "Education", "Bachelor of Arts", Some("2015/2019")),
            entry("minister", "Ministry Position", "Student Minister", Some("2025-09/2026-04")),
        ]
    }

    /// Ongoing entries outrank even a closed range ending later, because
    /// `date::sort_key` resolves an open end to the `PRESENT` sentinel — an
    /// in-progress degree is "current" in a way a finished placement isn't.
    #[test]
    fn date_desc_puts_ongoing_first_then_most_recent() {
        let mut entries = sample();
        sort_entries(&mut entries, SortMode::DateDesc);
        assert_eq!(keys(&entries), vec!["mdiv", "minister", "bath"]);
    }

    #[test]
    fn title_sorts_alphabetically_case_insensitively() {
        let mut entries = sample();
        sort_entries(&mut entries, SortMode::Title);
        assert_eq!(keys(&entries), vec!["bath", "mdiv", "minister"]);
    }

    #[test]
    fn category_groups_then_sorts_by_date_within_group() {
        let mut entries = sample();
        sort_entries(&mut entries, SortMode::Category);
        assert_eq!(keys(&entries), vec!["mdiv", "bath", "minister"]);
    }

    #[test]
    fn entries_without_dates_sort_last_not_first() {
        let mut entries = sample();
        entries.push(entry("undated", "Award", "Some Award", None));
        sort_entries(&mut entries, SortMode::DateDesc);
        assert_eq!(entries.last().unwrap().key, "undated");
    }

    #[test]
    fn sort_is_stable_for_equal_entries() {
        let mut entries = vec![
            entry("b-key", "Award", "Same Title", Some("2020")),
            entry("a-key", "Award", "Same Title", Some("2020")),
        ];
        sort_entries(&mut entries, SortMode::DateDesc);
        assert_eq!(keys(&entries), vec!["a-key", "b-key"]);
    }

    #[test]
    fn index_round_trips() {
        for mode in SortMode::ALL {
            assert_eq!(SortMode::from_index(mode.index()), *mode);
        }
    }

    #[test]
    fn from_index_out_of_range_falls_back_to_default() {
        assert_eq!(SortMode::from_index(99), SortMode::default());
    }
}
