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

**Phase 2 complete, Phase 3a underway.** The editor app is fully usable — browse, add, edit,
duplicate, and delete CV entries; toggle to raw YAML per entry; manage tags; bulk-edit via a
spreadsheet view. The workspace was just split into `skrizhal-core` (lib, no GTK) + `skrizhal`
(GUI bin) specifically so Zerkalo can depend on the core crate without pulling in a second,
incompatible copy of GTK4/libadwaita. Next: `cv_elements_path` config + a `cv-helpers.typ` Typst
helper in Zerkalo (Phase 3a), then `!`-triggered autocomplete in the editor (Phase 3b).

Run it with `cargo run`. Data defaults to `~/.local/share/skrizhal/cv-elements.yaml`; override via
`~/.config/skrizhal/config.toml` or the in-app "Choose Data File…" action.

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
- **Tags** (`tags.rs`) — usage counts, rename-with-merge across the whole entry set.

## Editor app (root `src/main.rs`, `src/ui/`)

- Sidebar: search + category/tag filters over an `adw::ActionRow` list, most-recent-first.
- Detail pane: structured form for common fields, a Date Type dropdown (Single Date / Date Range /
  Ongoing) with Start/End fields, a category suggestion popover with placeholder text, a dynamic
  add/remove list for category-specific ("Additional Fields") entries, and a "Raw YAML" toggle to
  edit the entry's serialized block directly.
- Key auto-generates from Organization + Title as you type a new entry, until you edit it directly
  or save — live error feedback (and a hard block at Save) prevent duplicates.
- Add seeds the new entry's category/tag from whatever filters are active, so it's actually
  visible in the current view instead of vanishing into a filtered-out state.
- Duplicate/Delete per row; Delete confirms first.
- Manage Tags dialog: rename a tag everywhere at once; renaming onto an existing tag name merges
  the two.
- Field Guide: shown on first run, reachable afterward via the header menu.
- Status bar: Spreadsheet toggle (a full editable grid — Key/Category/Title/Organization/
  Location/Date/Tags — with drag-to-fill on every column but Key) and a version button that opens
  the changelog.
- Data file location is configurable (`~/.config/skrizhal/config.toml` or "Choose Data File…"),
  and a data file that fails to parse blocks saving rather than risking a silent overwrite.

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
