# Skrizhal — Feature Plan

A YAML editor/reader for CV elements (jobs, education, awards, publications, service, etc.),
each keyed by a citation-style key so Zerkalo can pull them into a CV document in "CV mode"
instead of the user hand-typing every CV from scratch (see `zerkalo/IWK-CV.typ` for the manual
pain point this replaces — hardcoded content gated by hand-toggled booleans like
`show-volunteer`).

Standalone app/repo for now; core logic written so it can be lifted into `zerkalo/src/` later
if the two merge.

---

## Format: Hayagriva-*shaped* YAML, not the `hayagriva` crate

Storage format borrows Hayagriva's YAML shape (top-level key → nested fields, one block per
entry) because it's git-friendly and already familiar from `.bib`/`.yml` reference files. It does
**not** go through the `hayagriva` Rust crate's parser.

Why not: `hayagriva::EntryType` (the crate Zerkalo already depends on for real bibliography
entries, `src/bibliography.rs`) is a closed enum — `Article`, `Book`, `Thesis`, `Misc`, etc.,
`#[non_exhaustive]` but with no `#[serde(other)]` catch-all. A `type: ministry-position` value
fails to deserialize through it. So Skrizhal's core crate parses CV YAML with plain `serde_yaml`
(or `serde_yaml_ng`, since `serde_yaml` itself is unmaintained) against its own schema, with an
open `type: String`.

This has no downside for the Typst side: rendering uses Typst's native `yaml()` function to read
the file directly into a Typst dictionary, never touching the `hayagriva` crate or Typst's
`#bibliography()`/`#cite()` machinery at all.

## Rendering: custom accessor, not citations

A CV entry is a block to render (job title, dates, bullet points), not an inline citation +
reference-list entry. Typst's CSL styles also have no concept of `ministry-position`. So:

- `#let cv-data = yaml("cv-elements.yaml")`
- `#cv-entry(<key>)` — renders one entry as a block, dispatching on `type` for layout
- `#cv-section(type: "employment", tag: "ministry")` — filters + renders a group

Per-type rendering rules live in Typst show-code, the same pattern `styles.rs` already uses for
citation styles (Rust holds template strings; Typst does the formatting) — just for CV blocks
instead of headings/bibliography.

### Editor trigger: `!` instead of `@`

Zerkalo's `@` popup (`editor_pane.rs:3281`) only fires when the character before `@` is *not*
alphanumeric/underscore (the `prev_is_word` check) — so it already tolerates being typed mid-text
without false-triggering on words containing `@`-like patterns. `!` has the same property and
almost never appears mid-word in real prose (exclamation marks are glued to the end of a word,
which the boundary check blocks), while sitting right next to `@` on the keyboard. Typing `!`
opens a popup searching title/organization/tags; selecting an entry replaces `!query` with
`#cv-entry(<key>)` — same intercept-and-replace mechanism as `do_bib_complete`
(`editor_pane.rs:5737`), just a different trigger character and expansion text. `&` was
considered and rejected: CV content routinely has organization names like "Barnes & Noble" or
"Procter & Gamble" with a space before `&`, which would false-trigger the popup.

---

## Schema (core crate)

```rust
struct CvEntry {
    key: String,                       // "hope-united-2025"
    entry_type: String,                // namespaced, open string — see registry below
    title: String,
    organization: Option<String>,
    location: Option<String>,
    date: String,                      // Hayagriva-style range: "2025-09/2026-04", "2023/", "2020"
    tags: Vec<String>,                 // visibility/filtering: [ministry, academic, current]
    description: Option<OneOrMany<String>>, // bullet points, or single string
    #[serde(flatten)]
    extra: BTreeMap<String, serde_yaml::Value>, // type-specific fields (degree, doi, amount, ...)
}
```

`extra` is a flatten catch-all rather than a giant struct of `Option<T>` fields for every possible
type-specific key — new entry types don't need a Rust code change, just a registry entry (below)
for soft validation and GUI form hints.

### Type registry

A const table (mirrors `styles.rs`'s `STYLES` array): `(type_id, display_name, recommended_fields)`.
Drives soft-validation warnings (missing recommended field — never a hard error, YAML stays
flexible) and later the GUI's per-type form layout.

Initial namespace: `education`, `employment`, `ministry-position`, `publication`, `presentation`,
`award`, `service`, `committee-appointment`, `language-skill`, `certification`, `volunteer`,
`project`.

### Example entry

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

mdiv-2024:
  type: education
  title: Master of Divinity (in progress)
  organization: Atlantic School of Theology
  date: 2023/
  tags: [academic, ministry]
  degree: MDiv
  field-of-study: Divinity
```

One file, `cv-elements.yaml`, many entries — same shape as a `.bib` file today. A directory-per-entry
layout was considered and rejected for v1 (nicer diffs for single-entry edits, but needs new
directory-scanning logic Zerkalo's existing parser pattern doesn't have).

---

## Build order

### Phase 1 — Core crate, no UI
1. `CvEntry` struct + `serde_yaml` round-trip read/write
2. Type registry table + soft-validation (missing recommended field, unknown type, duplicate key)
3. Lightweight date-range parsing for sorting (`2025-09/2026-04` style) — good-enough, not full
   ISO 8601 arithmetic; falls back gracefully on exotic values
4. Tag/type/search filtering over an in-memory entry list
5. Unit tests mirroring `bibliography.rs`'s style (round-trip, multi-value description, duplicate
   key detection, missing-field warnings)
6. `cargo test` green — no GTK dependency yet

### Phase 2 — GTK4/libadwaita app
7. Window: sidebar entry list (search/tag/type filter) + detail form pane, `Paned` with the usual
   sidebar-toggle-at-headerbar-start convention
8. Per-type form fields driven by the type registry; raw-YAML toggle for power editing (Simple
   Mode-style: hidden by default, always reachable)
9. Add/duplicate/delete entry; tag rename/merge across entries
10. Config at `~/.config/skrizhal/`, data file path configurable (not hardcoded to one location)
11. Flatpak packaging — deferred until Cal wants to distribute it

### Phase 3 — Zerkalo integration (separate session, cross-repo)
12. Decide how the core crate reaches Zerkalo (path dependency during development; full lift into
    `zerkalo/src/` if/when the apps merge)
13. `cv-entry.typ` / `cv-section.typ` Typst helper library + a per-document "CV mode" setting
    pointing at a `cv-elements.yaml` path (same shape as the existing bib-file-path setting)
14. Reuse `bib_popup.rs`/`citation_panel.rs`/`ref_manager.rs` autocomplete UX, but searching
    title/organization/tags instead of author/year, inserting `#cv-entry(<key>)`
15. Extend `rename_key_in_text`-style logic to the `#cv-entry(<key>)` pattern

---

## Open questions deferred to Phase 3 planning

- Exact CV-mode toggle/setting UX in Zerkalo (new template marker vs. per-document config field)
- Whether `cv-section` needs Typst-side date-sort logic or whether Skrizhal pre-sorts before
  Zerkalo ever calls it
- Multi-CV-database support (one global `cv-elements.yaml` vs. per-document override) — not
  needed for v1, single file assumed
