# svelters-egui: what an all-Rust app makes possible

Status: research note, 2026-10-21. It covers one slice of the M7 design: the compiler integration, developer experience, tooling and deployment that become possible because a svelters-egui app is a single cargo build. The egui backend's widget mapping, layout, its own semantic spec and its event model belong to other slices. The tag names below are illustrative.

Probe: `docs/notes/scripts/egui-allrust-probe/` (standalone, own `[workspace]`). It contains a `build.rs` that runs `svelters-compiler`, a proc-macro crate (`svelte-macro`) with `include_svelte!` / `svelte!`, and `size-probe` (eframe + QuickJS-NG release size). Section 4 gives the exact commands and results.

## 0. Summary

egui is the one backend where the widgets, the backend, the host core, QuickJS and the compiler can all be Rust crates in the same process and the same cargo build. Three things follow, and most ideas below use one of them:

1. **We own the native tree.** GTK and Qt widgets are opaque toolkit objects. An egui "widget" is a row in the backend's own node table, re-rendered every frame. Snapshots, rewinds, overlays, CSS matching and semantics need no toolkit introspection: they read our own data structure.
2. **AccessKit comes for free.** egui builds an AccessKit tree every frame, and eframe enables `accesskit` by default (`eframe-0.37.0/Cargo.toml`: `default = ["accesskit", ...]`). So `egui_kittest` (headless tests in `cargo test`), egui's inspection protocol and `egui_mcp` (agents) work on any svelters-egui app once nodes carry labels.
3. **Rust code can join the Svelte tree, and the reverse.** A Rust closure can render a Svelte element, and a Svelte component can mount into any `&mut egui::Ui`. Rust types can describe themselves to the compiler at build time.

The rule "no ordinary `.svelte`/`.ts` edit invokes Cargo in development" (PLAN rule 5) still holds. Build-time compilation is for `cargo check`/`cargo build` diagnostics and for release embedding. The dev loop stays in-process.

### Ranked shortlist

Value is for svelters-egui users and the project. Cost: S ≤ 2 days, M ≤ 2 weeks, L > 2 weeks. "Cheap win" means S cost and either free or nearly free once the M7 backend exists.

| # | Idea | Value | Cost | Cheap win? | Depends on |
|---|---|---|---|---|---|
| 1 | Headless component tests in plain `cargo test` via `egui_kittest` (no Xvfb), with image snapshots | H | S–M | yes, given labels | M7 backend writes AccessKit labels/roles from nodes |
| 2 | Rust widgets in Svelte: register a Rust `fn(&mut Ui, &Props)` as an element kind; the egui spec and `.d.ts` are generated from Rust (no Python pipeline) | H | S–M | partly | M7 kind table |
| 3 | `svelters-build` in `build.rs`: diagnostics as `cargo::error`/`warning` with `.svelte:L:C`, typings, release bytecode table embedded with `include_bytes!` | H | M | diagnostics part: yes (probe) | Q1 module walk (QUICKJS-RUNTIME §3); Q3 CSS port for `<style>` |
| 4 | In-app devtools overlay (F12): component and node tree, props, op log, timings, JS heap | H | S (v0) / M (reactive graph) | v0 yes | host diagnostic sink (inspector-demo.md §3); runtime dev hooks for the reactive graph |
| 5 | Agents: eframe `inspection` feature + `egui_mcp` out of the box; Svelte-level MCP tools merged in through `egui_mcp::UiServer` | H | S (free part) / M | free part: yes | CLI decision 7 (no MCP yet) needs revisiting |
| 6 | Svelte islands: mount a component into a caller-owned `Ui` (egui-workbench pane, bevy_egui, any eframe app) | H | M | — | host start-order change (non-blocking run), also needed for mobile |
| 7 | Profiling: puffin/tracy scopes across checkpoint → JS → host ops → egui frame | M | S | yes | — |
| 8 | Panic/crash reports with source-mapped JS stack and the last N ops | M | S | yes | `sourcemap.rs` |
| 9 | Instant style reload plus CSS in Rust on our own scene (`cssparser` + `selectors`): `:hover`/`:focus`/`:active`, borders, radii, shadows, transitions | M–H | M | — | Q3 (Svelte CSS scoping in Rust) |
| 10 | Deterministic record/replay and time travel: inputs as `svelters_backend_log::Step`s plus egui `RawInput`; scene snapshots by `Clone` | M–H | M | — | virtual clock (exists in the log backend) |
| 11 | Shared Rust state ↔ `$state`: exported Rust models as observable native types over the R1 `__svelters_changed` batch | H | M | — | `#[svelters::export]` (other agent), R1 tracker |
| 12 | Single static binary and easy cross-compilation (C only from QuickJS) | M | S | yes (mostly free) | release bytecode (#3) |
| 13 | Rust hot-patching of user widget code (`subsecond`) without losing JS state | M | M | — | subsecond 0.8 alpha; `dx` as the patch producer |
| 14 | Web target: eframe web + QuickJS compiled to wasm, or the browser's engine behind `JsEngine` | M | M–L | — | async `load_module`; measurements |
| 15 | Editor support: `.d.ts` from Rust schemas (S); a diagnostics-only `svelters lsp` (M); full LSP (L) | M | S / M / L | `.d.ts`: yes | #2, #11 |
| 16 | Mobile via egui (Android/iOS through winit) | L–M | M–L | — | #6 start order; IME quality |
| 17 | Partial AOT to Rust egui code | L | L | no | — (a static-template op is the better variant) |

**Do first** (after the M7 slice renders a counter): 1, 2 and 7/8. Then 3 together with Q1, 4 v0, and 5's free part. Item 6 should be decided early, because it fixes the host/eframe ownership model. Everything else waits for demand.

## 1. What egui changes underneath

- **The backend is in-process Rust.** The C ABI (`crates/svelters-abi/include/svelters_backend.h`) stays the contract, and the egui backend implements the safe `svelters_abi::Backend` trait (`crates/svelters-abi/src/safe.rs:1320ff`). Unlike GTK and Qt, nothing outside the cargo graph is linked: no system GTK and no C++ leaf crate (`svelters-qt-build`).
- **The loop.** `schedule_wake(ms)` maps to `ctx.request_repaint_after` (already noted in `plans/architecture/backend-abi.md:219`). `wake` maps to `ctx.request_repaint()`. `egui::Context` is `Send + Sync`, so the waker is trivial. A checkpoint runs at the start of each egui pass, before any `Ui` borrow (the SVR14 re-entrancy rule in `plans/archive/phases/SVR14-egui-renderer.md`). Response-derived events are queued and dispatched only after the pass ends.
- **Identity.** A node handle maps to `egui::Id` (a stable salt from the handle), so focus, scroll and collapsing state persist. This matters for hot reload too: the full reload in `reload.rs` makes new handles, so egui memory resets unless ids derive from something reload-stable. Section 2.8 covers this.

## 2. The ideas

### 2.1 Compile at `cargo build` time (build.rs / `include_svelte!`)

**What it enables.**
- `cargo check` and `cargo build` fail on template errors, at `App.svelte:L:C` with the compiler's did-you-mean help.
- CI needs only cargo, and release builds need no Node or Vite.
- Release binaries carry precompiled QuickJS bytecode (`include_bytes!`).
- rust-analyzer surfaces the same errors, because it runs build scripts and proc macros.

**How it works here.**
- `svelters-compiler` is already a light, pure-Rust crate. Its dependencies are `serde`, `serde_json` and `sourcemap` (`crates/svelters-compiler/Cargo.toml`), and it embeds its specs with `include_str!` (`src/semantics/backend.rs:8-13`). It is directly usable as a build-dependency (probe, §4).
- Two forms were probed:

| Form | Diagnostics | File tracking | Verdict |
|---|---|---|---|
| `build.rs` + `cargo::error=` / `cargo::warning=` | `error: <pkg>: /abs/ui/Bad.svelte:7:6: error[unknown_element] (gtk): <buton> is not an element … help: did you mean one of button, action, box?`. Cargo 1.97 fails the build with "build script logged errors" | `cargo::rerun-if-changed=ui` | **Recommended.** It sees the whole module graph, can write typings and a bytecode table, and runs once per crate |
| proc macro `include_svelte!("ui/App.svelte")` | `compile_error!` at the macro call site; the message carries `file:L:C` | expansion contains `include_str!(path)`, so rustc tracks the file. Verified: editing the `.svelte` re-expands and the error appears | Good for single components or snippets. One file at a time, no graph |
| proc macro `svelte!(r#"…"#)` inline | `Literal::subspan` is **unstable on 1.97** (`proc_macro_span`, rust#54725), so the error lands on the whole literal, with `inline.svelte:L:C` in the text | — | Only raw strings work: Svelte text (`don't`, `{/if}`) does not tokenize as Rust. Low value |

**Probe finding that shapes the design.** QuickJS-NG's `Module::declare` resolves and loads imports even when it only compiles. The first attempt failed with `could not load module '@svelters/native-runtime/core'`. So build-time bytecode is not per-file. `build.rs` must walk the whole graph with a resolver, which is exactly the Q1 `svelters-modules` crate from `plans/research/QUICKJS-RUNTIME.md` §3.1. The `qjs-loader-probe` already does that walk and writes a `Module::write` table. Recommended shape: one `svelters-build` crate, used by `build.rs`, by the dev loader and by `svelters build`, so the three cannot drift:

```
svelters-build (build-dependency)
  ├─ svelters-modules (Q1): oxc_resolver + alias table + transforms (compile / compile_module / TS strip / CSS)
  ├─ diagnostics → cargo::error / cargo::warning (Diagnostic::render, one line each)
  ├─ typings → src/svelters.d.ts (elements from spec + Rust overlays, §2.2/§2.11)
  └─ PROFILE=release → OUT_DIR/app.qbc (R1 table: (importer, specifier) → id, Module::write, version stamp)
```

**Costs and risks.**
- **Cost M.** The diagnostics half is S and works now. The table half is Q1 + Q4 (~5–7 days per QUICKJS-RUNTIME §6), plus Q3 for apps with `<style>`.
- **Runtime JS without `node_modules`.** The compiler output imports `@svelters/native-runtime`, which imports stock `svelte@5.57.2` internals. For a cargo-only app, ship the runtime as a pre-bundled JS blob inside a crate (`svelters-runtime-js`: Vite/Rolldown runs once at publish, `include_str!` in the crate). Then an app with no npm dependencies needs no `node_modules` at all. Apps with npm dependencies still need a package manager (QUICKJS-RUNTIME §3.6).
- **Cost of the embedded specs.** Every binary that links the compiler carries all four specs (gtk.json alone is 813 KB). Loading the gtk spec takes 19–24 ms per build-script or macro invocation (probe, build scripts at opt-level 2). Add per-toolkit cargo features to `svelters-compiler`, and cache the parsed spec in a `OnceLock` inside proc macros.
- **Bytecode portability.** Bytecode written on the build host must load on the target (cross builds, wasm). It is tied to the exact QuickJS-NG build (QUICKJS-RUNTIME §3.5). Whether it is also independent of word size and target was not checked. Stamp it, test loading on each target in CI, and fall back to embedding source plus a first-launch cache.
- **Same-crate schemas.** `build.rs` runs before its crate compiles, so it cannot see `#[svelters::export]` items in that same crate. See §2.2.

### 2.2 Rust-defined widgets and types become element typings (no Python pipeline)

**What it enables.** The egui ecosystem is Rust: `egui_plot`, `egui_extras` tables, `egui_dock`, custom painters. A user writes a Rust function and uses it as a Svelte element, with compile-time checking and `.d.ts` typings. No GIR, qdoc or winmd-style ingestion is involved. The same holds for the egui backend's own built-in kinds.

**How it works here.**
- **The egui spec comes from Rust.** The backend's kind table (tags, props with types and defaults, signals, children policy, observe class) is Rust data. Emit `metadata/semantics/egui.json` from it: `svelters-egui --dump-spec`, or a test that writes and checks the file, in the spirit of `tools/metadata/*/ledger.py`. The spec format is `docs/notes/native-compiler/semantic-spec.md`. The Rust table becomes the single source, so drift is impossible. On every other toolkit, the Python ingestion stays.
- **User widgets** carry a derive on their props struct, for example `#[derive(svelters::Props)] struct SparklineProps { data: Vec<f64>, #[prop(default = 40.0)] height: f32 }`, plus `app.widget("sparkline", sparkline)`. The derive emits two things:
  - a `KindSpec` fragment (props with `PropType`, `writable`, `default`; signals from an `emit` list; `children: none` unless the widget takes a `Ui` callback for children);
  - the `FromValue` glue from `svelters_abi::Value`.
- **Getting the fragment to the compiler at build time:**
  - **Separate crate (recommended).** Put widgets and exported models in a `my-app-widgets` crate. Its `build.rs` scans its own sources with `syn`, as cbindgen does, and publishes the schema path with `cargo::metadata=schema=…`, which requires a `links = "…"` key. The app's `build.rs` reads it as `DEP_<LINKS>_SCHEMA`. This is standard cargo plumbing, with no side files in `target/`.
  - **Same crate.** `svelters-build` runs a `syn` scan over `src/`. This works well for `#[derive(svelters::Props)]` structs. It cannot see items generated by other macros.
  - **Check after build.** The macros register fragments with `inventory`/`linkme`, and a generated `#[test]` recompiles the app's components against the full set. The error then arrives at `cargo test` time, not `cargo build` time. Use this only as a fallback.
- **In the compiler.** `PlatformSemantics` already has an `Overlay` (`src/semantics/overlay.rs`), but it only overrides observability. Add `SpecBackend::merge(fragment)` (or an overlay that adds kinds and tags) and a `TypeTable` merge for exported non-widget types (`src/semantics/types.rs`, the same shape the NT0 types tables use). Cost S.

**What gets checked.** The compiler has no TS type checker: it strips TS (`svelte::ts::strip`). Build-time checks cover element, prop, event, slot and child-attribute existence; static literal coercion (`coerce_static`); `bind:` plans; and observability (R2's seven diagnostics). Expression types need the generated `.d.ts` plus `svelte-check` (Node) or a future native TS checker.

**Cost and risks.** S–M. The derive macro is the main work. Risk: this overlaps with the `#[svelters::export]` design in `docs/notes/egui-rust-exports.md`, so share one schema format with it (`KindSpec`/`TypeSpec` JSON) rather than inventing a second.

### 2.3 Rust UI inside a Svelte tree (`<rust-ui>` / registered kinds)

The reverse of islands. Because the egui backend draws every node itself, a node kind can be backed by a closure:

```rust
app.widget("sparkline", |ui: &mut egui::Ui, p: &SparklineProps, cx: &mut WidgetCx| {
    let r = ui.add(Sparkline::new(&p.data).height(p.height));
    if r.clicked() { cx.emit("picked", r.hover_pos()); }   // → onpicked in Svelte, queued (no JS during the pass)
    r
});
```

- Props arrive through the ordinary `set_prop`, decoded once per change, not per frame.
- Events go through the normal event queue.
- `bind:` works through R1 observation: the closure calls `cx.changed("value")`, and the host's compare/read-back path does the rest.
- A generic escape hatch `<egui-ui draw={id}>` needs no spec entry, but is untyped. Prefer registered kinds.

Cost S on top of the M7 node table, since this is just one more kind with a function pointer. The value is high: it is how svelters-egui gets the egui ecosystem without writing per-widget backend code. This replaces CUSTOM-WIDGETS.md's pack machinery for egui (no `svelters_pack.h` and no dlopen: it is all one link).

### 2.4 Svelte islands in egui (embedding)

**What it enables.**
- A Svelte panel inside egui-workbench (the `svelters-egui-workbench` adapter from `plans/archive/architecture/ecosystem-integrations.md`).
- A Svelte HUD or editor in a Bevy game through `bevy_egui`.
- A Svelte settings dialog inside any eframe app.

**How.**
- `SvelteIsland::new(app_source).show(ui)` runs a host checkpoint, then renders the island's root nodes into the caller's `Ui`.
- Today `Backend::run(host)` owns the event loop (`svelters_abi::Backend::run`). The egui backend needs a second mode: no `run`. The embedder's frame drives `checkpoint()`, and `wake`/`schedule_wake` turn into `request_repaint*`. STATUS already lists "the host start-order change (non-blocking `run` plus a started hook)" as a prerequisite for iOS and Android. This is the same change, so do it once for all three.
- Islands share one `egui::Context`. Whether each island gets its own QuickJS runtime is a choice: one runtime per island gives isolation and independent reload (memory cost: about 6 ms startup per runtime per `apple-gui-backend.md` §8); one runtime with several roots is cheaper.
- egui-workbench rules fit: "render state; execute semantics in logic" and queued events match SVR14's re-entrancy rule. Workbench commands can be exported to JS as a `svelters:workbench` module, so a Svelte button invokes a `Command` and gets its `CommandExecutionId` back.

**Cost.** M. **Risks.** Hot reload inside a host app needs the island's own watcher. The embedder's `Ui` width and layout constraints become the island's root constraints, so the egui backend layout must not assume it owns a window.

### 2.5 Headless component tests in `cargo test` (egui_kittest)

**What it enables.** GTK and Qt GUI tests need a private Xvfb (`tools/dev/headless`; STATUS: 1690 GUI tests, about 10 min wrapped, 20 min not). An egui component test is a plain `#[test]`:

```rust
#[test]
fn increments() {
    let mut h = svelters_egui::testing::harness(svelters_egui::include_app!()); // Harness::new_ui over an island
    h.get_by_label("Increment").click();
    h.run();                                   // steps frames until idle; host checkpoints run inside
    h.get_by_label("Count: 1");                // AccessKit query (kittest 0.5 Queryable)
    h.snapshot("counter_after_click");         // egui_kittest `snapshot` + `wgpu` features; UPDATE_SNAPSHOTS=1 to bless
}
```

The API names were checked against the cached `egui_kittest-0.37.0` README and `kittest-0.5.0/src/query.rs`.

**How.**
- The harness wraps an island (§2.4) in `Harness::new_ui`.
- The host's virtual clock (`HostConfig::scripted` in the log backend) makes timers deterministic.
- Semantics: map each node's role and label into egui's AccessKit output, which egui widgets mostly do already. Custom Rust widgets (§2.3) must set `Response::widget_info`.

**Extras.**
- Gallery screenshots for the docs site (`docs/site/public/screenshots/`) and the screenshot tests can come from `cargo test` on egui, with no display.
- The same harness drives op-log comparisons: the backend logs ops in the shared op-log format (`plans/architecture/backend-abi.md#op-log-format`), so the fixtures' `expected.oplog` can be replayed against egui, as `crates/svelters-gtk/tests/oplog_replay.rs` does for GTK.

**Cost.** S–M once M7 renders labels. Of the whole list, this has the best value for its cost.

### 2.6 Agents and MCP

- **Free.** Turn on eframe's `inspection` feature (it pulls `egui_inspection` + `accesskit`), and with `EGUI_INSPECTION=1` the app listens on 127.0.0.1:5719. `egui_mcp` can then `attach`, `widget_tree`, `click`, `type_text`, `wait_for`, `screenshot` and `resize` (tool list in `egui_mcp-fork/crates/egui_mcp/src/tools.rs`). egui-workbench's `docs/VISUAL_DEBUGGING.md` documents this workflow for a real app.
- **Svelte-level tools (M).** `egui_mcp` is a library built for this: `UiServer::router()` can be merged into another MCP server's `list_tools`, with non-connection calls delegated to `UiServer::dispatch` (`src/lib.rs` docs). A `svelters mcp` server can add:
  - `component_tree`, `node_props`, `op_log(since)` and `diagnostics` (the last compiler run);
  - `reload`, `set_tokens`, `eval` (dev only) and `docs` (CL0's pack, already Rust);
  - all next to egui's UI tools in one session.
- **Decision needed.** CLI decision 7 says no MCP server for now, with `--json` as the agent interface. egui changes the trade-off, because the UI half already exists upstream.
- **Security.** The listener drives input. Keep it dev-only and loopback-only, and never enable it in release by default.

### 2.7 Devtools overlay in egui itself, and time travel

**What it enables.** A toggle key (F12) opens egui windows over the app, in the same `Context` and the same frame:
- component tree, native node tree, props (live values, editable through `set_prop`), listeners;
- the op log ring, checkpoint timings, JS heap (`Runtime::memory_usage`, QUICKJS-RUNTIME §4 #8), reload history;
- a picker: hover a node to highlight its rect, which egui knows from the last frame.

It is cheap because the overlay reads Rust data structures directly. No inspection protocol is needed, unlike the cross-toolkit `svelters-inspector` design in `docs/notes/inspector-demo.md`.

**How, mapped onto inspector-demo.md's four views.**
- (2) The insertion graph and (3) the native tree are the same thing on egui: our node table.
- (1) The Svelte ownership/component graph needs compiler and runtime dev hooks. The compiler already labels sources in `dev` (`$.tag`, `CompileOptions::dev`). The runtime can report component instances and effect dependencies.
- The reactive dependency graph (signal → effects → host ops) is the M part. It needs a dev build of `@svelters/native-runtime/core` that exposes its dependency edges. The own core (`plans/research/RUNTIME.md` phase 2+) makes this much easier than going through Svelte's internals.
- The host side is a passive diagnostic sink in `crates/svelters-host/src/core/ops.rs`, as inspector-demo.md §3 proposes: subscribers that never consume the queues.

**Time travel.**
- **Scene-only rewind (S–M).** The egui scene is plain data, so `Clone` a snapshot of the node table at each checkpoint into a bounded ring, and scrub with a slider. Nothing on GTK or Qt allows this.
- **Full JS time travel (M).** Record the inputs, not the states: every event, sync call, `changed` batch, timer firing and `gone`, which is exactly `svelters_backend_log::Step` (`crates/svelters-backend-log/src/lib.rs`). Replay a prefix into a fresh runtime with the virtual clock to reach step N. Determinism needs `Date.now`, `Math.random` and `performance.now` virtualized in replay mode (the host owns timers already). Expose it as an overlay feature first; the same recording later serves bug reports (§2.10).
- egui-native op-log replay can reuse the parser shape in `crates/svelters-gtk/tests/oplog_replay.rs` and `tools/oplog-compare.mjs`.

**Cost.** v0 (trees, props, op log, timings, heap) is S, because egui makes the UI cheap. The reactive graph and JS time travel are M. **Risk.** The overlay must not perturb what it measures: give its own ops and allocations a separate account.

### 2.8 Hot reload: JS, styles, Rust

State of the art, late 2026 (web, §6):
- **subsecond** (Dioxus) is at 0.8.0-alpha.3 on crates.io. It routes calls through a jump table (`subsecond::call`), is active only with debug assertions, and supports Linux, macOS, Windows, Android, the iOS simulator and wasm32.
- Its limits: only the tip crate is patched; no struct layout changes; thread-locals reset; static initializers are not re-run.
- Patches are produced by `dx serve --hotpatch`. A standalone producer needs the subsecond-cli/harness protocol, the app must report `main`'s address, and ThinLink is dx-only.
- Bevy 0.17 shipped opt-in system hot-patching on it. There is no egui integration upstream, so ours would be manual.

| Layer | Today | With egui |
|---|---|---|
| `.svelte` / `.ts` | full reload in-process (`reload.rs`), ~1.7–2 s dominated by Vite (M2b) | Q1 in-process loader: about 2 ms per component compile (probe, `compile` at opt-level 2) + warm bytecode; component HMR = Q5. Rust state survives every JS reload by construction: it lives in the process, not the runtime |
| `<style>` only | full reload | **instant**: compile the changed component, see that only the style hash changed, send the new sheet through `style()`, repaint next frame. No JS reload. Needs Q3 (CSS in Rust) and a per-part hash in the compiler output |
| egui widget/state memory | lost (new handles) | make `egui::Id` derive from a reload-stable identity (component source path + template position + key path: the `NodeIdentity` of `plans/archive/architecture/semantic-graph.md`), so focus, scroll and collapse state survive a full reload |
| user Rust widget code (§2.3) | n/a | `subsecond::call` around each registered widget closure and exported method. A patch never restarts the process, so **all JS state is kept**. Only for code in the tip crate, so recommend widgets in `main.rs`/tip during development, or accept a rebuild |

**Cost.** The JS side is already planned (Q1/Q5). Instant styles: S after Q3. Stable ids: S. subsecond: M, experimental, and needs `dx` installed. Rank it low until subsecond leaves alpha. `hot-lib-reloader` (dylib reload of a dependency crate) complements subsecond's tip-only limit, but its current maintenance was not checked.

### 2.9 Theming: X6 on egui, and how much CSS

**Today.**
- X6 translates scoped `<style>` into GTK CSS or QSS and sends it through one ABI op, `style(op, handle, text)`.
- `--sv-*` tokens carry light and dark sets.
- Layout stays props (`plans/milestones/X6-styling.md` decisions 1–7).
- `styling-gtk-css-qss.md` already names "egui Style/Visuals" as the theme target.

**On egui.** We own matching and rendering, so the backend can implement real CSS for the visual subset.
- **Matching.** Implement `selectors::Element` (the Servo crate) for our nodes and parse with `cssparser`. This gives type, class and id selectors, descendant/child/sibling combinators, `:not`/`:is`, and the dynamic pseudo-classes `:hover`, `:active`, `:focus` and `:disabled`. egui reports hover, press and focus per `Id` from the previous frame, so a state change restyles one frame later, which is the same as egui's own widgets.
- **Computed style → egui.** Apply it per node, scoped with `ui.scope`:
  - `color` and `background` map to `Visuals`/`WidgetVisuals` fills;
  - `border` maps to stroke, `border-radius` to `CornerRadius`, `box-shadow` to `Shadow`;
  - `padding`/`margin` map to `Frame` inner/outer margins;
  - `font-size`, `font-family` and `font-weight` map to `FontId`/`TextStyle` (weights need loaded font variants);
  - `opacity` maps to `ui.set_opacity`.
- **Transitions.** `transition` on colors and opacity uses `ctx.animate_value_with_time`. This is cheap and a visible DX win over GTK/Qt.
- **Tokens.** Tokens map onto `Visuals` defaults, and `prefers-color-scheme` maps onto `ctx.theme()`.
- **Not reasonable.** `transform`, `filter`, `::before`/`::after` content, `z-index` stacking inside a layout, and selectors over toolkit internals. Layout properties (`display`, `flex`, `gap`, `width`) stay props, as X6 decision 7 says. If the egui slice adopts `taffy` for layout, those props feed taffy, still not CSS.

**Cost and dependencies.** M, after Q3 (Svelte CSS scoping ported to Rust), since the translator otherwise runs in Node. The "egui dialect" is then just "pass the scoped CSS through and match natively". **Risk.** Restyling cost on large trees: cache computed styles per node and invalidate on class, state or sheet change.

### 2.10 Profiling, crash reports, determinism, shared state

- **Profiling (S).**
  - Use `puffin` scopes (0.20 cached locally) or `tracing` + `tracing-tracy` in the host checkpoint, JS dispatch, op batches, `dispatch_changed` and the egui frame, and show them with `puffin_egui` inside the devtools overlay.
  - JS frames come from the interrupt-handler sampler (QUICKJS-RUNTIME §4 #10: M) and can be emitted as puffin scopes, which puts JS and Rust on one timeline.
  - The host already records JS-entry time, host-op time/count and checkpoints (inspector-demo.md §1).
- **Panic and crash reports (S).**
  - A `std::panic::set_hook` in `svelters-egui` adds the current dispatch (handle, signal), the source-mapped JS stack (`sourcemap.rs`) and the last N ops from the overlay ring.
  - Release builds keep line numbers with `strip_source` only (QUICKJS-RUNTIME §3.5) plus an external map file.
- **Record/replay bug reports (M).**
  - egui's `RawInput` is serde-serializable (`egui-0.37.0/src/data/input/raw_input.rs:17`, `serde` feature).
  - Recording per-frame `RawInput` plus timer firings plus the app version gives a file that reproduces a session in `egui_kittest` (§2.5).
  - Combined with §2.7's `Step` recording, this gives deterministic repro of user bugs. The older `deterministic-rs` idea in the ecosystem notes becomes unnecessary for this.
- **Shared Rust state ↔ `$state` (M).** The authority on exports is the sibling note `docs/notes/egui-rust-exports.md`. It puts them in an engine-neutral registry in the host, in an `app:` handle range, not in the backend ABI. What follows is only the reactivity path, which matches it.
  - Do not invent a new channel. An exported Rust model (`#[svelters::export] struct Counter { #[observable] count: i64 }`) is a **native type handle** with `observe` class `exact`, like NT0's GObject types.
  - JS reads go through `@svelters/types` reactive getters (`observed`/`trackProp`, R1). A Rust-side mutation calls `host.prop_changed(h, "count")`, and the host's R1 batch delivers one `__svelters_changed` per checkpoint. That is already measured at 0.2 ms for 10k changes (STATUS R1).
  - The compiler sees the type through a `TypeTable` overlay (§2.2), so `bind:value={counter.count}` gets R2's `bind_prop` plan.
  - Zero-copy sharing of large buffers: `ArrayBuffer::from_source` (rquickjs 0.12+) over Rust-owned memory, for plots and tables.

### 2.11 Editor support and types across Rust and TS

- **`.d.ts` from Rust (S, part of §2.2).** `svelters-build` writes `src/svelters.d.ts`. It augments `svelteHTML.IntrinsicElements` for egui kinds and user widgets, as the core typings do (`plans/research/CUSTOM-WIDGETS.md` §2.3), and declares `svelters:app` modules for exported models. `svelte-check` and the stock Svelte VS Code extension then know the Rust API, with no LSP work from us.
- **Renames.** A Rust field rename regenerates the `.d.ts` on the next `cargo check`, and `svelte-check` (or `cargo build` for template-level uses) reports every stale use. That is the "type-safe cross-language refactor" in practice. A true rename refactoring across languages needs an LSP that understands both; we don't need that.
- **`svelters lsp`, diagnostics only (M).** Wrap `compile()` with the app's spec plus overlays, and publish diagnostics on open and change. It is a few hundred lines with `tower-lsp`/`lsp-server`. It adds the native-semantics diagnostics (`unknown_property` with did-you-mean, R2 observability) that the Svelte extension cannot produce. Completion and hover from the spec plus the CL0 docs pack would be the L part. Run it next to the Svelte language server rather than replacing it.

### 2.12 Partial AOT to Rust

**Measurement.** All 41 GTK fixture and gallery components were compiled with `target/debug/svelters-compile --backend gtk`, and the ops in the output were counted:

| op | count |
|---|---|
| element/toplevel creates | 431 |
| `$n.set` total | 636 |
| `$n.set` with a constant value (static) | 535 (84%) |
| `$n.append` | 440 |
| `$.template_effect` + bind effects | ~152 |
| `$n.on` | 126 |
| if/each blocks | 18 |

Galleries are showcase-heavy, so this overstates "static" for real apps. Even so, most structure and most property writes are static.

**But AOT buys little on egui.**
- Mount cost is about 1 µs-scale host crossings per op. On egui those crossings are in-process Rust calls; P5 measured host/ABI under 20 ms for 1000 rows.
- Compiling components to Rust breaks PLAN rule 5 (a Cargo rebuild per edit), unless AOT is release-only. Release-only AOT means a second code generator whose semantics must match the JS path forever.
- The dynamic parts (effects, handlers, blocks) stay JS anyway.

**Better variant (M, all backends).** A **static-template op**. The compiler hoists each static subtree, with its constant props and slots, into a template table emitted next to the bundle, and the runtime calls `$n.template(id)` once. The egui backend instantiates it in Rust from the table, and other backends loop over it natively. That cuts bytecode size and op count, like Svelte DOM's `template()` cloning, and it keeps one semantics. Rank full AOT last.

### 2.13 Web / WASM

**(a) QuickJS in wasm inside eframe web.**
- `rquickjs-sys 0.14.0` already ships `wasm32-unknown-unknown` bindings, a vendored wasi-libc and a `wasm-shim/shim.c` for clock, stdio and abort (its `build.rs:197-219`).
- The host core is engine-agnostic. The C ABI backend boundary is unneeded in wasm (all Rust).
- Gaps:
  - the watcher, `notify`, and filesystem loading (use the embedded bytecode table);
  - timers map onto `request_repaint_after`;
  - QuickJS on wasm is an interpreter inside an interpreter/JIT.
- **Size not measured.** `wasm32-unknown-unknown` is not installed on this machine, and installing targets was out of scope. Measure an eframe hello world + rquickjs + host in release with `wasm-opt -Oz` before committing.
- **Risk.** Bytecode from a 64-bit host loading in wasm32 (§2.1).

**(b) The browser's own engine behind `JsEngine`.**
- `crates/svelters-host/src/engine.rs` isolates the engine. A `WebEngine` would implement `dispatch`, `dispatch_sync`, `run_frame`, `fire_timer`, `dispatch_changed` and `run_jobs` through `wasm-bindgen`/`js-sys`, calling the globals the runtime installs (`__svelters_dispatch` etc.).
- `__svelters_host` becomes an object of exported wasm functions.
- It gains:
  - a JIT;
  - `Intl` (QuickJS has none: QUICKJS-RUNTIME §4 #11);
  - the browser's debugger and source maps for free;
  - no 1 MB-class interpreter.
- Costs:
  - `load_module(&Path)` is synchronous, but browser `import()` is async, so the trait needs an async load step;
  - re-entrancy wasm → JS → wasm must avoid `RefCell` double borrows (`RunningCtx` exists for the QuickJS case);
  - the bundle is served as files, not bytecode.
- Cost L. It is the better web story if web matters.

**Caveat.** An egui web app renders to a canvas: weaker text input/IME, accessibility only through egui's AccessKit web bridge (verify), and no SEO. If "the same Svelte app on the web" is the real goal, a DOM backend is the honest answer. `metadata/semantics/dom.json` already exists for conformance, and in the browser the stock Svelte compiler already does this. Recommendation: list web as M7+ "measure first", and don't promise it.

### 2.14 Single binary, cross-compilation, mobile

- **Single binary.**
  - App = eframe + host + QuickJS + embedded bytecode + optional compiler (dev feature).
  - No system toolkit dependency. On Linux it dynamically links only libc, libgcc, libm and the GL/X11/Wayland libraries that eframe dlopens at runtime.
  - Measured (§4): eframe/glow + QuickJS is 9.8 MB stripped without AccessKit and 14.6 MB with the inspection feature. P5 binaries were 3.4 MB (GTK) and 2.6 MB (Qt), but those exclude the toolkit's shared libraries, which egui does not need. Keep AccessKit on in release (accessibility, §2.5), and keep the inspection listener dev-only.
- **Cross-compilation.** QuickJS is the only C (built by `cc`), so `cargo zigbuild` / `cross` work for Linux arm64. The `x86_64-pc-windows-gnu` target is installed here. macOS still needs the SDK (the MAC1 lane links Mach-O from Linux already). Compare Qt, which needs a C++ toolchain plus Qt per target. This is a real deployment advantage. Cost S: a CI matrix job.
- **Mobile.**
  - eframe runs on Android (winit + android-activity) and on iOS through winit. Weaknesses: soft keyboard/IME and native look.
  - rquickjs-sys has no pre-generated Android/iOS bindings in 0.14 (the list has none), so it needs the `bindgen` feature. `docs/notes/android-backend.md` already covers QuickJS through the NDK.
  - It gives a cheap "runs everywhere, looks like egui" fallback while the native UIKit/Views backends (I1–I9, A1–A10) are decided. Cost M–L. Same prerequisite as §2.4 (non-blocking host start).

## 3. Hello world, as it should feel

The names `svelters-egui`, `svelters-build`, `svelters:app`, the tag names and the macros are proposals. `#[svelters::export]` is the other agent's design, used here only as a placeholder.

```toml
# Cargo.toml
[package]
name = "hello"
edition = "2024"

[dependencies]
svelters-egui = "0.1"      # egui backend + host + QuickJS-NG + embedded @svelters runtime JS (no node_modules)
svelters = "0.1"           # macros: export, Props

[build-dependencies]
svelters-build = "0.1"     # compiler + module walk: diagnostics, typings, release bytecode table

[dev-dependencies]
svelters-egui = { version = "0.1", features = ["testing"] }   # egui_kittest harness

[features]
default = ["dev"]
dev = ["svelters-egui/dev"]   # in-process compiler + watcher + F12 overlay + EGUI_INSPECTION
# release: cargo build --release --no-default-features   (alias: cargo dist)
```

```rust
// build.rs
fn main() {
    svelters_build::App::new("src/App.svelte")
        .toolkit("egui")
        .typings("src/svelters.d.ts")   // egui kinds + #[derive(Props)] widgets + exported models
        .run();                          // errors → cargo::error (App.svelte:L:C); release → OUT_DIR/app.qbc
}
```

```rust
// src/main.rs
use svelters_egui::{egui, App, WidgetCx};

#[svelters::export]                     // a native type handle; `count` is observable (R1 batch)
pub struct Counter { #[observable] pub count: i64 }

#[svelters::export]
impl Counter {
    pub fn reset(&mut self) { self.count = 0; }
}

#[derive(svelters::Props)]              // → <sparkline data height> in the spec and the .d.ts
pub struct SparklineProps { pub data: Vec<f64>, #[prop(default = 40.0)] pub height: f32 }

fn sparkline(ui: &mut egui::Ui, p: &SparklineProps, _cx: &mut WidgetCx) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), p.height), egui::Sense::hover());
    let max = p.data.iter().cloned().fold(1.0, f64::max);
    let pts: Vec<egui::Pos2> = p.data.iter().enumerate().map(|(i, v)| {
        let x = rect.left() + rect.width() * i as f32 / p.data.len().max(2) as f32;
        egui::pos2(x, rect.bottom() - rect.height() * (*v / max) as f32)
    }).collect();
    ui.painter().line(pts, ui.visuals().widgets.active.fg_stroke);
    resp
}

fn main() -> eframe::Result {
    App::new(svelters_egui::include_app!())      // dev: source + watcher; release: embedded bytecode
        .export(Counter { count: 0 })            // `import { counter } from 'svelters:app'`
        .widget("sparkline", sparkline)
        .run_native("Hello")
}
```

```svelte
<!-- src/App.svelte -->
<script lang="ts">
  import { counter } from 'svelters:app';   // typed by the generated svelters.d.ts
  let history: number[] = $state([]);
  function bump() { counter.count += 1; history.push(counter.count); }
</script>

<window title="Hello">
  <vertical spacing={8}>
    <heading>Count: {counter.count}</heading>
    <horizontal spacing={4}>
      <button onclicked={bump}>Increment</button>
      <button onclicked={() => counter.reset()}>Reset</button>
    </horizontal>
    <sparkline data={history} height={60} />
  </vertical>
</window>

<style>
  heading { color: var(--sv-accent); }
  button:hover { background: var(--sv-accent-bg); border-radius: 6px; transition: background 120ms; }
</style>
```

```rust
// tests/app.rs: no Xvfb, plain cargo test
#[test]
fn bump_and_reset() {
    let mut h = svelters_egui::testing::harness(svelters_egui::include_app!());
    h.get_by_label("Increment").click();
    h.run();
    h.get_by_label("Count: 1");
    h.get_by_label("Reset").click();
    h.run();
    h.get_by_label("Count: 0");
}
```

The loop this gives:

| Action | What happens | Expected time |
|---|---|---|
| `cargo run` | window opens; dev feature compiles `.svelte` in-process | first build: a cargo build; launch: about 0.1 s cold graph (qjs-loader-probe) |
| edit markup or script | in-process recompile of the changed module + reload (HMR with Q5) | ≈ 30 ms debounce + ms-scale compile |
| edit `<style>` | new sheet sent, next frame repaints, no JS reload | one frame |
| typo `<sparklin>` | overlay shows `App.svelte:12:5 unknown_element … did you mean sparkline`; `cargo check` fails the same way | — |
| rename `SparklineProps::data` | next `cargo check`: `unknown_property data` in App.svelte; `svelters.d.ts` regenerated for svelte-check | — |
| edit `sparkline()` in Rust | rebuild and restart; or, experimentally, a `dx`-produced subsecond patch that keeps the process and all JS state | a cargo rebuild / about a second for a patch |
| F12 | devtools overlay | — |
| `EGUI_INSPECTION=1 cargo run` | agents attach with egui_mcp | — |
| `cargo dist` | one binary with embedded bytecode; no Node at any step for this app | — |

## 4. Probe record

`docs/notes/scripts/egui-allrust-probe/`. Build it with the target and build dirs outside the repo (the repo's `.cargo/config.toml` sets `build-dir = "{workspace-root}/target/build"`, so set both variables):

```sh
S=/tmp   # any directory outside the repo
CARGO_TARGET_DIR=$S/egui-target CARGO_BUILD_BUILD_DIR=$S/egui-target/build cargo run -p egui-allrust-probe
PROBE_BAD=1 … cargo build                                 # build.rs diagnostics as cargo::error
RUSTFLAGS="--cfg feature_inline_bad" … cargo build        # inline svelte!(r#"…"#) error placement
… cargo build --release -p size-probe [--features inspection]
```

Results (2026-10-21, cargo/rustc 1.97.0, machine load average about 32 from concurrent agents, so wall times are pessimistic):

- `build.rs` with `svelters-compiler` as a build-dependency works unchanged. Per-run costs: spec load (gtk) 19–24 ms, compile 2.0–2.4 ms, bytecode 0.9–1.2 ms (build scripts at opt-level 2). Output: 831 B JS → 1903 B bytecode (source kept).
- `cargo::error=` lines fail the build, and each prints `error: <pkg>@<ver>: <abs>/ui/Bad.svelte:7:6: error[unknown_element] (gtk): … help: did you mean one of button, action, box?`. The first run also caught a real stale fixture: `onclick` instead of N1's `onclicked`, with the help `did you mean onclicked?`.
- Rebuild after touching a `.svelte`: about 0.5–1.1 s (build script rerun + relink).
- `Module::declare` resolves imports at compile time (QuickJS-NG). Build-time bytecode needs a resolver over the whole graph; the probe stubs runtime modules as `export {}`.
- `include_svelte!` re-expands when the file changes, through an `include_str!` in its expansion, and its errors carry `file:L:C`. `Literal::subspan` is unstable on 1.97 (`E0658 proc_macro_span`), so inline-template errors cannot point inside the literal.
- A cold build of the probe took about 6 min under that load, most of it `svelters-compiler` and `rquickjs-sys`. A later clean build of the probe crate with dependencies cached took 33 s, mostly the rquickjs-sys C compile. Build-dependency compile cost is real but one-off per profile.
- Size probe (`size-probe`: eframe 0.37 with `glow`, `default_fonts`, `x11`, `wayland`, default features off so **no AccessKit**, plus rquickjs evaluating JS every frame; release, `strip = true`, thin LTO): **9.8 MB** (9,825,976 B). `ldd` shows only libc, libm, libgcc_s and the loader; GL, X11 and Wayland are dlopened. With `--features inspection` (which adds AccessKit + egui_inspection): **14.6 MB**. A real app adds the host core, the egui backend and the bytecode table, but no compiler in release. Not tried: `opt-level = "z"`, `panic = "abort"`, fat LTO, or wgpu instead of glow (wgpu is expected to be larger).

## 5. Decisions for the user

1. Is egui's host mode "embedder drives checkpoints" (islands, workbench, bevy, mobile) from the start, or `run()`-owns-the-loop first? This note recommends both from M7's first slice, because the kittest harness (§2.5) needs the embedder mode anyway.
2. Should the egui spec be generated from the backend's Rust kind table instead of a Python pipeline? (This note says yes.)
3. MCP: revisit CLI decision 7 for egui, given `egui_mcp::UiServer` exists?
4. Ship the runtime JS inside a crate (`svelters-runtime-js`) so cargo-only apps need no `node_modules`?
5. Should a `dev` cargo feature (compiler + watcher linked into the app) be the default dev story for egui, instead of the `svelters dev` runner binary?
6. Web: measure only, or a goal? If a goal, should we consider the browser engine behind `JsEngine`, or a DOM backend instead of egui-on-canvas?

## 6. Sources

- subsecond docs (0.8.0-alpha.3): https://docs.rs/subsecond/0.8.0-alpha.3/subsecond/ and https://docs.rs/subsecond/
- Dioxus hot-reload reference: https://dioxuslabs.com/learn/0.7/essentials/ui/hotreload ; Dioxus 0.7 release: https://dioxuslabs.com/blog/release-070
- Bevy 0.17 hot-patching: https://gamefromscratch.com/bevy-0-17-released/ ; https://docs.rs/crate/bevy_hotpatching_experiments/latest
- egui_kittest: https://docs.rs/egui_kittest (and the cached `egui_kittest-0.37.0/README.md`)
- quickjs-emscripten NG wasm notes: https://github.com/justjake/quickjs-emscripten/blob/main/doc/@jitl/quickjs-ng-wasmfile-release-asyncify/README.md
- Local: crates.io versions checked 2026-10-21 (`egui`/`eframe`/`egui_kittest` 0.37.0, `rquickjs` 0.14.0, `subsecond` 0.8.0-alpha.3); `rquickjs-sys-0.14.0/build.rs`; `egui_mcp-fork` (`FORK.md`, `src/lib.rs`, `src/tools.rs`); `egui-workbench` (`README.md`, `docs/VISUAL_DEBUGGING.md`).
