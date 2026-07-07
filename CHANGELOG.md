# Changelog

All notable changes to Skrizhal are recorded here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [0.1.0] — Core crate: CV entry schema, validation, and filtering

### Added
- **`CvEntry` schema** — common fields (key, type, title, organization, location, date, tags,
  description) plus an open `extra` map for type-specific fields, parsed from and serialized back
  to Hayagriva-shaped YAML via `serde_yaml_ng`.
- **Type registry** — namespaced CV entry types (education, employment, ministry-position,
  publication, presentation, award, service, committee-appointment, language-skill,
  certification, volunteer, project) with recommended fields per type.
- **Soft validation** — non-blocking warnings for unknown types, missing recommended fields, and
  duplicate top-level keys in the source YAML (detected via raw-text scan, since a `BTreeMap`
  parse silently collapses duplicates).
- **Date-range parsing and sorting** — `YYYY` / `YYYY-MM` / `start/end` ranges (including
  open-ended `"2023/"` for ongoing positions) parsed into a sortable key for most-recent-first
  ordering.
- **Filtering** — by entry type, tag, and free-text search across title/organization/description.
- 32 unit tests, clippy-clean.

No GUI yet — this release is the core library only (Phase 1 of `plan.md`).
