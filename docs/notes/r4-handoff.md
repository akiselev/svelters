# R4 handoff (AppKit + WinUI `observe`)

Branch `r4-appkit-winui`, worktree `/home/dev/projects/svelters/.claude/worktrees/r4`, pushed to `ci/r4` up to
3df9b019 (code head). The ledger import, coverage change, milestone record and this note are committed locally
after it and are **not pushed** (coordinator asked not to push again). Full design and record:
`plans/milestones/R4-appkit-winui-observe.md`.

## Done

- **AppKit** `crates/svelters-appkit/src/observe.rs` (child module of `backend.rs`): one KVO observer
  (`SveltersObserver`, context = boxed (h, prop)); notification and target-action fallbacks user-counted together with
  `listen` (`note_use`, `action_use`); teardown: OWNED/TOPLEVEL remove KVO in `cleanup` (destroy, window close) before
  the entry is released; BORROWED: autonotifying class with NSObject's `observationInfo` -> identity sentinel
  (associated object, `SVL_GONE_OWNER` on death), else retained while KVO-observed (`Observing.keep`); off-main KVO
  queued + deduplicated, delivered by the run-loop wake (`observe_hops`). Echo marker `Inner.setting` in `set_prop`.
  `SVL_M_OBSERVABLE` in `describe` (scalar/record props only). Table `OBSERVE: [ObserveInfo]` (`src/tables.rs`,
  `lookup::observe_info`) from `tools/metadata/appkit/gen_backend.py`.
- **WinUI** `crates/svelters-winui/src/backend.rs` (`observe_impl`, `emit_changed`), `sink.rs` (`observe_dp`),
  `table.rs` (`Observe`, `ObsClass`), generated `OBSERVE` (names.rs) and `OBSERVE_CONNECT` (dispatch.rs): DP callback
  tokens and event revokers per (h, prop) in `Entry.observers`, dropped on off/destroy (before the element ref)/native
  death/shutdown; UI-thread check (`DispatcherQueue.HasThreadAccess`, added to RUNTIME_FILTER); proportional echo
  suppression; `SVL_M_OBSERVABLE`. bindings.rs regenerated (`tools/metadata/winui/regen.sh`).
- **Rows**: AppKit `observe_row` in `crates/svelters-appkit/src/testing.rs`, rows in
  `examples/counter-mac/tests/generated.rs` (344); hand-written `observe_*` tests in
  `examples/counter-mac/tests/backend.rs` (typing acceptance, KVO hop, borrowed object, codes/flag). WinUI
  `testing::observe` + rows in `crates/svelters-winui/tests/generated.rs` (145). Facts manifests: section `observe`,
  kind = owner key, fingerprint includes testing.rs.
- **Rules**: `observe.verify` triggers in `metadata/rules/{appkit,winui}.yaml` (read by `observe_rules.py`);
  `TextBlock.SelectedText` added to WinUI `no_notify` (use SelectionChanged).
- **Ledger import** of the green runs: `metadata/verified/{appkit,winui}.json`; `tools/metadata/coverage.py` now counts
  AppKit/WinUI rows (`metadata/coverage/*`).
- **macOS 11 minimum**: `.github/workflows/macos-appkit.yml` (`MACOSX_DEPLOYMENT_TARGET`), `tools/dev/mac-cross`,
  `docs/notes/macos-setup.md`. Docs: `plans/architecture/backend-abi.md` AppKit/WinUI bullets.

## Left (ordered)

1. Push the local commits to `ci/r4` and confirm all five workflows green on that head (only ledgers/coverage/docs
   changed since 3df9b019, but the gate is CI on the final head).
2. Decide whether to exercise the identity-sentinel path (no current kind reaches it: NSView is not autonotifying by
   the backend's test, so borrowed views are retained). Needs a borrowed handle of an autonotifying class.
3. Optional: WinUI UIA user-path rows (ValuePattern/TextPattern peers need adding to the bindings filter); `fires` per
   fact from the identical-write data in the milestone record; a proportional-fact test.
4. Merge is the coordinator's call; don't edit STATUS.md.

## Checks and runs

Local (worktree, after the import; code = 3df9b019): `cargo check --workspace --all-targets` rc 0; AppKit
`--target aarch64-apple-darwin` rc 0; WinUI `--target x86_64-pc-windows-gnu --tests` rc 0; `pnpm test` rc 0
(pytest 140 passed, 1 skipped; all `--check`s; `ledger.py check` pass). `pnpm -r build` fails only in docs/site (no IR:
needs `python3 tools/docs/build_ir.py`), not a gate here. Logs in `/tmp/claude-1000/r4/`.

CI (`ci/r4`):
- 3ee417bf: macos-appkit 39538249180 failure (views-need-display burst bound, borrowed death); windows-winui
  39538249160 failure (456/495); js failure (coverage stale); gtk, qt success.
- 996b40de: macos 39541027676 failure (borrowed death only); winui 39541027728 failure (494/495, select-all repeat);
  js 39541027801, gtk 39541027691, qt 39541027688 success.
- 3df9b019: **all success**: macos 39542111803 (generated 570/570, backend 17/17), winui 39542111789 (495/495), js
  39542111853, gtk 39542111835, qt 39542111918.
- Not run: the local-only commits after 3df9b019.

## Facts changed

- WinUI `TextBlock:SelectedText`: notify -> event SelectionChanged (DP callback did not fire for SelectAll, run
  39538249160).
- `verify` trigger blocks added (record fingerprints changed; nothing was verified before).
- Result: AppKit 344 facts, 154 verified + 190 weak (was 0); WinUI 251 facts, 130 verified + 15 weak, 106 without row
  (was 0). Nothing parked.

## Deviations, decisions, traps

- None/set_only/no fact -> `SVL_OBSERVE_UNSUPPORTED`; constant/write-only/dead -> -1; WinUI off-UI-thread -> -1.
- "Coalescing at backend level" = at least one announcement for 10 changes; counts above 10 are logged, not failed
  (AppKit changes some props itself).
- The KVO-removed-before-dealloc check uses `observationInfo == NULL` on an object the test still retains, only when
  it was NULL before observing.
- Cross-compiling: counter-mac needs an SDK (QuickJS); to type-check its tests, copy them as a temporary test target
  of `svelters-appkit` and `cargo check --test <name> --target aarch64-apple-darwin`, then delete the copy.
- WinUI bindings: any new member needs `tools/metadata/winui/regen.sh` (first run fetches NuGet into target/winsdk).
- CI logs: `gh run view <id> --log-failed`; the AppKit runner stops listing after failures but prints every case;
  grep `panicked at` + next line. Identical-write/burst data are `println!` lines (`identical ... announced`).
- Import: `gh run download <mac run> -n appkit-probe` -> `ledger.py import --toolkit appkit
  appkit-results/generated.jsonl`; `gh run download <win run> -n windows-winui-report` -> `ledger.py import --toolkit
  winui <dir>`; then `coverage.py`, `gen_backend.py --check` both, `ledger.py check`.
