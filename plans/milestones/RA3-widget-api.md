# RA3: widget API completion (kind methods, fn form, `wrap_widget!`, containers, paint cache)

Design: `plans/research/TRACKS.md` RUSTAPI (RA3 row), `plans/research/ARCH-EMBED.md` §8.2 and the §10 RA3 row
("`collapsing` container and a wrapped `DragValue`; paint cache hit-rate test"), §13 D3,
`plans/research/ARCH-CRATES.md` §4.2 (the `<code-editor>` facet table: methods `focus()`, `revealLine(n)`,
`setSelection(sel)`, `executeCommand(cmd)`, `setDiagnostics(source, version, diags)`),
`docs/notes/egui-rust-exports.md` §7.1–§7.3. Builds on RA1 (`RA1-widget-kinds.md`) and SG4.

## What landed

### 1. Kind methods callable from JS (cadatomic K1 `<code-editor>`)

```rust
#[svelters::widget]
impl CodeEditor {
    /// Scrolls line `n` into view.
    pub fn reveal_line(&mut self, n: u32) { … }
    pub fn focus(&mut self, cx: &mut MethodCx<'_>) { cx.request_focus() }
    pub fn set_selection(&mut self, cx: &mut MethodCx<'_>, sel: Sel) { cx.set(&mut self.selection, sel) }
    pub fn set_diagnostics(&mut self, source: String, version: u64, diags: Vec<Diag>) { … }
}
```

```svelte
<code-editor bind:this={ed} … />   <!-- ed.revealLine(40); ed.call('revealLine', 40) works too -->
```

| Piece | Where |
|---|---|
| `#[svelters::widget]` on an inherent `impl`: every fn with `&self`/`&mut self` (pub or not) is a method, JS name lowerCamelCase, `#[method(rename = "revealLine")]` / `#[method(skip)]` (cadatomic K1's spelling; `#[js(rename/skip)]` too); fns without a receiver stay Rust-only; params `FromJs` (`Json<T>` for serde values), result `IntoJs` (`()` is void; `Result<T, E>` with `E: svelters::Error` throws the error in JS as a native error with its code); a `&mut MethodCx` param (anywhere) is the context. Names that are members of every runtime node (`call`, `tag`, `style`, `then`, …) are rejected at compile time | `crates/svelters-macros/src/lib.rs` (`widget_methods`) |
| `MethodEntry`, `KindMethods` (inventory), `MethodCx` (`id`, `node`, `scene`, `rect`, `state`/`set_state`, `set` exact write, `emit`, `ext`/`ext_mut`, `request_focus`, `request_measure`, `request_repaint`), `call` (a method's `NativeError` goes back as `Value::Error`, so JS gets `domain`/`code`) | `crates/svelters-egui/src/provider/methods.rs` |
| Method types in the export schema: a `KindTypesDef` keyed `<spec id>#methods` (each method's params, then its result), so records/enums/`Json<T>` work as for props | macro + `KindDecl::__NAMESPACE`/`__METHODS_KEY` |
| Registry: `ProviderKind::methods`/`method_types`/`method(i)`, `KindDef.methods` filled, `provider_method_desc` (host `MemberDesc`: named param slots, trailing `Option`s optional, `SVL_M_THROWS`), `provider_method_index`; duplicate impl blocks or an impl for an unregistered kind panic at startup | `kinds/registry.rs` |
| Backend: `describe(Method)` answers provider methods (the host checks argument count and types before Rust runs); `call` dispatches to `methods::call` | `ops.rs` |
| Writes a method makes that JS hears (`prop_changed`, events) go to `Inner::pending_out` and are flushed first by the next `after_pass`; state bits, ext data, focus and remeasure apply at once; the root is touched | `backend.rs`, `frame.rs` |
| Spec fragment: methods with `dispatch: "provider"`, `params` (name, type, nullable, optional, ts) and `returns` | `spec.rs` `kind_json` |
| Typings: `kinds.d.ts` exports `<Name>Element extends NativeElement` with the methods and maps the tag in `HTMLElementTagNameMap` (svelte-check types `bind:this` from it); `egui.d.ts` re-exports `NativeElement` | `spec.rs` |
| Runtime: a member a node lacks falls through to a proxy prototype of `Native.prototype`, which, only for provider kinds (kind id pack 62), asks the host once per kind and name (`methodId`) and returns a function calling `this.call(name, …)` | `packages/native-runtime/src/index.js` |

### 2. Fn form and `wrap_widget!`

- `#[svelters::widget(namespace = "cad")] pub fn level_meter(ui: &mut egui::Ui, #[prop] level: f64, label: &str) -> egui::Response`:
  a struct `LevelMeter` (fn name in PascalCase) with one prop per parameter after the `Ui` (`#[prop(..)]` options as on
  fields). `T` gets a clone, `&T` (`&str` → `String`, `&[T]` → `Vec<T>`) borrows, `&mut T` must be `#[prop(bind)]`
  and what the fn writes is announced (`cx.set`). The fn stays callable from Rust.
- `svelters::wrap_widget! { #[widget(namespace, tag)] struct … { #[prop(bind)] value: f64, … } |p, ui| ui.add(…) }`:
  the struct form plus a non-capturing closure. Bound fields are diffed after the call and announced.
- Both measure by running the body in an invisible, disabled sizing-pass `Ui` (`MeasureCx::sizing_pass`, new; ids from
  the new `MeasureCx::id`, per node) with the bound fields restored, so the measured size is the drawn one.

### 3. Container slots

`caps = CONTAINER` gives a provider kind the default slot `children` (`KindDef.slot`, spec `children`), so Svelte
inserts children into it. taffy lays them out from the node's style, or the kind's `Widget::layout`
(`CUSTOM_LAYOUT`); the kind draws them with `cx.children(ui)` (SG4). Example: `<collapsing-section>`.

### 4. `cached_shapes`

SG6 landed its own paint cache (`WidgetCx::cached_shapes(painter, version, |painter, rect, shapes| …)`, keyed on
version, size, `pixels_per_point` and the theme, counted in `SceneStats::paint_hits`/`paint_misses`) while RA3 was
open. Decided (lane, 2026-10-22, on merging origin/master befca905): RA3's version (`cache_mut` ext column,
`shape_cache_hits`/`shape_cache_misses`) was dropped; `<spark-line>` and `paint_cache_hit_rate` use SG6's, so a
provider kind built by the RA3 macros is covered by SG6's cache unchanged.

### 5. Object-handle props (K1 gap 1)

`#[prop] doc: Option<Obj<DocHandle>>` works: the host's exported-object table is shared (`Core::apps` is an `Rc`)
and pushed on a thread-local scope for every host op (`exports::objects::scope` in `Core::timed` and
`take_changes`), and `__private::with_cx` uses the innermost one. So a provider's `set_prop`, `get_prop` (the
observation read-back) and method arguments convert `Obj<T>`/`Arc<T>`. Outside an op (event payloads emitted
during a pass) the table is still empty, so object event payloads do not convert yet. Example: `<doc-title doc>`.

### 6. `Widget::text_input` (K1 gap 2)

`fn text_input(&self) -> Option<bool>` (`Some(multiline)`, default `None`): a provider whose egui text field has
the node's id is recorded as `Role::Text { multiline }` for EV3 (text-field key rules, R11), and
`provider::paint` runs `focus::before_text`/`after_text` around its `ui`, so `Embed::text_command` (the Edit
menu's Cut, Copy, Paste, Undo, Select All, Insert) reaches it as the input events they stand for. This is the
TextInput unit's entry point of ARCH-WIDGETS (OW4 builds the unit itself for `text-edit`). Its value is the unit's
`multiline` param (ARCH-WIDGETS §5 TextInput row), so when OW4 lands the macro can derive it for a kind with
`#[behavior] text: TextInput` (alongside `units_claims`); a hand-written kind returns it directly, as
`<quantity-field>` does.

### 7. `bind:x|commit` on provider kinds (CX3)

The CX3 rule is one function now, `spec::commit_binding` (the commit event has the shape of the prop's change
event): built-ins compare signal arguments, provider kinds compare the `commit` event's payload type with the
prop's type without nullability. A provider bound prop with such an event gets a `bindings.<prop>` entry
(`event: "commit"`, `commit: {"event": "commit", "arg": 0}`); bound props without one still have no entry and
lower onto observation. `<quantity-field>` has `#[event] commit: Event<f64>`.

## Decisions taken inside the design (no doc settled them)

1. **Method declaration: `#[svelters::widget]` on the kind's `impl` block**, matching `#[svelters::export]` on an impl;
   no flag on the struct. Methods reach the registry through `inventory` (`KindMethods`), like kinds themselves.
2. **JS names are lowerCamelCase** (`revealLine`), as exports are and as ARCH-CRATES §4.2 writes them; built-in kinds
   keep their kebab method names.
3. **Direct member calls (`ed.revealLine(n)`) only for provider kinds** (pack 62). Toolkit kinds keep `call(name)`:
   resolving every missing member through GIRepository on GTK would change behaviour there.
4. **Methods run between passes and never run JS.** Their JS-visible writes are delivered after the next pass, as a
   widget's own are; method results return synchronously. No async methods (an `async`/offloaded kind method can
   come later through the export offload machinery).
5. **Errors**: a `NativeError` from a method (its `Err`, or an argument that did not convert) reaches JS as a native
   error (`SVL_ERROR`: domain `app:<namespace>`, the `svelters::Error` code), message `<tag>.<method>: <message>`.
   Argument count and primitive type errors are the host's `TypeError`s, before Rust runs.
6. **Security**: every method call goes through `provider::methods::call`, the place a policy check (ARCH-SECURITY,
   deny by default) belongs. No hook is built (security work is cut); nothing bypasses that function.
7. **Container slot**: only the default slot (`children`). Named slots (`#[slot(name = "header-extra")]`) are not built.
8. **`cached_shapes` is SG6's** (see section 4); RA3 adds no paint cache of its own.

## Tests

- `examples/egui-kinds/tests/methods.rs`: registry, descriptor, fragment and typings of the methods; Svelte
  (`app/src/Methods.svelte`, loaded from source) calls `setText`/`commit`/`valueIn`/`focus` as members and through
  `call`; `cx.emit` reaches `onedited`, `cx.set` reaches `bind:value`, state bits apply, a wrong argument type throws.
- `examples/egui-kinds/tests/widgets.rs`: fn form (`level-meter` measured 120×14 by the sizing pass, `step-count`
  bound `&mut`), wrapped `DragValue` (`wrapped-drag-value`, since `drag-value` is a built-in: JS writes reach it, typing is announced to `bind:value`), container
  (`collapsing-section`: Svelte children placed under the header, hidden and relaid out when closed), and the paint
  cache hit-rate test (`spark-line`: 20 unchanged repaints are hits, a new `values` rebuilds once).
- `app/src/Methods.svelte` and `Widgets.svelte` are also checked by svelte-check in `app`'s `pnpm test`.
- `svelters-compiler/tests/cx1_check.rs` `fixes_in_the_corpus_resolve` now checks a corpus file that has an app's
  `.svelters/app/kinds.json` above it against the built-ins plus that fragment (`<collapsing-section>` alone got the
  fix-it `collapsing-header`, which has no `title`).

## Deferred

- **SV23** (dynamic export modules from a descriptor, publisher generations): not started. It needs LOADER's resolver
  for `svelters:app/*` (soft dependency LD1) and a `Schema` built from a descriptor rather than `inventory`; see
  "Open" below.
- Named container slots; `WidgetCx::store`/`object` (Rust reads of stores/objects); object values in event payloads
  and object *results* of kind methods (JS would get a bare handle, not the class wrapper).
- K1 gap 3 (deep `$state.snapshot` of nested arrays in record props) is RT1's, not touched here.
- Async kind methods; methods on behaviours (the macro accepts them, nothing calls them yet).

## Open

- SV23 shape (proposal for the coordinator): a `DynModule { descriptor: svelters-exports/1 JSON, handler: Rc<dyn
  DynHandler> }` registered on the host at run time returns a `Publisher` with a generation; the resolver serves
  `svelters:app/<name>` from it; handles minted by a module carry its generation and are refused after unregistering
  (the E3 rule); `Schema::from_descriptor` feeds the SV24 codegen for typings.
