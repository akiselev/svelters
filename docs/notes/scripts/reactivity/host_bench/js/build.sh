#!/usr/bin/env bash
# Builds dist/svelte_bench.js and dist/tracker_bench.js (R1) with the repo's Vite (packages/build/node_modules).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../../../../.." && pwd)
"$repo/packages/build/node_modules/.bin/vite" build --config "$here/vite.config.mjs"
"$repo/packages/build/node_modules/.bin/vite" build --config "$here/vite.tracker.config.mjs"
