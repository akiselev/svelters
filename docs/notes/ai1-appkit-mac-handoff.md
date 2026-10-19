# AI1 slice 4 (AppKit annotation inference): handoff for a Mac

Written on Linux (2026-10-19). Everything below the line "verified on CI" ran on the GitHub `macos-latest`
runner (arm64, macOS 26.6.2, Xcode 26.1.1 / macOS 26.1 SDK). Nothing has run on a real, interactive Mac.
Design: `docs/notes/ai1-inference.md` (slice 4). Setup of a Mac for this repo: `docs/notes/macos-setup.md`.

## What exists

| Piece | Path |
| --- | --- |
| Prober binary (KVC round trip, KVO observer, signal delivery, notification constants via dlsym) | `crates/svelters-appkit/examples/appkit_probe.rs` |
| Prober driver (plan, per-class runs, crash isolation, output) | `tools/metadata/appkit/probe.py` |
| Probe output (checked in) | `metadata/probe/appkit/26.1.json` |
| Rules | `metadata/rules/appkit.yaml` |
| Inference | `tools/metadata/appkit/infer.py` -> `metadata/inferred/appkit/` |
| Layers ingested < inferred < manual (+ `deny:`, parked) | `tools/metadata/appkit/layers.py` |
| Generator: table-driven rows for inferred facts and bindings, facts manifest | `tools/metadata/appkit/gen_backend.py` -> `examples/counter-mac/tests/generated.rs`, `crates/svelters-appkit/src/generated/facts.json` |
| Results file per test case (`SVELTERS_TEST_RESULTS`) | `crates/svelters-apple/src/testing.rs` |
| Ledger import / check / refresh for appkit | `tools/metadata/ledger.py`, `metadata/verified/appkit.json` |
| Coverage | `metadata/coverage/appkit.md` |
| CI: probe + results artifact (`appkit-probe`), import dry run, `infer.py --check` against the fresh ingest | `.github/workflows/macos-appkit.yml` |

## Counts (SDK 26.1)

- Probe: 10 classes, 175 props probed; 161 exact round trips (all with KVO firing on the programmatic KVC set),
  9 ignored, 3 normalized, 2 errors; no class-level crash. The first probe run crashed (SIGSEGV) on
  `NSWindow.released-when-closed` = YES; crash isolation recorded it and resumed, and the rules now deny it.
- Inferred and verified: 147 facts (122 props, 16 notification signals `verified: delivery`, 9 bindings). Manual:
  79, all verified. Ledger: 226 facts, 226 tests, 0 failing, 0 parked, 0 pending. 18 candidates held in
  `metadata/inferred/appkit/_review.json` (low band: normalized/ignored/error round trips and the bindings on them).
- New bindings (renderer `bind:`): `slider` float-value, int-value, integer-value; `button` int-value,
  integer-value; `text-field` (and `secure-text-field` by inheritance) double-value, float-value, int-value,
  integer-value. All marked `observability: partial, missed: [programmatic]`.

## Commands on the Mac

```sh
# toolchain (once): see docs/notes/macos-setup.md §1, then
sw_vers; uname -m
xcode-select -p
xcrun --sdk macosx --show-sdk-version          # 26.1 matches the checked-in metadata; anything else: see "Other SDK" below
rustc --version                                # 1.97.0 from rust-toolchain.toml
python3 -m venv /tmp/ai1-venv && /tmp/ai1-venv/bin/pip install pyyaml jsonschema pytest
PY=/tmp/ai1-venv/bin/python

# 1. the generated tests, with per-case results (226 cases; windows open briefly)
cargo test -p counter-mac --test generated
# 2. the prober (builds the example with --features exceptions; ~1 min). Writes metadata/probe/appkit/26.1.json
$PY -I tools/metadata/appkit/probe.py
git diff --stat metadata/probe/appkit         # expect: only the `runtime.macos` line if your macOS differs from 26.6.2
# 3. inference and everything generated from it
$PY -I tools/metadata/appkit/infer.py
$PY -I tools/metadata/appkit/gen_backend.py
$PY -I tools/metadata/common/gen_bindings.py
$PY -I tools/metadata/semantics/gen_semantics.py
# 4. run the tests here and record them (parks failing inferred facts, regenerates, reruns)
$PY -I tools/metadata/ledger.py refresh --toolkit appkit
$PY -I tools/metadata/coverage.py
$PY -I tools/metadata/ledger.py check
$PY -m pytest -q tools/metadata
git status                                    # commit probe/inferred/verified/coverage changes, if any
```

Without a Mac, the same refresh is: push a `ci/**` branch, `gh run download <run> -n appkit-probe`, copy
`appkit-probe/26.1.json` to `metadata/probe/appkit/`, run step 3, then
`python3 tools/metadata/ledger.py import --toolkit appkit appkit-results/generated.jsonl` and step 4's last three.

Other SDK (e.g. Xcode 26.2): `ingest.py` first (`docs/notes/macos-setup.md`), then probe; the probe and ledger file
names follow the SDK version and `ledger.py check` fails until both exist for it.

## Verify interactively (CI cannot)

CI only posts notifications and sends actions; it never types, drags or moves windows. With
`cargo run -p counter-mac -- --fixture gallery-appkit` (or a small page using the new `bind:`s):

1. **Real user edits.** Type into a text field bound with `bind:int-value` / `bind:double-value`: the bound
   state updates per keystroke (`NSControlTextDidChangeNotification` reads the field editor's text through
   `intValue`). Check non-numeric input ("12a"), deleting to empty, and undo (Cmd+Z) all announce a change.
2. **Slider drag and keyboard.** `bind:int-value` / `bind:float-value` on a slider update while dragging
   (continuous action) and with arrow keys after clicking the slider. The `partial` claim: setting
   `max-value` below the current value clamps it with no action; confirm the bound state is then stale (expected).
3. **Button.** `bind:int-value` on a checkbox (`button-type="switch"`) follows clicks; a push button's
   int-value snaps to 0/1 (the probe saw 3 -> 1).
4. **Window notifications** (inferred, delivery only on CI): listen to `window-did-resize`, `window-did-move`,
   `window-did-become-key` / `resign-key`, `window-did-miniaturize`, `window-did-enter-full-screen` and check they
   fire for real resizes, drags, Cmd+M, the green button. `view-frame-did-change` on a `view` when the window resizes.
5. **ProMotion** (120 Hz MacBook Pro): frame delivery through `CADisplayLink` stays at the display rate while
   dragging a slider bound to a progress indicator (`SVELTERS_APPKIT_TRACE_FRAMES=1`).
6. **Multi-display**: move a window to a second display with a different scale; `window-did-change-screen` and
   `window-did-change-backing-properties` fire; frames keep coming after the move (display link re-targets).

## Verified vs not

- Verified on CI (runner, not interactive): every fact in `metadata/verified/appkit.json` (226). Props: the sample
  round-trips through the backend. Bindings: a programmatic set does not emit the change signal, a sent action or
  a posted text-did-change notification is delivered and the prop reads the new value, listen off stops it.
  Inferred notification signals: delivery when posted (not native causation).
- Not verified: everything in "Verify interactively"; behaviour on macOS older than 26 (rows of props whose
  setter is missing skip; facts with `since macOS 26.0` such as `neutral-value`, `tint-prominence` were only run on 26);
  Intel Macs; real KVO semantics beyond "fires on a KVC set" (all 161 exact props fired, which says nothing about
  user edits; REACTIVITY.md 3.3).

## Known gaps

- Parking has never triggered on real data (0 failing inferred facts); it is covered by `test_ledger.py` only.
- `NSAttributedString` props (`attributed-string-value`, `placeholder-attributed-string`) are typed `string` by
  ingest; setting one raises (`-[NSTaggedPointerString string]: unrecognized selector`). Held in `_review.json`;
  the backend/ingest should box them as attributed strings or mark them read-as-string only.
- No methods or slots are inferred: AppKit ingest only reads the methods the overrides name. Swift names
  (`NS_SWIFT_NAME`) are not ingested, so no rule uses them.
- The probe file name is the SDK version, but behaviour depends on the runtime macOS (recorded as
  `runtime.macos`); a probe from a different macOS can differ without the file name changing.
- Probe outcomes `ignored` (e.g. `needs-display`, `needs-layout` on a view outside a window) are correct for an
  unrealized view/window and are not tried on a shown one.
