# qc-a11y lane fixes (svelters): focus and list follow-ups found by cadatomic's live QC

Date 2026-10-26. Coordinator: cloud session `session_01Kt5cvt9hDGVcXNCATLsKRK`; report with `send_message`
(claude-code-remote MCP) at each milestone and when done. svelters' `CLAUDE.md` applies; `BRIEF.md` in this folder
(qc-a11y wave 1) is the background. Branch `ci/qa11y-fixes` from `origin/master` (sha in the launch message); push only
there; no PRs. Lane S (`ci/rmegui-svelters`) is fixing the hovered-button key swallowing in skelter-widgets; don't
touch that, and merge `origin/ci/rmegui-svelters` if your work meets it.

Found by cadatomic's shell-a11y lane driving the live skelter app through AT-SPI:

1. **List rows as options.** skelter-widgets' list view publishes its rows as children with label/button roles, not
   `ListBoxOption` with `selected`, so a Svelte list used as a picker (cadatomic's command palette,
   `PalettePanel.svelte`) reads as a list box of buttons. Publish rows as `ListBoxOption` with `selected` (and
   `position_in_set`/`size_of_set`); let Svelte set `role` and `aria-selected` on a node (svelters-skelter Widget props),
   and let a text field name a list's selected row as its active descendant (`aria-activedescendant` by anchor; F4 added
   the prop on skelter-core), so a combobox pattern works from Svelte.
2. **Focus ring after a programmatic move.** After `Document::focus(node, FocusReason::Keyboard)` called by a host
   outside a dispatch (cadatomic moves focus between title-bar buttons on Left/Right), AT-SPI and the probe follow but
   the old button keeps drawing its focus ring and the new one draws none; moves made by `doc.tab` repaint correctly.
   Repaint the ring on FocusIn/FocusOut delivered outside a dispatch (and derive the ring's visibility from the focus
   reason, not the event context).
3. **Sibling dialog scopes opening in one frame:** the last processed sibling (NodeId order, generation bits) wins focus.
   Make it deterministic: outermost first, then tree order, and document it.

Each with a skelter-test test (the consumer/AT-SPI helper where it applies). Record `plans/milestones/QA11Y-fixes.md`
(commits, CI run ids, what cadatomic must change). Done when the five workflows are green on your last code commit.
Never ask the user; questions to the coordinator. Hand off near 400k tokens.
