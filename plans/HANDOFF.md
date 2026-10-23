# Handoff: svelters + egui for cadatomic (2026-10-23)

Written by the svelters coordinator (`svelters-agent`, WSL PC) when the user paused at 90% of the weekly quota
("stop after these active lanes are done and write a massive handoff"). Read this file, then `CLAUDE.md`, then
`STATUS.md`. Everything here is a pointer into those files and the milestone records; when they disagree, the milestone
record and the code win.

## 1. Where we are

**Goal (user, 2026-10-21/22):** svelters' egui backend is cadatomic's UI layer. Rust owns the shell (docking, tabs,
toolbars, menus, windows); Svelte renders panels, dialogs and overlays, many roots at once. **svelters draws every
widget itself** (user, 2026-10-22: egui is only painter, input, text shaping, layers and AccessKit), styled by our Rust
CSS engine, with VS Code's Modern look as the baseline plus our affordances (DESIGN.md).

**svelters-egui core, done and merged:**
- Embed: many roots per egui frame (EM1), secondary viewports (EM2), Rust exports to JS (E0–E3, RA1–RA3), the live module
  loader with `<style>` and hot reload (LD1–LD3, H1/H3), assets (AS1), automation and e2e (AU1–AU2b), deterministic
  scheduler (SCHED1/1b).
- Scene graph with full Rust access (SG1–SG6): columns + `NodeId`, extension columns, change journal, queries, provider
  passes, memory budget (500 B/node at 100k nodes), paint cache and culling. `ARCH-SCENE.md` §14.
- Events, focus and keys (EV1, EV2b, EV3, EV3b): focus scopes, ShortcutOverride, edit commands, cadatomic's keymap hooks.
- CSS engine on egui (CS0–CS5f, OW-CSS1/2, U2): cascade, layout props, transitions with longhands, `::part()` with a
  rule index, `-sv-platform` media.
- Own widgets (OW0–OW7, OW-RUST): button, toggle, checkbox, radio, switch, label (RT2 rich text, selectable), slider,
  range-slider, drag-value, scroll-area/scrollbar/split-view/panel, menus/menu-bar/context-menu/popup/combo/select/
  tooltip, virtual-list and tree rows with DnD and selection, calendar/color-editor/date-picker/color-picker, modal,
  inner-window, toasts; `imm` builders so Rust chrome (berth) draws with the same widgets. `docs/egui/START-HERE.md`.
- Icons: registry + `<icon>` by key with CSS-tinted colour slots (ICR1); zoom with SVG at physical size (ZM1).
- Utility classes (U0–U4): vocabulary + generator, resolver + utility layer, egui wiring (`EmbedConfig::utilities`,
  off by default), compiler checker with fix-its and `@apply`. Left: U5–U8.
- Visual gate vs VS Code (VIS1c): 213/354 checks on the Windows references, a ratchet over `conformance/visual/baseline-*.json`.

**Still on the egui wrapper (switch `SVELTERS_OWN_WIDGETS=none`), or missing:** text-edit (OW4, on quire-text),
tabs (OW5c), dialog-buttons/status-bar (OW7b), sectioned date field and path-field (OW6b), the
`use:` exports and collapsing-header extras (OW1a4), egui_extras/egui_table removal and lint to zero (OW-DONE).

**Master:** `2f882aac` (U3 on top of U4, FX6b, OW5b1, OW7b stage 1, WG3s, U1+U2, FX6, OW1a3, OW7, OW5a1b, U0, EV3b, CB1,
OW-RUST, VIS1c, DOC2); see §3. Every merge this session was CI-green on its lane branch and checked on the merged tree
(`cargo check` of the shared crates + the touched tests), and master CI was green at 943b4c67 and ec8809fe.

## 2. In flight at wind-down (finish, merge, then stop)

The user (2026-10-23): "You can let them finish and fix bugs they find. We just dont want any new lanes". Each lane
finishes its full scope, gets CI green, and writes a handoff section in its milestone file.
**Exception (user, 2026-10-23):** the ECAD work (ecadabra, under cadatomic's sessions) ignores the wind-down and uses
the rest of the quota; it is cadatomic's, not tracked here.

| Lane | Machine / session | Branch | What it is | Where it stops |
|---|---|---|---|---|
| U3 | WSL, coordinator's agent | `ci/u3` | utility classes wired into egui: `apply_classes` → `ensure_utilities`, `EmbedConfig::utilities`, theme token names, `@layer components, utilities`, GTK/Qt warning, `imm .class()`, gallery page | MERGED 2f882aac (ci/u3 d2cedb52); left: breakpoints from the active theme, U8 docs guide |
| U4 | WSL, coordinator's agent | `ci/u4` | compiler checker for utility classes (static class, `class:`, clsx, script scan), CX1 codes incl. `utility_bad_value`, per-kind validation, `@apply`, fix-its | MERGED f7f67386 (ci/u4 800f33e4); left: the live loader passes no utilities option or theme tokens yet |
| FX6b | WSL, coordinator's agent | `ci/fx6b` | embed flush via the host job drain instead of `flushSync` (interrupt inside flushSync left `is_flushing_sync` set), plus an e2e egui test if cheap | MERGED f7f67386 (ci/fx6b 21084825), with its e2e egui test and a second drain bug fixed |
| OW5b1 | thinkpad-svelters | `ci/ow5b` | own `table` without egui_table (virtual rows/cols, frozen header, resize, parts, sort, hidden columns, header-state) | MERGED 55cf5c9d (ci/ow5b cc427a51); OW5b2/OW5a2 handoff in OW5b-table.md |
| OW7b | thinkpad-svelters | `ci/ow7b` | stage 1: declarative `<toast>` (per-run CSS for cadatomic); then dialog-buttons, status-bar, auto_default, keyboard window resize, `:exiting`, built-in-kind `::part()` gap for modal/inner-window | MERGED (ci/ow7b 4079bd1a); OW7c (status-bar + status tips, ci/ow7c) finishing; left: `buttons=` shorthand, yes/no roles, destructive never default, orientation, keyboard window resize, `:exiting`, VIS1 probes |
| OW1a4 | thinkpad-svelters | `ci/ow1a4` | `use:` exports, collapsing-header accordion/`slot="end"`/icon | finishes only if its agent had started; otherwise queued (§4) |
| OW4 | macbook-svelters | `ci/ow4` | own text-edit on quire-text (https rev) | full scope |
| WG3s | macbook-gui (stopped) | `ci/wg3s` e317e65a | scroll pins 9 (wheel, chaining) and 10 (gesture, overscroll), `Driver::scroll_wheel/scroll_gesture`, the user's flick `conformance/scroll/macos-flick.json`, golden `nsscrollview-15.7.5.json`. egui vs AppKit 0/20 equal: wheel 40 vs 10 pt and animated, momentum 1:1 vs ~720 pt, no rubber band; AppKit doesn't chain the wheel | MERGED b0976c1e; left: the fixture's inner area never gets input, a Native::Mac scroll row for OW2, Linux/Windows drivers, noisy timing (±60%) |

**The coordinator merges each as it turns green** (rules in §6), records it in STATUS, and sends cadatomic the sha of
anything it uses (FX6b goes into cadatomic's next pin).

## 3. Merged this session (2026-10-22/23, in order)

OW-CSS1, LD2, RT1, AU1b, OW0, EV1, docs regen + `tools/dev/ci-wait`, OW1a, Q4b, RT2, SG6, LD3, AU2b, WG3m, OW-CSS2,
FX4, CS5f, OW2, RA3, OW1a2, ZM1, FX5, OW3, DOC2, utility-css research, ICR1, OW5a1, OW1b (with the with_cx allocation
fix), OW6, EV3b, CB1, OW-RUST, VIS1c, DOC2 follow-up, U0, OW5a1b, OW7, OW1a3, FX6, U1+U2, WG3s, OW7b stage 1, OW5b1, U4, FX6b, U3. Details per lane in STATUS.md
and `plans/milestones/<lane>*.md`.

## 4. Queue after the pause (in priority order)

Start each on a fresh agent (CLAUDE.md "Agents"). Lane designs are in the files named.

1. **Finish the own-widget set** (ARCH-WIDGETS.md, WIDGET-PARITY.md; the user fast-tracked "that list of widgets"):
   OW5b2 (table parity: selection-unit, edit triggers, movable columns, grid lines; split decided by the coordinator
   2026-10-22), OW5a2 (grid, rubber band, sticky headers, multi-column tree after OW5b1, rename, expand animation),
   OW5c (tabs), OW7b leftovers (`buttons=` shorthand, yes/no roles, destructive never default, orientation, keyboard
   window resize, `:exiting` on modal/inner-window/`show()` toasts, VIS1 probes for toast and dialog-buttons), OW6b (sectioned date field, month/year sub-pickers, rgb/hsv drag-value parts, `path-field`), OW7b
   remainder, OW3b (scrollable tall menus etc.), then **OW-DONE** (lint to zero, drop egui_extras/egui_table, audit every
   built-in kind for undeclared `::part()`s: OW7b found modal/inner-window/toast-host parts never reached CSS).
2. **Utility classes U5–U8** (UTILITY-CSS.md §7) after U3/U4, plus U3/U4's leftovers. Critical path U3 → U5/U6.
3. **Scroll conformance:** fix our egui scroll to match WG3s's macOS findings (Policy::scroll per platform, wheel
   step and animation, momentum, rubber band) and add the Linux/Windows drivers.
4. **Visual conformance:** VIS1c follow-ups (VIS1c-visual-gate.md §7/§8: affordance deviations listed as allowed
   differences, not failures; dark secondary-pressed was dropped on purpose), `ci/vis1m` (macOS references at 2x: 436
   files, not merged; decide whether the gate uses them), WG3s Linux/Windows halves, MATRIX WinUI numbers.
5. **Design system:** DS5/IC2 codicons as SVG through ICR1 (keep their CC-BY notice); our own icon style is the user's
   wish ("distinctive, consistent", "I love the icons with the colored accents"), codicons are interim.
6. **Research sweep backlog** (STATUS row): EM3 adaptive hidden roots + SV35, LS2 row-model trait (SV41/SV44), ID1
   identity, CX2, CX3b, CX4 then LSP, TK3+ a11y announce, SCHED1c, PK1 plot pack, CS-CQ container queries, Zed #2/#4.
7. Stopped and kept only as spec (don't merge): `ci/fx3`, `ci/fm1`, `ci/wg6`, `ci/vis1` (VIS1b), `ci/e3` (old),
   `ci/fix-alloc` (redundant with OW1b), `ci/ow-rust` head (only a master merge past what was merged).

## 5. Decisions

**By the user (do not reverse without asking):** CLAUDE.md "Current focus" and "Who decides" lists, plus:
- own widgets everywhere; egui is the engine only; widgets "configurable and stylable as much as qt and more";
- design: Modern, affordances on, icons separate, our own icon language later (codicons interim);
- utility classes (UTILITY-CSS.md §9): metadata + Tailwind names, runtime generation, below component CSS and opt-in,
  **our tokens only**, typed values, `part-<name>:`;
- security: leave API room only (`#[non_exhaustive]`, principal ids), build nothing now;
- licences: svelters MIT; quire MIT OR Apache-2.0 and public; berth, cadatomic, ecadabra declare none until the user
  decides (likely AGPL). No LICENSE files otherwise;
- committed files never use our credentials; agents may use `gh` interactively;
- never force-push; no build artifacts in git; hooks never run builds; WSL disk is `df -h /mnt/c`.

**By the coordinator this session (recorded in the files named):** OW5b split (OW5b milestone, WIDGET-PARITY);
OW1a3's VIS1c baseline edit (OW1a3 §2.3, VIS1c §7/§8); U2 merged with U1 rather than alone; FX6b's approach (host job
drain, no Svelte patch; STATUS FX6 row); FM1's `button-order` on any `box` not adopted, `dialog-buttons` is the one place
(STATUS OW row).

## 6. How the work runs (operations)

- **Machines** (CLAUDE.md table): WSL PC = coordinator + main-cadatomic; thinkpadx1 = thinkpad-svelters (own-widget
  lanes) + thinkpad-cadatomic (berth, CH1 pins); macbook = macbook-svelters (OW4), macbook-gui (macOS GUI checks),
  macbook-cadatomic (quire). MacBook ssh: `ssh -o HostName=192.168.86.82 -o HostKeyAlias=macbook.local macbook`.
- **MacBook** was thermally throttled: BUILD_SLOTS=1 (backup `~/.config/build-slots.env.bak`; restore when
  CPU_Speed_Limit is 100 for ~20 min). Disk: check `df` before slot builds, nothing below 15 GB.
- **Builds:** `tools/dev/cargo-slot` for every cargo command; check/clippy take no slot; filtered tests only; CI is the
  gate. `tools/dev/headless` for egui/GTK tests. pnpm is `corepack pnpm`.
- **CI waits:** `tools/dev/ci-wait <sha> [--once|--jobs]` (no credentials, 600 s) or one `gh run watch`. Every push to
  master cancels the previous master run, so fold STATUS edits into the merge commit and batch merges.
- **Merging** (coordinator): `--no-ff`; a lane that lacks current master gets merged and then
  `cargo check -p <shared crates> --all-targets` + the lanes' own tests. Generated files: rerun generators, in this
  order: `SVELTERS_UPDATE=1 tools/dev/headless tools/dev/cargo-slot cargo nextest run -p svelters-egui --test spec`,
  `python3 tools/metadata/style/gen_props.py`, `python3 tools/docs/egui/gen_reference.py`,
  `python3 tools/metadata/style/gen_utilities.py`. Fixture bundles some tests need: `corepack pnpm --filter
  ./fixtures/lists-egui build`, `--filter ./fixtures/gallery-egui build` (after `cargo build -p svelters-compiler --bin
  svelters-compile`).
- **Known local-only quirk:** the VIS1c gate (`-p svelters-testkit --features snapshots -E 'binary(vis1)'`) prints its
  score and then SIGSEGVs at exit under WSL's Mesa; CI passes it. Compare `target/vis1c/windows/report.md` with the baseline.
- **cadatomic interface:** cadatomic pins svelters by sha (CH1 on the ThinkPad). Requests flow through cadatomic's
  `docs/plan/04-kernel-requests.md` and STATUS "Requests from cadatomic". Send main-cadatomic (`cadatomic-6b`) the sha
  of each merge it uses.

## 6b. cadatomic at the pause (from main-cadatomic, 2026-10-23)

- cadatomic master 6275526 or later (CH1 merged). Its handoff: cadatomic `STATUS.md`; decisions `docs/plan/00-decisions.md`;
  deferred `docs/plan/REVISIT.md` (handoff commit e98384d).
- Pins: svelters 943b4c67 (next: 5aefc137 once green: FX6, OW1a3, U1, U2; the advance drops cadatomic's
  selectable_labels override), berth 3380aa0, quire 2087b43, cadabra3 ecdb54b, resolvent b604f93, parasolid-rs 02c5699.
- SV requests proposed by the ECAD GUI note, filed when EU1 is scheduled: SV49 SG3 hit tests from a non-scene-node
  ViewClass pane; SV50 pointermove for overlay roots; SV51 automation drag/pointer_move/wheel; SV52 columns and tree rows
  ahead of CH4. Finding: svelters has no GPU canvas node (canvas gestures tested through kittest for now).
- cadatomic next: the svelters pin advance, then K1c, LI1b, FW2, CH2, FW3, FW4, MD1–3 (WSL, once C: has 40 GB free).
  berth: BD6o macOS rerun, then the berth pin advance. quire: P1 part 2.
- ecadabra E2 keeps running on the WSL PC through the pause (user exemption). **Check with main-cadatomic before the
  `wsl --shutdown` for the VHDX compaction.**

## 7. For the user to revisit

- Defaults taken overnight 2026-10-22 ("we will revisit later"): ARCH-* "Decided" sections, FW1 §6, EV2b/SG2 notes.
- quire's E5 re-highlight target (7.8 ms vs 2 ms on a 20k-line file).
- SH1 macOS IME/AZERTY/Reduce Motion manual steps (moved to cadatomic's revisit docs, user 2026-10-22).
- Whether VIS1 should gate on the macOS 2x references (`ci/vis1m`).
- Our own icon visual language (after codicons interim).

## 8. Housekeeping still owed

- **WSL VHDX compaction** (user: at the end, needs `wsl --shutdown`): after every session has pushed. C: had 35 GB free.
- Delete merged lanes' worktree `target/` dirs (each lane deletes its own; check `du -sh .claude/worktrees/*/target`).
- parasolid-rs: origin/master stays `b9f8b46` (no force-push); the ThinkPad's RE commits are on `wip/thinkpad-re-r37-r89`.
- ThinkPad auctions stack is stopped; restore with `systemctl --user start auctions-postgres auctions-embed
  auctions-rerank cloudflared-auction-watch && sudo systemctl start postgresql`. Its docker logs should get a size cap.
