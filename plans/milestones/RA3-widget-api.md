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
| `#[svelters::widget]` on an inherent `impl`: every `pub fn` with `&self`/`&mut self` is a method, JS name lowerCamelCase (`#[js(rename)]`, `#[js(skip)]`); params `FromJs`, result `IntoJs`; a `&mut MethodCx` param (anywhere) is the context. Names that are members of every runtime node (`call`, `tag`, `style`, `then`, …) are rejected at compile time | `crates/svelters-macros/src/lib.rs` (`widget_methods`) |
| `MethodEntry`, `KindMethods` (inventory), `MethodCx` (`id`, `node`, `scene`, `rect`, `state`/`set_state`, `set` exact write, `emit`, `ext`/`ext_mut`, `request_focus`, `request_measure`), `call` | `crates/svelters-egui/src/provider/methods.rs` |
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

`WidgetCx::cached_shapes(ui, version, |shapes, size| …)`: shapes relative to the rect's top left, rebuilt only when
`version`, the rect size or `pixels_per_point` change; otherwise the cached shapes are painted again, moved to the
rect. Stored per node in an unjournaled ext column (`ExtColumns::cache_mut`, new: caches only), counted in
`SceneStats::shape_cache_hits`/`shape_cache_misses`.

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
5. **Errors**: a `NativeError` from a method (argument conversion included) reaches JS as an `Error` whose message is
   `<tag>.<method>: <message>`; the backend ABI carries strings only, so the code is not kept.
6. **Security**: every method call goes through `provider::methods::call`, the place a policy check (ARCH-SECURITY,
   deny by default) belongs. No hook is built (security work is cut); nothing bypasses that function.
7. **Container slot**: only the default slot (`children`). Named slots (`#[slot(name = "header-extra")]`) are not built.
8. **`cached_shapes` takes a `Vec<egui::Shape>`**, not a `Painter` (egui has no recording painter); the version covers
   theme changes (the cache key adds only size and scale).

## Tests

- `examples/egui-kinds/tests/methods.rs`: registry, descriptor, fragment and typings of the methods; Svelte
  (`app/src/Methods.svelte`, loaded from source) calls `setText`/`commit`/`valueIn`/`focus` as members and through
  `call`; `cx.emit` reaches `onedited`, `cx.set` reaches `bind:value`, state bits apply, a wrong argument type throws.
- `examples/egui-kinds/tests/widgets.rs`: fn form (`level-meter` measured 120×14 by the sizing pass, `step-count`
  bound `&mut`), wrapped `DragValue` (`drag-value`: JS writes reach it, typing is announced to `bind:value`), container
  (`collapsing-section`: Svelte children placed under the header, hidden and relaid out when closed), and the paint
  cache hit-rate test (`spark-line`: 20 unchanged repaints are hits, a new `values` rebuilds once).
- `app/src/Methods.svelte` and `Widgets.svelte` are also checked by svelte-check in `app`'s `pnpm test`.

## Deferred

- **SV23** (dynamic export modules from a descriptor, publisher generations): not started. It needs LOADER's resolver
  for `svelters:app/*` (soft dependency LD1) and a `Schema` built from a descriptor rather than `inventory`; see
  "Open" below.
- Named container slots; `WidgetCx::store`/`object` (Rust reads of stores/objects); object handles (`Obj<T>`) in kind
  props and method arguments (conversions still use `with_cx`, without the host's object table).
- Async kind methods; methods on behaviours (the macro accepts them, nothing calls them yet).

## Open

- SV23 shape (proposal for the coordinator): a `DynModule { descriptor: svelters-exports/1 JSON, handler: Rc<dyn
  DynHandler> }` registered on the host at run time returns a `Publisher` with a generation; the resolver serves
  `svelters:app/<name>` from it; handles minted by a module carry its generation and are refused after unregistering
  (the E3 rule); `Schema::from_descriptor` feeds the SV24 codegen for typings.
