# Skrizhal

A YAML editor/reader for CV elements — jobs, education, awards, publications, service, and more —
each keyed by a citation-style key so [Zerkalo](https://github.com/calstfrancis/zerkalo) can pull
them into a CV document on request, instead of the user hand-typing (and hand-toggling) every CV
from scratch.

Two-crate Cargo workspace: `core/` (`skrizhal-core`, no GTK dependency) holds everything Zerkalo
would eventually depend on; the root crate is the GTK4/libadwaita editor app built on top of it.
See [`plan.md`](plan.md) for the full architecture and phased build plan.

---

## Status

**Phase 3 complete; Phase 4 complete except live preview.** The editor is fully usable — browse,
add, edit, duplicate and delete entries, with edits autosaving as you type. Zerkalo's CV mode
renders individual entries (`#cv-entry`), filtered groups (`#cv-section`), and whole named
profiles (`#cv-profile`). On top of that: CV Profiles, BibTeX import, database health checks,
and git-backed file history.

Still open: ORCID and LinkedIn import (ORCID needs network access the flatpak deliberately
doesn't have), and an embedded Typst preview inside Skrizhal — see `plan.md`'s item 31 for why
that one is parked on a packaging question rather than a design one.

Run it with `cargo run`. Data defaults to `~/Documents/Zerkalo/cv-elements.yaml` (where Zerkalo's
CV mode looks for it); override via `~/.config/skrizhal/config.toml`, Preferences, or "Open…".

## Core crate (`core/src/`, package `skrizhal-core`)

No GTK/libadwaita dependency — just `serde`/`serde_yaml_ng`/`thiserror` — so it can be depended on
(e.g. by Zerkalo) without dragging in a GUI toolkit.

- **Schema** (`entry.rs`) — `CvEntry` holds the common fields every CV element shares (key,
  category, title, organization, location, date, tags, description) plus an open `extra` map for
  category-specific fields (degree, DOI, award amount, …), so new categories don't need a Rust
  code change.
- **Category registry** (`registry.rs`) — a namespaced table of CV categories, canonical Title
  Case names (`Education`, `Employment`, `Ministry Position`, `Publication`, `Presentation`,
  `Award`, `Service`, `Committee Appointment`, `Language Skill`, `Certification`, `Volunteer`,
  `Project`) with recommended fields per category; lookup is case-insensitive.
- **Soft validation** (`validate.rs`) — non-blocking warnings for unknown categories, missing
  recommended fields, and duplicate top-level keys in the source YAML.
- **Date handling** (`date.rs`) — parses Hayagriva-style date ranges (`2025-09/2026-04`, `2023/`
  for ongoing) into a sortable key, plus `DateMode`/`split_date_string`/`join_date_string` to
  decompose/recompose a range into Single/Range/Ongoing + start/end for the editor UI.
- **Filtering** (`filter.rs`) — filter a list of entries by category, tag, and free-text search.
- **Sorting** (`sort.rs`) — `SortMode` (newest-first / title / category) over a list of entries.
- **Profiles** (`profile.rs`) — named, ordered CV section sets with filter + explicit
  include/exclude rules, and `resolve_section`/`resolve_profile` to apply them.
- **Health** (`health.rs`) — file-level checks (near-duplicates, likely tag typos, untagged
  entries, unknown categories) that per-entry validation can't see.
- **Import** (`import.rs`) — hand-rolled BibTeX parsing onto `CvEntry`.
- **Tags** (`tags.rs`) — usage counts, rename-with-merge across the whole entry set.

## Editor app (root `src/main.rs`, `src/ui/`)

- Sidebar: search + category/tag filters over an `adw::ActionRow` list, plus a sort selector
  (newest-first by default, or by title/category) that persists across launches.
- Edits autosave — debounced while typing, and flushed immediately on selection change or window
  close. Save (Ctrl+S) stays as an explicit commit, and is the only thing that commits Raw YAML.
- Detail pane: structured form for common fields, a Date Type dropdown (Single Date / Date Range /
  Ongoing) with Start/End fields, a category suggestion popover with placeholder text, a dynamic
  add/remove list for category-specific ("Additional Fields") entries, and a "Raw YAML" toggle to
  edit the entry's serialized block directly.
- Key auto-generates from Organization + Title as you type a new entry, until you edit it directly
  or reload it — live error feedback, plus a hard block in both save paths, prevent duplicates.
- Add seeds the new entry's category/tag from whatever filters are active, so it's actually
  visible in the current view instead of vanishing into a filtered-out state.
- Duplicate/Delete per row; Delete confirms first.
- Manage Tags dialog: rename a tag everywhere at once; renaming onto an existing tag name merges
  the two.
- CV Profiles dialog: build named, ordered CV section sets, each section showing a live count of
  how many entries currently match its rules. Zerkalo renders one with `#cv-profile("name")`.
- Database Health dialog: near-duplicate entries, likely tag typos, untagged entries, and
  unrecognized categories, each linking to the entry it's about.
- File History dialog: git snapshots of the data file — browse, snapshot, and restore (the
  current state is committed first, so restoring never loses work). Auto-snapshots on close.
- Import from BibTeX, additive and confirmed before anything is written.
- Description bullets are individual rows with reorder, remove, and a live character count.
- Field Guide: shown on first run, reachable afterward via the header menu.
- Status bar: a version button that opens the changelog.
- Data file location is configurable (`~/.config/skrizhal/config.toml` or "Choose Data File…"),
  and a data file that fails to parse blocks saving rather than risking a silent overwrite.
- A file monitor raises a banner when something else changes the data file on disk, offering
  Reload rather than letting the next save clobber it.

## Format

CV elements live in a single YAML file, `cv-elements.yaml`, one block per entry — the same shape
as a `.bib` file, git-diff-friendly by construction:

```yaml
hope-united-2025:
  category: Ministry Position
  title: Student Minister
  organization: Hope United Church
  location: Halifax, NS
  date: 2025-09/2026-04
  tags: [ministry, current]
  description:
    - Preaching and worship leadership on a rotating basis
    - Liturgical preparation for seasonal services
```

This intentionally borrows Hayagriva's YAML *shape*, not the `hayagriva` crate's parser — see
`plan.md` for why (its `EntryType` enum is closed and would reject custom categories like
`Ministry Position`).

## Installing

```bash
flatpak remote-add --user calstfrancis \
  https://calstfrancis.github.io/flatpak/calstfrancis.flatpakrepo
flatpak install calstfrancis io.github.calstfrancis.Skrizhal
```

## Building

```bash
cargo test --workspace
```

If you hit `error: linker `clang` not found`, see `.cargo/config.toml` — this system's `rustc`
defaults to `clang`, but only `gcc` is installed; the config pins the linker to `gcc`.

To build the flatpak: `flatpak-builder` is run only by `dev-build.sh` / `publish-flatpak.sh`, not
directly — see `packaging/io.github.calstfrancis.Skrizhal.yml`.

## License

MIT
