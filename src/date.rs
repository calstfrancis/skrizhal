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
  type: employment
  title: Old Job
  date: 2018-01/2019-06

current-job:
  type: employment
  title: Current Job
  date: 2023/

mid-job:
  type: employment
  title: Mid Job
  date: 2020-01/2022-12
"#;
        let mut entries = parse_str(yaml).unwrap();
        sort_entries_by_date_desc(&mut entries);
        let order: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(order, vec!["current-job", "mid-job", "old-job"]);
    }

    #[test]
    fn missing_date_sorts_last() {
        let yaml = r#"
no-date:
  type: award
  title: No Date

dated:
  type: award
  title: Dated
  date: 2020
"#;
        let mut entries = parse_str(yaml).unwrap();
        sort_entries_by_date_desc(&mut entries);
        assert_eq!(entries[0].key, "dated");
        assert_eq!(entries[1].key, "no-date");
    }
}
