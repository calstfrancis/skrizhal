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

echo "==> Launching Skrizhal against demo data inside the isolated display"
dbus-run-session -- env -u WAYLAND_DISPLAY GDK_BACKEND=x11 HOME="$DEMO_HOME" DISPLAY=":$DISPLAY_NUM" "./$BINARY" &
APP_PID=$!

echo "==> Waiting for window to render"
sleep 6

echo "==> Capturing screenshot"
DISPLAY=":$DISPLAY_NUM" magick x:root -crop "${WINDOW_W}x${WINDOW_H}+0+0" +repage "$OUT"

echo "Done. Wrote $OUT"
