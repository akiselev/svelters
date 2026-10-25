# Lane TEST-V2: presto-test at kittest parity for cadatomic (W-T2)

Repository: svelters. Branch `ci/presto-test-v2`, made from `origin/ci/presto-base`; push only there.
`plans/cloud/presto-b1/BRIEF.md` applies: minimal building and testing, rules, attribution.

**Read first:**
- `plans/research/presto/plan.md` §2.4 (W-T2);
- APP-TEST's record, `plans/milestones/PRESTO-app-test.md`;
- `crates/presto-test`;
- cadatomic's kittest usage: `crates/cadatomic-app/tests/*.rs` at cadatomic `master`, read-only. List every
  kittest and egui_kittest verb and query it uses.

**Deliver in presto-test:**
- Every one of those verbs and queries, with the same names and semantics.
- The `egui::Event`-shaped injection adapter, completed.
- `Harness::render` on lavapipe, through the real compositor and renderer offscreen (`WGPU_BACKEND=vulkan` on
  Linux CI).
- The pixel tier: snapshot compare with a threshold, the same API shape as egui_kittest's `snapshot`.
- Islands: queries descend into presto-egui-island's grafted AccessKit tree (lane EGUI-ISLAND, branch
  `ci/presto-egui-island`). Code against its API.
- An automation bridge, so svelters-automation (`crates/svelters-automation`) can drive a presto app with the same
  selector and MCP queries as the egui backend.

**Output:** a table in the record of every cadatomic verb with its presto-test status.

**Building:** `cargo check -p presto-test --all-targets`. Write tests, but don't run them; CI runs them.

**Record:** `plans/milestones/PRESTO-test-v2.md`.

**Report** to the coordinator `session_01CqZ417NuvkRujZaYSm7Pwo` with `send_message`. Don't ask the user anything.
