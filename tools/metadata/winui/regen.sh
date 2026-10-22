#!/usr/bin/env bash
# Regenerate everything the WinUI backend derives from the Windows App SDK metadata:
#
#   1. fetch.py        download the pinned NuGet packages (cache: target/winsdk, not committed)
#   2. winmd-tool      build the Rust half (winmd reader + windows-bindgen), via the build queue
#   3. ingest.py       winmd -> metadata/winui/*.json
#   4. gen_backend.py  metadata + overrides -> crates/svelters-winui/src/generated/*, tests, bindings.filter
#   5. bindgen         bindings.filter -> crates/svelters-winui/src/bindings.rs
#   6. strip_inherited.py  drop the base-class members windows-bindgen repeats on every derived class (WP-W)
#
# Steps 3-6 are deterministic: rerunning them on the same inputs writes byte-identical files.
#   tools/metadata/winui/regen.sh            # all steps
#   tools/metadata/winui/regen.sh --no-fetch # reuse target/winsdk (offline)
set -euo pipefail
cd "$(dirname "$0")/../../.."
here=tools/metadata/winui
cache=${SVELTERS_WINSDK_CACHE:-target/winsdk}
mkdir -p "$cache"

if [ "${1:-}" != "--no-fetch" ]; then
  python3 "$here/fetch.py" --cache "$cache" --winmds > "$cache/winmds.txt"
fi

slot=tools/dev/cargo-slot
[ -x "$slot" ] || slot=env
$slot cargo build --quiet --release --manifest-path "$here/winmd-tool/Cargo.toml" --target-dir target/winmd-tool
tool=target/winmd-tool/release/winmd-tool

python3 "$here/ingest.py" --tool "$tool" --cache "$cache"
python3 "$here/gen_backend.py"

grep -v '^#' crates/svelters-winui/bindings.filter > "$cache/filter.txt"
"$tool" bindgen --out crates/svelters-winui/src/bindings.rs --filter-file "$cache/filter.txt" \
  --implement Microsoft.UI.Xaml.Markup.IXamlMetadataProvider -- "@$cache/winmds.txt"
python3 "$here/strip_inherited.py" crates/svelters-winui/src/bindings.rs
echo "regen: crates/svelters-winui/src/bindings.rs: $(wc -l < crates/svelters-winui/src/bindings.rs) lines"
