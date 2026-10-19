#!/usr/bin/env bash
# Builds and runs gtk_notify_probe.c, rerunning with the crashing type skipped until a run completes.
# Output (stdout): the probe's TSV; skipped types as `#crashed <type>` lines.
set -u
here=$(cd "$(dirname "$0")" && pwd)
bin=${TMPDIR:-/tmp}/gtk_notify_probe
cc -O1 -w -o "$bin" "$here/gtk_notify_probe.c" $(pkg-config --cflags --libs gtk4) || exit 1
skip=""
out=$(mktemp)
for _ in $(seq 1 40); do
  SVL_SKIP="$skip" timeout 300 env -u DISPLAY -u WAYLAND_DISPLAY GDK_BACKEND=x11 xvfb-run -a "$bin" >"$out" 2>/dev/null
  rc=$?
  [ $rc -eq 0 ] && break
  last=$(grep '^#begin' "$out" | tail -1 | cut -f2)
  [ -z "$last" ] && break
  skip="$skip $last"
done
grep -v '^#begin' "$out"
for t in $skip; do printf '#crashed\t%s\n' "$t"; done
rm -f "$out"
