# Lane FOCUS: one focus owner, and focus that reaches AccessKit

Repository svelters, branch `ci/qa11y-focus` from `origin/claude/determined-gates-c0yj5i`. `BRIEF.md` in this
folder applies. Read `PLAN-focus-and-text.md` §1.1, §1.3, §2 (all), §3.3, §5 (skelter-core and consumer tests),
§6, §8.

## Steps, in order (plan §6)

1. **Q1, focus half, first:** the skelter-test helper that folds a frame's `TreeUpdate`s into a real
   `accesskit_consumer::Tree` (`is_host_focused = true`) and feeds the same updates to an
   `accesskit_atspi_common::Adapter` with a recording `AdapterCallback` (Linux only, `cfg`). Push it as soon as it
   compiles: lane text uses it.
2. **F1:** deferred focus requests (`pending_focus`, applied after restyle), the focus fixup rule, and the
   generalised restore stack (dialog scopes and overlays; `toplayer.rs` `restore_focus`).
3. **F2:** a `dialog` scope that appears takes focus (`autofocus` or first tab stop); `autofocus` outside a
   dialog; initial focus, **opt-in** by an explicit prop.
4. **F3:** `FocusTarget { node, sub }`; the graft-sent guard (never name an unsent graft in `TreeUpdate.focus`;
   no consumer panic); the ancestor fallback; the root's `scale(window.scale)` transform and the A11Y mark on
   a scale change; `set_root_window_bounds` at adapter creation; skelter-test's `A11yTree` bounds kept in
   points.
5. **F4:** the ROLES additions and the common props `aria-haspopup`, `aria-checked`, `aria-modal`,
   `aria-activedescendant`, `aria-controls` (plus svelters-skelter's tier0 aliases).
6. **F5:** skelter-egui-island's tab-out protocol (`focus_entered(dir)`; Tab consumed only when egui moved
   focus inside without wrapping; respect egui's `lock_focus`).

Each step lands with its §5 tests. Push after each step.

## Not yours

T1 (text runs) is lane text's. F6 pane groups, menus' roles in cadatomic and everything in cadatomic are wave 2.
The AccessKit fork for expanded state is later.

**Record:** `plans/milestones/QA11Y-focus.md`.
