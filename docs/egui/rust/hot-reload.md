---
title: Hot reload
description: "What an edit does to a running app: component swaps, kept $state, style-only restyles, and when it falls back to a full reload."
status: available
verified_at: d0542a3a
sources:
  - crates/svelters-host/src/{lib,reload,quickjs}.rs
  - crates/svelters-host/src/hot.js
  - packages/native-runtime/src/index.js   # $n.hmr, hmr_keep, setHmrPreserveState
  - crates/svelters-egui/tests/{hmr,hmr_state,hmr_sites,live_styles}.rs
  - crates/svelters-modules/src/source/mod.rs   # source_edits, $n.hmr_edit
  - plans/milestones/H1-component-hmr.md
  - plans/milestones/H3-state-preservation.md
  - plans/milestones/H1b-H3c-propagation-and-child-state.md
  - plans/milestones/LD2-live-styles.md
  - plans/milestones/LD3-loader-fixes.md
  - plans/milestones/DZ0-designer-seams.md  # call-site keys (pending)
---

# Hot reload

With the source loader in `dev` + `hmr` mode, an edit swaps the changed components in place: the rest of
the UI, the Rust side and the component's `$state` stay. This works in release builds too (the GUI edits
itself). Vite is not involved.

## Turn it on

```rust
// not run (the shape of crates/svelters-egui/tests/hmr_state.rs)
let mut cfg = SourceConfig::for_path(&dir)?;
cfg.dev = true;
cfg.hmr = true;
let entry = embed.load_from(Rc::new(SourceProvider::new(cfg)?))?;

embed.host().watch()?;                                   // a file watcher thread, or:
let report = embed.host().apply_changes(vec![path]);     // you report the changed files (editor saves)
```

`svelters-egui-run --entry ui --dev --watch` does the same for a standalone app. `request_reload()` always
does a full reload.

`apply_changes` returns a `ReloadReport { generation, ok, elapsed, error, hot, fallback }`. `hot` lists the
swapped modules; `fallback` says why it reloaded fully instead.

## What an edit does

| You edit | Result |
|---|---|
| a component's markup or script | that component's instances are swapped in place; parents, siblings and other roots are untouched |
| a component's `<style>` only (egui) | its sheet is replaced and restyled; **no remount**, all state kept |
| a `.svelte.js` / `.ts` module | its importers re-run up to the nearest accepting component |
| a file the module graph doesn't reach | nothing |
| `package.json`, the spec | full reload |
| a broken edit (compile or runtime error) | the running version stays; the error is reported; the next good edit applies |

A swap builds the new version **before** destroying the old one. If it throws, the partial build is
dropped and the old version keeps running.

## What is kept

| Kept | Not kept |
|---|---|
| Rust: services, stores held by Rust, everything outside JS | — |
| other components, roots and their native widgets | — |
| top-level `let x = $state(…)` / `$state.raw(…)` in the edited component, per instance | `$state` in functions, blocks, class fields; destructured declarations |
| top-level `$state`, `writable(…)` stores and `createContext()` in `.svelte.js` (same objects) | `$derived` (recomputed), `$props` (come from the parent) |
| children of an edited parent, by call site: on egui in dev, the component tag's place in the parent's source and the keyed `{#each}` items around it (DZ0); elsewhere by position (`<parent>/<component>#n`) | children created later by `{#if}`/`{#each}` updates; on backends without call sites, a sibling of the same component inserted before them shifts the numbering |
| `bind:this` objects | native UI state of egui widgets (scroll offsets etc.) |

**The rule is all or nothing per component.** The kept declarations (kind, name and initializer text,
in order) form a signature. If it is unchanged, the values carry over. If you add, remove, rename a
`$state` or edit an initializer, the whole component starts fresh. Template and `<style>` edits and
comment or whitespace changes don't change the signature.

Only values that changed carry over: a `$state` still equal to what its initializer produced runs the new
initializer, so a changed callee shows. Objects and proxies are always carried.

**Call sites follow the edit (egui, dev).** The loader tells the runtime how the parent's source
changed (whole lines inserted or removed, or the changed part of each line edited in place), and the
swap moves each child's call site through it. A call written on a new line starts fresh; the calls
around it keep their state, and so does a call whose own line changed after its tag (an attribute
added). A keyed list reordered before the edit keeps each item's state with its key.

**Reset on purpose:** put `// @hmr:reset` anywhere in a component, or call
`setHmrPreserveState(false)` from `@svelters/native-runtime` for everything.

## Full-reload fallbacks

- No module on the path from the change accepts it. A component accepts its `default` export; if another
  export changed, the change goes up to its importers, and only if none accepts does it fall back.
- `import.meta.hot.invalidate()`.
- The swap budget: `HostConfig.hmr_max_swaps` (200) or `hmr_memory_fraction` (0.5 of `memory_limit`),
  because replaced module versions aren't freed while a runtime lives.

**In an embed** (`load_from`), a change that can't be applied in place is *not* turned into a full reload:
the report has `ok == false` and the host reports an error. You decide (for example remount the roots).

## `import.meta.hot` in your own modules

`accept(cb)`, `acceptExports(names, cb)`, `dispose(cb)`, `data`, `invalidate()`, as in Vite. Only
self-accepting modules are boundaries. Components get it from the compiler; write it yourself only for
plain modules that can replace themselves.

## Mistakes we have seen

- Expecting `$state` inside a function or `{#each}` block to survive: only top-level declarations are
  keyed.
- Editing an initializer and wondering why the state reset: that changes the signature. Edit the value in
  the running UI instead, or accept the reset.
- Filtering `EmbedEvent::Error` messages about `svelters:entry` in hot mode: fixed in the host; remove the
  filter.

## Not done

Keys from compiler call-site ids (for children inside `{#each}`/`{#if}`), `$bindable` fallback state,
native widget state, a build-time `preserveState` option.
