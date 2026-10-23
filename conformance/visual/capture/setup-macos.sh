#!/usr/bin/env bash
# VIS1m: setup.sh for macOS. Prepares a clean VS Code profile and the same small workspace under
# $VIS_ROOT (default /tmp/vis1m), then launches a VS Code app with the Chrome DevTools Protocol on
# 127.0.0.1:9229. Use a separate copy of the version the Windows references use (1.133.0, the
# official darwin zip unpacked anywhere: $VSCODE_APP), never the installed one: its own
# --user-data-dir and --extensions-dir keep the user's profile untouched.
# The capture runs inside that app as Node (macnode). See plans/milestones/VIS1-visual-conformance.md.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="${VIS_ROOT:-/tmp/vis1m}"
app="${VSCODE_APP:-/tmp/vis1m-vscode/Visual Studio Code.app}"

mkdir -p "$root/ud/User" "$root/ext" "$root/out" "$root/ws/src/components" "$root/ws/docs"
# the Windows settings, at the display's own scale: zoom 0 (the Retina display gives DPR 2)
sed 's/"window.zoomLevel": [-0-9.]*/"window.zoomLevel": '"${VIS_ZOOM:-0}"'/' "$here/settings.json" > "$root/ud/User/settings.json"
for f in main.ts util.ts index.ts; do printf 'export const x = 1;\n' > "$root/ws/src/$f"; done
printf '<script>\n</script>\n' > "$root/ws/src/components/Button.svelte"
printf '# Readme\n' > "$root/ws/README.md"
printf '# Notes\n' > "$root/ws/docs/notes.md"
seq 1 400 | sed 's/^/line /' > "$root/ws/docs/long.txt"
cp "$here"/*.js "$root/"

if [ "${1:-}" = "--launch" ]; then
  exec "$app/Contents/MacOS/Code" --user-data-dir "$root/ud" --extensions-dir "$root/ext" --disable-extensions \
    --disable-workspace-trust --skip-welcome --skip-release-notes --new-window \
    --remote-debugging-port=9229 "$root/ws"
fi
