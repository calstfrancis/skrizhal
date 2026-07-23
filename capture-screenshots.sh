#!/usr/bin/env bash
# capture-screenshots.sh — capture a fresh screenshot of Skrizhal against demo data
#
# Runs the existing target/release/skrizhal binary (build one first with
# `cargo build --release` if it doesn't exist or is stale) under a throwaway
# $HOME (Skrizhal's config/data paths are resolved via shellexpand::tilde,
# which only consults $HOME — no XDG_CONFIG_HOME/XDG_DATA_HOME override
# needed here, unlike Zerkalo), inside an isolated Xvfb display forced via
# GDK_BACKEND=x11 (GTK4 otherwise prefers the real Wayland session and would
# render on the actual desktop). Also runs under its own private D-Bus
# session (dbus-run-session) — GApplication enforces single-instance per
# app ID over the session bus, so without this a real running Skrizhal
# instance (dev or flatpak) just gets relay-activated instead of the
# throwaway one actually launching. Waits for the window to render,
# screenshots it, and overwrites screenshots/skrizhal-main.png.
#
# Requires: Xvfb, dbus-run-session, ImageMagick (magick), a built and
# current target/release/skrizhal binary.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

BINARY="target/release/skrizhal"
if [[ ! -x "$BINARY" ]]; then
  echo "ERROR: $BINARY not found. Run 'cargo build --release' first." >&2
  exit 1
fi

DEMO_HOME=$(mktemp -d /tmp/skrizhal-demo-home.XXXXXX)
OUT="screenshots/skrizhal-main.png"
OUT_DARK="screenshots/skrizhal-main-dark.png"
WINDOW_W=1000
WINDOW_H=650

cleanup() {
  [[ -n "${APP_PID:-}" ]] && kill "$APP_PID" 2>/dev/null || true
  [[ -n "${XVFB_PID:-}" ]] && kill "$XVFB_PID" 2>/dev/null || true
  rm -rf "$DEMO_HOME"
}
trap cleanup EXIT

echo "==> Seeding demo home in $DEMO_HOME"
mkdir -p "$DEMO_HOME/.config/skrizhal" "$DEMO_HOME/Documents/Zerkalo"
cp screenshots/demo-cv-elements.yaml "$DEMO_HOME/Documents/Zerkalo/cv-elements.yaml"
cat > "$DEMO_HOME/.config/skrizhal/config.toml" <<EOF
data_path = "$DEMO_HOME/Documents/Zerkalo/cv-elements.yaml"
has_seen_field_guide = true
EOF

# Isolated Xvfb display, well clear of any real display number in use.
DISPLAY_NUM=228
while [[ -e "/tmp/.X${DISPLAY_NUM}-lock" ]]; do
  DISPLAY_NUM=$((DISPLAY_NUM + 1))
done

echo "==> Starting isolated Xvfb on :$DISPLAY_NUM"
Xvfb ":$DISPLAY_NUM" -screen 0 "${WINDOW_W}x${WINDOW_H}x24" &
XVFB_PID=$!
sleep 2

export BINARY DISPLAY_NUM WINDOW_W WINDOW_H DEMO_HOME

# Capture the app once per colour scheme. libadwaita normally resolves
# light/dark from the desktop's settings portal, which on this machine is
# answered by a backend that ignores our isolated home and always reports
# light. ADW_DISABLE_PORTAL=1 makes libadwaita read the GSettings color-scheme
# key instead, and GSETTINGS_BACKEND=keyfile feeds it a value we write into
# the throwaway config — forcing either scheme deterministically without
# touching the real desktop. XDG_CONFIG_HOME is redirected into the throwaway
# home *only for the child* so that keyfile never lands in Cal's real
# ~/.config (Skrizhal itself resolves its own config via $HOME, so this
# doesn't change where it reads config.toml).
#
# The whole launch+capture runs inside dbus-run-session so the private bus is
# torn down when the inner shell exits — backgrounding dbus-run-session and
# killing it instead leaves an orphaned dbus-daemon holding stdout open.
capture_scheme() {
  local scheme="$1"
  export OUTFILE="$2"
  mkdir -p "$DEMO_HOME/.config/glib-2.0/settings"
  cat > "$DEMO_HOME/.config/glib-2.0/settings/keyfile" <<KEYFILE
[org/gnome/desktop/interface]
color-scheme='$scheme'
KEYFILE

  echo "==> Capturing Skrizhal ($scheme) -> $OUTFILE"
  dbus-run-session -- bash -c '
    env -u WAYLAND_DISPLAY GDK_BACKEND=x11 HOME="$DEMO_HOME" XDG_CONFIG_HOME="$DEMO_HOME/.config" \
      ADW_DISABLE_PORTAL=1 GSETTINGS_BACKEND=keyfile DISPLAY=":$DISPLAY_NUM" "./$BINARY" &
    app=$!
    sleep 6
    DISPLAY=":$DISPLAY_NUM" magick x:root -crop "${WINDOW_W}x${WINDOW_H}+0+0" +repage "$OUTFILE"
    kill "$app" 2>/dev/null || true
    wait "$app" 2>/dev/null || true
  '
}

capture_scheme default     "$OUT"
capture_scheme prefer-dark "$OUT_DARK"

echo "Done. Wrote $OUT and $OUT_DARK"

# Publish web-ready copies into the personal website repo, one PNG + WebP per
# scheme, named as the site expects (<slug>.png/.webp + <slug>-dark.png/.webp).
# The capture crop already matches the site's image dimensions, so this is a
# straight convert+copy — no resize. Override the destination with
# WEBSITE_DIR=/path ./capture-screenshots.sh; if it doesn't exist the export is
# skipped with a note rather than failing. The website is a separate repo —
# commit and push it there yourself after reviewing the refreshed images.
SLUG="skrizhal"
WEBSITE_DIR="${WEBSITE_DIR:-$(dirname "$SCRIPT_DIR")/calstfrancis.github.io}"
if [[ -d "$WEBSITE_DIR" ]]; then
  echo "==> Publishing web images to $WEBSITE_DIR"
  cp "$OUT"      "$WEBSITE_DIR/$SLUG.png"
  cp "$OUT_DARK" "$WEBSITE_DIR/$SLUG-dark.png"
  magick "$OUT"      -quality 80 "$WEBSITE_DIR/$SLUG.webp"
  magick "$OUT_DARK" -quality 80 "$WEBSITE_DIR/$SLUG-dark.webp"
  echo "    wrote $SLUG.{png,webp} and $SLUG-dark.{png,webp}"
else
  echo "NOTE: website dir not found ($WEBSITE_DIR) — skipping web export."
fi
