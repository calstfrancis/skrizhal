# Skrizhal — Claude Instructions

YAML editor/reader for CV elements, built to work with Zerkalo. See `plan.md` for the full
architecture and phased build plan, and `README.md` for current status.

## Version

Single source of truth: `version` in `Cargo.toml`. No separate release-name/build-script
machinery yet — that's a Zerkalo-specific pattern (flatpak packaging) that doesn't apply here
until Skrizhal has a distributable app of its own (Phase 2+).

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
- No GTK/flatpak dependencies yet (Phase 1 is a pure Rust lib crate). Phase 2 will follow
  Zerkalo's GTK4/libadwaita conventions — see the root CLAUDE.md's "UI design standard" section
  when that work starts.

## Architecture

- `src/entry.rs` — `CvEntry` schema, YAML load/save
- `src/registry.rs` — namespaced CV entry type table
- `src/validate.rs` — soft-validation warnings
- `src/date.rs` — date-range parsing/sorting
- `src/filter.rs` — tag/type/search filtering

Parses CV YAML with plain `serde_yaml_ng` against this crate's own schema — deliberately does
**not** go through the `hayagriva` crate's parser, since its `EntryType` enum is closed and would
reject custom types like `ministry-position`. See `plan.md` for the full reasoning.
