# Lane CADATOMIC: cadatomic's shell on presto (W-D1)

Repository: **cadatomic** (github.com/akiselev/cadatomic). Branch `claude/presto-adoption`, made from `master`; push
only there. cadatomic's `CLAUDE.md` applies, except as stated below. This is a presto lane, not a cadatomic batch,
so it never merges `master`.

**Read first:**
- svelters `plans/research/presto/plan.md` §2.4 (W-D1, W-T2);
- `architecture.md` §14 and §15 (windows, the egui island, migration);
- `notes/round2/cadatomic-inventory.md` (what cadatomic uses from egui and eframe);
- cadatomic's `crates/cadatomic-app` (the eframe app, adapter selection, the shell).

**Inputs** (svelters by git, all on `https://github.com/akiselev/svelters`):
- `crates/presto`: App, AppHandler, latency;
- `presto-test`: Harness;
- `presto-egui-island`: lane EGUI-ISLAND, on `ci/presto-egui-island`;
- `svelters-presto`: lane SVELTERS-PRESTO;
- berth's `presto_berth`: berth branch `claude/presto-berth`.

Build against `ci/presto-integration` once consolidation 1 has pushed it. Until then, use `ci/presto-base` plus
the lane branches. Take svelters by git `branch =` on this branch, and record the sha in your report. Don't move
the svelters path pin in `pins.toml`: this branch has its own presto dependency behind a cargo feature `presto`,
off by default, so `master`'s eframe build stays untouched.

**Deliver, behind `--features presto`:**
- presto owns the window, the device and the frame loop. Port `adapter::select` and the limits.
- One egui island hosts the whole current eframe app. This is the first milestone: the app runs unchanged inside
  one island.
- Then, step by step:
  - the shell chrome goes native, in this order: title bar, menus, status bar, edge panels, ribbon, the palette
    modal;
  - keyboard pre-dispatch goes through presto-core's keymap, and egui `Key` names round-trip;
  - `CancelClose`;
  - each pane becomes an island in a `presto_berth` pane slot.
- The job, wait-idle and command-path rules stay as they are. The UI never runs the engine.

**Building:** `scripts/cargo.sh check -p cadatomic-app --features presto`. Write tests, but don't run them; the
consolidation runs the scenarios and tours. Take one screenshot through presto-test or `scripts/xrun.sh` when
the single-island app first renders, and send it to the coordinator. Never commit screenshots.

**Record:** `docs/notes/presto-adoption.md` in cadatomic: what landed, what was not run, and questions with
recommendations.

**Report** to the coordinator `session_01CqZ417NuvkRujZaYSm7Pwo` with `send_message` at each milestone and when
done. Don't ask the user anything.
