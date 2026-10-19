#!/usr/bin/env bash
# Builds dist/svelte_bench.js with the repo's Vite (packages/build/node_modules).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../../../../.." && pwd)
"$repo/packages/build/node_modules/.bin/vite" build --config "$here/vite.config.mjs"
