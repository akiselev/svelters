# rmegui lane S2 (svelters): skelter bugs found while cadatomic leaves egui

Date 2026-10-26. Coordinator: cloud session `session_01Kt5cvt9hDGVcXNCATLsKRK`; report with `send_message`
(claude-code-remote MCP) at each milestone and when done. svelters' `CLAUDE.md` applies. Branch `ci/rmegui-svelters-2`
from `origin/master` (sha in the launch message); push only there; no PRs. Each item with a test (skelter-test, the
consumer/AT-SPI helper where it applies); push after each; record `plans/milestones/RMEGUI-S2.md`. Done when the five
workflows are green on your last code commit.

Found by cadatomic's rmegui wave 1 lanes on the native shell (cadatomic fix-queue rows in brackets):

1. **Sync signal handlers get the event object as their first argument** [FQ-65]. `native-runtime/src/index.js` calls a
   handler of a sync (cancelable) signal as `fn.call(node, ev, ...args)` (around line 1371) while the posted path (around
   1285) passes the args alone, so a Svelte handler like `onchange={(v) => setVisible(v)}` on a checkbox gets the event
   (`expected boolean, got object`). Make both paths pass the signal's args the same way, and give a sync handler its way
   to cancel without shifting the arguments (match what svelters-egui does; document it). A test with a cancelable
   checkbox signal and a value handler.
2. **`<table>` exposes no rows or cells** [FQ-66]: skelter-widgets' table publishes only a `Table` node with counts
   (`table/kinds.rs` ~750). Publish `Row` and `Cell` (or `ColumnHeader`) virtual children for the realised rows, named
   from the cell text, with row/column index, selected state and a click/select action, as the list view does since
   qa11y-fixes (`VirtualChildren::row_a11y`).
3. **ListView rows named and anchorable** [SV59]: a `label(item)` prop (as `<TreeView>` has) passed as the row labels, so a
   row's `ListBoxOption` has the item's name; and a row anchor on the option node, so a combobox's
   `aria-activedescendant` can name the option itself.
4. **`aria-activedescendant` / `aria-controls` re-resolve** [SV60]: skelter-core resolves these anchors when the
   referring node's a11y node is built; a target that appears later (a row realised after the field set the prop) stays
   unresolved until the referring node changes again. Re-emit the referring node when its anchor appears.
5. **A clickable label wider than its text takes clicks only on its text**: a `label` stretched by layout (flex-grow,
   width) with a click handler should hit-test its whole box, as a button does.

Read `plans/milestones/QA11Y-fixes.md` and `RMEGUI-S.md` first. Never ask the user; questions go to the coordinator
with a recommendation. Hand off near 400k tokens.
