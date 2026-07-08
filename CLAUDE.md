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
- `tags.rs` — tag rename/merge, usage counts

**GUI (root `src/`, package `skrizhal`):**
- `config.rs` — `~/.config/skrizhal/config.toml` (data file path, field-guide-seen flag)
- `ui/state.rs` — shared `AppState` + the `ChangeCallback` that persists + refreshes after any mutation
- `ui/sidebar.rs`, `ui/detail.rs`, `ui/dialogs.rs`, `ui/field_guide.rs`,
  `ui/changelog.rs`, `ui/app_window.rs`

Parses CV YAML with plain `serde_yaml_ng` against `skrizhal-core`'s own schema — deliberately
does **not** go through the `hayagriva` crate's parser, since its `EntryType` enum is closed and
would reject custom categories like `"Ministry Position"`. See `plan.md` for the full reasoning.

## Phase 3 (Zerkalo integration) status

Not started. See `plan.md`'s Phase 3a/3b breakdown. The workspace split above was Phase 3a's
first step, done ahead of the rest since it was low-risk and unblocks everything else.
