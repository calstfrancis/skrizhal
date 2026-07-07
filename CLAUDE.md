# Skrizhal — Claude Instructions

YAML editor/reader for CV elements, built to work with Zerkalo. See `plan.md` for the full
architecture and phased build plan, and `README.md` for current status.

## Version

Single source of truth: `version` in `Cargo.toml`. Releases get a two-word name (adjective +
noun) per the root CLAUDE.md convention — named in the CHANGELOG heading, the metainfo release
description, and the commit message. Unlike Kopilka/Zerkalo, there's no in-app "What's New"
window yet, so the name doesn't need a source constant anywhere.

## Documentation policy

Keep `CHANGELOG.md` up to date with every set of meaningful changes, not just releases — add a
new entry at the top with the version number and a short title, per the root `/home/calstfrancis/Projects/CLAUDE.md`
documentation policy.

## Code style

- No comments unless the WHY is non-obvious
- No multi-line docstrings or comment blocks
- `cargo clippy --all-targets` should be clean before committing

## Build

- `.cargo/config.toml` pins the linker to `gcc` — this system's `rustc` defaults to `clang`,
  which isn't installed. Without this, even `cargo build` on a trivial crate fails.
- GTK4/libadwaita GUI lives in `src/main.rs` + `src/ui/`, following Zerkalo's conventions (see
  the root CLAUDE.md's "UI design standard" section) where they fit — this is a simpler CRUD
  app, so not every Zerkalo pattern applies (no command palette, no hand-built hamburger popover
  needed yet since the header menu only has three items).

## Flatpak packaging

- **Never run `flatpak-builder` directly — that's always `dev-build.sh`/`publish-flatpak.sh`'s
  job**, run by Cal. This applies even for "just testing the manifest" — prep the manifest, then
  hand off.
- Manifest: `packaging/io.github.calstfrancis.Skrizhal.yml`. Two modules — `skrizhal-deps`
  (vendors and pre-builds all Cargo dependencies, cached independently) and `skrizhal` (builds
  just this crate against the pre-built deps) — same caching split Zerkalo uses, so dev-build
  iterations that don't touch `Cargo.lock` skip re-vendoring.
- Runtime: `org.gnome.Platform`//`50` with the `rust-stable` SDK extension. Needs the 25.08
  branch specifically (rustc 1.96.1) — the 48/24.08 branch only ships rustc 1.89, which is too
  old for current gtk4-rs/libadwaita-rs (they need 1.92+). Re-check this pairing before bumping
  `gtk4`/`libadwaita` crate versions in the future; a newer crate release could raise the MSRV
  again.
- `packaging/cargo-sources.json` vendors all crates.io dependencies for the offline flatpak
  build. Regenerate it whenever `Cargo.lock` changes:
  `python3 flatpak-cargo-generator.py Cargo.lock -o packaging/cargo-sources.json` (script from
  `flatpak/flatpak-builder-tools` on GitHub; needs `tomlkit` + `aiohttp`).
- `finish-args` are intentionally minimal — no `--share=network` (no network features exist yet),
  no Typst cache filesystem access (that's Zerkalo's concern). `--filesystem=home` is needed for
  "Choose Data File…" to reach arbitrary paths.

## Architecture

- `src/entry.rs` — `CvEntry` schema, YAML load/save, `slugify`/`unique_key`/`duplicate_with_key`
- `src/registry.rs` — namespaced CV entry type table
- `src/validate.rs` — soft-validation warnings
- `src/date.rs` — date-range parsing/sorting
- `src/filter.rs` — tag/type/search filtering
- `src/tags.rs` — tag rename/merge, usage counts
- `src/config.rs` — `~/.config/skrizhal/config.toml` (data file path)
- `src/ui/` — GTK4/libadwaita app: `state.rs` (shared `AppState` + the `ChangeCallback` that
  persists + refreshes after any mutation), `sidebar.rs`, `detail.rs`, `dialogs.rs`, `app_window.rs`

Parses CV YAML with plain `serde_yaml_ng` against this crate's own schema — deliberately does
**not** go through the `hayagriva` crate's parser, since its `EntryType` enum is closed and would
reject custom types like `ministry-position`. See `plan.md` for the full reasoning.
