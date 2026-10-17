#!/usr/bin/env bash
# Generate the oracle corpora (if missing or older than gen.mjs; FORCE=1 to always regenerate)
# and run the Rust oracle tests. Extra args go to `cargo test` (e.g. `--release` is the default;
# set PROFILE=debug for a debug build). Env: UPSTREAM, ORACLE_FILTER, ORACLE_SHOW,
# ORACLE_UPDATE_BASELINE (see tools/oracle/README.md).
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

if [ ! -d node_modules/.pnpm ]; then
	pnpm install --frozen-lockfile
fi

for suite in acorn svelte; do
	out="target/oracle/$suite.jsonl"
	if [ "${FORCE:-0}" = 1 ] || [ ! -f "$out" ] || [ tools/oracle/gen.mjs -nt "$out" ]; then
		node tools/oracle/gen.mjs "$suite"
	fi
done

profile=(--release)
[ "${PROFILE:-release}" = debug ] && profile=()
cargo test -p svelters-compiler "${profile[@]}" --test oracle_acorn --test oracle_svelte "$@" -- --nocapture --test-threads=1
