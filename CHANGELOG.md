# Changelog

All notable changes to Skrizhal are recorded here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [0.4.0-dev2] — CV profiles, autosave, importing, history, and health checks

### Added — CV Profiles
- **CV Profiles** — a named, ordered set of CV sections, each with a heading, category/tag
  filters, and explicit include/exclude lists. This is the feature tags alone couldn't provide:
  a filter has no ordering and no way to express the one-off exception ("drop that job from
  *this* CV only") without inventing a single-use tag. Edited in a new "CV Profiles" dialog,
  where each section shows a live count of how many entries currently match it, updating as the
  rules are typed. Stored under a reserved `_profiles` key in the same `cv-elements.yaml`.
- **`#cv-profile("name")` in Zerkalo** renders a whole profile — every section, in order, with
  its heading — replacing a hand-assembled run of `#cv-section` calls. Needed no new plumbing:
  it reads the same `skrizhal-cv-data` sys.input the entries already travel through.
- **Optional `order` field** on an entry, pinning it to the top of its rendered section.
  Opt-in — entries without it stay chronological, which is still the right default for most.

### Added — everything else
- **Autosave for the detail pane.** Field edits commit on their own — debounced ~600ms after
  typing stops, and immediately on selecting another entry or closing the window. Previously
  every other mutation (delete, duplicate, tag rename) wrote to disk instantly while form edits
  needed an explicit Save, and navigating away discarded them silently. Undo is the safety net,
  and coalesces an entire editing session on one entry into a single step. Save remains as an
  explicit commit (now **Ctrl+S**) and is still the only thing that commits Raw YAML — debouncing
  a parse of half-typed YAML would fail on nearly every keystroke.
- **External-change detection.** A file monitor on the data file raises a banner when something
  else modifies it, offering Reload. Zerkalo already re-read the CV data on every compile; this
  closes the other direction, where a save here could silently clobber an edit made elsewhere.
- **File History** — git-backed snapshots of the data file: browse past versions, snapshot on
  demand, restore any of them (committing the current state first, so restoring can't lose
  work), and an automatic snapshot on close. Offers to `git init` if the file isn't in a repo
  yet. Shells out via `flatpak-spawn --host git`, the same approach Retseptura uses, so it
  needs no new Rust dependency — only `--talk-name=org.freedesktop.Flatpak` in the manifest.
- **Import from BibTeX** — publications and presentations from a `.bib` file, mapped onto the
  right categories, with `journal`/`booktitle` folded into the registry's recommended `venue`
  field. Always additive and always confirmed first, with imported keys made unique against
  what's already there. Parser is hand-rolled in `skrizhal-core` to avoid a vendored dependency.
- **Database Health** — file-level checks that per-entry validation structurally can't do:
  near-duplicate entries, tags used exactly once that closely resemble a common one, untagged
  entries, and unrecognized categories. Tag typos are the reason this exists — a mistyped tag
  silently drops an entry from every CV that filters on the real one, with no error anywhere.
- **Description bullets are now real rows** — add, remove, reorder, and a live character count
  per bullet, instead of one text box split on newlines.
- **Category-driven form fields.** Choosing a category offers that category's recommended
  fields as ready-made rows (pick "Education", get a `degree` row). The registry has driven
  validation warnings since it was written; this puts the same knowledge in front of the user
  *before* they get warned. Unfilled rows are dropped on save rather than written out empty.
- **Sort selector in the sidebar** (Newest First / Title / Category), persisted to config.
- `skrizhal-core`: new `sort`, `profile`, `health`, and `import` modules.

### Fixed
- **The sidebar sorted alphabetically by title while the README documented it as most-recent-
  first — and `core::sort_entries_by_date_desc`, written for exactly this, was never called by
  the GUI at all.** Newest First is now the default; alphabetical remains available in the new
  sort selector, since it's genuinely better for hunting an entry down by name.
- **Zerkalo's `cv-section` would have rendered the `_profiles` block as if it were a CV entry**,
  since it iterated every top-level key. It now skips reserved (`_`-prefixed) keys.
- Sidebar row Duplicate/Delete read their entry key from the row at click time instead of
  capturing it at build time — autosave can rename an entry (auto-generated keys follow
  Title/Organization), which would have left those buttons pointing at a key that no longer
  exists.
- An unknown `_`-prefixed key in the data file is now preserved through a save instead of being
  reported as a parse failure, so a file written by a newer Skrizhal survives an older one.

### Also in this dev cycle
- **Multi-select tag filter** — the sidebar's tag filter is now a checkbox popover instead of a
  single-choice dropdown, matching any of the selected tags (`core::FilterOptions.tags` replaces
  the old single `tag` field).
- **Syntax-highlighted Raw YAML view**, via GtkSourceView5 (new dependency, already present in
  the pinned GNOME 50 runtime/SDK — no flatpak manifest changes needed), with line numbers and a
  style scheme that follows the system light/dark color scheme live.
  - Adding the dependency, remember to update `packaging/cargo-sources.json` if `Cargo.lock` ever
    changes again (`python3 flatpak-cargo-generator.py Cargo.lock -o packaging/cargo-sources.json`).
- **Inline per-field validation warnings** — Category, Organization, Location, Date, and Tags now
  show a warning icon with a tooltip directly on the offending row, in addition to the existing
  summary banner.
- **Recently-used tags quick-pick** on the Tags field, mirroring the existing Category suggestion
  popover, sourced from actual tag usage counts across the file.
- **"Add Entry" button on the empty-state placeholder**, so a first-time or fully-filtered view
  isn't a dead end.
- **Focus Mode**: the sidebar now collapses via an animated slide (`GtkRevealer`) instead of an
  instant show/hide, and the pane position is restored (not just re-shown) when toggled back on.
- **Sidebar/detail split position now persists** across launches (debounced, matching Zerkalo's
  pane-position idiom).
- Real keyboard shortcuts: **Ctrl+N** (New File), **Ctrl+O** (Open), **Ctrl+Shift+S** (Save As),
  **Ctrl+F** (focus search) — shown as shortcut labels in the header menu.

### Changed
- **Add Entry button** is now an accent-colored (`suggested-action`) button instead of a plain
  bordered icon button, so it reads as the primary action in the header bar instead of blending
  in as a tiny `+`.
- **Date fields** (Date Type / Start Date / End Date) are now part of the same "Entry"
  `PreferencesGroup` as Title/Organization/Location/Tags instead of a separate "Date" group below
  it — removes an unnecessary visual break between fields that belong together.
- **Window default height** increased from 650 to 760px so the full form (including the Date
  rows) fits without scrolling on a freshly opened window.
- **Tab now selects the destination field's full text** when moving between entry fields (Key,
  Category, Title, Organization, Location, Start/End Date, Tags), so typing immediately replaces
  the value instead of requiring a manual select-all first.
- **Sidebar row Delete** is now a direct destructive-styled button on the row instead of buried
  one level inside the kebab menu — cuts a click for the most common destructive action. Duplicate
  remains in the kebab menu.

---

## [0.3.1] "Clear Ledger" — Undo/redo, New File/Open/Save As/Preferences, spreadsheet mode removed

### Removed
- **Spreadsheet mode** — the bulk-edit grid view (toggle, `ui/spreadsheet.rs`, and every
  cell/fill-drag/keyboard-nav path built around it) has been removed. It never fully replaced
  what it was meant to (bulk data entry), and its "add row" flow caused a freeze — a
  `RefCell` re-entrancy where `sidebar::refresh_list` held a live borrow across a `ListBox`
  rebuild loop that GTK's own `row-selected` signal, fired mid-rebuild, tried to re-borrow.
  Rather than keep patching a view nobody was getting value from, it's gone; entries are added
  and edited one at a time through the sidebar + detail pane.

### Added
- **Undo/redo** — Ctrl+Z / Ctrl+Shift+Z (and header-bar buttons) revert or replay the last 50
  changes, covering every mutation: add, edit, duplicate, delete, tag rename. The delete
  confirmation dialog now says "You can undo this with Ctrl+Z" instead of "This can't be
  undone."

### Changed
- **Changelog window** — the Markdown-to-Pango converter now folds a bullet's hard-wrapped
  source lines (CHANGELOG.md wraps prose at ~90 columns for editor readability) into one
  paragraph that reflows to the dialog's actual width, instead of keeping the source file's
  arbitrary line breaks (previously every wrapped line became its own short, ragged line).
  Added `#`-level heading support (the top `# Changelog` title rendered as literal text before),
  underlined section headings (`### Added`/`Fixed`/`Changed`) so they read as a distinct
  hierarchy from inline `**bold**` text rather than the same weight, and wrapped the content in
  an `Adw.Clamp` so prose stays a comfortable reading width if the window is resized wider.

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
