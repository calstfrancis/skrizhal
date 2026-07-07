# Changelog

All notable changes to Skrizhal are recorded here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [0.2.0] "First Etching" — GTK4/libadwaita editor app + first flatpak package

### Added
- **Main window** — sidebar (search + type/tag filters + entry list) beside a detail pane, in a
  resizable `Paned` with a headerbar sidebar toggle.
- **Structured entry form** — key, type (with a suggestion popover listing the type registry),
  title, organization, location, date, and tags as `adw::EntryRow`s; description as a multi-line
  bullet list; type-specific fields as a dynamic add/remove key-value list ("Additional Fields").
  Soft-validation warnings for the loaded entry are shown inline.
- **Raw YAML toggle** — swaps the structured form for the selected entry's serialized YAML block,
  editable directly; committing re-parses it back into the entry.
- **Add / Duplicate / Delete** — header "+" seeds the new entry's type/tag from the active filters
  so it's actually visible in the current view; each row has a Duplicate/Delete menu; Delete asks
  for confirmation first.
- **Manage Tags dialog** — lists tags with usage counts; renaming a tag applies across all entries
  and merges into an existing tag name if one is typed.
- **Config-driven data file** — defaults to `~/.local/share/skrizhal/cv-elements.yaml`
  (`~/.config/skrizhal/config.toml` overrides the path); "Choose Data File…" and "Reload from
  Disk" in the header menu. A data file that exists but fails to parse blocks saving (rather than
  silently overwriting it with an empty list) until reloaded successfully.
- `tags::rename_tag`/`all_tags_with_counts` and `entry::{slugify, unique_key,
  duplicate_with_key}` added to the core crate to back the above, with unit tests.
- **Flatpak packaging** — first packaged release, published to the personal flatpak repo
  (`calstfrancis/flatpak`). GNOME 50 runtime + `rust-stable` SDK extension (25.08 branch;
  GNOME 48's rust-stable only ships rustc 1.89, but the current gtk4-rs/libadwaita-rs crate
  versions need 1.92+). Two-module manifest (deps cached separately from the app crate, same
  pattern as Zerkalo) so dev-build iterations skip re-vendoring/re-compiling dependencies.

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
