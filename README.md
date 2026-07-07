# Skrizhal

A YAML editor/reader for CV elements — jobs, education, awards, publications, service, and more —
each keyed by a citation-style key so [Zerkalo](https://github.com/calstfrancis/zerkalo) can pull
them into a CV document on request, instead of the user hand-typing (and hand-toggling) every CV
from scratch.

Standalone Rust crate/app for now; written so its core logic can be lifted into `zerkalo/src/`
later if the two projects merge. See [`plan.md`](plan.md) for the full architecture and phased
build plan.

---

## Status

**Phase 1 complete: core crate, no UI yet.** `skrizhal` is currently a library (`src/lib.rs`)
with no GTK application on top — that's Phase 2. Phase 3 is Zerkalo integration.

## What it does today

- **Schema** (`src/entry.rs`) — `CvEntry` holds the common fields every CV element shares (key,
  type, title, organization, location, date, tags, description) plus an open `extra` map for
  type-specific fields (degree, DOI, award amount, …), so new entry types don't need a Rust code
  change.
- **Type registry** (`src/registry.rs`) — a namespaced table of CV entry types (`education`,
  `employment`, `ministry-position`, `publication`, `presentation`, `award`, `service`,
  `committee-appointment`, `language-skill`, `certification`, `volunteer`, `project`) with
  recommended fields per type.
- **Soft validation** (`src/validate.rs`) — non-blocking warnings for unknown types, missing
  recommended fields, and duplicate top-level keys in the source YAML.
- **Date sorting** (`src/date.rs`) — parses Hayagriva-style date ranges (`2025-09/2026-04`,
  `2023/` for ongoing) into a sortable key so entries can be listed most-recent-first.
- **Filtering** (`src/filter.rs`) — filter a list of entries by type, tag, and free-text search.

## Format

CV elements live in a single YAML file, `cv-elements.yaml`, one block per entry — the same shape
as a `.bib` file, git-diff-friendly by construction:

```yaml
hope-united-2025:
  type: ministry-position
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
`plan.md` for why (its `EntryType` enum is closed and would reject custom types like
`ministry-position`).

## Building

```bash
cargo test
```

If you hit `error: linker `clang` not found`, see `.cargo/config.toml` — this system's `rustc`
defaults to `clang`, but only `gcc` is installed; the config pins the linker to `gcc`.

## License

MIT
