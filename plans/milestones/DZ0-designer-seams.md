# DZ0: designer seams (svelters side of cadatomic's visual designer)

Requested by cadatomic's visual designer research (cadatomic
`docs/research/2026-10-25-visual-ui-designer.md`, §5 DZ0). The user asked for it directly on
2026-10-25: "lets go ahead and do DZ0". It covers the svelters prerequisites: per-element source
identity (SV16's dev part), container picking, CSS rule to source, SV30's preview overrides, and
call-site child keys for HMR.

Branch `claude/practical-lamport-y8z11o` (cloud session), on master `3f637701`.

## Delivered

| Item | What | Where |
|---|---|---|
| DZ0a identity | Dev builds, backends with `source-loc`. The compiler emits `$n.source_loc` per element and call sites on static component regions; the runtime sends `file#offset` and the call sites once per node; the egui `source-loc` prop is appended to Widget and menu kinds; `NodeRef::source_loc`, `SceneRef::by_source`; the automation `source=` engine and `scene.query` `source` | compiler `codegen/component.rs`, `semantics::{source_loc, source_locs}`; runtime `source_loc`; egui `scene/source.rs`; automation `selector.rs`, `engine.rs`, `session.rs` |
| DZ0b pick | `SceneRef::hit_stack`: every node under a point, boxes included, innermost first, ignoring `HIT_TRANSPARENT` | egui `scene/hit.rs` |
| DZ0c CSS source | `MagicString::segments`; `ScopedCss::source_map` (runs relative to the `<style>` content); the loader passes `file` and `map` to `registerStyles` in dev; the engine target's SHEET payload gains `SEP file SEP map`; `EguiBackend::matched_rules`; `Stylist::rule_sheet`, `Rule::selector_text` | compiler `svelte/css/{magic_string,transform,mod}.rs`, `scoped_css.rs`; modules `source/mod.rs`; runtime `style.js`; egui `style.rs`; style `cascade.rs`, `sheet.rs` |
| DZ0d overrides | SV30's effect in the scene instead of compiled hooks: `set_override`, `clear_override`, `clear_overrides` and `overrides` hold back the runtime's writes; `force_style` is above every rule | egui `overrides.rs`, `ops.rs`, `style.rs` |
| DZ0e call sites | HMR instance keys `<component>` U+001F `<site>[keys]#n` when the call site is known; the loader's `source_edits` (line-aligned, then per line) as `$n.hmr_edit(file, runs)`; parked keys are translated through the edit | runtime `instancePath`, `hmr_edit`, `parkDescendants`; modules `source_edits` |

## Decisions (made in this lane, open to the coordinator)

- **SV30 as scene overrides, not compiler hooks.** The request asked the compiler to wrap every
  attribute expression in a hook keyed by D4 identity. The designer needs only the effect, a preview
  value that wins over the app's writes until the source is written. Holding writes back in the
  backend gives that with no compiled code and nothing in release builds. Compiler hooks can come
  later if an inspector needs to override values that are not node props.
- **Offsets, not persistent ids.** Identity is `file#offset` at the build that rendered the node. A
  designer re-bases held offsets through its own text edits (cadatomic register, VD4).
- **CSS offsets relative to `<style>`**, so a template edit neither invalidates them nor re-runs the
  sheet (LD2's style-only path is unchanged).
- **Engine target only.** GTK and Qt sheets and their payloads are unchanged. Backends without
  `source-loc` get no new code, so the conformance and op-log goldens are untouched.

## Completion record

Tests added:
- svelters-egui `source_loc` (3), `style_source` (2), `overrides` (3), `hmr_sites` (2). `hmr_sites` fails
  with call-site keys disabled.
- svelters-automation `source_loc` (2) and the selector unit test.
- svelters-modules `source_edits` unit test.
- compiler `MagicString::segments` unit test.

Also run and passing: svelters-compiler (all), svelters-style, svelters-modules, svelters-host
`modules`/`hmr`/`sched`, svelters-egui `spec`/`hmr`/`hmr_state`/`live_styles`, native-runtime node tests
(138). `egui.json`, `egui.d.ts` and the widget reference were regenerated
(`gen_reference.py --check` and `gen_utilities.py --check` clean).

Also, with the fixture bundles built (`pnpm build`), the full svelters-egui and svelters-automation
suites pass. The one exception is `tier0::text_edit_uses_the_primary_selection`: it passes alone (3 of
3 runs) and fails only when the whole `tier0` file runs. It installs a process-global provider, so this
is most likely interference between tests; it was not checked against `3f637701`. The python half of
`pnpm docs:test` passes.

Not run: the five GitHub workflows (no CI access from this session), GTK, Qt, AppKit and WinUI,
conformance, and the Starlight half of `pnpm docs:test`.

## Follow-ups

- Dynamic components (`<svelte:component>`, `{@render}` of a component) carry no call site yet. They
  keep positional HMR keys and add no site to `source-loc`.
- Toplevel windows mounted at the root get no `source-loc`.
- `style.matched` / `style.computed` JSON methods for agents (AU4 planned `style.computed`).
- The Vite plugin path doesn't pass `file`/`map` to `registerStyles` (dev through Vite has no CSS
  offsets).

## DZ0b2 follow-ups

Lane `dz0-followups` (designer wave 1, `plans/cloud/designer-wave1/LANE-dz0-followups.md`), branch
`ci/dz-followups`, based on DZ0 (`5c1f83c6`, on `claude/practical-lamport-y8z11o`) because DZ0 had not
merged into `master` yet. Consumer: cadatomic DZ1 (read-only inspect and outline).

### What landed

| Item | What | Where |
|---|---|---|
| 1 `style.matched`, `style.computed` | Automation methods over the JSON path (target `{selector}` or `{ref}`, `read` grant), listed in `session.hello` capabilities and `docs/egui/rust/automation.md`. `style.matched`: DZ0's matched rules in cascade order (lowest first) with sheet, selectors, specificity, line, column, `source {file, style_offset}` and decls, each decl marked `winner`. `style.computed`: E2E-INSPECTOR §1.4's shape, `{values, custom, inherited, rules}`: computed values as CSS text, custom properties, inherited-from (the nearest declaring ancestor's ref, per inherited property the node doesn't declare) and the matched rules | automation `session.rs`; egui `style.rs`; style `cascade.rs` |
| 2 dynamic call sites | `<svelte:component this>`, `<X>` with a reactive or member `X` and `{@render}` (static and reactive) are call sites in a dev build for a backend with `source-loc`: the compiler places their block in `$n.region(container, $$src, start)` (the call's or the render tag's span start). HMR instance keys take every site between the anchor and the parent instance (innermost first, U+001F-separated), so the edit translation already covers them; `source-loc` lists them through its owner walk | compiler `codegen/component.rs` (`site_region`); runtime `instancePath` |
| 3 root toplevels | Cause: the egui `Window` kind had no `source-loc` prop, so the compiler emitted no `$n.source_loc` for `<window>` (the runtime's `attach()` was right). The prop is appended to `WINDOW_PROPS`; `egui.json`, `egui.d.ts` and the window reference page regenerated | egui `kinds/containers.rs` |
| 4 Vite style map | `svelters-compile --css --json` gains `css.map` (the same `o,s,l;…` text, `ScopedCss::source_map_text`). In dev the Vite plugin scopes every egui `<style>` with `svelters-compile --css` and passes `file` and `map` to `registerStyles`; release builds without `@apply` still use stock Svelte (CS5e). Other backends' Vite paths are unchanged | compiler `scoped_css.rs`, `svelters-compile.rs`; `packages/native-runtime/vite.js` |

The seam for item 1: the automation `Session` holds a `SceneHandle`, not the `EguiBackend`, so the
style reads moved from the backend onto `NodeRef` (the scene owns the style engine).
`EguiBackend::matched_rules` now delegates to the same code.

### Public API (all additive)

- svelters-style: `Stylist::winners(el, inline, caches) -> BTreeMap<String, &Declaration>`: the winning
  declaration per property (a shorthand keyed by each longhand).
- svelters-egui: `NodeRef::matched_rules()`, `NodeRef::computed_style()`, `NodeRef::inherited_from() ->
  Vec<(String, Option<NodeId>)>`; `style::DeclInfo::winner: bool` (a new field: code that builds a
  `DeclInfo` literal must add it; nothing outside svelters does yet). The `Window` kind's `source-loc` prop.
- svelters-automation: `Session::style_matched(Ref) -> Vec<MatchedRuleInfo>`, `Session::style_computed(Ref)
  -> ComputedInfo`, `ComputedInfo {values, custom, inherited, rules}` with `to_json`, `rule_json`; JSON
  methods `style.matched` and `style.computed`.
- svelters-compile: `css.map` in `--css --json` output.
- Runtime: HMR keys of calls under `{@render}` gain the render sites (dev, `source-loc` backends only);
  `source-loc` of nodes under `{@render}` and dynamic components lists those sites.

`winner` is decided among rules, `style=` and forced declarations. A prop the runtime set
(`NodeCore::explicit`) still wins over all of them and is not reflected (CSS.md §2.8); `@layer` names are not
in the output (the stylist keeps only a rank).

### Tests

- svelters-automation `style` (2): JSON shapes, cascade order, winners, `<style>` offsets, ref and
  selector targets, `node.not_found`, inherited-from for `color` and a custom property.
- svelters-egui `hmr_sites` (+2): a dynamic call inserted before another and a `{@render}` inserted before
  another keep the later one's state. Both fail with the runtime change reverted (checked).
- svelters-egui `source_loc` (+2): sites of `<svelte:component>`, `<Comp>`, `<parts.Named>` and a call
  inside a rendered snippet (call, then render tag); a root `<window>` from `TestApp` has its `source-loc`.
- svelters-compiler `q3w_css_wiring::cli_css_flag`: `css.map` runs are untouched `<style>` text.
- native-runtime `vite-style-map.test.mjs` (4): dev egui sheets, with and without `@apply`, carry `file`
  and `map` whose runs are untouched `<style>` text; release ones don't.

Run locally, all passing: the tests above, svelters-egui `spec` (after `SVELTERS_UPDATE=1`),
`gen_reference.py --check`, `gen_utilities.py --check`, native-runtime `pnpm test` (138 + the new file),
the python half of `pnpm docs:test`. Not run locally: everything else (CI is the gate).

CI, all five green on `db74c6d6` (after merging `master` `d6debc5c`): js `40169258991`, linux-gtk
`40169259022`, linux-qt `40169259007`, macos-appkit `40169258986`, windows-winui `40169259047`. The commit
after it only adds these ids.

### Decided (coordinator, 2026-10-25)

- egui's Vite path compiles every `<style>` with `svelters-compile --css` in dev, so both load paths give
  the designer the same offsets. Reasons: the user chose svelters' own compiler and loader as the real
  path (Vite is a dev convenience only), and the port is byte-identical to svelte@5.57.2 on the corpus.
  Other backends' Vite paths stay as they are.

### Left, and why

- `@layer` names and "inherited from" for properties set by explicit props are not reported (see above).
