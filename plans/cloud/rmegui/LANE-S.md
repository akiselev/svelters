# rmegui lane S (svelters): egui-free skelter-test and svelters-automation, a designer scene API, window readback

Date 2026-10-26. Coordinator: the cadatomic coordinator, cloud session `session_01Kt5cvt9hDGVcXNCATLsKRK`; report with
`send_message` (claude-code-remote MCP) at each milestone and when done. svelters' `CLAUDE.md` applies (build and test
as little as possible; GitHub CI's five workflows are the gate). This brief stands alone.

## Why

cadatomic is removing egui (the user, 2026-10-26: "Remove egui sooner"). Its plan is cadatomic
`docs/notes/rmegui/PLAN.md`; the parts in svelters are below. When cadatomic is done, `cargo tree -p cadatomic-app`
(dev edges included) must name no egui-family crate, so these svelters crates must not pull egui unless asked.

Branch `ci/rmegui-svelters` from `origin/master` (sha in the launch message); push only there; no PRs.

## Steps

1. **skelter-test without egui by default:** `EguiHarness`, `CoreIslands` and the `egui`, `egui-wgpu`,
   `skelter-egui-island` dependencies behind a non-default `egui` feature; everything else (Harness, the consumer/AT-SPI
   helper, A11yTree) builds without it. Update svelters' own users to enable the feature where they need it.
2. **svelters-automation without egui:** split into an egui-free core (selectors, `SceneSource`, queries, waits) and the
   svelters-egui source behind a feature (or a separate crate), so a skelter-only host links no egui. cadatomic uses it
   from `waits.rs`, `tour/runner.rs` and `skelter/probe.rs`.
3. **A designer scene API in svelters-skelter** (what cadatomic's designer reads today through svelters-egui's scene):
   `by_source` (nodes for a template element), `hit_stack` (nodes under a point, deepest first), `matched_rules` (the CSS
   rules that apply to a node, with sources), `force_state` (force :hover/:active/:focus for inspection). Read
   cadatomic `crates/cadatomic-ui/src/designer.rs` and `src/app_embed.rs` (read-only, public on GitHub as part of
   akiselev/cadatomic? if you can't read it, ask the coordinator for the call list) to match what it calls.
4. **Window readback** (cadatomic request 5 / SV58): the compositor reads back a presented window's image (for
   `screenshot.window` and tour shots on every platform), with a test on lavapipe.

Each step with tests; push after each; record `plans/milestones/RMEGUI-S.md` (what landed, CI run ids, what cadatomic
must change, questions with recommendations). Done when all five workflows are green on your last code commit.
Never ask the user; questions go to the coordinator. Hand off near 400k tokens of context.
