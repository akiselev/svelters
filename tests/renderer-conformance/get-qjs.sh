#!/usr/bin/env bash
# Fetch the quickjs-ng release binary used for the QuickJS half of the conformance run.
# Pinned: quickjs-ng v0.17.0, linux x86_64. Override with QJS=/path/to/qjs to use another build.
# (On other platforms download the matching asset from the release page and set QJS.)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
version=v0.17.0
sha256=0bfc02511a9f549c28b53880d988fc7cd5d361e90c5e8afdfcd7dc6774ceace5
dest="$here/.qjs/qjs"
mkdir -p "$here/.qjs"
if [ -x "$dest" ] && echo "$sha256  $dest" | sha256sum -c --status; then
  echo "$dest"
  exit 0
fi
curl -fsSL -o "$dest" "https://github.com/quickjs-ng/quickjs/releases/download/$version/qjs-linux-x86_64"
echo "$sha256  $dest" | sha256sum -c --status || { echo "qjs checksum mismatch" >&2; rm -f "$dest"; exit 1; }
chmod +x "$dest"
echo "$dest"
