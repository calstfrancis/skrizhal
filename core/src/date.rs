use crate::entry::CvEntry;

/// A sentinel end-date for open-ended ranges (`"2023/"` = ongoing/present),
/// so they sort as the most recent thing on the CV.
const PRESENT: (i32, u32) = (i32::MAX, 12);

type YearMonth = (i32, u32);
type DateRange = (Option<YearMonth>, Option<YearMonth>);

/// Parses `"2020"` or `"2020-09"` into `(year, month)`. Month defaults to 1
/// when absent. Not full ISO 8601 (no day-of-month, no validation of month
/// range) — good enough for chronological sorting, which is all this is for.
fn parse_year_month(s: &str) -> Option<(i32, u32)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut parts = s.splitn(2, '-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u32 = match parts.next() {
        Some(m) => m.parse().ok()?,
        None => 1,
    };
    Some((year, month))
}

/// Parses a Hayagriva-style date or date range (`"2020"`, `"2020-09"`,
/// `"2025-09/2026-04"`, `"2023/"`) into `(start, end)`. Unparseable or empty
/// input yields `(None, None)` rather than an error — dates in CV data are
/// display strings first, sort keys second.
pub fn parse_date_range(s: &str) -> DateRange {
    let s = s.trim();
    match s.find('/') {
        Some(idx) => {
            let before = s[..idx].trim();
            let after = s[idx + 1..].trim();
            let start = parse_year_month(before);
            let end = if after.is_empty() {
                Some(PRESENT)
            } else {
                parse_year_month(after)
            };
            (start, end)
        }
        None => {
            let d = parse_year_month(s);
            (d, d)
        }
    }
}

/// Which shape a date takes in the editor: a single point in time, a closed
/// range with both ends known, or an open-ended range still in progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateMode {
    Single,
    Range,
    Ongoing,
}

/// Splits a stored date string into its editing shape plus start/end text,
/// preserving whatever precision the user originally typed (`"2020"` vs
/// `"2020-09"`) rather than round-tripping through `parse_year_month`.
pub fn split_date_string(date: &str) -> (DateMode, String, String) {
    let date = date.trim();
    match date.find('/') {
        Some(idx) => {
            let start = date[..idx].trim().to_string();
            let end = date[idx + 1..].trim().to_string();
            if end.is_empty() {
                (DateMode::Ongoing, start, String::new())
            } else {
                (DateMode::Range, start, end)
            }
        }
        None => (DateMode::Single, date.to_string(), String::new()),
    }
}

/// Recomposes a `(mode, start, end)` triple back into the single stored
/// date string. The inverse of `split_date_string`.
pub fn join_date_string(mode: DateMode, start: &str, end: &str) -> String {
    let start = start.trim();
    let end = end.trim();
    match mode {
        DateMode::Single => start.to_string(),
        DateMode::Range => format!("{start}/{end}"),
        DateMode::Ongoing => format!("{start}/"),
    }
}

/// A `(end_year, end_month, start_year, start_month)` tuple such that a
/// larger key sorts as more recent — descending sort puts current/ongoing
/// entries first, unparseable/missing dates last.
pub fn sort_key(date: Option<&str>) -> (i32, u32, i32, u32) {
    let (start, end) = date.map(parse_date_range).unwrap_or((None, None));
    let (ey, em) = end.unwrap_or((0, 0));
    let (sy, sm) = start.unwrap_or((0, 0));
    (ey, em, sy, sm)
}

/// Sorts entries most-recent-first by `date`, using an ongoing range's
/// present-day end and falling back to the start date to break ties.
pub fn sort_entries_by_date_desc(entries: &mut [CvEntry]) {
    entries.sort_by_key(|e| std::cmp::Reverse(sort_key(e.date.as_deref())));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::parse_str;

    #[test]
    fn parses_single_year() {
        assert_eq!(parse_date_range("2020"), (Some((2020, 1)), Some((2020, 1))));
    }

    #[test]
    fn split_single_date() {
        assert_eq!(
            split_date_string("2020"),
            (DateMode::Single, "2020".to_string(), String::new())
        );
    }

    #[test]
    fn split_closed_range() {
        assert_eq!(
            split_date_string("2025-09/2026-04"),
            (DateMode::Range, "2025-09".to_string(), "2026-04".to_string())
        );
    }

    #[test]
    fn split_ongoing() {
        assert_eq!(
            split_date_string("2023/"),
            (DateMode::Ongoing, "2023".to_string(), String::new())
        );
    }

    #[test]
    fn split_empty_is_single() {
        assert_eq!(split_date_string(""), (DateMode::Single, String::new(), String::new()));
    }

    #[test]
    fn join_round_trips_each_mode() {
        assert_eq!(join_date_string(DateMode::Single, "2020", ""), "2020");
        assert_eq!(
            join_date_string(DateMode::Range, "2025-09", "2026-04"),
            "2025-09/2026-04"
        );
        assert_eq!(join_date_string(DateMode::Ongoing, "2023", ""), "2023/");
    }

    #[test]
    fn split_then_join_is_identity_for_well_formed_input() {
        for input in ["2020", "2020-09", "2025-09/2026-04", "2023/"] {
            let (mode, start, end) = split_date_string(input);
            assert_eq!(join_date_string(mode, &start, &end), input);
        }
    }

    #[test]
    fn parses_year_month() {
        assert_eq!(parse_date_range("2020-09"), (Some((2020, 9)), Some((2020, 9))));
    }

    #[test]
    fn parses_closed_range() {
        assert_eq!(
            parse_date_range("2025-09/2026-04"),
            (Some((2025, 9)), Some((2026, 4)))
        );
    }

    #[test]
    fn parses_open_ended_range_as_present() {
        assert_eq!(parse_date_range("2023/"), (Some((2023, 1)), Some(PRESENT)));
    }

    #[test]
    fn unparseable_date_falls_back_gracefully() {
        assert_eq!(parse_date_range("some day"), (None, None));
        assert_eq!(parse_date_range(""), (None, None));
    }

    #[test]
    fn sort_entries_most_recent_first_and_ongoing_wins() {
        let yaml = r#"
old-job:
  category: Employment
  title: Old Job
  date: 2018-01/2019-06

current-job:
  category: Employment
  title: Current Job
  date: 2023/

mid-job:
  category: Employment
  title: Mid Job
  date: 2020-01/2022-12
"#;
        let mut entries = parse_str(yaml).unwrap().entries;
        sort_entries_by_date_desc(&mut entries);
        let order: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(order, vec!["current-job", "mid-job", "old-job"]);
    }

    #[test]
    fn missing_date_sorts_last() {
        let yaml = r#"
no-date:
  category: Award
  title: No Date

dated:
  category: Award
  title: Dated
  date: 2020
"#;
        let mut entries = parse_str(yaml).unwrap().entries;
        sort_entries_by_date_desc(&mut entries);
        assert_eq!(entries[0].key, "dated");
        assert_eq!(entries[1].key, "no-date");
    }
}
