#!/bin/bash
# Check a change in the real app (FAMILY §11 step 5): run the debug build on a virtual display,
# play xdotool steps, and save screenshots. Needs Xvfb, xdotool and ImageMagick (import).
#   cargo build -p datum-mesh && tests_ui/realapp.sh steps.sh out/
# steps.sh is plain shell run while the app is open, e.g.:
#   xdotool mousemove 711 160 click 1; sleep 2      # File › DC motor example
#   xdotool key F5; sleep 3                         # Run
#   import -window root "$OUT/run.png"
set -e
STEPS=${1:?steps file}; export OUT=${2:-shots}; mkdir -p "$OUT"
TMPHOME=$(mktemp -d)                                # fresh options and recovery every time
export DISPLAY=:99
Xvfb :99 -screen 0 1440x900x24 >/dev/null 2>&1 & XV=$!
sleep 1
HOME=$TMPHOME WEBKIT_DISABLE_COMPOSITING_MODE=1 ./target/debug/datum-mesh >"$OUT/app.log" 2>&1 & APP=$!
sleep ${WAIT:-12}                                # a fresh WebView profile takes a while to start
bash "$STEPS" || true
kill $APP $XV 2>/dev/null; rm -rf "$TMPHOME"
