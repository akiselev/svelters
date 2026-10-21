# EGUI: the svelters-egui backend (M7)

Status: research proposal, 2026-10-21. Nothing here is implemented. It combines five research notes written the same day, each with its own probe:

| Slice | Note | Probe |
|---|---|---|
| Retained scene graph, layout, events, frames, a11y | `docs/notes/egui-scene-graph.md` | `docs/notes/scripts/egui-scenegraph-probe/` |
| Custom widgets in JS, JS speed, JIT | `docs/notes/egui-js-widgets.md` | `docs/notes/scripts/egui-jsdraw-probe/` |
| Exposing app Rust types/functions/widgets, threads | `docs/notes/egui-rust-exports.md` | `docs/notes/scripts/egui-exports-probe/` |
| What all-Rust enables: compiler, devex, tooling | `docs/notes/egui-all-rust.md` | `docs/notes/scripts/egui-allrust-probe/` |
| egui 0.37 state, ecosystem, element catalog, styling, versions | `docs/notes/egui-ecosystem.md` | none |

**Measurement caveat.** Every probe ran on this WSL2 machine while the other agents were building (load average 10–35). Absolute times are pessimistic upper bounds; ratios within one table are the reliable part. Re-run on a quiet machine before treating any number as a budget. Probes are standalone crates with their own `[workspace]`. The repo's `.cargo/config.toml` sets `build-dir = "{workspace-root}/target/build"`, so set both `CARGO_TARGET_DIR` and `CARGO_BUILD_BUILD_DIR` outside the repo when running them.

## Summary

1. **The backend owns the tree; egui only paints it.** svelters-egui keeps a generational arena of typed nodes (`gen << 24 | idx` handles, never reused, stale handles detected). Host ops (`create`, `insert`, `set_prop`, ...) only edit that arena: they never touch egui. Each frame the backend walks the *visible* part of the tree and emits egui widgets at rects it already knows. This is the structural difference from every other backend, and it is what makes most of the rest possible. (§1)
2. **Layout is taffy over our own tree, measured with egui's fonts.** egui's single-pass layout can't do flexbox/grid. Taffy runs before the pass, only when something is dirty, so the first frame is right. egui-native containers (`scroll-area`, `panel`, `grid`, ...) stay as extra kinds. (§1.3)
3. **JS never runs while egui is drawing.** Events and widget-side value changes are queued during the pass; JS runs between frames, at the checkpoint. Sync handlers (`preventDefault`, close veto) run *before* the pass on raw input. Every widget-mutable prop is an `exact` observation with no echo, which corrects REACTIVITY.md §9.4's "egui = set-only + polling" assumption. (§1.4)
4. **JS widgets record a paint list once and Rust replays it every frame.** Hover fades, spinners, crosshairs and drag previews are late-bound slots evaluated in Rust, so they run no JS. A cached JS widget costs the same per frame as a hand-written Rust one. (§2)
5. **No JIT.** quickjs-jit 0.14.1 (matching our exact engine) was measured: 17–26x on a bare loop, 0.82–0.93x (slower) on paint code. V8 costs +48 MB and ~37 ms startup for gains we don't need once JS is off the frame path. The accelerator worth building is **our compiler compiling a typed TS paint subset to Rust in release builds** (~170x on recording, measured as the Rust baseline), while dev keeps interpreting the same source. (§2.3)
6. **App Rust types go through a host-level registry, not the toolkit ABI.** Macros (`#[svelters::object]`, `#[svelters::export]`, `#[svelters::store]`, `#[svelters::widget]`) register thunks in `svelters-host`. Because it lives in the host, **the same exports work on GTK, Qt, AppKit and WinUI**, so this can start before the egui backend exists. Where a call runs is in its signature (`#[js(worker)]`, `async fn`, ...). Big data crosses with no copy: a 64 MiB `Arc<[f32]>` became a read-only `Float32Array` in 0.02–0.045 ms, against 315–652 ms through today's ABI. (§3)
7. **Being all-Rust buys tooling no other backend can have.** Component tests in plain `cargo test` with no Xvfb, Svelte errors in `cargo build` at `App.svelte:L:C` (working today in the probe), the egui spec and `.d.ts` generated from Rust, an F12 devtools overlay drawn in egui, agents driving apps through `egui_mcp`, Svelte panels inside existing egui apps, and a single 10–15 MB binary. (§4)
8. **egui 0.37 is a credible base with known holes.** It has text shaping, system font fallback (CJK, colour emoji), AccessKit, `egui_kittest`, `Modal`, menus with keyboard navigation, and an experimental class-based theme system. It has no RTL/bidi, no OS-modal or transient windows, no native menus, and no tabs/splitter/tree in core. It ships a breaking minor every 2–3 months. (§5)

## 1. The scene graph (answers "egui has no persistent tree")

Source: `docs/notes/egui-scene-graph.md`.

### 1.1 Node model

- **Arena of typed nodes.** One `Node` per handle: kind, lifetime class, parent, named slot, ordered children, child attributes (`parent:row`, `parent:flex-grow`), an `egui::Id`, dirty flags, listen/observe bitmasks, common props (tooltip, a11y name/role, classes), taffy style + cache, last frame's rect, and a per-kind typed `Widget` enum (generated).
- **Typed props, not a map.** egui widgets take `&mut String` / `&mut bool` / `&mut f64`, so they bind straight into node fields. A dynamic map would force a conversion every frame.
- **Who owns which state.** Anything JS can read or write lives in the node: text, checked, value, open, selected, bound scroll offsets. Ephemeral state stays in egui memory under the node's Id: cursor/selection, IME preedit, scroll velocity, animations. Destroying a node clears its egui memory.
- **Ids.** v1: `Id::unique(("svl", handle))`. Measured in the probe: it survives keyed reorders (egui's positional ids don't: typing went into the wrong field), but not hot reload, since reload makes new handles. That matches GTK/Qt today. v2: an optional compiler-emitted `identity` prop (hash of file + template position + key path + instance path) survives reload too, and gives agents and screen readers stable ids. A `Ui` scope must never share an Id with the widget inside it: when the probe did that, the text edit stopped taking clicks.

### 1.2 The frame

```text
wake / timer / input / animation  →  eframe frame
  before the pass (raw_input_hook / logic, no Ui borrowed):
     checkpoint (JS runs, edits the arena) · rAF callbacks · sync input handlers on raw events
     (prevented → event removed) · virtual-list setup/bind for predicted rows · taffy if dirty
  the pass (ui):
     walk visible nodes → egui widgets at known rects; responses → event queue;
     &mut field changed → prop_changed (queue only); record rects; a11y annotations
  after the pass:
     flush queue; unbound visible rows → bind now; anything queued → checkpoint next frame + request_repaint
  idle: nothing requested → winit waits → 0% CPU
```

- egui multipass: input goes only to the first pass, so the outbox accumulates across passes and flushes once. The walk must not have side effects beyond the outbox.
- **Measured frame cost** (headless, no GPU):
  - a full walk is about 5 µs per node: about 3k nodes fit in 16 ms;
  - with culling by known rects, one dense 1280×800 screen (~380 nodes) costs 1.8–3.5 ms + ~1 ms tessellation, **whether the tree holds 1.1k or 55k nodes**;
  - AccessKit adds 1–2 ms per screen, only when a screen reader, kittest or egui_mcp turns it on.

### 1.3 Layout

- **Taffy on the arena** (flexbox, grid, block). Leaves are measured with `ctx.fonts_mut(..layout..)` plus egui's widget chrome. Bevy just deleted its mirrored `TaffyTree` for the same direct adapter, and Blitz does the same. egui_taffy and egui_flex only use last frame's sizes, which is why they jitter.
- **Measured** (mirrored tree; direct-on-arena unmeasured):

| Nodes | Cold layout | One label changed | Clean check |
|---:|---:|---:|---:|
| 1.1k | 25–30 ms | 1.5–2.3 ms | ~1 µs |
| 11k | ~160 ms | 8–9 ms | ~1 µs |
| 55k | 0.8–0.86 s | 43–53 ms | ~1 µs |

- Two guards found by the probe: only mark a leaf dirty when its intrinsic size actually changes (a counter going 1→2 usually doesn't), and the window root must have a definite width (with `auto`, one growing row re-measured every leaf: 218 ms at 55k nodes).
- Taffy's property names are CSS names, which fits ST1's setter tables.

### 1.4 Events, sync handlers, reactivity

- `post_event` and `prop_changed` only enqueue, so calling them mid-pass is safe. The "no JS while a `Ui` is borrowed" rule is enforced by *where* `checkpoint` and `call_sync` are called, and a debug "pass active" flag turns a violation into a panic.
- **Latency:** JS reacts at the start of the next frame, about one vsync later than GTK. An option is same-frame dispatch at the end of `ui()` plus `request_discard` (a second walk on interaction frames). Measure in a windowed demo before choosing.
- **Sync handlers** run before the pass: keys go to the focused node, pointer presses to the node hit-tested on last frame's rects, window close through `CancelClose`. Anything raised mid-pass falls back to async (`SVL_SYNC_BUSY`, as today).
- **Frames and wakes:** `wake` → `AtomicBool` + `request_repaint()` (thread-safe); `schedule_wake` → `request_repaint_after`; `request_frame` → repaint + `host.frame()` next frame. One caveat: eframe always runs `ui()` after `logic()`, so every JS timer tick costs a full frame even if nothing changed. A custom winit runner fixes that if it ever shows up in profiles.

### 1.5 Accessibility, testing, windows, lists

- AccessKit node ids *are* egui Ids, so stable node ids give stable a11y ids. The walk projects node role/name/description onto egui's AccessKit nodes; we don't build `TreeUpdate`s ourselves.
- `egui_kittest` drove a Svelte-style counter end to end in the probe. egui 0.37's kittest *fails* if an input has no accessible name, so the backend must forward labels. A compiler warning for unlabeled inputs would help every backend.
- Real OS windows are viewports (`show_viewport_immediate` in v1; `deferred` needs `Send` and our scene isn't). Popups, menus, tooltips → `Popup`/`Area`; modals → `egui::Modal`.
- `virtuallist` keeps the M5 protocol on `ScrollArea::show_viewport`; `setup`/`bind` run before the pass for the predicted range, and a row that still arrives unbound shows a placeholder for one frame.

## 2. Custom widgets in JS, and the JIT question

Source: `docs/notes/egui-js-widgets.md`.

### 2.1 Run JS per change, not per frame

`<egui:custom paint={fn}>` runs `fn` in a Svelte effect that **records a compact binary list of draw ops** (an extension of the X8 canvas recorder). Rust replays it every frame and re-records only when props, state or size change.

- **Late-bound slots** (evaluated in Rust each frame): rect, hover/press/focus and their eased `_t` versions, pointer position, drag deltas, time, theme colours, pixels-per-point. A small expression tape (`add`, `mul`, `lerp`, `mix_color`, `sin`, ...) combines them, and `when(reg)` skips ops (hover-only decorations, focus rings).
- **Hit regions** inside a widget (node-graph ports, range-slider handles) get their own hover/press slots and events.
- **Resize:** a program records the size it was made for. If egui gives a different size, the backend posts `resize` and uses `request_discard`, so JS re-records before the frame shows.
- **Example:** a knob is 4 ops (two `arc`s, a dot, a `when(focus)` ring). Moving the mouse over 100 knobs runs no JS; dragging one runs one event plus a 4-op re-record.

### 2.2 Measured (rquickjs 0.14.0, egui 0.37.0)

| What | Result |
|---|---|
| one draw op recorded from JS via a native host function | 1.0–1.1 µs (half is QuickJS evaluating the arguments) |
| same op in Rust | ~6 ns |
| batching into a `Float32Array` / JS-side encoder / today's X8 JSON | 1.2–1.4 / 2.1–2.3 / 4.8–6.0 µs per op: batching doesn't help in QuickJS |
| knob: JS vs Rust | 118–128 µs vs 2.3–2.5 µs |
| 2000-point sparkline: JS vs Rust | 2.45 ms vs 14 µs |
| 1000 rects per frame: native / cached list / slot-evaluated / JS re-records every frame | 0.62–0.73 / 0.70–0.73 / 0.71–0.79 / 2.8–2.9 ms |

So a cached JS widget costs what a Rust widget costs. Speed only matters for re-recording, and rich primitives (`arc`, `series(buffer)`, `mesh`, gradients) turn O(points) JS into O(ops) JS.

### 2.3 Acceleration options, ranked

1. **AOT paint subset (recommended).** Paint functions in a typed TS subset (numbers, fixed-shape objects, arrays/typed arrays, loops, `Math`, painter calls) compile to Rust through `build.rs` in release builds. Dev interprets the same source in QuickJS, so hot reload is unchanged. A content hash picks the native version, and a mismatch falls back to JS. A generated differential test runs both and compares the op lists. Unsupported functions stay JS with a diagnostic (`--strict-native` can make it an error). Estimate: 3–4 weeks.
2. **Rich primitives and data by reference** (cheap, first).
3. **V8**, only if *app-level* JS turns out to dominate: +48 MB, ~37 ms startup, jitless on iOS.
4. **Wasm widgets**, only for a third-party plugin ecosystem.
5. **Rejected:** quickjs-jit (measured, see Summary 5); Hermes (no `FinalizationRegistry`, which our handle release depends on, no ES modules); Boa (~5.7x slower than QuickJS); Nova, Porffor, SpiderMonkey; whole-component Svelte→Rust codegen (JS is already off the frame path).
6. Free-ish: QuickJS-NG PR #1744 (a GCC 14+ codegen fix, ~1.2x on everything) as a patch or with the next rquickjs.

## 3. Exposing app Rust code to Svelte (types, functions, threads)

Source: `docs/notes/egui-rust-exports.md`.

### 3.1 Where it lives

A Rust registry inside `svelters-host`, engine-neutral. The host routes handles with bit 63 set and kinds named `app:<module>.<Type>` to it instead of the backend. It reuses the existing JS wrapper classes, identity map, `dispose`, `{ signal }` cancellation, `NativeError`, the op log and R1's batch.

| Option | Per call (measured) | Why not |
|---|---|---|
| Backend C ABI | 2.9–4.0 µs | user types aren't toolkit code; ties an export to one backend |
| rquickjs `#[class]` | 0.72–1.04 µs | rquickjs lifetimes in app code; instances die on every hot reload; reimplements identity/dispose/batching |
| **Host registry** | in between (not measured yet) | recommended |

A direct-rquickjs fast path for scalar methods stays possible later behind a feature, if an app shows a chatty hot path.

### 3.2 What a developer writes

```rust
#[derive(svelters::Record)] pub struct SimParams { pub gravity: f32, #[js(default = 4096)] pub particles: u32 }

#[svelters::object(shared, module = "sim")]           // Arc<Self>, Send + Sync checked
pub struct Simulation { world: RwLock<World>, events: Emitter<SimEvent> }

#[svelters::export]
impl Simulation {
    #[js(constructor)] pub fn new(p: SimParams) -> Result<Self, SimError> { … }
    #[js(getter)]      pub fn particle_count(&self) -> u32 { … }
    #[js(worker)]                                       // rayon; Promise in JS; cancel + progress via cx
    pub fn step(self: Arc<Self>, n: u32, cx: WorkCx<f32>) -> Result<StepReport, SimError> { … }
    pub fn positions(&self) -> Shared<[f32]> { … }      // zero-copy, read-only Float32Array
}

#[svelters::store(module = "sim")]                      // any thread writes; Svelte reads reactively
#[derive(Default, Clone)] pub struct SimStats { pub step: u64, pub energy: f64, #[store(writable)] pub speed: f32 }
```

```svelte
<script lang="ts">
  import { Simulation, simStats } from 'svelters:app/sim';
  const sim = new Simulation({ gravity: 9.81 });
  const ctl = new AbortController();
  const run = () => sim.step(100_000, { signal: ctl.signal, onProgress: (p) => (progress = p) });
  let progress = $state(0);
</script>
<slider bind:value={simStats.speed} min={0.1} max={10} />
<label text={`step ${simStats.step}`} />
<button onclicked={run}>Run</button>
```

- Also: data enums → discriminated unions, `#[derive(svelters::Error)]` → `NativeError` codes, `Emitter<E>` events, `async fn` on tokio, `#[js(blocking)]`, `#[js(local)]`, streams (`Sink<T>` → `AsyncIterable`, bounded), `supersede` (a new call cancels the previous one, for "recompute while the slider drags"), `max_in_flight`, JS-implemented Rust traits (lowest priority).
- **Compile-time thread checks:** a worker method on a non-`shared` object, or a `JsFn` argument crossing to a worker, is a targeted compile error with a fix-it.
- **Results** go to a host completion queue drained at the start of each checkpoint (wake = `request_repaint` on egui, `g_main_context_wakeup` on GTK). The probe kept the JS thread free during a 200 ms worker job (156–166 1 ms ticks delivered meanwhile). Settling through today's callback path costs ~20 µs each, so the registry settles promises directly in the engine.
- **Zero-copy (measured):** 64 MiB `Arc<[f32]>` → read-only `Float32Array` 0.02–0.045 ms (copy: 80–94 ms); `ArrayBuffer.transfer()` JS → worker → JS 0.024–0.031 ms with the same memory coming back; the `Arc` is released on GC. One new owned value variant, `Value::Buffer`; C backends never see it. Writes to an immutable view are *silently ignored*, so typings mark them read-only.
- **Stores** set an atomic dirty bit and wake once; the checkpoint folds them into R1's dirty set, so store changes, native prop changes and events arrive in one batch, and `bind:` lowers onto writable store fields (they're `exact` by construction).
- **Hot reload:** Rust state outlives the JS runtime. Services, `StoreStatic`s and `persist("key", ...)` objects survive; handles carry a runtime generation and are released with it.
- **Types:** macros register a schema with `inventory`. A debug run writes `svelters-exports/1` JSON, `app.d.ts` and JS stubs (rewritten only on change), the host serves the same stubs as built-in modules, and `svelters-compile` reads the JSON through `PlatformSemantics::native_type`.

### 3.3 Rust-authored egui widgets as Svelte elements

```rust
#[svelters::widget(tag = "knob")]
pub struct Knob {
    #[prop(bind)] value: f32, #[prop(default = 0.0)] min: f32, #[prop(default = 1.0)] max: f32,
    #[event] change: Event<f32>,
    #[state] drag_origin: Option<f32>,          // per-node Rust state, survives prop updates
}
impl Widget for Knob {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx) -> egui::Response {
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::drag());
        if resp.dragged() { let v = …; cx.set(&mut self.value, v); cx.emit(&self.change, v); }
        paint_knob(ui.painter(), rect, self.value);
        resp
    }
}
```

- `cx.set` marks (node, prop) dirty → `prop_changed` after the pass, so `bind:` props are `exact` observations. A function form exists for stateless widgets; containers declare `#[slot] content: Children`.
- `wrap_widget!` wraps any `impl egui::Widget` built from props (it diffs the bound value after the call). Sketched wrappers: `egui_plot` (series as `Shared<[f32]>`, so data never copies), `egui_extras`/`egui_table` (Rust table model, virtualized), `egui_dock` (tabs as keyed child slots, layout bindable).
- Widgets are statically linked, registered in the app-widget id range. On egui, this replaces the `svl_pack`/dlopen machinery of CUSTOM-WIDGETS.md: it's all one link.
- **High-rate data skips JS:** a Rust plot reads a store or `Shared` slice inside `ui()`, so a worker can drive a 60 fps visualisation while JS only changes structure.

## 4. What all-Rust makes possible

Source: `docs/notes/egui-all-rust.md` (ranked list in its §0).

| Idea | Value | Cost | Notes |
|---|---|---|---|
| Component tests in plain `cargo test` via `egui_kittest`, image snapshots, no Xvfb | high | S–M | best value for its cost; GTK/Qt need a private Xvfb (1690 GUI tests, ~10 min) |
| Egui spec + `.d.ts` generated from Rust (built-in kinds and user widgets), no Python pipeline | high | S–M | one source, no drift |
| `svelters-build` in `build.rs`: `cargo::error` at `App.svelte:L:C`, typings, embedded release bytecode | high | M | **diagnostics work today** (probe: ~2 ms per component; caught a real stale `onclick`); bytecode needs the whole-graph Q1 loader because QuickJS resolves imports even when only compiling |
| F12 devtools overlay drawn in egui: component/node tree, live props, op log, timings, JS heap; scene rewind by cloning the arena | high | S (v0) | reads our own data; nothing to inspect remotely |
| Agents: eframe `inspection` + `egui_mcp` for free; Svelte-level tools merged via `egui_mcp::UiServer` | high | S (free part) | needs revisiting CLI decision 7 (no MCP server) |
| Svelte islands inside egui-workbench, bevy_egui or any eframe app | high | M | needs the non-blocking host start that iOS/Android need too |
| puffin/tracy across JS + Rust; crash reports with source-mapped JS stacks + last N ops | medium | S each | |
| Instant `<style>` reload; real CSS on our scene (`:hover`, borders, radii, transitions) | medium–high | M | after Q3 (Svelte CSS scoping in Rust) |
| Deterministic record/replay (`RawInput` is serde) and JS time travel | medium–high | M | |
| Single binary, easy cross-compilation | medium | S | eframe(glow)+QuickJS 9.8 MB stripped; 14.6 MB with inspection; QuickJS is the only C |
| Rust hot-patching (`subsecond`) without losing JS state | medium | M | 0.8 alpha, tip crate only, needs `dx` |
| Web via eframe web | medium | M–L | measure first; a DOM backend may be the better answer |
| Partial AOT of components to Rust | low | L | 84% of prop writes in our 41 GTK components are constants, but on egui host calls are already in-process; a **static-template op** (all backends) is the better variant |

**Hello world as it should feel** (names are proposals):

```toml
[dependencies]
svelters-egui = "0.1"    # backend + host + QuickJS + runtime JS embedded (no node_modules)
svelters = "0.1"         # macros

[build-dependencies]
svelters-build = "0.1"   # compile-time diagnostics, typings, release bytecode
```

```rust
fn main() -> eframe::Result {
    svelters_egui::App::new(svelters_egui::include_app!())   // dev: source + watcher; release: bytecode
        .widget::<Knob>()
        .run_native("Hello")
}
```

`cargo run` opens the window; editing `.svelte` reloads in-process; editing `<style>` restyles next frame; a typo in a tag fails `cargo check`; tests are plain `cargo test`; `cargo build --release` gives one binary with no Node at any step.

## 5. egui today, the element catalog, styling, versions

Source: `docs/notes/egui-ecosystem.md`.

- **egui 0.37.0** (2026-10-15, MSRV 1.95; we pin 1.97). Ten breaking minors in 27 months; the breaks are wide but shallow (renames, moved types). 0.38 likely brings bidi (PR #8577).
- **Tier 0, egui core only** (~25 elements + small composites we build): `window` (viewport), `inner-window`, `modal`, `panel`, `box`, `grid`, `sides`, `frame`, `scroll-area`, `collapsing-header`, `label`, `hyperlink`, `button`, `toggle-button`, `checkbox`, `radio-button`, `switch`*, `slider`, `range-slider`, `drag-value`, `text-edit`, `image`, `icon`*, `separator`, `progress-bar`, `spinner`, `combo-box`, the X4 menu kinds, `menu-button`, `popup`, `canvas`, `virtuallist`, `stack`* (* = built by us).
- **Tier 1:** `table` (`egui_table`: virtual rows *and* columns), `date-picker` (`egui_extras`), `color-picker` (core), `file-dialog` and `message-box` returning Promises via `rfd` (native dialogs; never lags egui), drag-and-drop; we build `tab-view`, `split-view`, `tree-view`, `toast`.
- **Tier 2, feature-gated packs:** `plot`, `tiles`, `dock`, `node-graph` (egui-snarl), `markdown`, `code-editor`, icon fonts, `native-menus` (muda; macOS/Windows), `inspector`.
- **Naming** follows N1 (kebab of the egui type), which already overlaps the other backends on `button`, `slider`, `progress-bar`, `grid`, `frame`, `image`, `separator`, `spinner`, `scroll-area`, `combo-box`, `radio-button`. Synthetic kinds take GTK-familiar names (`box`, `stack`, `switch`).
- **Known degradations to document:** no transient-for/OS-modal windows (`show="modal"` → in-window `Modal`), no native menubar without muda, no RTL text, tooltips are per-response props (not elements), no interactive widgets nested in a button (diagnostic).
- **Styling:** the backend owns the cascade because it owns the tree. The X6 translator emits a typed data sheet (selector AST, specificity, parsed declarations; tokens symbolic); the backend matches type/class/id/combinators and picks `:hover`/`:active`/`:focus`/`:disabled`/`:checked` from last frame's response (one frame late, the same as egui's own theming). Nearly every X6 property maps (`font-weight`/italic need real font faces). `--sv-*` tokens map almost 1:1 onto `Visuals`/`Spacing`. `transition` on colours/opacity works via `animate_value_with_time`, better than Qt. Adopt egui's experimental `StyleProvider` once it stabilizes.
- **Versions:**
  1. one egui minor per svelters-egui line;
  2. re-export `egui`/`eframe`/`egui_extras` from svelters-egui and a `svelters-egui-pack` crate;
  3. enforce a single copy: `links = "svelters_egui"` on the pack crate, and `cargo tree -d -i egui` in `svelters doctor`/CI (two eguis link fine and fail as `expected egui::Ui, found egui::Ui`);
  4. first-party elements depend only on same-week crates (egui_extras, kittest, inspection, egui_plot, egui_table, egui_tiles) or crates with no egui dependency (rfd, muda, taffy);
  5. lagging crates (egui_dock, snarl, ltreeview, file-dialog, phosphor) are packs, ported with the `egui_tiles-fork` pattern when needed;
  6. a per-minor upgrade lane: bump, fix, re-run kittest facts and gallery snapshots, rustdoc-JSON diff of egui as a drift detector.

## 6. Where the notes disagreed, and the proposed resolution

| Topic | Positions | Proposal |
|---|---|---|
| Style dialect | scene-graph: setter-only (`TARGET = NONE`); ecosystem + all-rust: backend cascade with pseudo-classes | **backend cascade** over a typed data sheet with our own small matcher (ecosystem: ~15 properties, 5 pseudo-classes; Servo `selectors` is overkill). Setter tables remain for layout props |
| Layout props in `<style>` | X6 decision 7: layout stays props; scene-graph: taffy speaks CSS, so setters can read like CSS | keep props as the portable form; allowing them in `<style>` on egui is a decision for you (D3) |
| Where the compiler gets user widget/export schemas at build time | exports: from the running binary (exact), build-dep variant for a separate crate; all-rust: `syn` scan in `build.rs` or `links` metadata from a separate crate | the **macro-registered schema is the source of truth**. Build-time checking needs exports in a separate crate (a build-dependency exposing `schema()`); otherwise the dev host writes the schema and checks run there and in `cargo test` |
| Widget authoring form | exports: `#[svelters::widget]` struct + `Widget` trait; all-rust: `#[derive(Props)]` + `app.widget("tag", fn)` | one macro family (`#[svelters::widget]`) with both struct and function forms; one schema format shared with `#[svelters::export]` |
| egui metadata source | ecosystem: hand-written `metadata/egui/*.yaml`; all-rust: generated from the backend's Rust kind table | **Rust kind table as the single source**, dumped to `metadata/semantics/egui.json` and checked; rustdoc-JSON diff of egui catches drift; every fact still gets a kittest ledger test |

## 7. Phasing (proposal)

Lanes can run in parallel where marked. Sizes are planning estimates from the notes.

| Phase | Content | Needs | Acceptance |
|---|---|---|---|
| **G0** (now, any backend) | export registry core (E0: `object`, sync `export`, `Record`/`Enum`/`Error`, schema dump, `svelters:app/*` modules); host non-blocking start + "started" hook (shared with iOS/Android and egui embedding) | — | `examples/exports` calls a Rust object from Svelte **on GTK**; identity/dispose/GC tests; svelte-check passes |
| **G1** backend skeleton | `svelters-egui` crate: arena, eframe runner + embed runner + kittest runner, taffy `box`, `window`, `label`, `button`, `checkbox`, `text-edit`, `slider`; events after the pass, sync before; observe `exact`; Rust kind table → `egui.json`; kittest harness | G0 start-order | counter fixture passes under kittest in `cargo test`; P1 op logs replay; idle CPU 0 |
| **G2** Tier 0 + styling | rest of Tier 0, X4 menus, `virtuallist`, X7 toplevels (viewports, `Modal`), backend cascade + tokens, `#[svelters::widget]` + `wrap_widget!` (E4), `<egui:custom>` E-W0 + X8 `Canvas2D` port | G1 | gallery pages render on egui with kittest snapshots; ledger facts per kind |
| **G3** offload + tooling | E1 workers/async/cancel/progress, E2 `Value::Buffer`, E3 stores/services/persist; E-W1 slots + regions + rich primitives; `svelters-build` diagnostics; F12 overlay v0; `inspection` + egui_mcp | G2, Q1 loader for bytecode | a 200 ms worker job keeps 60 fps; 64 MiB round trip without a copy; state survives hot reload |
| **G4** Tier 1 | table, tab-view, split-view, tree-view, date/colour pickers, rfd dialogs, toast, DnD | G2 | |
| **G5** speed + packs | AOT paint subset (E-W2) with differential tests; Tier 2 packs; compiler `identity` prop; static-template op | G3 | AOT recording ≈ Rust baseline; packs behind features |
| Deferred | subsecond hot-patching, web target, V8 option, wasm widgets, same-frame dispatch | evidence | |

Two cross-backend wins fall out and can be scheduled independently: the svpaint binary op list could replace X8's JSON canvas path on GTK/Qt (~5x cheaper per op, one recorder everywhere), and the static-template op cuts op counts and bytecode on every backend.

## 8. Decisions for you

**Decided 2026-10-21 (user):**
- **D1: yes.** The host supports "embedder drives the frame" alongside `run()`-owns-the-loop, from G1 on (the non-blocking start is in G0).
- **D2: taffy** on our arena.
- **D3: layout properties are allowed in `<style>` wherever the backend supports them.** The backend's style table says which properties a backend accepts; egui (taffy) accepts the layout set; GTK/Qt keep refusing them with the existing diagnostic. This refines X6 decision 7 per backend instead of reversing it.
- **D4: yes, if possible.** The compiler emits a stable per-element `identity` where it can derive one (file + template position + key path + instance path); nodes without one fall back to handle ids.
- **D5 (was D7): yes, MCP.** egui_mcp gives the UI half for free; Svelte-level tools are merged into it. Revisit CLI decision 7 accordingly.
- **Handoff:** the Rust exports work (§3, phases E0–E5 in `docs/notes/egui-rust-exports.md` §10) goes to the svelters dev agent, starting with G0/E0 on GTK.

The list below is the original question list; D6 and D8–D10 are still open.

1. **D1 Host ownership.** Support "embedder drives the frame" (islands, kittest, workbench, bevy, mobile) from G1, alongside `run()`-owns-the-loop? Recommended: yes, since the kittest harness needs it anyway, and it's the same start-order change mobile needs.
2. **D2 Layout engine.** Taffy on our arena (recommended) vs egui `Layout` vs `egui_flex`.
3. **D3 Styling.** Backend cascade with pseudo-classes (recommended). Allow layout properties (`gap`, `flex-direction`, `width`) in `<style>` on egui, or keep X6 decision 7 (props only)?
4. **D4 egui spec source.** Generated from the backend's Rust kind table (recommended) vs hand-written YAML.
5. **D5 Identity prop.** Have the compiler emit a stable per-element `identity` (benefits every backend's devtools and a11y; lets egui keep focus/scroll across hot reload)?
6. **D6 Exports.** Specifier `svelters:app/<module>`? Default executor for `async fn`: tokio (recommended) or the JS thread? Reject immediately on abort (recommended) or wait for the worker? `i64`/`u64` as checked `number` (recommended) or `bigint`?
7. **D7 MCP.** Revisit CLI decision 7 for egui, given `egui_mcp::UiServer` already exists?
8. **D8 Packaging.** Ship the runtime JS inside a crate so cargo-only apps need no `node_modules`; make a `dev` cargo feature (compiler + watcher linked into the app) the egui dev story instead of the `svelters dev` runner?
9. **D9 Scope.** Start on egui 0.37 (recommended; 0.38 is the first upgrade lane). Web: measure only, or a goal? `egui_table` as the default table? `native-menus` on by default on macOS?
10. **D10 Canvas.** Move GTK/Qt `<canvas>` to the svpaint op list too?

## 9. Risks

- **Per-frame cost scales with what's visible**, not what changed: any animation (even the text cursor blink) repaints the whole screen. Mitigations: culling, galley cache per node, cached canvas shapes, animations on the egui side.
- **Id hygiene:** duplicate ids break interaction silently. Debug assertions, egui's id-clash warnings, a test per kind.
- **Re-entrancy:** one sync call placed mid-pass re-enters the borrowed scene. "Pass active" assertion; `SVL_SYNC_BUSY` fallback.
- **Layout fidelity:** taffy placement won't match hand-written egui pixel for pixel; apps wanting the egui-native look use the native container kinds. Text measurement must reproduce widget chrome per kind, or it costs a `request_discard`; generated "measured == rendered" tests per kind and theme.
- **AOT drift** between interpreted and compiled paint code: differential tests mandatory, hash fallback at runtime.
- **egui churn** every 2–3 months, kittest's accessible-name enforcement, experimental `StyleProvider` possibly changing.
- **Cycles across the boundary** (a Rust object holding a JS callback that closes over its own wrapper) leak; scope-bound listeners, explicit callback scopes and a dev-mode leak report keep them rare.
- **Unexplained probe observations:** a 16–28 ms job drain after resolving a completion in the exports probe (likely QuickJS GC, unconfirmed), and pure-Rust egui rows up to 3x faster in the quickjs-jit binary (partly allocator behaviour). Neither changes a conclusion; both are worth a re-run on a quiet machine.
