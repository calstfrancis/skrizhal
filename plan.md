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
`#[non_exhaustive]` but with no `#[serde(other)]` catch-all. A `category: Ministry Position` value
fails to deserialize through it. So Skrizhal's core crate parses CV YAML with plain `serde_yaml`
(or `serde_yaml_ng`, since `serde_yaml` itself is unmaintained) against its own schema, with an
open `category: String`.

> **Naming update (Phase 2):** the field was originally called `type` throughout this doc — it's
> `category` now (clearer for end users, and avoids double-meaning with Rust's own `type` keyword).
> Registry values are also canonical Title Case strings (`Ministry Position`) rather than
> kebab-case ids (`ministry-position`), chosen so raw YAML reads naturally without a separate
> id/display-name split. `lookup()` is case-insensitive so a hand-typed category still matches.

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

Re-planned after Phase 2 shipped, once an engineer verified the actual current Zerkalo code
rather than trusting this doc's original (now-stale) assumptions. Two findings changed the plan:

- **Version conflict, not just "decide how to depend"**: Zerkalo pins `gtk4 = "0.7"` /
  `libadwaita = "0.5"` / `glib = "0.18"`; Skrizhal's GUI uses `gtk4 = "0.11.4"` /
  `libadwaita = "0.9.2"` / `glib = "0.22.8"`. These aren't semver-compatible — Zerkalo depending
  on the `skrizhal` package as it stood (a single crate with both the core logic and the GTK app
  in one `Cargo.toml`) would try to compile two incompatible versions of the same GTK bindings
  into one binary. Fixed by splitting into a workspace: `core/` (package `skrizhal-core`, no GTK
  deps at all — just `serde`/`serde_yaml_ng`/`thiserror`) and the root package `skrizhal` (the
  GUI, depending on `skrizhal-core` + gtk4/libadwaita). Zerkalo depends only on `skrizhal-core`,
  pinned to a git tag (`skrizhal-core = { git = "https://github.com/calstfrancis/skrizhal", tag =
  "vX.Y.Z" }`) — reproducible, and `flatpak-cargo-generator.py` already supports vendoring pinned
  git dependencies for Zerkalo's own flatpak build.
- **CV templates aren't real `.typ` files**: only the four non-CV templates (blank/essay/
  journal-thesis/theological-journal) are on-disk files loaded via `include_str!`
  (`zerkalo/templates/`). "CV — Modern/Academic/Classic/Two-Column" are procedurally generated as
  Rust string-building code in `zerkalo/src/ui/template_dialog.rs`. So `#cv-entry`/`#cv-section`
  shouldn't be wired into that generator — instead they get their own new
  `zerkalo/templates/cv-helpers.typ` (a genuine file, `include_str!`'d), spliced into the preamble
  the same way `styles.rs` already splices citation-style blocks, independent of which template —
  or no template — the user started from.

Everything else the original plan assumed still holds: the `@` popup mechanism
(`editor_pane.rs:3281`, `prev_is_word` check ~3309–3318), `do_bib_complete`/
`insert_completion_text` (`editor_pane.rs:5737`/`5714`), and `rename_key_in_bib_file`/
`rename_key_in_text` (`bibliography.rs:152`/`167`) are all still exactly where they were, and are
still the right patterns to mirror. `bib_path` resolution — per-project `.zerkalo/config.toml`
overrides global `~/.config/zerkalo/config.toml` (`app_window.rs:115`) — is the exact pattern
`cv_elements_path` should follow.

**Split into two sub-phases** rather than one pass, since it's a lot of surface area:

#### Phase 3a — Foundation (no editor UX yet) — ✅ done

12. Workspace split: `skrizhal-core` (lib, no GTK) + `skrizhal` (GUI bin, depends on
    `skrizhal-core`) — a Skrizhal-repo-only change, doesn't touch Zerkalo, done first regardless
    of when the rest of Phase 3 happens.
13. `cv_elements_path` in Zerkalo's `Config`/`ProjectConfig` (`src/config.rs`), resolved the same
    way `bib_path` is: `effective_cv_elements` in `app_window.rs` (`proj_cfg.cv_elements_path.or
    (config.cv_elements_path)`), right next to the existing `effective_bib`. **This resolves the
    "what turns CV mode on" question below**: a project/document is in CV mode exactly when
    `cv_elements_path` resolves to `Some(path)` — no separate boolean flag, no new template
    marker. No Settings-dialog row yet (per-project `.zerkalo/config.toml` only, for now) — the
    global config field round-trips but isn't user-editable through the UI yet.
14. `zerkalo/templates/cv-helpers.typ` — `#cv-entry(key)` / `#cv-section(category:, tag:)`.
    **Corrected from the original sketch in two ways, both discovered while actually building
    this, not while planning it:**
    - **String keys, not labels.** `#cv-entry(<key>)`'s label syntax (`<...>`) was always just
      illustrative shorthand from the original brainstorm — the actual function does a plain
      dictionary lookup (`data.at(key)`), which needs a string. The real call is
      `#cv-entry("hope-united-2025")`. Worth remembering for Phase 3b: the `!` popup must insert
      a quoted string, not a label.
    - **`sys.inputs`, not `yaml(path)`.** The plan assumed `yaml(cv_elements_path)` could read the
      data file directly by path. It can't in general: Typst sandboxes file access to the
      compilation root, and `cv_elements_path` is frequently *outside* it (Skrizhal's own default
      is `~/.local/share/skrizhal/cv-elements.yaml`, never inside whatever folder a Zerkalo
      document lives in). Fixed by reading the real file's content in Rust and passing it through
      as a `sys.inputs` string (`skrizhal-cv-data`), decoded Typst-side via `yaml(bytes(...))` —
      sidesteps path resolution entirely. `cv-helpers.typ`'s own presence is a separate, solved
      problem: it's genuinely static, so it's injected as a virtual file (see below) rather than
      needing `sys.inputs` at all.
    - Function bodies use `entry.at("field", default: ...)` throughout rather than `"field" in
      entry` + direct access, so entries missing optional fields (no `location`, no
      `description`, etc.) don't error.
15. **Wiring** (`app_window.rs`, `preview_pane.rs`, `compiler.rs`): no new `compiler.rs` API and
    no file-splicing of the user's actual document. `PreviewPane` already had exactly the right
    extensibility point for this — `set_buffer_snapshot(path, text)`, originally built for
    unsaved-editor-content overrides during compile, is really just a generic "when compiling,
    pretend this path contains this text" hook, and needed zero changes to reuse: CV mode calls it
    once with `project_root.join("cv-helpers.typ")` → the embedded file's content
    (`include_str!`), so `#import "cv-helpers.typ": ...` resolves without a real file ever
    existing on disk. A new parallel `extra_sys_inputs` field + `set_sys_input(key, value)` setter
    (same shape, new — `sys_inputs` wasn't previously settable from outside `PreviewPane`) carries
    `skrizhal-cv-data`. Both are merged in at every compile call site in `preview_pane.rs`.
    **Known gap, not yet covered**: `export_dialog.rs`, `library_window.rs`, and
    `template_dialog.rs`'s compile call sites don't get the CV overrides yet, so PDF export of a
    CV-mode document won't yet resolve `#cv-entry`/`#cv-section` — only the live preview path
    does. Same file-shape fix needed there before Phase 3b ships anything user-facing. Also: the
    `skrizhal-cv-data` sys.input is set once (when the window opens), not re-read if the user
    edits entries in Skrizhal while Zerkalo is open — fine for proving the foundation, not fine
    long-term.
16. Milestone (✅ verified): `compiler::tests::compile_cv_entry_and_section_with_skrizhal_helpers`
    in `zerkalo/src/compiler.rs` — a document doing `#import "cv-helpers.typ": cv-entry,
    cv-section`, `#cv-entry("hope-united-2025")`, `#cv-section(category: "Education")`, and a
    deliberately-unknown key (renders "Unknown CV entry" in red rather than failing the whole
    compile) — compiles to a valid PDF using the exact same override + sys.inputs mechanism
    `app_window.rs` wires up for the real app. 156/156 tests pass.

#### Phase 3b — Editor integration

**3a's known gaps closed first, as planned:**
- PDF export (`export_dialog.rs`) now gets the same CV overrides as the live preview —
  `cv_mode_compile_extras` (moved into a new `src/cv_mode.rs`, alongside `CV_HELPERS_TYPST`)
  computes them once per export. `library_window.rs`'s single-document "Save As PDF" is a
  documented remaining gap (it has no `Config` access and can target any document in the
  library, not just the active one — needs its own per-document config resolution, deferred as
  low-traffic). `template_dialog.rs`'s two compile sites were never a gap — they render static
  preview-gallery thumbnails, never real user content.
- Live preview no longer caches `skrizhal-cv-data` at window-open: `PreviewPane` now stores just
  the *path* (`set_cv_elements_path`) and re-reads it fresh on every compile
  (`cv_data_sys_input`), so edits made in Skrizhal while Zerkalo is open show up on the next
  recompile without restarting.
- `rename_cv_entry_key_in_text` (`src/cv_mode.rs`) exists and is tested (4 new tests, 160/160
  total passing) — mirrors `bibliography::rename_key_in_text`'s pattern but kept in its own
  function rather than folded into the same regex: a bib-key rename and a CV-entry-key rename are
  triggered by different UI actions on independent key namespaces, so keeping them separate means
  renaming one can never accidentally touch the other's references. **Not wired to any UI trigger
  yet** — there's no "rename this CV entry" action in Zerkalo until item 19 exists to trigger it.

**Blocked on a real decision, not a technical unknown:** items 17 and 19 below both need Zerkalo
to depend on `skrizhal-core` as an actual Rust crate (to search/list/parse entries in the popup
and panel) — the pure-Typst trick that made 3a possible doesn't apply once Rust code needs to
read entries itself. That dependency can only resolve from a **pushed, tagged** git ref. Skrizhal
is tagged only at `v0.2.0`; all of the category/key-autogen/date-split/spreadsheet work and the
workspace split itself are uncommitted, and nothing's been pushed. Needs Cal's go-ahead to
commit + tag (e.g. `v0.3.0`) + push before either item can start.

17. `!` popup mirroring `@`'s mechanism (same `prev_is_word` boundary check, same
    intercept-and-replace via `insert_completion_text`), searching Skrizhal entries by
    title/organization/tags instead of author/year — inserts `#cv-entry("key")` (a **string**, not
    a label — see item 14's correction above; this bit the original sketch too)
18. ~~`rename_key_in_text`-style support~~ — done above, ahead of schedule (it turned out to need
    no Rust dependency, unlike 17/19, so there was no reason to wait).
19. **The citation panel itself switches modes in CV mode — not a separate/additional panel.**
    Confirmed the actual structure: `citation_panel.rs:26-116` builds its own header row (title
    Label "Citations", a `bib_name_label` showing the connected filename, and a `choose_btn` for
    picking a different file) above a `SearchEntry` + `ListBox` of entries, mounted directly in
    the left sidebar (`app_window.rs:3752-3762` — there's no tab/section switcher on the left
    side to add a second section to; the right sidebar's `Notebook` for Plan/Notes is a separate
    thing). `citation_panel.rs` currently has zero mode concept — it only ever renders
    `Vec<BibEntry>` fed from `bibliography::load_bib(&bib_path)`. Making it mode-aware:
    - Title label: "Citations" → "CV Elements" when `cv_elements_path` is active for the document
    - `bib_name_label`: shows the cv-elements filename instead of the bib filename
    - `choose_btn`: picks a `cv-elements.yaml` instead of a `.bib`/`.yml`
    - Entry list: needs a small shared abstraction (a two-variant enum or a trait covering
      "displayable key/title/subtitle") so the same `SearchEntry`+`ListBox` code renders either
      `BibEntry` or `skrizhal_core::CvEntry` — search matches title/organization/tags instead of
      author/year for CV entries
    - Click-to-insert emits `#cv-entry("key")` instead of `@key`
    This is the same swap the `!`/`@` popup makes (item 17), just for the sidebar browse panel
    instead of the inline autocomplete — both read from whichever path won the precedence rule
    in item 13.

---

## Open questions deferred to Phase 3b planning

- Multi-CV-database support (one global `cv-elements.yaml` vs. per-document override) — not
  needed for v1, single file assumed; `cv_elements_path`'s global/per-project override (item 13)
  already covers "different CV database per project" if that turns out to be wanted sooner
- No Settings-dialog row for `cv_elements_path` yet (item 13) — worth adding once there's a UI
  reason to (e.g. once the citation-panel mode-swap in item 19 gives users something to point it
  at interactively, via `choose_btn`, rather than hand-editing `.zerkalo/config.toml`)

(Resolved while building 3a: `cv-section`'s date-sort — it sorts Typst-side in `cv-helpers.typ`
directly, no Skrizhal pre-sorting needed, since the function already has all the entries' raw
date strings in hand.)
