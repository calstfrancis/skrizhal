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

---

# Phase 4 — From element database to CV builder

Phase 1–3 built the thing this document set out to build: a database of CV elements, and a way
for Zerkalo to pull them into a document. Reviewing the result against the pain point in this
plan's opening paragraph — `IWK-CV.typ`'s hardcoded content gated by hand-toggled booleans like
`show-volunteer` — turned up a gap worth naming explicitly, because it shapes everything below:

**Tags plus `#cv-section(tag: ...)` are a better version of those booleans, not a different
thing.** The toggles moved from the document into the database, which is a real improvement (one
source of truth, no copy-paste between CVs), but the *shape* of the problem is unchanged. There's
still no way to say "this CV, in this order, with these entries and not those" and give it a name
you can come back to in six months. A filter is not a document plan: it has no ordering, no
explicit include/exclude for the one-off exception, and nothing to reopen next time.

So Phase 4's centre of gravity is the **CV Profile** (items 20–21). Everything else is either
groundwork for it, or a fix for something that's plainly wrong today.

## 4a — Fixes for things that are already wrong — ✅ done

These are small, independently shippable, and don't depend on any of the design work below.

20. **The sidebar sorts alphabetically; the README says chronologically; the chronological sort
    function is dead code.** `core::sort_entries_by_date_desc` is exported from `core/src/lib.rs`
    and called by nothing in `src/`. `sidebar.rs`'s `refresh_list` sorts
    `by_key(|e| e.title.to_lowercase())`. For a CV database, most-recent-first is the right
    default and alphabetical-by-title is close to useless (titles like "Student Minister" and
    "Master of Divinity" have no meaningful alphabetical relationship). Fix: use the existing
    date sort as the default, and add a sort selector (Date / Title / Category) in the sidebar
    persisted to config, since alphabetical is genuinely useful when hunting for a known entry
    in a long list.

21. **Two different save models in one window.** Sidebar Delete/Duplicate and the Manage Tags
    dialog all route through `on_change`, which persists to disk immediately. Detail-pane field
    edits require pressing the Save button (`detail.rs:383`). Nothing warns on navigation away —
    `select_row_by_key` loads the newly-selected entry over whatever was typed, and the edit is
    gone with no toast, no dialog, no dirty marker. Fix: **autosave the detail pane** to match
    everything else — commit on field change, debounced (~600ms) plus an immediate commit on
    focus-out and on selection change. Undo already exists and is 50 deep (`state.rs`'s
    `UNDO_LIMIT`), so it's the safety net. Keep the hard block on duplicate/empty keys: an entry
    with an invalid key simply doesn't commit, and the existing inline error already says why.
    Demote Save to a Ctrl+S accelerator rather than deleting the concept outright, since Raw YAML
    mode still needs an explicit "parse this now" moment.

22. **No external-change detection.** Phase 3b deliberately made Zerkalo re-read the YAML on every
    compile so Skrizhal edits show up live — but the reverse doesn't hold. Skrizhal reads the file
    once at launch; "Reload from Disk" is a manual, unprompted menu item. With two apps over one
    file (and git in the picture once item 26 lands), Skrizhal silently overwriting an
    externally-changed file is a real data-loss path. Fix: a `gio::FileMonitor` on `data_path`,
    surfacing an `adw::Banner` ("This file changed on disk") with Reload / Keep Mine actions —
    banner, not toast, per the root CLAUDE.md's rule that banners are for actionable one-off
    suggestions tied to file state. Re-arm the monitor on Open/New File/Save As, and suppress the
    self-triggered event from Skrizhal's own `persist()`.

## 4b — CV Profiles — ✅ done

23. **Profile schema (core crate).** A profile is a name, an ordered list of sections, and per
    section: a heading, a set of tag/category rules, an explicit include list, and an explicit
    exclude list. Explicit includes/excludes are what filters can't express — the one-off
    exception ("this CV only, drop the retail job, keep the volunteer thing") that otherwise
    forces a new single-use tag every time.

    Stored under a reserved `_profiles:` top-level key in the same `cv-elements.yaml`, not a
    sibling file: one file to point Zerkalo at, one file to version, one file to hand to another
    machine. The leading underscore keeps it out of the entry namespace — `load_file` skips
    `_`-prefixed keys rather than trying to parse them as entries, and (critically) they must
    round-trip through the existing `raw_failed` passthrough mechanism untouched by older
    versions of the app.

    ```yaml
    _profiles:
      academic-2026:
        label: Academic CV (2026)
        sections:
          - heading: Education
            categories: [Education]
          - heading: Ministry
            categories: [Ministry Position]
            tags: [ministry]
            exclude: [some-early-placement]
          - heading: Publications
            categories: [Publication, Presentation]
            include: [one-off-key-not-otherwise-matched]
    ```

24. **Profile editor UI + `#cv-profile("name")`.** A profile manager dialog (list of profiles,
    add/duplicate/delete/rename) plus a per-profile section editor with drag-reorder. On the
    Zerkalo side this collapses a hand-assembled run of `#cv-section` calls into a single
    `#cv-profile("academic-2026")` — a new function in `cv-helpers.typ`, resolving the profile
    from the same `skrizhal-cv-data` sys.input that already carries the entries, so it needs no
    new plumbing in `preview_pane.rs` or `cv_mode.rs` at all.

25. **Manual ordering within a section.** Real CVs lead with what's relevant, not with what's
    most recent. An optional numeric `order` field on `CvEntry`, respected by `cv-section`/
    `cv-profile` ahead of the date sort (entries without it fall back to date order, so nothing
    changes for anyone who never sets it), plus drag-to-reorder in the sidebar when exactly one
    category filter is active. Without this, profiles can't express "put the current ministry
    position above the older but more prestigious academic post."

## 4c — Getting data in and keeping it healthy — ✅ done, except ORCID/LinkedIn import

26. **Importers.** Every entry is hand-typed today, and first run against an empty file is a blank
    wall. In value order: **BibTeX/Hayagriva `.bib`** → Publication entries (note the irony worth
    stating plainly — this plan rejected the `hayagriva` *crate* for CV storage because its
    `EntryType` enum is closed, but for importing genuine publications that closed enum is
    exactly right, and Zerkalo already depends on it; the importer lives behind a
    default-off cargo feature on `skrizhal-core` so the core crate stays dependency-light for
    Zerkalo); **ORCID** JSON; **LinkedIn** data-export CSV for employment/education. Import is
    always additive with a preview-and-confirm step and key-collision handling, never a
    wholesale replace.

27. **Database health panel.** Validation is per-entry today. A file-level pass catches the class
    of problem that per-entry validation structurally can't: near-duplicate entries (fuzzy
    title+organization match, since three near-identical versions of the same job accumulate over
    years), tags used exactly once (almost always a typo — `minstry` vs `ministry`), entries with
    no tags at all, unknown categories. **Tag typos are the specific silent killer**: a mistyped
    tag means an entry quietly vanishes from a generated CV, with no error raised anywhere in
    either app.

28. **Per-category dynamic forms.** `CATEGORY_REGISTRY` already carries `recommended_fields` and
    `validate.rs` already warns when they're missing — but `detail.rs` builds an identical form
    for every category and pushes category-specific fields into the generic "Additional Fields"
    add/remove list. Selecting `Education` should materialize real *Degree* / *Field of Study*
    rows; `Publication` should give *Venue* / *DOI*. The data needed is already in the registry
    and simply unused by the form layer. Unrecognized `extra` keys keep falling through to the
    Additional Fields list exactly as now.

29. **Description bullets as a real repeater.** `description` is a `Vec<String>` in the schema but
    a raw `TextView` split on newlines in the UI. Bullets are the highest-churn, highest-value
    content on a CV and deserve per-bullet rows with add/remove/reorder and per-bullet character
    counts. Also the natural future home for per-profile bullet variants, if that's ever wanted.

30. **Git-backed history**, lifted from Retseptura's existing git-backed YAML storage rather than
    designed fresh. A CV database is exactly the artifact you want versioned — "what did my CV
    say when I applied there in 2024?" Auto-commit on save, per-entry history, restore a previous
    version. Also upgrades item 22 from *detecting* concurrent edits to *recovering* from them.

## 4d — Live preview — ⛔ still deferred, see below

31. **Embed the Typst renderer in Skrizhal** so a CV can be previewed without opening Zerkalo,
    reusing `cv-helpers.typ` and previewing a whole *profile*, not just one entry. This is the
    item that would change what Skrizhal *is* — from a YAML form into a CV builder that happens
    to store YAML.

    **Deferred behind 4a–4c on purpose**, for reasons that are packaging, not design: the `typst`
    crate is a very large dependency tree, every crate of which has to be vendored into
    `packaging/cargo-sources.json` for the offline flatpak build, and the flatpak SDK's rustc is
    pinned at **1.89** by the GNOME 50 runtime (see CLAUDE.md — this already bit the project once,
    via gtk4-rs's MSRV). A Typst version bump raising its MSRV past 1.89 would strand the flatpak
    with no in-manifest fix available. Prerequisite before starting: confirm the target `typst`
    release builds under rustc 1.89, and decide whether preview is worth roughly an order of
    magnitude increase in vendored crate count. Profiles (4b) deliver most of the user-facing win
    without any of this risk, which is why they come first.

## Sequencing

4a (20–22) first — small, independent, and each fixes something demonstrably wrong. Then 4b
(23–25) as the next release's headline. 28 is cheap whenever, since the registry already holds
the data. 26 matters most for anyone who isn't Cal (i.e. it's the difference between "my CV
database" and "a CV app"). 31 stays parked until its packaging question has a real answer.

---

## Phase 4 outcome

Items 20–30 are implemented, tested, and verified in a running app. Notes on what changed
against the plan as written, and what's left:

**Corrections found while building, not while planning:**

- **`include` is a Typst keyword.** `cv-profile`'s section rules bind `let included = …`, not
  `let include = …`. This wasn't a cosmetic problem: the invalid binding made the *whole* of
  `cv-helpers.typ` fail to parse, which took the pre-existing
  `compile_cv_entry_and_section_with_skrizhal_helpers` test down with it. Caught only because
  that test existed. The YAML field is still named `include`.
- **`cv-section` had to learn to skip reserved keys.** It iterated `data.keys()` directly, so
  the moment `_profiles` appeared in the data file it would hand a profile block to the entry
  renderer. Fixed with `cv-entry-keys`, and the profile compile test asserts a bare
  `#cv-section()` still works alongside a `_profiles` block.
- **`order` has no form field, so `read_form` reports `None` for it.** `commit_edit` copies the
  stored value across before comparing, or every autosave would silently strip an entry's manual
  position. Raw-YAML edits set it explicitly and are exempt.
- **Item 30 needed no new Rust dependency.** The plan assumed lifting Retseptura's git-backed
  storage meant adding `git2` — which would have meant regenerating
  `packaging/cargo-sources.json`, needing tooling not installed here. Retseptura doesn't use a
  library at all: it shells out via `flatpak-spawn --host git` (org.gnome.Platform bundles no
  git binary). Skrizhal now does the same, in `src/git_backup.rs`. The only packaging change is
  one `--talk-name=org.freedesktop.Flatpak` line in the manifest.
- **Autosave (item 21) turned out to constrain the sidebar.** Row Duplicate/Delete captured
  their entry key at build time, but autosave renames entries as auto-generated keys follow
  Title/Organization. They now read the key off the row at click time, via weak references —
  a strong `row` inside a closure the row itself owns is a reference cycle.

**Not done:**

- **ORCID and LinkedIn import (part of item 26).** BibTeX is implemented, hand-rolled in
  `core/src/import.rs` specifically to avoid a vendored dependency. ORCID needs network access
  (Skrizhal's finish-args deliberately carry no `--share=network`) and LinkedIn's export is a
  multi-file, unstable CSV bundle — both want their own design pass rather than being bolted on.
- **Item 31, live Typst preview.** Still parked on the same question it was parked on: whether
  the target `typst` release builds under the GNOME 50 runtime's rustc 1.89, and whether preview
  justifies roughly an order-of-magnitude increase in vendored crates. Nothing learned while
  building 4a–4c changes that calculus. Profiles delivered the user-facing win without it, as
  predicted.
