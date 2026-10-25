# Designer wave 1, svelters lane `dz0-followups` (DZ0b2)

The user approved this lane on 2026-10-25 ("ok lets do the stuff we can now"). It finishes the seams that the visual UI
designer needs from svelters, left over from DZ0. The design is cadatomic
`docs/research/2026-10-25-visual-ui-designer.md`. To read it, run `add_repo akiselev/cadatomic` (read access), then
read §3.2 (identity), §3.5 (styles) and §3.12 (agents). The record of what DZ0 built is
`plans/milestones/DZ0-designer-seams.md`. Read it first, especially "Follow-ups".

svelters' `CLAUDE.md` binds you, with the cloud differences in `plans/cloud/batch10/BRIEF.md`:
- setup, `tools/dev/cargo-slot` with the cloud settings;
- build little;
- CI is the gate (`tools/dev/ci-wait`);
- commit and push often.

Where this brief differs:

- **Coordinator:** the cadatomic batch coordinator, cloud session `session_01RQkPkX9QqiFN52Rrofmi7z`. It also
  coordinates svelters (user, 2026-10-25). It merges your branch into `master` with `--no-ff` once all five
  workflows are green. Questions go in your record, each with a recommendation. Never ask the user.
- **Branch:** `ci/dz-followups`. Its base is `master` after DZ0 merges (`ci/dz0-designer-seams`, DZ0 commit `a22930a4`).
  If DZ0 hasn't merged when you start, base on `origin/claude/practical-lamport-y8z11o` and merge `master` in later.
  Push only to your branch. No pull requests.
- **Record:** add a section `## DZ0b2 follow-ups` to `plans/milestones/DZ0-designer-seams.md`, covering:
  - what landed;
  - public API;
  - tests;
  - CI run ids;
  - what you left, and why.

  Don't edit `STATUS.md` or `MATRIX.md`.
- **Consumer:** cadatomic's DZ1, the read-only inspect and outline pane, after its svelters pin advances. Keep every
  API additive and list it in the record.

## Scope, in order (each item lands with its test)

1. **`style.matched` and `style.computed` automation methods**, in `crates/svelters-automation`
   (`session.rs`, `Session::call`). The JSON path over a target (`{"selector"}` or `{"ref"}`) works like `scene.node`:
   - `style.matched` returns DZ0's `EguiBackend::matched_rules(h)` as JSON: sheet, selectors, specificity, line,
     column, `source {file, style_offset}` and decls. The order is cascade order, lowest priority first, as
     `matched_rules` gives it. Mark the declaration that wins for each property.
   - `style.computed` returns the computed style map. AU4 planned it; check `plans/research/E2E-INSPECTOR.md` for the
     shape it promised, and match that shape.
   - Both need read grants only. Add them to the `session.hello` capabilities list, and to the automation docs
     (`docs/egui/rust/automation.md`) and the MCP tool list if the server lists methods.
   - If the automation session can't reach the backend's style data generically, add the narrowest seam that can.
     Say which in the record.
2. **Call sites for dynamic components.** `<svelte:component this={X}>`, `<X>` where `X` is a reactive or member
   expression, and `{@render snippet()}` where the snippet renders components. Today these keep positional HMR keys
   and add no site to `source-loc`.
   - Pass `$$src` and the call's span start, as the static component path does (`codegen/component.rs`, the
     `$n.region({container}, $$src, {start})` call).
   - `{@render}` gets the render tag's span as its site.
   - Extend `crates/svelters-egui/tests/hmr_sites.rs`: inserting a dynamic call before a stateful one keeps its
     state, and `source-loc` lists the dynamic site.
3. **`source-loc` on toplevel windows mounted at the root.** `attach()` in `packages/native-runtime/src/index.js`
   sends the location first, but toplevels mounted at the root don't get one. Find out why (the root mount path
   bypasses it, or the toplevel's node isn't alive yet), fix it, and add a case to
   `crates/svelters-egui/tests/source_loc.rs`.
4. **The Vite path's style source map.** `registerStyles(id, { css, file, map })` gets `file` and `map` only from
   the Rust loader (`svelters-modules` `style_module`). Make dev builds through the Vite plugin
   (`packages/native-runtime/vite.js` and whatever emits `registerStyles` there) pass them too. Use the same
   `o,s,l;…` map text, relative to the `<style>` content start.
   - If the Vite path compiles with stock Svelte, which has no span map, record that as left with the reason and
     skip it. Don't build a second mapper.

## Not in scope

- Anything in cadatomic.
- Designer commands (`designer.*`), which live in cadatomic.
- Changes to DZ0's override and forced-style API.
- Persistent ids in source (rejected, see the research note §3.2).

## Done when

All five workflows are green on your head. The record lists the public API and the CI run ids, and you've reported to
the coordinator (`send_message` to the session above) with the sha.
