# Skrizhal — Claude Instructions

YAML editor/reader for CV elements, built to work with Zerkalo. See `plan.md` for the full
architecture and phased build plan, and `README.md` for current status.

## Workspace layout

Two Cargo packages, one workspace (root `Cargo.toml` has both `[workspace]` and `[package]` —
the root package is automatically a workspace member):

- **`core/`** — package `skrizhal-core`, a lib with **no GTK/libadwaita dependency at all** (just
  `serde`/`serde_yaml_ng`/`thiserror`). This is deliberate: Zerkalo pins `gtk4 = "0.7"` /
  `libadwaita = "0.5"`, while Skrizhal's GUI uses `gtk4 = "0.11.4"` / `libadwaita = "0.9.2"` —
  incompatible versions that can't both be compiled into one binary. Zerkalo depends only on
  `skrizhal-core`, which pulls in nothing that could conflict.
- **root (`src/`)** — package `skrizhal`, the GUI (`main.rs` + `ui/`), depends on
  `skrizhal-core = { path = "core" }` plus gtk4/libadwaita/glib.

When adding a new core module (schema, validation, parsing — anything Zerkalo might eventually
want too), it goes in `core/src/`, not `src/`. GUI code imports it as `skrizhal_core::...`.

## Version

Single source of truth: `version` in `Cargo.toml` (both packages should stay in lockstep — bump
both together). Releases get a two-word name (adjective + noun) per the root CLAUDE.md convention
— named in the CHANGELOG heading, the metainfo release description, and the commit message.
Unlike Kopilka/Zerkalo, there's no in-app "What's New" window yet, so the name doesn't need a
source constant anywhere.

## Documentation policy

Keep `CHANGELOG.md` up to date with every set of meaningful changes, not just releases — add a
new entry at the top with the version number and a short title, per the root `/home/calstfrancis/Projects/CLAUDE.md`
documentation policy.

## Code style

- No comments unless the WHY is non-obvious
- No multi-line docstrings or comment blocks
- `cargo clippy --workspace --all-targets` should be clean before committing

## Build

- `.cargo/config.toml` pins the linker to `gcc` — this system's `rustc` defaults to `clang`,
  which isn't installed. Without this, even `cargo build` on a trivial crate fails.
- `cargo build`/`test`/`clippy` without `--workspace` only covers the root `skrizhal` package —
  add `--workspace` (or `cd core && cargo test`) to also cover `skrizhal-core`.
- GTK4/libadwaita GUI lives in `src/main.rs` + `src/ui/`, following Zerkalo's conventions (see
  the root CLAUDE.md's "UI design standard" section) where they fit — this is a simpler CRUD
  app, so not every Zerkalo pattern applies (no command palette, no hand-built hamburger popover
  needed yet since the header menu only has a handful of items).
- **GApplication single-instance gotcha when testing under Xvfb**: if a Skrizhal instance (dev
  build or the installed flatpak) is already registered on the *real* session D-Bus, a new test
  launch will just relay-activate it and exit immediately (exit 0, no output) instead of actually
  running your rebuilt binary — easy to mistake for a successful silent run. Check
  `flatpak list --app | grep -i skrizhal` and `ps aux | grep skrizhal` if a test launch exits
  suspiciously fast with no log output.

## Flatpak packaging

- **Never run `flatpak-builder` directly — that's always `dev-build.sh`/`publish-flatpak.sh`'s
  job**, run by Cal. This applies even for "just testing the manifest" — prep the manifest, then
  hand off.
- Manifest: `packaging/io.github.calstfrancis.Skrizhal.yml`. Two modules — `skrizhal-deps`
  (vendors and pre-builds all Cargo dependencies for the whole workspace, cached independently)
  and `skrizhal` (builds both workspace members against the pre-built deps) — same caching split
  Zerkalo uses, so dev-build iterations that don't touch `Cargo.lock` skip re-vendoring. The
  deps module's stub-build step stubs *both* `src/main.rs` and `core/src/lib.rs` (and its source
  list includes `core/Cargo.toml`, since the workspace won't resolve without it) — remember both
  halves if this ever needs touching again.
- Runtime: `org.gnome.Platform`//`50` with the plain `rust-stable` SDK extension (unpinned —
  it resolves to whatever branch matches the runtime, currently rustc 1.89 on the 24.08 branch).
  **`gtk4`/`libadwaita`/`glib` are deliberately pinned to the same versions Zerkalo uses**
  (`gtk4 = "0.7"`, `libadwaita = "0.5"`, `glib = "0.18"`) specifically so they stay compatible
  with rustc 1.89 — newer gtk4-rs releases (0.9+) raise the MSRV to 1.92+, which the GNOME 50
  runtime's SDK extension can't provide. This bit us once: an earlier version of this project
  used gtk4 0.11.4/glib 0.22.8, which built fine locally (system rustc is newer) but failed in
  the flatpak sandbox with "rustc 1.89.0 is not supported by the following packages." Flatpak's
  `sdk-extensions` manifest field has no way to pin an extension to a branch other than the
  one matching the runtime (flatpak-builder 1.4.8 silently ignores any `//branch` suffix and
  resolves it anyway) — so bumping gtk4-rs past the current pins means either waiting for a
  GNOME runtime built on a newer freedesktop-sdk branch, or re-vendoring against a plain
  `org.freedesktop.Platform`/`Sdk` base instead of `org.gnome.*`. Re-check this before ever
  bumping `gtk4`/`libadwaita`/`glib` versions.
- `packaging/cargo-sources.json` vendors all crates.io dependencies for the offline flatpak
  build. Regenerate it whenever `Cargo.lock` changes:
  `python3 flatpak-cargo-generator.py Cargo.lock -o packaging/cargo-sources.json` (script from
  `flatpak/flatpak-builder-tools` on GitHub; needs `tomlkit` + `aiohttp`).
- `finish-args` are intentionally minimal — no `--share=network` (no network features exist yet),
  no Typst cache filesystem access (that's Zerkalo's concern). `--filesystem=home` is needed for
  "Choose Data File…" to reach arbitrary paths.

## Architecture

**Core (`core/src/`, package `skrizhal-core`, no GTK deps):**
- `entry.rs` — `CvEntry` schema, YAML load/save, `slugify`/`unique_key`/`duplicate_with_key`
- `registry.rs` — namespaced CV category table (canonical Title Case names, e.g. `"Ministry Position"`)
- `validate.rs` — soft-validation warnings
- `date.rs` — date-range parsing/sorting, `DateMode`/`split_date_string`/`join_date_string`
- `filter.rs` — tag/category/search filtering
- `sort.rs` — `SortMode` (newest-first/title/category) + `sort_entries`
- `profile.rs` — CV profiles (`_profiles` in the data file) + `resolve_section`/`resolve_profile`
- `health.rs` — file-level checks: near-duplicates, tag typos, untagged, unknown categories
- `import.rs` — hand-rolled BibTeX → `CvEntry` (no external parser dependency, deliberately)
- `tags.rs` — tag rename/merge, usage counts

**GUI (root `src/`, package `skrizhal`):**
- `config.rs` — `~/.config/skrizhal/config.toml` (data file path, field-guide-seen flag)
- `ui/state.rs` — shared `AppState` + the `ChangeCallback` that persists + refreshes after any mutation
- Autosave lives in `ui/app_window.rs` (`commit_edit`/`flush_autosave`). Two invariants worth
  knowing before touching it: `suppress_autosave` must be held (save-and-restore, not a bare
  `set(false)`) around *any* programmatic write into the form, or a list rebuild will re-commit
  stale form contents — deleting an entry would resurrect it; and an autosave patches the one
  affected row via `sidebar::update_row_in_place` rather than calling `refresh_list`, because a
  rebuild reloads the detail pane and eats the keystroke the user is mid-way through typing.
- `git_backup.rs` — git via `flatpak-spawn --host git` (no `git2` dependency; see below)
- `ui/sidebar.rs`, `ui/detail.rs`, `ui/dialogs.rs`, `ui/field_guide.rs`, `ui/profiles.rs`,
  `ui/health.rs`, `ui/history.rs`, `ui/changelog.rs`, `ui/app_window.rs`

**Reserved keys:** top-level keys starting with `_` in `cv-elements.yaml` are configuration, not
entries — `_profiles` today. `parse_str` routes them away from entry parsing, and unknown ones
round-trip untouched so a file written by a newer version survives an older one. Anything
iterating the data file's keys (including Zerkalo's `cv-helpers.typ`) must skip them; forgetting
this once meant `cv-section` would have rendered the profiles block as a CV entry.

**Git:** `git_backup.rs` shells out rather than using `git2`, because `org.gnome.Platform` bundles
no git binary and adding a Rust git library would mean regenerating `packaging/cargo-sources.json`.
Same approach (and same `flatpak-spawn --host` + `-C` reasoning) as Retseptura's `git_backup.py`.
Needs `--talk-name=org.freedesktop.Flatpak` in finish-args.

Parses CV YAML with plain `serde_yaml_ng` against `skrizhal-core`'s own schema — deliberately
does **not** go through the `hayagriva` crate's parser, since its `EntryType` enum is closed and
would reject custom categories like `"Ministry Position"`. See `plan.md` for the full reasoning.

## Phase status

Phase 3 (Zerkalo integration) and Phase 4 are both complete apart from two items, both recorded
with their reasons at the end of `plan.md`: ORCID/LinkedIn import (item 26) and an embedded Typst
preview (item 31, parked on whether the target `typst` release builds under the GNOME 50
runtime's rustc 1.89).

Zerkalo-side code lives in that repo: `templates/cv-helpers.typ` (`cv-entry`, `cv-section`,
`cv-profile`) and the compile-path wiring in `src/cv_mode.rs`/`src/ui/preview_pane.rs`. When
changing the data format, change both — `cv-helpers.typ` reads the same YAML directly, and
`compiler.rs`'s two CV tests are what catch a mismatch.
