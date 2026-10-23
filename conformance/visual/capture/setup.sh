#!/usr/bin/env bash
# VIS1: prepares a clean VS Code profile and a small workspace under C:\vis1 (from WSL), then launches
# the installed VS Code with the Chrome DevTools Protocol on 127.0.0.1:9229 (Windows side). The
# capture script (capture.js) runs on Windows too, inside Code.exe as Node (ELECTRON_RUN_AS_NODE),
# because WSL's NAT network cannot reach a Windows-loopback port. See plans/milestones/VIS1-visual-conformance.md.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root=/mnt/c/vis1
code="/mnt/c/Program Files/Microsoft VS Code/Code.exe"

mkdir -p "$root/ud/User" "$root/ext" "$root/out" "$root/ws/src/components" "$root/ws/docs"
cp "$here/settings.json" "$root/ud/User/settings.json"
for f in main.ts util.ts index.ts; do printf 'export const x = 1;\n' > "$root/ws/src/$f"; done
printf '<script>\n</script>\n' > "$root/ws/src/components/Button.svelte"
printf '# Readme\n' > "$root/ws/README.md"
printf '# Notes\n' > "$root/ws/docs/notes.md"
seq 1 400 | sed 's/^/line /' > "$root/ws/docs/long.txt"
cp "$here"/*.js "$root/"

if [ "${1:-}" = "--launch" ]; then
  exec "$code" --user-data-dir 'C:\vis1\ud' --extensions-dir 'C:\vis1\ext' --disable-extensions \
    --disable-workspace-trust --skip-welcome --skip-release-notes --new-window \
    --remote-debugging-port=9229 'C:\vis1\ws'
fi
