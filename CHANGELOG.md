# Changelog

All notable changes to Skrizhal are recorded here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [0.3.1-dev1] — New File/Open/Save As/Preferences, parse-crash fix, changelog & spreadsheet polish

### Changed
- **Changelog window** — the Markdown-to-Pango converter now folds a bullet's hard-wrapped
  source lines (CHANGELOG.md wraps prose at ~90 columns for editor readability) into one
  paragraph that reflows to the dialog's actual width, instead of keeping the source file's
  arbitrary line breaks (previously every wrapped line became its own short, ragged line).
  Added `#`-level heading support (the top `# Changelog` title rendered as literal text before),
  underlined section headings (`### Added`/`Fixed`/`Changed`) so they read as a distinct
  hierarchy from inline `**bold**` text rather than the same weight, and wrapped the content in
  an `Adw.Clamp` so prose stays a comfortable reading width if the window is resized wider.
- **Spreadsheet view** — columns had no minimum width and mostly showed truncated text
  ("Example Organiz…", "Example Awarding…"); each column now has a sensible per-column minimum
  (wider for Title/Organization, narrower for Date/Tags). Added a bottom border under the header
  row so it reads clearly as a header, and subtle alternating row shading for readability. The
  fill-handle (drag-to-copy) square previously overlapped the last character or two of cell
  text; cells now leave a small margin so the handle sits clear of the text.

### Added
- **New File**, **Open**, **Save As**, and **Preferences** in the header menu — the data file
  location was previously only changeable via "Choose Data File…" (an open-existing-file picker
  with no way to create a fresh file or explicitly re-save elsewhere). Preferences shows the
  current path and a "Change…" button that works for both switching to an existing file or
  starting a new one at a chosen location.
- Default data file location is now `~/Documents/Zerkalo/cv-elements.yaml` (previously
  `~/.local/share/skrizhal/cv-elements.yaml`) — Zerkalo's CV mode looks for the file there, and it
  keeps CV data next to the documents that reference it instead of tucked away in a data directory
  nobody browses.

### Fixed
- **A file with even one entry missing a required field (`category`/`title`) used to fail to load
  entirely**, dropping every other entry in the file into a read-only, unsaveable state. Parsing
  is now per-entry: a malformed entry is skipped (with a toast naming which key and why) while
  every other entry loads and stays fully editable. The malformed entry's raw YAML is preserved
  unchanged on every subsequent save, so nothing is silently deleted just because Skrizhal
  couldn't understand it — only a genuinely broken YAML file (bad syntax) still blocks loading
  entirely, same as before.
- **Old files using `type:` instead of `category:`** (the field's name before an early rename,
  documented in `plan.md`) now load correctly — `type` is accepted as an alias for `category`.
  This was the specific cause of "can't parse the file that was open previously" for any file
  written before that rename.
- Fixed a bug introduced partway through today's work, before it ever shipped: the per-entry
  parsing above initially used `from_value` on already-generically-parsed YAML, which is stricter
  about scalar types than parsing straight from text — an unquoted `date: 2020` (a YAML integer)
  was silently rejected instead of coerced to a string, which would have dropped any entry with a
  bare numeric date. Fixed by round-tripping each entry through YAML text before typed parsing,
  restoring the original lenient behavior. Added a regression test.

- Flatpak build was broken — `gtk4`/`libadwaita`/`glib` had drifted to versions (0.11.4/0.9.2/0.22.8)
  requiring rustc 1.92+, but the `org.gnome.Platform//50` runtime's `rust-stable` SDK extension
  only provides rustc 1.89. Flatpak has no way to pin an SDK extension to a different branch than
  the runtime, so the fix is downgrading to the same versions Zerkalo already uses successfully
  in this exact setup (`gtk4 = "0.7"`, `libadwaita = "0.5"`, `glib = "0.18"`) rather than chasing
  a newer runtime. Regenerated `packaging/cargo-sources.json` accordingly. No UI code changes were
  needed — it compiled clean against the older API on the first try.

---

## [0.3.0] "Full Ledger" — auto-generated keys, categories, field guide, spreadsheet view

### Changed
- **Split into a Cargo workspace** — `core/` (package `skrizhal-core`: schema, registry,
  validation, date handling, filtering, tags — no GTK dependency at all) and the root package
  (the GUI, depending on `skrizhal-core` + gtk4/libadwaita). Done ahead of Phase 3 (Zerkalo
  integration): Zerkalo pins `gtk4 = "0.7"`/`libadwaita = "0.5"`, incompatible with Skrizhal's
  `0.11.4`/`0.9.2` — a direct dependency on the old single-crate layout would have tried to
  compile two incompatible copies of the same GTK bindings into Zerkalo's binary. No user-visible
  change; internal only.
- **`type` renamed to `category` everywhere** — YAML field, UI label, and internal naming
  (`entry_type` → `category`, `TYPE_REGISTRY` → `CATEGORY_REGISTRY`). Category values are now
  canonical Title Case strings (`Ministry Position`) rather than kebab-case ids
  (`ministry-position`), so raw YAML reads naturally; lookup is case-insensitive.
- **Date split into Start/End** — the detail form now has a Date Type dropdown (Single Date /
  Date Range / Ongoing) plus separate Start Date and End Date fields (End hidden except in Range
  mode), instead of one raw range string. Storage format is unchanged — `date::split_date_string`/
  `join_date_string` convert between the form fields and the existing single stored string.

### Added
- **Auto-generated entry keys** — a new entry's Key follows `slugify(organization + title)` live
  as you type, until you edit Key directly (auto-follow stops permanently for that entry) or save
  it (a saved entry's key never silently changes again on further edits).
- **Live duplicate-key feedback** — the Key field shows an error state immediately for an empty
  or already-used key, on top of the existing hard block at Save.
- **Category suggestion popover and placeholder** — lists all registered categories in Title Case;
  the field also shows "Education, Employment, Awards, etc..." placeholder text once focused
  while empty (via the `AdwEntryRow`'s `GtkEditable` delegate, since the row has no
  `placeholder-text` property of its own).
- **Field Guide** — a startup popup (first run only, reachable afterward via the header menu)
  explaining what each field is for, with the most attention on Tags: the mechanism for filtering
  one CV-element database down into different CVs.
- **Status bar** — bottom bar with a Spreadsheet toggle (left) and a version button (right,
  `v0.3.0`, opens a Changelog window rendering `CHANGELOG.md`, embedded via `include_str!` so it's
  always available regardless of install method), matching the standard app design.
- **Spreadsheet view** — toggles the whole window into an editable grid (Key/Category/Title/
  Organization/Location/Date/Tags columns, one row per entry, sorted by key). Every fillable
  column (all but Key, where it'd just create duplicates) has a drag-to-fill handle: drag a cell's
  corner down or up over other rows and release to copy its value into all of them — handy for
  setting a batch of entries to the same category or tag in one motion.

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
