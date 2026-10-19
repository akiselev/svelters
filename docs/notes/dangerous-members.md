# Dangerous native members

A **dangerous member** is a native method, a writable property (the write side) or a signal that app code can still
reach at run time (GTK's GIRepository `call`, AppKit's selector/KVC fallback, a generated attribute, an annotated method),
but whose effect the backend owns. Calling it can corrupt the tree svelters manages, fight the backend's lifecycle,
or crash. Nothing blocks these calls at run time. The metadata marks them, and the docs and typings warn about them.

This came out of the GTK fact review (`docs/notes/ai1-gtk-review.md`). The review rejected `set_parent`, the child
setters, `set_application`, `set_display`, `set_startup_id` and writes to `Window.application` / `Window.display`.
Rejecting a fact only keeps it out of the inferred facts, the typings and the tests. GI still reaches it.

## Model

Each mark is an annotation:

```yaml
severity: corrupts-tree | owned-by-backend | crashes | other
reason: one or two sentences, citing the backend code that owns the state (file:line)
use_instead: the svelters way (a slot, {#if}, an attribute, an event)   # optional
reachable: false     # optional; default true. See below.
evidence: [path:line, ...]                                            # optional
```

The severities:

- `corrupts-tree`: re-parents or removes native children behind the renderer.
- `owned-by-backend`: state the backend sets and relies on, such as a delegate, a target or the application.
- `crashes`: can free a native object the backend still holds.
- `other`: anything else. The reason says why.

`reachable: false` marks state the backend owns that JS cannot reach today. The usual cause is that the generated
tables do not expose the member, or that a runtime guard refuses it. It is marked so that a later override or the
inference never exposes it. This is mostly Qt, whose backend only exposes annotated members.

Properties mean the write side: a rule never marks a read-only or construct-only property. Reading is harmless.

### Where to declare

The marks are layered, lowest precedence first:

1. **`metadata/dangerous/<tk>.yaml`** holds pattern rules for whole families. The schema is
   `tools/metadata/common/dangerous.schema.json` (`svelters-dangerous/v1`). A rule has an `id`, a `section`
   (`methods` | `properties` | `signals`), a `name` regex (full match on the native name in
   `metadata/<tk>/<Kind>.json`) and optional `kinds`. A member matches when its declaring kind, or one of that kind's
   bases, is listed. Then come the annotation fields.
2. **The `dangerous:` block of `metadata/overrides/<tk>/<Kind>.yaml`** holds single members:
   `{methods|properties|signals: {<native name>: annotation | false}}`. `false` exempts a member from every rule.
   Every override schema carries the same block; `dangerous.py --sync-schemas` writes it.

Why a separate file per toolkit, and not `metadata/rules/<tk>.yaml`:

- The rules files steer inference. Their deny patterns also deny members that are merely untestable or need a
  realized window. "Dangerous" is a different decision, and it is user-facing: docs, typings, and later a runtime guard.
- AppKit's and WinUI's rules have other shapes, or none at all. One schema for all four toolkits lets a guard load the
  file as it is.
- The two decisions are still cross-checked. Every dangerous GTK method must also match a `deny_methods` pattern of
  `metadata/rules/gtk.yaml`, so a re-probe cannot infer it. The container-op pattern was added for this.

The layer is **read at use time and never merged into the facts**. The resolver is
`tools/metadata/common/dangerous.py` (`Dangerous(tk).lookup(kind, section, name)`). `svmeta.merge` skips
`READ_TIME_KEYS`, and `svmeta.validate` rejects a merged kind that carries one. GTK `ingest.py`, `qt_layers.py`,
`appkit_layers.py` and `winui_layers.py` leave the block out. So no fact record, fingerprint or ledger entry changes
when a mark changes, and `metadata/gtk/*.json` needs no re-ingest. The inferred layers may not declare it.

### Checks (`pnpm test:meta` runs `dangerous.py --check`)

`dangerous.py --check` covers six things:

- the rules files and override blocks validate against the schema;
- every rule matches at least one member;
- every override entry names a real member, and every `false` exempts something;
- no inferred record of a kind or its bases (props, methods, signals, bindings) is a dangerous member;
- in the generated fact manifests (what the ledger verifies), no inferred fact and no binding is a dangerous member.
  Manual method and property facts may be dangerous: Qt `QMenuBar.clear` is reachable on purpose and is warned on;
- every dangerous GTK method is denied by `metadata/rules/gtk.yaml`.

The pytest tests are in `tools/metadata/common/test_dangerous.py`: the merge and pattern logic, precedence,
exemptions, write-side only, schema copies and the repository check. The docs tests are in `tools/docs/test_ir.py`
(every dangerous member is in the IR, and no example uses one) and `docs/site/test/{golden,smoke}.test.mjs`
(the callouts, the row marks, the per-kind list and inherited members).

## What the outputs show

- **Docs IR** (`tools/docs/ir/schema.json`): `dangerous` on property, event and method records (annotation plus
  `source`), and `coverage.dangerous` counts. The GTK and Qt frontends mark every member the metadata lists, and they
  list every GI/qdoc method, annotated or not. So a dangerous member is always documented, with its warning. Generated
  examples skip dangerous properties.
- **Site** (`docs/site/scripts/lib/render.mjs`):
  - a `:::danger` callout on the member's details (methods get a details block even without docs);
  - a `⚠ dangerous` mark on its table row and in the inherited lists;
  - a **Dangerous members** section per kind page, listing own and inherited members with links;
  - a count on the coverage page.
- **Typings**:
  - `packages/renderer/types/gtk.d.ts` gives dangerous attributes a JSDoc block,
    `@remarks Dangerous (<severity>): <reason>. Use instead: <use_instead>.`, next to `@unverified`;
  - `GtkDangerousMethods` (exported from `@svelters/renderer/types`) lists the dangerous methods by `<Kind>.<method>`,
    because `call()` is untyped;
  - `qt.d.ts` marks the dangerous annotated methods of each element.
  - `@deprecated` is not used: these members are not going away, and a strike-through would read as "use the newer API".
  - AppKit and WinUI have no element typings yet.
- **Coverage** (`tools/metadata/coverage.py`): a summary row in `metadata/coverage/{gtk,qt,appkit,winui}.md`, a
  `dangerous` count per kind in the JSON, and `metadata/coverage/dangerous.md`, the full list for all four toolkits.

## Populated (2026-10-19)

| Toolkit | Methods | Properties | Signals | Reachable today | Not reachable |
| --- | ---: | ---: | ---: | ---: | ---: |
| GTK | 91 | 40 | 0 | 131 | 0 |
| Qt | 49 | 1 | 0 | 5 | 45 |
| AppKit | 8 | 7 | 0 | 12 | 3 |
| WinUI | 0 | 2 | 0 | 1 | 1 |

The Qt docs IR counts 60 methods, not 49, because overloads are separate records. The member list is
`metadata/coverage/dangerous.md`.

**GTK** (`metadata/dangerous/gtk.yaml`, 19 rules). The backend records each child's slot (`backend.rs:687-692`),
`remove` trusts that record (`:726-733`), and `insert` refuses a child that already has a parent (`:677-678`).

- **Child setters**: `set_child`, `set_{start,end}_child`, `set_{center,start,end,label,title}_widget`,
  `set_titlebar`, `set_popover`, and their widget-typed property twins (`child`, `titlebar`, `start-child`,
  `label-widget`, `popover`, and so on). These are generated as writable attributes.
- **`Widget` tree ops**: `set_parent`, `unparent`, `insert_before`, `insert_after`.
- **Container ops that the slots use**:
  - Box: `append`, `prepend`, `remove`, `insert_child_after`, `reorder_child_after`;
  - ListBox and FlowBox: `insert`, `remove`, `remove_all` and the rest;
  - Grid: `attach` and `remove*`;
  - Fixed: `put` and `remove`;
  - Overlay: overlays;
  - HeaderBar and ActionBar: `pack_*`;
  - Stack: `add_*`;
  - Notebook: `*_page`.
- **Toplevel binding**:
  - `set_application` and `application` (set at creation, `backend.rs:472-475`);
  - `set_transient_for` and `transient-for` (`insert_transient`, `:618-634`);
  - `set_display` and `display`, `set_startup_id` and `startup-id` (window-system state; AI1 review).

Not marked:

- The review's rejections for testability: `get_name`, `set_group`, `Image.resource`, `extra-menu`, `visible-submenu`.
- `present` and `close`, which the backend handles: `close` → `native_destroyed`.
- `Window.destroy`: the backend reports a destroyed window and its descendants (`native_destroyed`).

**Qt** (`metadata/dangerous/qt.yaml` plus override blocks). Qt has no generic invoke path. `b_resolve` only finds
annotated methods and an object-getter table, so most marks are `reachable: false`.

- **Reachable**:
  - `QMenuBar.clear` and `QToolBar.clear` drop backend-held actions (`menus.hpp.in:69-83`);
  - `QMdiArea.closeAllSubWindows` deletes children behind the renderer;
  - `QMainWindow.menuBar` and `statusBar` lazily create unmanaged bars (object getters).
- **Not reachable**: `setParent`, the window flags, `setLayout`, the slot ops of QMainWindow, QScrollArea,
  QDockWidget, QStackedWidget, QSplitter, QStatusBar, QTabWidget, QToolBox, QMdiArea and the layouts, and `exec` on
  QDialog, QMenu and QMessageBox.

Not marked:

- `deleteLater`: QObject has no metadata file, and it is not reachable.
- `close` and `show`/`hide`: handled by the close filter, or harmless.
- `QTabBar.addTab` and the rest: they are the API of the standalone `<tab-bar>`. They are dangerous only on the bar
  borrowed from a `<tab-widget>`, which a per-kind mark cannot express.

**AppKit** (`metadata/dangerous/appkit.yaml` plus `NSWindow.yaml`). Any selector is reachable (`dyn_method` →
`find_selector`) and any property is writable through KVC.

- **Reachable**:
  - NSView `add-subview`, `add-subview-positioned-relative-to`, `remove-from-superview`;
  - NSStackView arranged-subview ops;
  - `set-content-view` on NSWindow and NSBox;
  - NSControl `target` and `action` (`listen()` installs them);
  - NSWindow `delegate` (`create()` installs the WindowDelegate);
  - NSWindow `releasedWhenClosed` (`crashes`: the handle table holds the window).
- **Not reachable**: `subviews`, and `contentView` on NSWindow and NSBox. `slot_owning_key` refuses these.

Not marked:

- `close`, `perform-close`, `order-out`: handled by the delegate.
- `set-subviews` and `set-views`: reachable but not in the metadata. Worth ingesting.

**WinUI** (`metadata/dangerous/winui.yaml` plus `ContentControl.yaml`). JS reaches only the generated tables.

- `ContentControl.Content` is reachable. `set_prop` does not check the `content` slot, so a write drops an inserted
  child.
- `Window.Content` is the window slot and is not reachable.

Not marked:

- `Window.Close` is handled by the close rule.
- `Panel.Children` is a read-only property. Its danger, mutating the collection, has no write side to mark.

## Follow-ups: a runtime guard (not implemented)

The marks are data a guard can load: `metadata/dangerous/<tk>.yaml`, the override blocks, or a table generated from
`Dangerous(tk).all()`, like `GtkDangerousMethods`. There are three options, cheapest first:

1. **Dev-mode warning in the renderer** (`packages/renderer`): `NativeElement.call(name)` and `setProp(name)` look up
   the generated table by the node's kind chain and `console.warn` once per member, with the reason and `use_instead`.
   This needs no backend change and costs nothing in release builds. It cannot see calls made from native code.
2. **Hard refusal in the dynamic call paths**:
   - GTK: `resolve_dynamic` / `Gi::resolve` (`crates/svelters-gtk/src/backend.rs:1700`, `gi.rs:480`) and `set_prop`
     for widget-typed child properties;
   - AppKit: `dyn_method` / `dyn_prop` (`backend.rs:1182-1193`), next to the existing `slot_owning_key` guard;
   - WinUI and Qt: the generated tables simply never include them.

   Refusal returns an error like `slot_owning_key` does today. This is safer, but it breaks apps that use a member
   deliberately (for example a list factory calling `ListItem.set_child` on a node it created outside the tree). It
   would need a per-call escape hatch, or a narrower rule: refuse only when the argument or receiver is a node the
   renderer manages.
3. **A narrower check of state rather than names**: for example, refuse `set_child` only when the child handle has a
   slot record. This is the most precise option and has the most code in each backend.

Option 1 first, behind the renderer's dev flag. Then option 2 for the `crashes` severity (AppKit
`releasedWhenClosed`), where a warning is not enough.

Other open points:

- Mark Qt's borrowed `QTabBar` of a `<tab-widget>`. This needs a receiver-aware mark.
- Ingest AppKit `setSubviews:` and `setViews:`, then mark them.
- Element typings for AppKit and WinUI would carry the same `@remarks`.
