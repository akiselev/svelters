# qc-a11y wave 1: real focus and text in skelter's accessibility tree (common brief)

Date: 2026-10-26. Coordinator: the cadatomic coordinator, cloud session `session_01Kt5cvt9hDGVcXNCATLsKRK`.
Report to it with `send_message` (claude-code-remote MCP) at each milestone and when you finish. svelters'
`CLAUDE.md` applies throughout; this brief stands alone.

## Why

cadatomic's first agent QC round (qc1, driving the live skelter app through the X server and reading AT-SPI)
found that under skelter:

- **Tab never moves focus**, the palette opens unfocused, menus can't be opened from the keyboard;
- **AT-SPI never sees focus or text:** the window frame is always "focused", text fields expose neither
  Text nor Value, so a screen reader (or a QC agent) can't read what the user typed;
- AT-SPI bounds ignore the UI zoom.

The plan is `PLAN-focus-and-text.md` in this folder (a copy of cadatomic's research note). Its §1 has the root
causes with file:line references at `cd2adc4f`, §2 and §3 the design, §5 the tests, §6 the work breakdown,
§8 the risks. The user decided one change to it: fork AccessKit for the Linux expanded state rather than
filing upstream; that fork is a later step, not this wave.

## Lanes

| Lane | Branch | Steps (plan §6) |
|---|---|---|
| focus | `ci/qa11y-focus` | F1–F5, and the focus half of Q1 (the consumer/atspi_common test helper and §5's focus, restore, bounds tests) |
| text | `ci/qa11y-text` | T1, and the text half of Q1 (§5's text tests) |

Both branch from `origin/claude/determined-gates-c0yj5i` (svelters `master` plus cadatomic QC fix
`94400eeb` and these briefs). Push only your branch; no PRs.

**Shared ground.** The test helper (Q1) is one module in skelter-test: lane **focus** creates it first
(`crates/skelter-test/src/consumer.rs` or similar) and pushes it early, in its first commit that compiles;
lane **text** uses it once pushed (merge `origin/ci/qa11y-focus` into your branch then) and, until then,
writes its tests against the API as the plan describes it. Lane focus owns `skelter-core`'s `focus.rs`,
`a11y.rs`, `frame.rs`, `window.rs`, `common.rs`; lane text owns `skelter-widgets`' text field and kind and adds
`EventCx::request_a11y` in `skelter-core/src/cx.rs` (the only skelter-core file it touches). If you need to
change the other lane's files, tell the coordinator and the other lane first.

The coordinator merges `ci/qa11y-focus` first, then `ci/qa11y-text` (which merges master before its merge).
Then it moves cadatomic's svelters pin and starts wave 2, the cadatomic lane (plan §6, C1–C5).

## Rules that matter most here

- **Build and test as little as possible** (svelters `CLAUDE.md`): `cargo check -p <crate>` while writing;
  run only the tests you wrote, filtered (`tools/dev/headless` for anything that opens a window). Push; the
  five GitHub workflows are the gate. Fix what CI reports in one batch.
- **Tests wait on frames and events**, never the wall clock.
- **No public actions:** no upstream PRs or issues (AccessKit or anyone else). Note them in your record.
- **Don't change behaviour for other skelter apps by default:** the initial-focus change is opt-in (plan §2.2
  item 4, risk 8).
- **No release panics:** the graft-focus guard (plan §2.1, risk 1) needs its test.
- svelters is coordinated, but cadatomic is not yours: don't edit it. What cadatomic needs (wave 2) goes in
  your record as a list.
- Hand off near 400k tokens of context: push, write the handoff in your record, tell the coordinator.
- Never ask the user. Questions go to the coordinator with a recommendation; keep working meanwhile.

## Done means

- Your steps implemented with the plan's §5 tests for them, `cargo check -p <crates> --all-targets` clean,
  `origin/master` merged in, all five CI workflows green on your last push (`tools/dev/ci-wait <sha> --jobs`).
- Record `plans/milestones/QA11Y-<lane>.md`: what landed (commits), the tests and their CI run ids, what you
  did not run, deviations from the plan with reasons, open questions with a recommendation each, and the
  **cadatomic wave 2 notes**: every API, prop or behaviour change cadatomic must adopt (for example the
  root-scale change that the cadatomic probe and tour `A11yFold` must follow, plan risk 6).
- A `send_message` to the coordinator with the sha, the CI run ids and the record's path.
