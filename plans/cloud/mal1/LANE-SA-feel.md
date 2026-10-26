# MAL1 lane S-A: feel (svelters)

Copied from cadatomic `docs/notes/mal1/LANE-SA-feel.md` at cadatomic master `b82ea4b9` (2026-10-26), with the svelters-side context.
svelters' `CLAUDE.md` applies. Coordinator: cadatomic session `session_016CTvCHJyR4hx83LMP4XD5y` (send_message).

svelters' `CLAUDE.md` applies. Its CI is the gate: all five workflows green, waited on with
`tools/dev/ci-wait`. Branch `ci/mal1-feel` in akiselev/svelters, from svelters master. Record in
`plans/milestones/MAL1-FEEL.md`.

**Requests:** SV61–SV65 in cadatomic's `docs/plan/04-kernel-requests.md` (filed in step 0). The detail and the evidence
are in architecture §I.13 and `notes/2026-10-26-cfgarch/settings.md` §4.7. **Keep every seam generic:** no cadatomic
names, keys or concepts.

| # | Request |
|---|---|
| SV61 | Show the `tooltip` surface for the common `title` prop: hover, then `Tooltip::step` with the document's `Policy`, then the overlay. Fold in `interact::Tooltipable`. |
| SV62 | `host::set_policy` re-derives the copied fields. Every reader reads `Policy` on use: caret blink, `auto_default`, single selection, a focused text field. The table uses `Policy`, not its own 4.0 px and 500 ms. |
| SV63 | The host subscribes to `SettingsBackend::subscribe` and invalidates `s:<key>` on the JS thread. |
| SV64 | The key hook takes `(&Document, WindowId, target, &KeyEvent)` through an extensible context struct. |
| SV65 | `restyle_node` sets `inherited` when the custom map changed (pointer-equal first), with a test. |

**Order:** SV65 and SV64 first (small, independent), then SV62, SV61, SV63.

**Done:** CI green; the record names the sha and the CI run ids; one line per request for cadatomic's
`04-kernel-requests.md`.

## SV66 (rev 3)

Also SV66: host-set media preferences (reduced motion, contrast) on the document, plus an OS probe, generic. In this lane if small (≤0.5 d), else record it for MAL2.

## Request text

| SV61 | Show the `tooltip` surface for the common `title` prop: hover, then `Tooltip::step` with the document's `Policy`, then the overlay; fold `interact::Tooltipable` into it. The `tooltip` kind and its two timers have no caller outside tests (`views/tooltip.rs:199`, `interact/tooltip.rs:35`; WIDGET-PARITY OW3 "MISSING"). Checked at svelters `6689112c` (MAL1, design §I.13) | MAL1's live tooltip delay (B1); lane S-A | filed 2026-10-26 (MAL1 step 0); lane S-A |
| SV62 | A live `Policy`: `host::set_policy` beside `set_theme`, re-deriving the copied fields (`auto_default`, single selection, caret blink), with every reader reading the policy when it uses it. The table uses `Policy` instead of its own 4.0 px / 500 ms (`host.rs:419-425`, `skelter/src/backends.rs:99-113`, `table/mod.rs:82`, `text/kind.rs:322`) | MAL1's feel keys (B1's `Policy` sink) | filed 2026-10-26 (MAL1 step 0); lane S-A |
| SV63 | The host subscribes to `SettingsBackend::subscribe` and invalidates `s:<key>` on the JS thread (`dx/settings.rs:71` has no caller; `refresh()` is manual) | the Svelte `AppSettings` bridge over one store (B1, CFG-F3) | filed 2026-10-26 (MAL1 step 0); lane S-A |
| SV64 | The key hook takes `(&Document, WindowId, target: NodeId, &KeyEvent)` through an extensible context struct, so a later pre-dispatch pointer filter is additive (`input/dispatch.rs:699` passes no focus target) | `describe.key` and the keymap's focus-depth resolution (B4) | filed 2026-10-26 (MAL1 step 0); lane S-A |
| SV65 | A custom-property change on a node restyles its descendants: `inherited \|= !old.custom_eq(&new)`, pointer-equal first (`skelter-core/src/style/cascade.rs:694-708, 815`) | theme and token switches without stale descendants (CFG-F1, B1) | filed 2026-10-26 (MAL1 step 0); lane S-A |
| SV66 | Host-set media preferences (reduced motion, contrast) on the document, plus an OS probe, generic (no cadatomic keys) (design §V.7, rev 3; the design's SV66–SV73 shift to SV67–SV74) | B1's media preferences (`ui.motion` turning off camera transitions) | filed 2026-10-26 (MAL1 step 0); lane S-A if small, else MAL2 |

## Also

- Mirror SV61–SV66 in svelters `STATUS.md` "Requests from cadatomic" in your first commit.
- The cadatomic pin is `6c1ea458` (svelters master with S2); you branch from svelters master `6c1ea458`.
- Never ask the user. Questions go to the coordinator with a recommendation.
