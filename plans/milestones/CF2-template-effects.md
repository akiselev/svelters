# CF2: one template effect per fragment

Follows CF1 section 6, option (a), which the user chose: the compiler writes a fragment's updates
the way Svelte 5.57.2 does. Each fragment gets one `$.template_effect`, built from Svelte's
Fragment visitor phases (init / update / after_update), so a reactive prop's first write comes
after the element's children are inserted. Scope: make the 7 "template effect order" conformance
samples pass, regenerate the op-log references the change touches, show the changes are order-only
and state-equal, and flag native widgets that need a prop before their children.

## 1. What changed

**Compiler** (`crates/svelters-compiler/src/codegen/component.rs`, `transform/mod.rs`):

- A fragment is the component root, a block branch, a snippet, component children, or an element's
  children that declare snippets or `{@const}`s. Its code is written in three phases
  (runtime-contract.md "Shape of generated code"):
  1. **init**, in source order. Each element is created and gets its static props, child
     attributes, one-off writes and `$n.track`. Its children are inserted into it, then its
     actions, attachments and `bind:this` run, then it is inserted into its parent. Blocks,
     components and `{@render}` appear in place.
  2. **update**: one `$.template_effect` holding every reactive write of the fragment, in element
     order with an element before its children. Call expressions are memoized as its
     dependencies (Svelte's `Memoizer`). It is created after the fragment's blocks and child
     components, so their template expressions run first.
  3. **after_update**: events (`$n.on`, an element's before its children's), then two-way and
     read-only bindings (`$n.bind*`, an element's after its children's), as Svelte's
     `BindDirective` pushes them to `after_update`.
- The per-op render effects and the `DEFER_TEMPLATE_EFFECTS` switch are gone.
- A component prop whose value reads no state is a plain property, evaluated once (Svelte's
  `build_component`: `b.init` unless `has_state`).
- A call is state only when it `has_call` in upstream's sense (an impure callee, or any call in an
  expression with dependencies). So a pure global call (`Math.random()`, `console.log(..)`) is
  written once, not in the template effect, where it would run again whenever another value
  changes. With the grouping, these last two changes are what CF1 said
  `lifecycle-render-order-for-children-6` and `props-default-value-lazy{,-accessors}` needed
  beyond the deferral.
- Object-literal thunk bodies are parenthesized.

**Runtime** (`packages/native-runtime/src/index.js`):

- A toplevel appended at the root is shown (`present`, or a dialog's modal method) when the
  outermost build returns (`mount`, or a block branch created in a later update). That is after
  its fragment's template effect has written its props. Toplevels are shown in append order.
- `set` on a tracked prop (`$n.track`) also skips the value it was given last time. Otherwise the
  template effect, re-running because another value of its fragment changed, would write the old
  value over the user's native edit. Svelte's DOM setters compare with their last value too.

**Hand-written components** (`experiments/native-compiler/handwritten/*/App.js`) are rewritten in
the new shape. **Docs**: `docs/notes/native-compiler/{deviations,runtime-contract}.md` and
`experiments/native-compiler/handwritten/README.md`.

Remaining departures from Svelte (deviations.md "Template effects"):

- An element's init order is native: its children go into it, then it goes into its parent, all
  before the template effect. Svelte clones the whole template first and appends the fragment
  last.
- Toplevels are shown when the outermost build returns. Svelte has no toplevels.
- A reactive `{@const}` or snippet scope inside an element is its own fragment.

## 2. Conformance

Before (CF1, `baseline.json`): **565 / 774** pass, 161 expected rejections, 45 failing, 3 skipped.

The last full run (`run.mjs --check`, `TIMEOUT_MS=60000`) is in the committed `results.json` and
`REPORT.md`. It was made by the previous session before the power cut, on the WIP compiler:
**569 / 774** pass, 41 failing.

- +8: the 7 F-order samples (`lifecycle-render-order-for-children`, `-2`, `-3`, `-4`, `-6`,
  `props-default-value-lazy`, `props-default-value-lazy-accessors`) and
  `input-type-before-value-spread` (CF1's F-dom-host) now pass.
- −4: `await-resolve` (mismatch) and three timeouts at 60 s (`effect-loop-infinite`,
  `error-boundary-20`, `mount-unmount-anchor`).

All four −4 samples, plus `await-resolve-2`, **pass** in a filtered run (`node run.mjs await-resolve
effect-loop-infinite error-boundary-20 mount-unmount-anchor`) on the current compiler (`b323bae0`,
rebuilt). So the run above came from an intermediate build or from load. On the current code the
expected score is 573 / 774 (565 + 8), with no regression, but no full run has confirmed it yet.
`baseline.json` is unchanged (565): `--update-baseline` was not run (section 9).

## 3. Op-log churn

`165a7501` regenerated 79 op logs. For the `fixtures/*` logs: `UPDATE=1 pnpm test`. For the
compiled own fixtures and the hand-written ones: `UPDATE=1 run.mjs` in
`experiments/native-compiler/{compiled,handwritten}`.

| backend | files | changed lines |
| --- | ---: | ---: |
| GTK (`fixtures/*` 22, compiled `expected.gtk.oplog` 14) | 36 | 519 |
| Qt (`fixtures/*-qt` 16, compiled `expected.qt.oplog` 14) | 30 | 462 |
| hand-written (GTK test host) | 6 | 66 |
| egui | 4 | 50 |
| AppKit | 2 | 17 |
| WinUI | 1 | 18 |
| **total** | **79** | **1132** |

77 of the 79 are pure reorderings: the multiset of lines is unchanged (checked file by file,
`git show 165a7501~1:f` against `165a7501:f`). In every one, reactive props' first `set`s move
after the element's children are inserted, and `listen`s and `observe`s move after those. Updates
inside a flush follow the fragment's template effect.

`e1430389` regenerated the compiler snapshots (`UPDATE=1 cargo test -p svelters-compiler --test
r2_observe --test runes`): 4 `tests/r2/*.expected` files and 13 `tests/runes/*/output.js` files.
All 17 are reorderings: same lines, the template effect and bindings after the fragment's appends.
`packages/native-runtime/test/toplevels.test.mjs` now expects root toplevels to be shown when
`mount` returns, in append order.

## 4. The two op logs that are not reorderings

Both follow from Svelte's own order, and both end in the same state as before.

- **`fixtures/appkit-types`**: one extra `call #1 host []` after `make-key-and-order-front`.
  - `string-value={home.host}` reads a getter on a native `NSURL`. `home` is a plain `const`, and
    upstream counts it as state (`2-analyze/visitors/Identifier.js`: `has_state` unless the binding
    is `static`, a function, or a known value). So the read sits in the fragment's template effect.
  - When the promise sets `loaded`, the whole effect runs again and re-reads `home.host`. Svelte
    does the same: only calls are memoized, not member reads.
  - The value has not changed, so `set` writes nothing. The extra op is a read; no state is
    written.
- **`fixtures/bind-observe`**: one extra `set #9 label "pos -1"`.
  - The label now shows the initial state (`pos = -1`) first. Then the read-only
    `bind:cursor-position` reads the native value (null on the op-log host), and the next flush
    writes `"pos "`.
  - Svelte puts bindings in `after_update` (`3-transform/client/visitors/BindDirective.js`, line
    284), after the template effect, so a DOM read-only binding behaves the same way.
  - The old log wrote `"pos "` once, because the binding ran before the label's first write.
  - The final state is the same. The label holds `-1` only until the first flush after `present`.

## 5. Widgets that need a prop before their children

Static props are still written right after `create`, before the children. Only reactive props
move.

**No backend's `insert` needs a reactive prop first.** `insert`, `create` and `slot_insert` read
no prop of the parent or the child. They dispatch on kind, slot and class only (for example
AppKit's `sizes_itself`, a class check).

Child attributes cannot arrive early either. Every backend refuses `set_child_attr` before insert
(GTK `backend.rs`, Qt `backend.cpp.in`, WinUI `backend.rs`). The runtime stores them and sends
them right after insert.

**The new order fixes props that need their children to exist:**
- GTK `Stack` `visible-child-name` / `visible-child`. Before, GTK warned and ignored the value.
- GTK `Notebook` `page`.
- Qt `QStackedWidget`, `QTabWidget` and `QToolBox` `current-index`.
- AppKit `NSTabView` `selected-tab-view-item`, and `NSPopUpButton` `index-of-selected-item` with
  `menu` slot items.
- A reactive `visible` on a nested Qt toplevel. Before, the `setParent` on insert hid it again.

**Flagged: a prop and a child slot that set the same native thing.** A static prop is still
overridden by the slot child. A reactive prop now overrides the slot child, and the backend's
slot table still lists the replaced child, so a later `remove` of it fails. Each case below is an
app that sets both, which is a conflicting use:
- GTK Button, Expander and Frame `label` against the child / `label` slot.
- GTK `HeaderBar` `title` against the `title` slot.
- WinUI ContentControl `content` against the `content` slot.
- AppKit `NSPopUpButton` `item-titles` against a `menu` slot child.
- AppKit `translates-autoresizing-mask-into-constraints` on a stack or grid view used as a scroll
  view's document view (`pin_document` sets it to false on insert).

**Neutral:** GTK and Qt menus (radio groups are rebuilt or regrouped), GTK virtual lists, egui
(lays out lazily), WinUI Grid attached properties.

## 6. Gates

GATES-PENDING

## 7. CI

`ci/cf2` at `e1430389` (origin/master `7aee41e5`, with ST2 and the iOS simulator work, merged in by
`a5d4ffa8`). All five workflows are green:

| workflow | run | result |
| --- | --- | --- |
| js | 39732936956 | success |
| linux-gtk | 39732936933 | success on attempt 2; attempt 1 failed only `svelters::reactive an_event_flood_warns_once`, the known flake being fixed in FU3 |
| linux-qt | 39732936970 | success |
| macos-appkit | 39732936935 | success (including `counter-mac todo` from ST2) |
| windows-winui | 39732936944 | success |

Earlier heads were red for reasons fixed since:
- `b323bae0` and `a5d4ffa8`: js, from the toplevels unit test;
- `b323bae0`, `a5d4ffa8` and `d2332581`: linux-gtk, from the compiler snapshots.

## 8. Conflict risks

- `crates/svelters-compiler/src/codegen/component.rs` is largely rewritten: fragment phases,
  `Frag`, `template_effect`, `memo_exprs`, `phase`, `update`, `listen`. SP1 (spread) and TR1
  (transitions) also emit element ops there. A new op kind has to be given a phase (`fn phase`):
  init, update or after_update.
- `crates/svelters-compiler/src/transform/mod.rs`: `ScopeRewriter`'s state test (`has_call`).
- `packages/native-runtime/src/index.js`: toplevel show deferral, and `set` / `track`
  (`requested`).
- Any branch that adds or regenerates an op log, a `tests/r2` or `tests/runes` snapshot, or a
  hand-written component must regenerate it after merging this. Never hand-edit those files.
- `docs/notes/native-compiler/{deviations,runtime-contract}.md`.

## 9. Open items

- **Full conformance run and baseline.** Run `UPSTREAM=... TIMEOUT_MS=60000 JOBS=6 node run.mjs
  --check`, then `--update-baseline`, on the merged tree, and commit `results.json`, `REPORT.md`
  and `baseline.json`. Expected: 573 / 774 with no regression (section 2). This was not done
  because the wrap-up was stopped before it.
- **A parent prop that names a child through a reactive child attribute** (Stack
  `visible-child-name` pointing at a child with a reactive `name`). Inside the one template effect
  the parent's write comes before its children's, so the name may not exist yet. It recovers on
  the next update. This was not checked further.
- **Prop and slot setting the same native thing** (section 5): consider a compile-time diagnostic
  when an app sets both.
- `plans/milestones/CF1-conformance.md` still describes `DEFER_TEMPLATE_EFFECTS`. That is
  historical; the switch is gone.
