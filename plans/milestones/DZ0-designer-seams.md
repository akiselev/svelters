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
