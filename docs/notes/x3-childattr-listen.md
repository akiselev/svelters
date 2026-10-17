# X3: child attributes and listen phases (working notes)

Lane X3, started 2026-10-17 on master 3070367. Plan: `plans/milestones/X3-childattr-listen.md`. ABI text: `plans/architecture/backend-abi.md`, "ABI v2.2 additive".

## JS syntax: why prefixed attributes

`<label parent:column={1} parent:row={2}>` was chosen over an `sv-item` wrapper and `{@attach childAttrs(...)}`.

- The Svelte compiler accepts any `prefix:name` attribute whose prefix is not a directive; the renderer receives it as `setAttribute(el, 'parent:column', v)`, so it is reactive through the ordinary attribute path with no compiler support. Verified with `fixtures/child-attrs`.
- The attribute is set while the node is still being built, before the child is inserted. The renderer sends it right after the native `insert` (same op sequence), which Qt needs because a grid cell is fixed at `addWidget` time.
- Moving a child (keyed `{#each}` reorder, a new parent) is remove + insert; the renderer replays the stored attributes after the insert. A removed child keeps its values, the backend clears its own state on `remove`.
- `sv-item` adds a JS-only node to the placement walk (`flat`/`placement`), a second place that decides the child's slot, and a component per cell; its only advantage is a type-checked wrapper API, which a Svelte component can still provide on top (`<Cell row col>` rendering `<label parent:row={row}>`).
- `{@attach}` runs after the element is mounted, so every first placement would be insert + move, and it needs its own reactivity and cleanup.
- Cost of the choice: `parent:` is a reserved attribute prefix. TypeScript needs the names declared per element (`'parent:column'?: number`); the generators emit them from the `attrs` facts.

## Renderer and host shape

- Renderer: `setAttribute`/`removeAttribute` with the `parent:` prefix go to `setChildAttr` (stored in `el.cattrs`); `applyChildAttrs` runs after `host.insert` in `nativeInsert` and in `realize`. `host.setChildAttr(parent, child, name, value)` takes names (the host resolves with `SVL_ATTR` scoped by the parent kind); `null` resets.
- Host: `op_set_child_attr` checks that `child` is inserted under `parent` (host tree), resolves the name against the parent's kind and converts the value with the untyped slot (numbers, bools, strings). `op_get_child_attr` reads back.
- Listen phases: `renderer.addEventListener(target, type, handler, options)` takes `options.capture` (Svelte passes it for `on<event>capture`) and `options.after`. Bookkeeping is per (native signal, flags). `host.listen(h, sig, on)` for flags 0 and `listen(h, sig, on, flags)` otherwise. `dispatch(h, signal, args, deferred, flags)` and `dispatchSync(h, signal, args, flags)` run only the listeners registered with exactly those flags.
- Flags travel in bits 28..30 of the signal id of `post_event`/`call_sync`. Reason: `post_event` and `call_sync` keep their signatures and old backends are untouched; the cost is that signal ids must stay below 2^28, which holds (id spaces are per-kind tables of a few thousand).
- Op log: `listen #h sig on|off [after|capture|after+capture]`, `attr #parent #child name json`, and `event`/`sync` lines carry the flags word as a trailing token. Scripts: `{"event": [h, sig, args, "capture"]}`. The Node TestHost and the log backend print identical lines (checked by `fixtures/child-attrs`, `fixtures/listen-phases`).

## Tests so far

- `packages/renderer/test/renderer.test.mjs`: child attrs (order, change, move, reset, remove, detached), phases (flags per listen, dispatch by flags, `listen()` attachment).
- `crates/svelters-host/tests/child_attrs.rs` over the log backend; fixtures `child-attrs`, `listen-phases` (testhost, log backend).
- `packages/testhost/bin/svelters-test.mjs`: the "node: specifier" bundle check now looks for quoted specifiers only; `{@attach}` pulls in a JSDoc comment containing `node:`.

## Metadata format (shared by both toolkits)

`attrs[]` on a slot (A1's schema had the key as a free-form array; GTK's is now typed, Qt's is added by the Qt step). Names are camelCase JS names; the full list for a kind is the union over its slots.

GTK record (`tools/metadata/gtk/override.schema.json`, `$defs/attr`):

- `name`, `type` (`bool|i32|u32|i64|f64|string`), `default` (`null` allowed: the value after `null` and after remove + insert), `samples` (two different non-default values for the generated test), `provenance`.
- Either a property of a related object: `via: layout_child` (the child's `GtkLayoutChild`, e.g. `GtkGridLayoutChild`) or `via: {native: "GtkNotebook::get_page(GtkWidget*)"}` (an object a method of the parent returns), plus `property`. Reset writes the property's own default.
- Or native calls: `set: {native, args}` and `get: {native, args, result?}` or `get: {class, expr}` (a Rust expression over `p` and `child`). Arg expressions: `child`, `value`, `current:<attr>` (the live value of another attr of the slot, for `GtkFixed::move(w, x, y)`), `const:<n>`. `rust:` names the gtk4-rs method when it differs (bool getters are `is_*`).
- `expect`: a Rust bool expression checked natively after sample 1 is set (`h`, `parent`, `c`, `w`, `pw`).
- Slot `order: unordered` (new): `insert` ignores `before`. The renderer passes a `before` anchor on every keyed move, and a grid has no order.

Generated: `ATTRS` table, `slots::attr_object/attr_set_native/attr_get_native`, one test per attr (`generated::verify::attr_<kind>_<slot>_<name>`), fact id `<kind id>:slot:<slot>:attr:<name>`, and `'parent:<name>'?: <type>` on the `Widget` props of `packages/renderer/types/gtk.d.ts`.

## GTK: what works

| Kind | Slot | Attributes | Mechanism |
| --- | --- | --- | --- |
| `grid` | `cells` (unordered) | `column`, `row`, `columnSpan`, `rowSpan` | `GtkGridLayoutChild` properties: moving a child is a property write, no re-attach |
| `fixed` | `children` (unordered) | `x`, `y` | `gtk_fixed_move`; read back from the layout child's transform |
| `notebook` | `pages` | `tabLabel`, `menuLabel`, `reorderable`, `detachable`, `tabExpand`, `tabFill` | label text through `set_tab_label_text`/`set_menu_label_text`; flags are `GtkNotebookPage` properties |
| `stack` | `pages` | `name`, `title`, `iconName`, `needsAttention`, `useUnderline` | `GtkStackPage` properties |
| `overlay` | `overlays` | `measure`, `clip` | `set_measure_overlay`/`set_clip_overlay` |

`GtkBox` has no per-child attribute (packing is the child's `hexpand`/`halign`), so there is nothing to annotate. The `overlay` `child` slot and the other Overlay facts belong to `gtk-containers-a`; the file `Overlay.yaml` carries only the `overlays` slot.

Surprises (GTK 4.22):

- `gtk_fixed_get_child_position` returns 0,0 even after `gtk_fixed_move`; the position is only in the layout child's transform.
- The `GtkNotebookPage:tab-label` and `menu-label` properties only store a string; they do not create the visible label, and `gtk_notebook_set_tab_label_text` does not update them. So these two use the native calls and the other page flags use the properties (`reorderable` and `detachable` were confirmed natively).
- A notebook page's parent is an internal `GtkStack`, so the slot's `get` is `contains: ancestor`.
- Setting a string property to NULL resets it; the page string properties and `GtkStackPage` ones read back as `null` until set.

## Listener phases per backend

| Flag | GTK | Qt |
| --- | --- | --- |
| `after` | `connect_closure(after = true)` on any signal: the handler runs after the class handler and after every plain handler | see the Qt section |
| `capture` | only on an event controller: the controller is put in `GtkPropagationPhase::Capture` while any capture listener exists and its previous phase is restored after the last. Any other object is refused ("only controllers have a propagation phase") | see the Qt section |

Each (handle, signal, flags) is its own native connection and reports `flags` in the high bits of the signal id. Because a capture listener changes the controller's phase, a controller element with a `…capture` listener should not also carry bubble listeners; use two controller elements.

GTK tests: `crates/svelters-gtk/tests/phases.rs` (order with the after listener connected first, sync results per connection, capture phase set and restored, refusals), `host_childattr.rs` (the real host runs the `child-attrs` bundle, a click reorders the keyed list and changes a span, the grid follows), the oplog replay of `child-attrs` (`attr` lines and `listen` flag words), and 19 generated attr tests plus the slot tests for the new slots.

## Qt

Qt half of X3 (generated C++ backend, `tools/metadata/qt/`). Commits: the generator, template and overrides first, then the Qt integration tests.

### What was done

- `svl_backend` vtable: `set_child_attr`, `get_child_attr`, `listen_ex` after `request_frame` (`backend.cpp.in`). `resolve(SVL_ATTR, parent_kind, name)` gives `A_<tag>_<attr>`: one id per (element, attribute name). When two slots of one element have an attribute of the same name, the implementation is picked by the slot the child was inserted through: `Entry::slot` and `Entry::placed` are set by `b_insert` (after `gen_insert` resolved slot 0 to the default slot) and cleared by `b_remove`.
- Qt keeps the state, the backend caches nothing: `get_child_attr` reads the live native value (`getItemPosition`, `tabText`, `itemAt(i)->alignment()`). "Remove clears the attributes" therefore holds because Qt drops the layout item / tab; the one exception is a splitter's stretch factor (size policy of the child), which the slot resets itself (`reset: true`: the default is written before a removal and after an insertion).
- Errors: a child that is not inserted through this backend into this parent gives "child attributes need an inserted child"; a value of the wrong type or an unknown enum name is an error; `SVL_NULL` resets to the attribute's `default`.
- Generator: `attrs[]` on a slot (`tools/metadata/qt/override.schema.json`, `$defs/attr`): `name`, `type` (`bool|i32|f64|string|enum` with `values:` JS name -> C++ expression), `default`, `samples`, `set` (`native`, `args`, `readd`), `get` (`native`, `args`, `out`, `member` or `expr`), `reset`, `expect`, `provenance`. Argument forms: `child`, `value`, `child_index`, `attr.<other>` (live value of another attribute of the slot), `const:<c++ literal>`. Natives are validated against the qdoc metadata (`native_params`), including that the class is a base of the slot host.
  - `readd: true` (QGridLayout fixes the cell at `addWidget` time): every argument is read first (including the other attributes), then the slot's remove call (without its `then` steps) runs and the setter re-adds the widget. A grid attribute write therefore costs a remove + add; the grid is unordered, so the changed item order does not matter.
  - Pointer parameters of a getter (`getItemPosition(int, int*, int*, int*, int*)`) are out parameters: they are not listed in `args`, `out` names the one that is the value.
  - `get.member`: a method called on what the native call returns (`itemAt(int)` -> `alignment()`); `get.expr`: a C++ expression over `h`, `ch`, `ci`, for state with no getter (the splitter's stretch is `ch->sizePolicy().horizontalStretch()`, which is where `setStretchFactor` writes it).
  - Slot `order: unordered`: `insert` ignores `before`; a child already in the host is left alone (so attributes survive the renderer's keyed-move insert). The slot test is `unordered_ignores_before` instead of `order`.
- Facts and tests: fact id `<element fact prefix>:slot:<slot>:attr:<name>` (section `attrs`); the slot fact's fingerprint excludes `attrs`. One generated test per attribute (`attr/<tag>.<slot>.<name>`): stray child is an error, sample 1 and 2 read back, the toolkit really holds it (`expect`), `null` resets, remove + insert returns to the default, a removed child has no attributes, an unknown enum name is refused.
- The generic `order` slot test now checks `isAncestorOf` instead of `parent() ==`: a QTabWidget page sits in the tab widget's inner stack, not directly under the tab widget. This changed the test source (and the fingerprint) of every many-slot `order` test, so the ledger was refreshed.
- `.d.ts`: Qt has no common widget interface in `qt.d.ts` (it is a per-tag descriptor table), so the `'parent:<name>'?: <type>` attributes are one exported interface, `QtChildAttrs`.

### Elements and attributes

| Element | Slot | Attributes | Native calls |
| --- | --- | --- | --- |
| `grid` (new, in `QWidget.yaml`; composite QWidget + QGridLayout) | `cells` (unordered) | `column`, `row` (default 0), `columnSpan`, `rowSpan` (default 1) | `addWidget(w, row, col, rowSpan, colSpan)` after `removeWidget` (readd); `getItemPosition` |
| `box` | `children` | `stretch` (i32, 0), `align` (`none|left|right|hcenter|top|bottom|vcenter|center`, default `none`) | `QBoxLayout::setStretch(index, v)` / `stretch(index)`; `QLayout::setAlignment(w, a)` / `itemAt(i)->alignment()` |
| `tabwidget` (new, `QTabWidget.yaml`, slot facts only) | `pages` | `tabLabel` (""), `tabToolTip` (""), `tabEnabled` (true) | `setTabText`, `setTabToolTip`, `setTabEnabled` and their getters |
| `splitter` (new, `QSplitter.yaml`, slot facts only) | `children` | `stretch` (0, `reset`) | `setStretchFactor(index, v)`; read from the size policy |

Not done: `tabIcon` (`setTabIcon` takes a `QIcon`; an attribute value has no icon form yet), and Q5 (QMainWindow toolbars and docks, see below).

### Listener phase mapping (Qt)

| Request | Result |
| --- | --- |
| plain (flags 0), any signal | unchanged: `listen` is `listen_ex(.., 0)`; the wire id is the signal id |
| `after` on a QObject signal | accepted. Qt runs slots in connection order and a signal is emitted after the widget changed its own state, so "after" means behind every plain listener of that signal: the after-connection is kept in a recipe (`Core::after_make`) and re-made behind every plain connection added later (`keep_after_last`). Events are posted to the host asynchronously anyway, so what the host sees is the order of the `post_event` calls |
| `after` on `key-press` or `close-request` | refused: "no after phase for event-filter signals" (a key handler runs before the widget sees the key; the close handler decides through `call_sync`) |
| `capture` on `key-press` | accepted: an application-level event filter (`qApp->installEventFilter`, installed with the first capture listener, removed with the last) sees every `KeyPress` before its receiver. For each listened widget that is a proper ancestor of the receiver (`isAncestorOf`, receiver excluded), outermost first, it calls `call_sync` with `signal | CAPTURE << 28`; a result of `true` consumes the key before the receiver or its own plain `key-press` listener see it. A key sent to the listened widget itself is not captured |
| `capture` on anything else (QObject signals, `close-request`) | refused: QObject signals have no capture phase |
| `after+capture`, unknown flags | refused |
| `off` | removes only the connection with the same (handle, signal, flags) |

Because `key-press` was only on `entry`, which has no descendants, `box` got the synthetic `key-press` signal (`QWidget.yaml`) so that capture has something to listen on.

### Tests

- Generated (`svl_qt_verify.cpp`, run by `examples/counter-qt/tests/generated.rs`): 4 grid slot/attr groups, the attr tests above, and the runtime tests `listen_after_runs_behind_plain`, `listen_phase_refusals`, `capture_reaches_descendant_key_before_its_own_listener`.
- `examples/counter-qt/tests/oplog_replay.rs`: there was no Qt op-log replay before. It replays every `fixtures/*/expected.oplog` the Qt element set can run (skips, with the reason, fixtures with `sync`/`gone` lines or GTK-only kinds, props, signals or methods): `attr` lines are set and read back, `listen` words go through `listen_ex`, `event ... clicked` is produced with the `click` method. `child-attrs` must replay.
- `examples/counter-qt/tests/host_childattr.rs`: the real host runs `fixtures/child-attrs/dist/app.js`; the grid cells (`svl_qt_grid_cell` hook) match the GTK test before and after the click.

### Surprises

- A QGridLayout cell is fixed at `addWidget`: an attribute write is remove + add with the other three values read first.
- QTabWidget pages are children of the inner QStackedWidget, not of the tab widget: any generic "child's parent is the parent" check is wrong there.
- `QSplitter` has no stretch getter; `setStretchFactor` writes the child's size policy, which outlives the slot (hence `reset`).
- A grid element cannot live in `QGridLayout.yaml`: an override file's name is the class its elements construct (`grid` constructs a QWidget), so it sits next to `box` in `QWidget.yaml`.
