# svelters-egui: custom widgets written in JS, and keeping JS off the frame path

Status: research note, 2026-10-21, one slice of M7 (egui). Nothing here is implemented.

- **Question.** egui ships few widgets, so users will write their own. How do they write *performant* custom widgets in JS/Svelte for an immediate-mode toolkit, and can the JS side be sped up or JIT-compiled?
- **Probe.** `docs/notes/scripts/egui-jsdraw-probe/` is a standalone crate (own empty `[workspace]`). It uses rquickjs 0.14.0 (QuickJS-NG 0.16.2+, the host's engine and features) and egui 0.37.0 headless (`run_ui` + `tessellate`, no GPU). A feature builds the same probe against `quickjs-jit` 0.14.1. §9 has the commands.
- **Builds on.**
  - X8 canvas: `plans/milestones/X8-events-draw.md`, `docs/notes/x8-events-draw.md` §3, `packages/canvas/src/{recorder.js,draw.svelte.js}`. JS records a display list and the toolkit replays it in its draw function, so no JS runs during paint. That is the seed of this design.
  - `docs/notes/quickjs-jit-cranelift.md`: quickjs-jit gave 20x on numeric kernels and about 1.0x on Svelte runtime code. NG removed its inline caches.
  - `plans/research/QUICKJS-RUNTIME.md` §5 (engines), `docs/notes/js-native-serdes.md` §4 (call cost), `plans/archive/phases/SVR14-egui-renderer.md` (the reentrancy rule: never call JS while an `egui::Ui` borrow is active).
  - Sibling M7 notes from the same day: `docs/notes/egui-all-rust.md` (an egui app is one cargo build, and `build.rs` runs `svelters-compiler`), `docs/notes/egui-rust-exports.md` (Rust widgets and functions exposed to Svelte, with their own call-cost numbers), `docs/notes/egui-ecosystem.md`.

## 0. Summary

1. **Don't run JS per frame. Run it per change.** A JS widget is a Svelte effect that *records a paint program*: a compact binary list of draw ops. The backend replays it in Rust on every egui frame, and egui repaints the whole UI on every mouse move, so this matters.
   - Things that change every frame are bound late, in Rust: the widget rect, hover/press/focus and their eased animations, pointer position, time, theme colours, drag deltas.
   - They are slots in a tiny expression tape, evaluated in Rust. So hover effects, spinners, crosshairs and drag previews run **no JS at all**.
   - This is X8's recording canvas plus slots, hit regions and egui `Response` events.
2. **Measured, QuickJS-NG on this box** (heavily loaded WSL2; ratios hold, absolute numbers are pessimistic; §2.1).
   - **Recording one draw op from JS costs about 1.0–1.1 µs** through a native host function per op. About half of that is QuickJS just evaluating the argument expressions (0.46–0.51 µs for the loop and arithmetic alone).
   - **Batching into a `Float32Array` does not help in QuickJS.** Inline typed-array writes cost 1.2–1.4 µs per op. A JS `painter.rect()` that encodes into a buffer costs 2.1–2.3 µs. Today's X8 JSON path costs 4.8–6.0 µs.
   - The same scene built in Rust costs 6 ns per op, about 170x less.
   - A realistic knob (two 48-segment arcs plus a dot) takes 118–128 µs in JS against 2.3–2.5 µs in Rust. A 2000-point sparkline takes 2.45 ms against 14 µs.
   - Replaying a cached display list into egui costs the same as native Rust painting, because egui's own tessellation dominates:
     - 1000 rects: 0.70–0.73 ms cached, 0.62–0.73 ms native Rust, 0.71–0.79 ms with the slot IR evaluated per frame;
     - JS re-recording every frame: 2.8–2.9 ms.
3. **So the retained design costs nothing at idle, and the JS engine's speed matters only for re-recording.** Re-recording happens on prop or state changes, drags and data updates. At roughly 1 µs per op, a 20-op widget re-records in about 20 µs, so dozens of widgets can change per frame comfortably.
4. **Make recording cheap with rich primitives, not a faster engine.**
   - Primitives such as `arc`, `roundedRect`, `series(data)`, text, mesh, plus slot expressions, turn O(points) JS work into O(ops) JS work. Rust then generates the points.
   - Large data goes by reference (a buffer), never as per-point calls.
5. **Acceleration, ranked.**
   - **(a) An AOT "paint subset" compiled by our own compiler.** TS paint functions, using numbers, structs, loops, `Math` and draw calls, compile to Rust in release through the `build.rs` path in `egui-all-rust.md` §2.1. In dev the same source is interpreted by QuickJS, so hot reload is unchanged. Expected recording speedup is 50–170x (the Rust rows above). This is the only option that is both large and cheap to maintain, and we already own the parser, scope analysis and codegen.
   - **(b) `quickjs-jit` 0.14.1: measured, no.** It now targets our exact rquickjs 0.14 / NG 0.16.2+ (released 2026-10-20). It was active (27 functions, 93–95% of entries in tier 2) and ran a bare loop 17–26x faster. Realistic paint code ran 0.82–0.93x (slower), and per-op host calls 0.7–0.9x (§5.1.1).
   - **(c) An engine swap to V8.** About 20x on Svelte-style code, but +48 MB, ~37 ms startup and jitless on iOS. Only worth it if app-level JS, not widgets, turns out to dominate.
   - **(d) Wasm widgets.** Only for third-party binary plugins.
   - **(e) Whole-component Svelte→Rust codegen: no.** The egui graph is already retained in Rust, so JS is not on the frame path anyway.
6. **Phases.**
   - E-W0: `<egui:custom>` with ops, sense and events, the X8 `Canvas2D` port and kittest snapshots.
   - E-W1: slots, the expression tape, regions, `egui::Id` state, two-pass resize.
   - E-W2: data by reference, and the AOT paint subset with differential tests.
   - Deferred: wasm and engine swap. Details in §6.

## 1. The problem

- In egui, widget code runs on every frame, and egui repaints on every pointer move, key, animation tick or `request_repaint`. A screen of 200 custom widgets whose paint runs in JS pays for 200 Rust→JS calls plus 200 JS bodies on each mouse move.
- QuickJS-NG is an interpreter. A paint function's per-point arithmetic runs at hundreds of nanoseconds per expression on this box (§2), against nanoseconds in Rust.
- The SVR14 reentrancy rule forbids calling JS while a `Ui` borrow is active. A naive "call JS from inside the widget function" design breaks that rule anyway.
- The fix is the same one X8 used for GTK and Qt: **JS records, native replays.** egui adds three needs that X8 did not have:
  - per-frame interaction state (hover, press, drag) that should not need JS;
  - hit testing inside the widget;
  - a size that egui only knows during layout.

## 2. Measurements

### 2.1 Setup and caveats

- rquickjs 0.14.0 with `default-features = false, features = ["std"]`, the same as `Cargo.toml:36` of the workspace. An interrupt handler is installed, as the host does. Release profile.
- QuickJS C asserts are on, which is the host default. A `--features ndebug` build (rquickjs `disable-assertions`) measured the same within noise: the empty loop took 118–150 ns against 117–124 ns, and a host call per op took 1.07–1.10 µs against 1.00–1.09 µs.
- Turning the interrupt handler off (`PROBE_NO_INTERRUPT=1`, quick 5-sample runs) changed a host call per op from 0.96–1.18 µs to 0.81–1.10 µs. That is within noise, at most ~15%.
- egui 0.37.0 runs headless: each frame is `Context::run_ui` + `Context::tessellate` at 1920x1080 and 1 point per pixel. No GPU upload.
- **Timing uses thread CPU time** (`CLOCK_THREAD_CPUTIME_ID`), because the machine was shared with other agents' builds: load average 11–33 on 16 hardware threads, an AMD Ryzen 7 5700U under WSL2.
  - Each row is the median of 11 samples. Each sample runs at least 20 ms of CPU.
  - The table shows two full runs, run 2 and run 3, minutes apart. They agree within about 15%.
- **Calibration.** A 2-cycle dependent add+rotate chain ran at 1.80–1.85 ns per iteration, an *effective* clock of about 1.1 GHz. A quiet core of this CPU boosts to about 4.3 GHz.
  - So absolute numbers are probably about 3–4x pessimistic for ALU-bound code (estimate).
  - They are likely more pessimistic for the memory-bound QuickJS interpreter, which suffers most from a busy SMT sibling and shared caches. For example, `node --jitless` ran `s += i` at 100 ns per iteration on this box.
  - **Read the ratios. They held across runs.**
- **The pure-Rust egui rows also depend on the binary.** The same code ran up to 3x faster in the quickjs-jit build, partly because of allocator behaviour (§5.1.1). Compare egui rows only within one table.

### 2.2 Results

ns per op is per draw op for the `rust`/`ir`/`js` rows, per widget for the `call`/`widget` rows, and per rect for the `egui` rows. "Fit in 16 ms" is 16 ms divided by ns/op: how many ops would fit if the whole frame went to this one thing.

| group | case | N | µs / frame (run 2 / run 3) | ns / op | fit in 16 ms |
|---|---|---:|---:|---:|---:|
| rust | push `Cmd` (what an AOT paint fn does) | 1000 | 6.1 / 5.9 | 6 | 2.6M |
| rust | push `egui::Shape` (slot scene, native) | 1000 | 32.1 / 31.0 | 31–32 | 500k |
| ir | evaluate slot IR → `egui::Shape` | 1000 | 72.4 / 68.2 | 68–72 | 220k |
| ir | clone cached `Vec<Shape>` | 1000 | 27.0 / 28.5 | 27–29 | 590k |
| js | empty counted loop (`s += i`) | 1000 | 124 / 117 | 117–124 | 130k |
| js | loop + argument arithmetic, no draw call (floor) | 1000 | 493 / 463 | 463–493 | 33k |
| js | **one native host fn call per op** (`rect(x,y,w,h,rgba)`) | 1000 | 1092 / 1003 | **1000–1092** | **15k** |
| js | one native host *method* per op (`painter.rect`) | 1000 | 1115 / 1127 | 1115–1127 | 14k |
| js | inline `Uint32Array`/`Float32Array` writes, one flush | 1000 | 1354 / 1395 | 1354–1395 | 12k |
| js | JS `painter.rect()` encoding into the buffer, one flush | 1000 | 2157 / 2111 | 2111–2157 | 7k |
| js | plain `Array` of numbers → `Vec<f64>` | 1000 | 1447 / 1285 | 1285–1447 | 11k |
| js | JSON string, X8 today | 1000 | 5274 / 4774 | 4774–5274 | 3k |
| js | JSON string, X8 today | 10000 | 60255 / 56725 | 5673–6025 | 3k |
| call | Rust→JS `noop()` | 1000 | 142.5 / 144.5 | 143–145 | 112k |
| call | Rust→JS `paint(x,y,w,h,t)`, 3 rects via the JS buffer painter | 1000 widgets | 6475 / 6208 | 6.2–6.5 µs | 2.5k widgets |
| call | same, rect passed as an object built in Rust | 1000 widgets | 9571 / 8084 | 8.1–9.6 µs | 1.7k |
| call | Rust equivalent (3 rects) | 1000 widgets | 20.0 / 21.7 | 20–22 | 760k |
| widget | sparkline, 200 points: JS / Rust | 1 | 249 / 266 vs 1.6 / 1.6 | | |
| widget | sparkline, 2000 points: JS / Rust | 1 | 2449 / 2464 vs 14.5 / 13.1 | | |
| widget | knob (2 arcs of 48 segments + dot): JS / Rust | 1 | 128 / 118 vs 2.5 / 2.3 | | |
| widget | 100 knobs: JS / Rust | 100 | 12702 / 12050 vs 262 / 206 | | |
| egui | empty frame | – | 11.5 / 8.8 | | |
| egui | 1000 filled rects, Rust `Painter` | 1000 | 730 / 617 | 617–730 | 22k–26k |
| egui | 1000 rects, slot IR evaluated each frame | 1000 | 793 / 709 | 709–793 | 20k–23k |
| egui | 1000 rects, cached shapes | 1000 | 728 / 699 | 699–728 | 22k–23k |
| egui | 1000 rects, **JS re-records every frame** | 1000 | 2772 / 2870 | 2772–2870 | 5.6k–5.8k |
| egui | 10000 rects: Rust / IR / cached / JS each frame | 10000 | Rust 19.3 / 17.0 ms; IR 18.4 / 15.9 ms; cached 16.0 / 15.9 ms; JS 36.3 / 37.4 ms | | |
| egui | 1000 rounded (r=3), stroked rects, Rust | 1000 | 13637 / 14289 | 13.6–14.3 µs | 1.1k–1.2k |

Each scene is checked for equality across paths:
- every JS transport produces the same 1000 commands as the Rust path, last rect included;
- the JS knob's 101 points match the Rust knob's to 1e-3.

### 2.3 What the numbers say

1. **The JS→Rust call is not the bottleneck in QuickJS.** The interpreter evaluating the arguments is.
   - A per-op native call adds about 0.55 µs on top of a 0.46–0.49 µs floor.
   - Every "batching" variant is *slower*, because a typed-array store in QuickJS is itself an interpreted `put_array_el`, and there are six of them per op. A JS→JS method call (`painter.rect()` in JS) costs more than a JS→native call.
   - This agrees with `js-native-serdes.md` §4.3 ("batching does not pay inside QuickJS"). It would be the opposite on V8 or JSC, where typed-array stores are JIT-compiled to single instructions and native calls are not free (§3.7).
2. **JSON (X8 today) costs 5–6x a native call per op**, and it grows superlinearly: at 10k ops, `JSON.stringify` plus parsing cost about 57–60 ms per frame. Fine for X8's redraw-on-change on GTK and Qt, but the wrong encoding for egui.
3. **One Rust→JS call costs 0.14 µs**, so per-widget entry overhead is small. What a JS paint function *does* is what costs. Passing the rect as an object built in Rust adds about 2–3 µs per call, so pass scalars.
4. **Interpreted geometry is the real cost.** The knob is 50x slower in JS, the sparkline 150–170x. Per-point loops belong in Rust: in primitives (`arc`, `series`) or in AOT-compiled paint code.
5. **egui's own tessellation dominates replay.**
   - About 0.6–0.7 µs per filled rect on this box, and 13.6–14.3 µs per rounded, stroked rect: rounded corners tessellate to many more vertices.
   - Evaluating the slot IR per frame adds about 5–10% over native Rust. Cloning a cached list is free in comparison.
   - So a JS widget whose program is cached costs **the same per frame as a hand-written Rust widget**.
6. **Budget for re-recording.** At about 1 µs per op on QuickJS (on this box), one frame's worth of re-recording is about 15k ops if it got the whole frame. Plan for a few thousand ops per frame of change, which is roughly 100–200 typical widgets changing in the same frame. Animating everything through JS every frame is the one thing to avoid, and slots exist for exactly that.

## 3. Design: retained paint programs

### 3.1 Execution model

```text
 JS (QuickJS, at checkpoints only)              Rust egui backend (every frame)
 ─────────────────────────────────              ──────────────────────────────────────────────
 <egui:custom paint={fn}>                       pass start: pump queued JS work if a list is stale
   $effect: re-runs when props/state/            (e.g. size changed) - no Ui borrow is alive here
   size it read change
     p = recorder(widget)                       traverse retained graph; at a custom node:
     fn(p)  ── host fns append ops ──▶ core      (rect, resp) = ui.allocate_exact_size(size, sense)
     p.commit() ── one ABI op (BYTES) ──▶ node   slots = {rect, hover_t = animate_bool(id, hovered),
                                                          press, focus, pointer, t, theme, drag}
 events (queued, after the pass):               eval expression tape(slots) → regs
   ondrag/onclick/onhover/onkey/onlayout ◀──     replay ops into ui.painter() (translate, clip)
   → handlers update $state → effect             interact() each hit region → Responses
     re-records → new list next frame            push Response-derived events to the queue
                                                 request_repaint_after if the list uses `t`
```

- A custom widget is a graph node like any other, holding `paint: Bytes`, `sense`, `size` and a11y props.
- JS writes it through the existing `setProp` path, as one value per commit.
- The node lives in the backend's retained table, which the sibling scene-graph slice owns. This design needs from it only a node kind with a blob property and an event queue.
- **Staleness**: a program records the size it was recorded for.
  - If egui allocates a different size, and the program is not resolution-independent (§3.3), the backend posts `resize` and calls `ctx.request_discard("custom widget resized")`.
  - Then the host pumps JS at the start of the second pass, before the root `Ui` exists. JS re-records, and the second pass paints the new list, so there is no visible stretch frame.
  - egui allows `max_passes` 2 by default (`egui-0.37.0/src/memory/mod.rs:345`). If the discard is declined, the old list is painted scaled, or clipped, for one frame.
- **Ids**: `egui::Id` = `Id::new(("svelters", handle))`, which is stable across frames. Regions use `id.with(region_key)`. Animation state (`animate_bool`) and egui `Memory` data hang off these ids, so they survive re-recording. Across a hot reload they survive where the sibling slice preserves node identity.

### 3.2 The paint IR ("svpaint" v0)

- **Encoding**: little-endian words. Each op is `u32 header = opcode | (slot_mask << 8)` followed by f32/u32 arguments.
  - Bit *i* of `slot_mask` says argument *i* is a register index into the expression tape rather than a literal.
  - Strings, data buffers, textures and meshes live in side tables referenced by index.
  - The probe's decoder (`main.rs::decode`) is this format minus the mask.
  - One `commit` hands the core `(ops, strings, blobs)`. The core validates once and keeps a decoded `Vec<Op>`.
- **Coordinates**: widget-local points. The origin is the allocated `rect.min`, applied at replay as a translation, so a widget that moves does not re-record.
- **Ops v0**:
  - state: `save`/`restore`, `translate`, `rotate`, `scale`, `clip_rect`, `opacity`;
  - shapes: `rect`, `rounded_rect` (per-corner radius), `circle`, `ellipse`, `line_segment`, `polyline`, `path` (move/line/quad/cubic/close, fill and/or stroke), `arc`, `dashed_line`;
  - text: `text(str, pos, font, colour, anchor)`, laid out by egui's galley cache at replay;
  - images: `image(tex, rect, uv, tint)`;
  - meshes: `mesh(blob)` for gradients and arbitrary geometry (egui has no gradient primitive; a gradient is a `Mesh` with per-vertex colours);
  - data: `series(blob, rect, min, max, stroke, fill)`.
  - All of them map one-to-one onto `epaint::Shape` or `Painter` calls. The X8 `Canvas2D` ops (`moveTo`, `arc`, `fillRect`, `setTransform`…) lower onto them.
- **Late-bound expression tape**: a few nodes evaluated once per frame into registers before replay: `const`, `slot`, `add`, `sub`, `mul`, `mul_add`, `div`, `min`, `max`, `clamp`, `lerp`, `step`, `select`, `sin`, `cos`, `fract`, `mix_color`, `alpha`.
  - The probe's `eval_program` is the minimal version: four node kinds and colour lerp. It costs about 2x a hand-written Rust loop but only 5–10% of a whole frame (§2.2).
- **Groups**: `when(reg) … end` skips ops while a register is ≤ 0, for hover-only decorations and focus rings. `for_each_region`-style repetition is not needed: JS emits the ops.
- **Validation**: register and table indices are bounds-checked, and op counts and blob sizes are capped. A bad list is an error on `commit`, as X8 does for invalid JSON, and never a panic at paint time.

### 3.3 What is late-bound (no JS when it changes)

| Slot | Source each frame | Typical use |
|---|---|---|
| `x, y` (origin) | `rect.min` | free translation; never re-record on scroll or move |
| `w, h` | `rect.size()` | anchored geometry (`w - 4`, `h * 0.5`): the list survives resizes; otherwise re-record + `request_discard` |
| `hover`, `press`, `focus`, `enabled`, `selected` (0/1) | `Response` / props | instant state styling |
| `hover_t`, `press_t`, `focus_t` | `ctx.animate_bool(id.with(k), state)` | eased hover fades with no JS |
| `px, py` | `response.hover_pos()` − origin | crosshairs, hover dots, wire previews |
| `drag_dx, drag_dy`, `drag_total_*` | `response.drag_delta()` | drag previews before JS confirms |
| `t` | `ctx.input(\|i\| i.time)` − mount time | spinners, marching ants, pulses (backend `request_repaint` while used) |
| theme colours (`fg`, `bg`, `accent`, `weak`, `stroke`, `selection`, …) and `dark` | `ui.visuals()` and the X6 token table | dark/light switch without re-recording |
| `ppp` | `ctx.pixels_per_point()` | hairlines |
| user slots `state(key)` | egui `Memory` data at `id.with(key)` (f64) | values that Rust interaction rules move (phase E-W2) |

### 3.4 Interaction

- **`sense`**: `hover | click | drag | click-and-drag | focusable` maps to `egui::Sense`. A `cursor` prop maps to `CursorIcon`.
- **Hit regions**: a `region(key, shape, sense, cursor)` op registers a sub-area: a rect, a circle, or a rounded rect with a radius test. At replay the backend calls `ui.interact(region_rect, id.with(key), sense)`.
  - Each region gets its own hover/press slots, `hover@key`, so a node-graph port or the handles of a range slider light up without JS.
  - Events carry the `region` key.
- **Events**: one coalesced event per kind per frame, queued after the pass and dispatched at the checkpoint, like every other event. This obeys SVR14's rule.
  - `click`, `dblclick`, `contextmenu`, `dragstart`, `drag` (`dx, dy, x, y` local), `dragend`, `hoverenter`/`hoverleave`, `wheel`, `key` (when focused) and `layout` (rect changed: `{x, y, w, h}` in window coordinates, for wires between widgets).
  - Drag-and-drop between custom widgets uses egui's DnD payload (`Response::dnd_set_drag_payload` / `dnd_release_payload`) with a JS-side payload id.
- **Latency**: a JS reaction to a drag is visible one frame later (JS runs after frame *k*; the list paints in *k+1*). That is fine for most widgets.
  - Where it is not (a slider thumb under the pointer), draw the preview from `drag_dx` slots, so it is in the same frame, and let JS commit the value.
- **Accessibility**: `role`, `label`, `value`, `min`/`max` props become `response.widget_info(...)`. egui then builds the AccessKit node, so custom widgets are visible to `egui_kittest`, `egui_inspection` and agents (`egui-all-rust.md` §0).

### 3.5 Layout and measure

- **Default**: size is data. `width`, `height`, `aspect`, `min*`/`max*` and `hexpand`/`vexpand` props are read by the backend's layout without JS.
- **Content-dependent size** (text-wrapping widgets, responsive charts): a `measure` callback is not called mid-pass.
  - Instead the backend posts `available` (the width egui offered) when it changes beyond a threshold.
  - JS sets the desired size and re-records. Then the backend uses `request_discard` as in §3.1.
  - A result cache keyed by width bucket in the node keeps a resize drag from thrashing.
- **Text measurement while recording**: `p.measureText(str, font)` is a synchronous host function backed by `ctx.fonts_mut(|f| f.layout_no_wrap(...))` (`egui-0.37.0/src/context.rs:1379`).
  - Recording runs at a checkpoint, so no `Ui` is borrowed.
  - The galley is cached by egui, so a later `text` op at replay hits the cache.

### 3.6 When JS must run per frame

- **Physics, custom layout or continuous data-driven animation**: the widget uses `requestAnimationFrame`, which already runs through `JsEngine::run_frame` on the toolkit's frame clock (PLAN rule 6). On egui that frame clock fires at the start of each frame, before the pass.
  - Each callback re-records.
  - Budget: about 1 µs per op on QuickJS on this box, so keep it under ~2k ops per frame across all such widgets.
- **Svelte `tweened`/`spring` on props** are the same thing: short-lived per-frame JS for the widgets being animated.
- **Prefer slots**: `t`-driven spinners and `hover_t` fades run in Rust forever at no JS cost.

### 3.7 The recorder is engine-specific glue; the format is not

- On QuickJS the fastest recorder is **a native host function per op** (§2.3). The core owns a `PaintRecorder` (engine-agnostic Rust). `quickjs.rs` binds about 25 `Function::new` closures to it: `rect`, `rrect`, `circle`, `path*`, `text`, `region`, `slot`, `commit`…
- On V8 or JSC the fastest is likely the opposite: a JS encoder writing into a `Float32Array` (JIT-compiled stores) plus one flush. This is not measured here, but follows from §2.3 point 1 and published fast-API constraints.
- So the painter object comes from the engine adapter, not from `@svelters/canvas`. The wire format and the core decoder are shared.
- This keeps `JsEngine` honest: QUICKJS-RUNTIME item 13 forbids `#[rquickjs::class]` for widgets, and a recorder is not a widget. It is a small fixed function table that any engine binds.

## 4. Authoring API

### 4.1 Element and painter

```ts
// @svelters/egui (typings; the element is <egui:custom>, alias <canvas> keeps X8 code working)
interface CustomProps {
  width?: number; height?: number; aspect?: number; hexpand?: boolean; vexpand?: boolean;
  sense?: 'hover' | 'click' | 'drag' | 'click-and-drag' | 'focusable';
  cursor?: CursorIcon;
  role?: A11yRole; label?: string; value?: number; min?: number; max?: number;
  paint: (p: Painter) => void;          // runs in an effect: re-runs when what it reads changes
  onclick?, ondblclick?, ondragstart?, ondrag?, ondragend?, onhoverenter?, onhoverleave?,
  onwheel?, onkey?, onlayout?: (e: CustomEvent) => void;  // e.region set for region hits
}

interface Painter {
  readonly w: number; readonly h: number;          // recorded size (numbers)
  readonly W: Expr; readonly H: Expr;              // late-bound size (anchored geometry)
  readonly hover: Expr; readonly press: Expr; readonly focus: Expr;
  readonly hoverT: Expr; readonly pressT: Expr;    // eased 0..1
  readonly px: Expr; readonly py: Expr; readonly t: Expr; readonly dragDx: Expr; readonly dragDy: Expr;
  readonly theme: { fg: Color; bg: Color; accent: Color; weak: Color; stroke: Color; selection: Color; /* … */ };
  // expression builders (no operator overloading in JS; the compiler can lower `a * b` on Expr, §5.3)
  add(a: Num, b: Num): Expr; mul(a: Num, b: Num): Expr; lerp(a: Num, b: Num, t: Num): Expr;
  mix(a: Color, b: Color, t: Num): Color; alpha(c: Color, a: Num): Color; /* min, max, clamp, sin, cos, fract, step, select */
  // shapes: Num = number | Expr anywhere
  rect(x: Num, y: Num, w: Num, h: Num, style: Style): void;
  rrect(x: Num, y: Num, w: Num, h: Num, r: number | [number, number, number, number], style: Style): void;
  circle(cx: Num, cy: Num, r: Num, style: Style): void;
  arc(cx: Num, cy: Num, r: Num, a0: Num, a1: Num, stroke: Stroke): void;
  line(x0: Num, y0: Num, x1: Num, y1: Num, stroke: Stroke): void;
  polyline(points: Float32Array | number[], stroke: Stroke): void;
  path(): PathBuilder;                              // moveTo/lineTo/quadTo/cubicTo/close → fill/stroke
  series(data: Float32Array | Float64Array, opts?: { min?: number; max?: number; stroke?: Stroke; fill?: Color }): void;
  text(s: string, x: Num, y: Num, opts?: { font?: Font; color?: Color; anchor?: Anchor }): void;
  measureText(s: string, font?: Font): { width: number; height: number };
  image(src: TextureRef, x: Num, y: Num, w: Num, h: Num, opts?: { uv?: Rect; tint?: Color }): void;
  mesh(m: { pos: Float32Array; uv?: Float32Array; color: Uint32Array; index: Uint32Array }): void;
  linearGradient(x: Num, y: Num, w: Num, h: Num, stops: [number, Color][], angle?: number): void; // → mesh
  save(): void; restore(): void; translate(x: Num, y: Num): void; rotate(a: Num, cx?: Num, cy?: Num): void;
  clip(x: Num, y: Num, w: Num, h: Num): void; opacity(a: Num): void;
  when(cond: Expr, draw: () => void): void;         // ops skipped at replay while cond ≤ 0
  region(key: string, shape: RegionShape, opts?: { sense?: Sense; cursor?: CursorIcon }): void;
  state(key: string, init: number): Expr & { get(): number; set(v: number): void }; // egui::Id-scoped
}
```

### 4.2 Example: a knob

```svelte
<script lang="ts">
  let { value = $bindable(0.5), label = 'Gain', size = 48 } = $props();
  const A0 = Math.PI * 0.75, SWEEP = Math.PI * 1.5;
</script>

<egui:custom width={size} height={size} sense="click-and-drag" cursor="resize-vertical"
  role="slider" {label} {value} min={0} max={1}
  ondrag={(e) => (value = Math.min(1, Math.max(0, value - e.dy / 200)))}
  ondblclick={() => (value = 0.5)}
  paint={(p) => {
    const cx = p.w / 2, cy = p.h / 2, r = p.w / 2 - 4;
    p.arc(cx, cy, r, A0, A0 + SWEEP, [3, p.theme.weak]);                         // track
    p.arc(cx, cy, r, A0, A0 + SWEEP * value, [3, p.mix(p.theme.accent, p.theme.fg, p.hoverT)]);
    const a = A0 + SWEEP * value;
    p.circle(cx + Math.cos(a) * r * 0.7, cy + Math.sin(a) * r * 0.7, p.lerp(3, 4, p.pressT), { fill: p.theme.fg });
    p.when(p.focus, () => p.circle(cx, cy, r + 3, { stroke: [1, p.theme.selection] }));
  }} />
```

Execution model:
- **Recorded once, then on `value` changes and resize.** Four ops. Rust tessellates the arcs. The JS work is O(4), not O(97 points) as in the probe's hand-rolled knob, which cost 118–128 µs.
- **Hover fade, press growth and focus ring are slots.** Moving the mouse over 100 knobs runs no JS.
- **A drag runs JS once per frame for the dragged knob only.** That is one event plus a four-op re-record, tens of µs.

### 4.3 Example: a sparkline with a hover crosshair

```svelte
<script lang="ts">
  let { data }: { data: Float64Array } = $props();   // replaced (not mutated) when new samples arrive
  let hoverIndex = $state(-1);
</script>

<egui:custom height={32} hexpand sense="hover"
  onhoverleave={() => (hoverIndex = -1)}
  onhover={(e) => (hoverIndex = Math.round((e.x / e.w) * (data.length - 1)))}
  paint={(p) => {
    p.series(data, { stroke: [1.5, p.theme.accent], fill: p.alpha(p.theme.accent, 0.15) });  // scaled to W×H in Rust
    p.when(p.hover, () => p.line(p.px, 0, p.px, p.H, [1, p.theme.weak]));                    // crosshair: slots only
    if (hoverIndex >= 0) p.text(data[hoverIndex].toFixed(2), 4, 2, { color: p.theme.fg });
  }} />
```

Execution model:
- `series` takes the buffer by reference: one blob per commit, transferred rather than copied. The `Value::Buffer` design in `egui-rust-exports.md` §4 measured 64 MiB at 0.02–0.05 ms.
- Rust does the min/max scan and the scaling, which the probe measured at 14 µs against 2.45 ms in JS for 2000 points.
- The list is resolution-independent (`series` and `H` are late-bound), so a resize does not re-record.
- The crosshair follows the pointer with no JS.
- Only the value label needs JS, once per hovered index change. If even that is too chatty, `onhover` is throttled to index changes on the JS side.
- New data re-records three ops.

### 4.4 Example: a node-graph port and its wires

```svelte
<!-- Port.svelte -->
<script lang="ts">
  import { graph } from './graph.svelte.ts';           // shared $state: ports' window positions, drag state
  let { node, name, kind }: { node: string; name: string; kind: 'in' | 'out' } = $props();
  const key = `${node}.${name}`;
</script>

<egui:custom width={14} height={14} sense="click-and-drag" cursor="crosshair"
  onlayout={(e) => (graph.ports[key] = { x: e.x + 7, y: e.y + 7 })}
  ondragstart={() => (graph.dragging = { from: key })}
  ondragend={() => graph.dropAt(key)}
  paint={(p) => {
    p.circle(7, 7, p.lerp(4, 6, p.hoverT), { fill: graph.colorOf(kind), stroke: [1, p.theme.stroke] });
    p.when(p.press, () => p.circle(7, 7, 7, { stroke: [2, p.theme.selection] }));
  }} />

<!-- in the editor's background canvas -->
<egui:custom hexpand vexpand sense="hover" paint={(p) => {
  for (const w of graph.wires) bezier(p, graph.ports[w.from], graph.ports[w.to]);
  if (graph.dragging) {                                   // live wire to the pointer: slots, no JS per move
    const a = graph.ports[graph.dragging.from];
    p.path().moveTo(a.x, a.y).cubicTo(p.add(a.x, 60), a.y, p.add(p.px, -60), p.py, p.px, p.py).stroke([2, p.theme.accent]);
  }
}} />
```

Execution model:
- Hover and press are slots, so a port records **once** and never re-records on interaction.
- `onlayout` fires when egui places the port somewhere new (panning the graph). JS then updates `graph.ports`, and the background canvas re-records its wires. That is O(wires) ops once per pan frame, not per mouse move.
- During a wire drag the preview curve's end follows `px`/`py` in Rust. JS runs only at `dragstart` and `dragend`.
- egui's DnD payload carries `key` to the drop target, so `dropAt` can use `e.payload` instead of geometry.

## 5. Acceleration options

### 5.1 QuickJS-side

| Option | Evidence | Verdict |
|---|---|---|
| quickjs-jit 0.14.1 (longbridge, Cranelift; released 2026-10-20 on crates.io, rebased onto rquickjs 0.14.0 / NG `0fdea21` = 0.16.2+26, i.e. **exactly our engine**) ([PR #36](https://github.com/longbridge/quickjs-jit/pull/36), [crate](https://crates.io/crates/quickjs-jit)) | **This probe, `--features jit`: see §5.1.1.** Earlier repo measurement: 20.8x on a monomorphic numeric kernel, 0.93–1.06x on the real Svelte 5 runtime (`quickjs-jit-cranelift.md` §5) | **No (measured, §5.1.1).** It is a win only on bare numeric loops. There is also a bus factor of one, the code is agent-written, there is no NG 0.17 and no iOS |
| QuickJS-NG [#1744](https://github.com/quickjs-ng/quickjs/pull/1744) (GCC 14+ JSValue codegen fix) | 1.19–1.20x geomean on GCC 14/15 per the PR. We build with **GCC 16.2.1** here. Not in rquickjs 0.14.0 | Carry it as a patch or take it with the next rquickjs. A free ~1.2x for everything |
| NG 0.17.0 (2026-09-18): a peephole, a bigger atom table ([releases](https://github.com/quickjs-ng/quickjs/releases)) | small | Take it with rquickjs 0.15 (QUICKJS-RUNTIME item 16) |
| C asserts off (`disable-assertions`) | measured: no difference (§2.1) | Don't bother |
| Interpreter inline caches, superinstructions, template interpreter (PrimJS) | NG removed its ICs for mixed results. A second attempt (#1521) closed flat. PrimJS is arm64-only and ES2019 ([repo](https://github.com/lynx-family/primjs)) | No |
| `qjsc`-style bytecode→C partial evaluation ([ivankra/quickjs-aot](https://github.com/ivankra/quickjs-aot)) | 1.36x on v8-v7, proof of concept | No: a fork for a small gain. Our own subset AOT (§5.3) is 50x+ |

#### 5.1.1 quickjs-jit measured on this probe

**Setup.**
- `--no-default-features --features jit` builds the same probe against `quickjs-jit` 0.14.1 + `quickjs-jit-runtime` 0.14.1 (`compiler` feature), with `Jit::attach(&rt, JitConfig::default())`, i.e. automatic tiering.
- The JS section runs twice. The first pass warms up and its rows are dropped. Between the passes the probe calls `Jit::poll()` 100 times, 10 ms apart, so compiled code gets installed. `poll` must run outside `Context::with`: inside it, it panics with "RefCell already borrowed".
- Measured pass: **27 functions installed, 104k–121k native entries (93–95% in tier 2), 875 KB of code, 16 unsupported-opcode rejections**. So the JIT was active.
- Stock and JIT binaries ran interleaved (stock, jit, stock, jit) at load average 21–24. Ratios are stock time over JIT time, one per pair.

| Case | stock µs (2 runs) | quickjs-jit µs (2 runs) | speedup |
|---|---:|---:|---:|
| empty counted loop, n=10000 | 1323 / 1189 | 51 / 68 | **17–26x** |
| loop + argument arithmetic (floor), n=1000 | 549 / 562 | 409 / 357 | 1.3–1.6x |
| one native host fn call per op, n=1000 | 1196 / 1210 | 1336 / 1696 | **0.7–0.9x** |
| inline typed-array writes, n=1000 | 1636 / 1668 | 1324 / 1362 | 1.2x |
| JS `painter.rect()` → buffer, n=1000 | 3825 / 2610 | 2324 / 2385 | 1.1–1.6x |
| JSON (X8), n=1000 | 5602 / 6250 | 8110 / 6482 | 0.7–1.0x |
| Rust→JS `noop()` ×1000 | 142.5 / 142.5 | 188.8 / 193.3 | **0.74x** |
| Rust→JS `paint(x,y,w,h,t)` ×1000 | 7901 / 8950 | 7625 / 6533 | 1.0–1.4x |
| sparkline, 2000 points | 2326 / 2560 | 2742 / 2740 | **0.85–0.93x** |
| knob ×100 | 13174 / 14434 | 16158 / 17468 | **0.82x** |

**Reading.**
- The JIT works on a tight counted loop (17–26x, consistent with its README and `quickjs-jit-cranelift.md`). On paint code it does nothing or slightly hurts:
  - native calls still cross the same C boundary;
  - Rust→JS entry gets 26% dearer;
  - the realistic widgets (method calls on a painter object, `Math.cos`/`sin`, typed-array loads) run 7–18% *slower*.
- The binary grows from 7.6 MB to 14.2 MB (the probe binary with egui, release with `debug = false`, not stripped; it includes Cranelift).
- **Verdict: no**, not even as a desktop flag for widgets. Revisit only if its tier 2 starts winning on call- and method-heavy code. The AOT subset (§5.3) attacks the same code at 50–170x.

**Caveat found while measuring.** Pure-Rust egui rows differ between the two binaries:
- 1000 rounded, stroked rects: 11.7–12.8 ms stock against 3.5–4.2 ms in the JIT binary;
- 10k-rect frames: up to 2x.
`cargo tree -e features` shows no feature difference in egui's dependency graph. Forcing glibc's `mmap_threshold` to 256 MiB brought the stock rounded-rect row to 7.8–10.0 ms, so allocator behaviour explains part of the gap. I did not find the rest. Conclusions:
- comparisons *within* one binary (cached vs native vs IR vs JS) stand;
- absolute egui tessellation costs here are uncertain to about 3x.

### 5.2 Engine swap behind `JsEngine` (for the egui backend)

Sources are as linked. test262 figures come from [test262.fyi](https://data.test262.fyi/index.json) (2026-10-21 run). Sizes and startup marked "measured" were measured by a research sub-agent on this machine: a stripped release hello-world in the session scratchpad, not in the repo.

| Engine | JIT | Binary added | Startup | Rust embedding | Svelte 5 (Proxy, WeakRef, FinalizationRegistry, private fields, ESM) | iOS | Verdict |
|---|---|---|---|---|---|---|---|
| QuickJS-NG (today) | no | 1.5 MB | 6 ms (`apple-gui-backend.md` §8) | rquickjs 0.14.0, active | yes; 98.1% test262 excluding Temporal/Intl | yes | keep |
| V8 ([`v8` 152.2.0](https://github.com/denoland/rusty_v8/releases/tag/v152.2.0), [deno_core 0.412](https://crates.io/crates/deno_core)) | yes ([`--jitless`](https://v8.dev/blog/jitless) available) | **48.1 MB** (measured) | **~37 ms** (measured, no snapshot) | very active; a 39.8 MB prebuilt `.a` download, or a 30+ min source build | yes, 97.7% | prebuilt for `aarch64-apple-ios`, jitless on device | the only big general speedup (about 20x on the Svelte signals graph, `quickjs-jit-cranelift.md` §5). Optional engine later, if app JS dominates |
| JavaScriptCore | yes, except on iOS | 0 on Apple; ~37 MB `.so` on Linux | 27–32 ms | `objc2-javascript-core` 0.3.2, `javascriptcore-rs` 2.0.0 (GObject API); [rusty_jsc](https://github.com/wasmerio/rusty_jsc) is stale | yes, 98.8% | LLInt only (1.6–1.8x QuickJS, `apple-gui-backend.md` §8) | Apple-only option, as QUICKJS-RUNTIME §5 says |
| SpiderMonkey ([mozjs](https://github.com/servo/mozjs#readme) 0.26.9 / ESR 153) | yes | **35.1 MB** (measured) | ~33–35 ms (measured) | prebuilt archives are now default (19 MB gz); turning off the JIT or Intl forces a source build | yes, 98.5% | **no iOS archive** | no |
| Boa ([0.22.0](https://github.com/boa-dev/boa/releases/tag/v0.22)) | no | 13.2 MB (measured) | ~3–4 ms (measured) | pure Rust | 95.6% | yes | no: about 5.7x slower than QuickJS on bench-v8 ([Boa bench data](https://github.com/boa-dev/data/blob/main/bench/results/score.json)) |
| Nova ([nova_vm 1.0](https://trynova.dev/)) | no | (unverified) | (unverified) | pure Rust, few users | 77% overall (Proxy 77%, Promise 61%) | yes | no |
| Hermes / Static Hermes ([Features.md](https://github.com/facebook/hermes/blob/static_h/doc/Features.md)) | no (AOT `shermes`) | (unverified) | 18 ms (native `tsc` demo, [deck](https://speakerdeck.com/tmikov2023/compiling-a-real-world-cli-tool-to-native-with-static-hermes)) | no Rust crate; a 0.0.0 placeholder ([rusty_hermes](https://github.com/rust-hermes/rusty_hermes)); C++ JSI / `hermes_abi` | **no FinalizationRegistry** (our handle release uses it), **no ES modules** | designed for it | no. Typed-mode AOT gives ~9.8x on nbody ([deck](https://speakerdeck.com/tmikov2023/static-hermes-react-native-eu-2023-announcement)), but brings a second runtime and is "unstable" ([TypedLanguage.md](https://github.com/facebook/hermes/blob/static_h/doc/TypedLanguage.md)) |
| Porffor ([repo](https://github.com/CanadaHonk/porffor)) | AOT JS→wasm/C | ~34 KB hello world | (unverified) | none; a node CLI, `0.0.0-alpha.19` | 91.2% on test262.fyi; Proxy 79%, partial GC | probably (unverified) | no: alpha, whole-program only |

- **Why not V8 now.** The retained design takes JS off the frame path, so engine speed only shows on change bursts.
- V8 would cost about 32x our engine's binary, about 6x startup, a second build story, and no JIT on iOS.
- Keep `JsEngine` engine-agnostic, as QUICKJS-RUNTIME §5 says. That includes the recorder table of §3.7. Revisit if profiling real egui apps shows app-level JS (data wrangling, big reactive graphs) as the bottleneck.

### 5.3 AOT-compile a typed paint subset with our own compiler (recommended accelerator)

**Why it fits.**
- We own the compiler: the acorn port with its TypeScript plugin (`crates/svelters-compiler/src/acorn/typescript/`), scope analysis and codegen.
- An egui app is one cargo build whose `build.rs` already runs `svelters-compiler` (`egui-all-rust.md` §2.1).
- Paint functions are leaf code: numbers in, draw calls out, no DOM, no Svelte runtime.

**Subset (`paint` functions, and functions they call, in `lang="ts"`).**
- **Values**:
  - `number` (f64, exactly JS semantics) and `boolean`;
  - `Int` via `x | 0` patterns;
  - `readonly number[]`, `Float32Array`/`Float64Array` (→ `&[f32]`/`&[f64]`);
  - tuples, and interfaces/object literals with a fixed shape (→ generated `struct`s);
  - `string` only as an opaque value passed to `text`, plus `toFixed`/template literals of numbers (→ a JS-compatible number formatter, needed anyway for differential tests);
  - `Expr`/`Color` painter values.
- **Statements**: `const`/`let`, `if`/`else`, `?:`, `for` (counted and `for…of` over arrays), `while`, `break`/`continue`, `return`, `switch` on numbers or strings, and local non-escaping arrow functions, which are inlined.
- **Calls**: `Math.*` (`sin`, `cos`, `atan2`, `sqrt`, `min`, `max`, `abs`, `floor`, `ceil`, `round` with JS's round-half-up, `hypot`, `PI`), painter methods, other subset functions, and `theme` lookups.
- **Inputs**: what the function reads from props/`$state`/`$derived`. Scope analysis already knows this.
  - The compiler generates an input struct.
  - The JS stub marshals a snapshot (scalars, plus buffers by reference) through one host call when the effect re-runs.
  - Recording is then native.
- **Rejected with a diagnostic, and that function stays in JS**: closures that escape, dynamic property access, `any`, prototypes and classes, exceptions, `async`, `$state` proxies inside the body (only the snapshot is visible), and calls to non-subset functions.
  - Rejection is per function and never fatal: the function just runs interpreted.
  - `--strict-native` (the R2 flag) could turn it into an error for teams that want a guarantee.

**Dev and release.**
- **Dev (no cargo, PLAN rule 5)**: the same source runs in QuickJS through the recorder. Hot reload is unchanged.
- **Release**: `build.rs` emits `OUT_DIR/paint_fns.rs`: one `fn paint_<hash>(inputs: &InputsN, p: &mut PaintRecorder)` per accepted function, registered by content hash. The JS stub checks the registry first and calls native if the hash matches. A stale or mismatched hash falls back to JS, so it is never wrong, only slower.
- **Equivalence**: the IR is the oracle. A generated test runs each accepted function in QuickJS and natively over sampled inputs, and compares the command buffers: bit-exact for integers and colours, ≤1 ulp for f32 coordinates.
  - Edge cases (NaN, −0, `Math.round(-0.5)`, `% ` on negatives, ToInt32) get dedicated cases.
  - JS `number` is f64. Generated Rust uses f64 throughout and converts to f32 only when writing an op, as JS does through `Float32Array`.
- **Optional dev accelerator (defer)**: a debug-build-only Cranelift backend or wasm emission from the same typed IR, for apps whose *dev* loop is too slow interpreted.
  - wasmtime is an option here: Winch baseline, Pulley on iOS, a ~35 ns host↔wasm call ([PR 14189](https://github.com/bytecodealliance/wasmtime/pull/14189)), and 1.2–2.1 MB for a minimal build ([docs](https://docs.wasmtime.dev/examples-minimal.html)). So is wasmi 2.0, a pure-Rust interpreter ([blog](https://wasmi-labs.github.io/blog/posts/wasmi-v2.0/)).
  - Not needed unless dev builds hurt: they already run the same code as GTK and Qt.
- **Operator lowering for `Expr`**: inside a `paint` function, local dataflow can identify values that come from slot reads, such as `p.hoverT` or `p.px`, or from combinations of them. The compiler can then rewrite `a * 4 + b` on them into `p.add(p.mul(a, 4), b)`. Then users write ordinary arithmetic on late-bound values in both dev JS and release Rust.
  - This needs no type checker: only intra-function provenance, which `scope.rs` can carry.
  - Phase E-W2 or later. The builder API works without it.

**Expected gain**: the Rust rows of §2.2. 6 ns against about 1 µs per op (~170x), knob 50x, sparkline 150–170x.

**Effort (planning numbers)**:
- subset checker and Rust emitter for scalars, arrays and painter calls: about 2–3 weeks;
- differential test generator: about 1 week;
- number formatting parity: a few days.
- The risk is semantic drift on edge cases, which the differential tests are there to catch.

Prior art: Static Hermes typed mode, AssemblyScript (build-time only: it needs node and binaryen), Static TypeScript (its nbody ran 20x slower than node *because* `number` was boxed, which is the lesson to use unboxed f64), and [Roto](https://codeberg.org/NLnetLabs/roto/) (a typed Rust-hosted scripting language with a Cranelift JIT, the closest Rust precedent). No existing compiler drops in.

### 5.4 WASM as the custom-widget format

- **What it buys.** Widgets in Rust, AssemblyScript or Zig, compiled to wasm, hot-reloaded and sandboxed. [egui_wings](https://github.com/DouglasDwyer/egui_wings) does this for egui (pinned to egui 0.33).
- **Costs.**
  - wasmtime adds 1.2–19 MB depending on features; Pulley is the only mode on iOS, at ~10x slower than native ([#12251](https://github.com/bytecodealliance/wasmtime/issues/12251)).
  - A second toolchain for widget authors.
  - An ABI for the painter, events and inputs: the same IR, so not much extra.
- **Verdict: defer.** For first-party app code, §5.3 gives native speed with Svelte authoring. Wasm is for a third-party plugin ecosystem, which is not on the plan. If it comes, the svpaint IR *is* the plugin ABI: a wasm widget writes the same command buffer.
- For Rust-authored widgets in dev, Dioxus [subsecond](https://docs.rs/subsecond) hot-patching is the lighter alternative. It works in debug builds only, needs the `dx` CLI, and has no egui adapter yet.

### 5.5 Whole-component Svelte→Rust codegen

- **The idea.** The compiler emits Rust egui code for components that are pure or static, bypassing JS.
- **Assessment: no.**
  - In svelters-egui the per-frame traversal is already Rust: the retained graph, which the sibling slice owns. JS runs only when state changes. Codegen would remove JS from *changes*, not from frames.
  - It would fork the execution model (two semantics for one component).
  - It would break hot reload (Rule 5) unless dev kept JS.
  - Fully static components are rare: anything with `$state`, an `{#each}` or a handler needs the runtime.
- **The cheap part worth taking.** Static *subtrees* compiled to a data blob (a template the backend instantiates in one op, like Svelte's `$.template` cloning). That is a data format, engine-agnostic, and helps every backend's startup. It is a compiler note, not an egui one.

## 6. Phased plan

| Phase | Content | Depends on | Size | Perf envelope after it |
|---|---|---|---|---|
| **E-W0** | `<egui:custom>` node kind with `paint` (svpaint v0 ops, no slots), `sense`, `cursor`, a11y props, events (click/drag/hover/wheel/key/layout/resize), the native recorder table in `quickjs.rs` + `PaintRecorder` in core, `Canvas2D` (X8) lowered onto it so `@svelters/canvas` code runs on egui, size props, re-record on `resize`. Tests: `egui_kittest` pixel/AccessKit snapshots, recorder unit tests, invalid-list errors | M7 base (retained graph, event queue) | M (1.5–2 wk) | idle: native cost; change: ~1 µs/op on QuickJS (this box) |
| **E-W1** | Expression tape and slots (§3.3), `when` groups, theme colour refs, `animate_bool` slots, `t` + repaint scheduling, hit regions, `egui::Id` state, `request_discard` two-pass resize, `measureText`, rich primitives (`arc`, `rrect`, `series`, `linearGradient` → mesh, dashed lines), images/textures | E-W0 | M (2 wk) | hover/animation/pointer-follow: 0 JS; replay +5–10% over native |
| **E-W2** | Data by reference (`Value::Buffer` from `egui-rust-exports.md`), paint-subset AOT → Rust in `build.rs` + differential tests, `Expr` operator lowering, NG #1744 patch | E-W1, `svelters-build` (egui-all-rust §2.1) | L (3–4 wk) | AOT'd recording ~6 ns/op (≈170x) |
| Deferred | interaction rules in Rust (drag → state slot, same-frame), wasm widget plugins, V8 engine option, Cranelift/wasm dev accelerator | evidence from real apps | — | — |

## 7. Perf envelope

Numbers are from this box (§2.1). A quiet desktop core should be roughly 3–4x faster on absolute numbers; that is an estimate from the calibration loop.

- **Idle and interaction-only frames** (hover, focus, pointer-follow, spinners): no JS. The per-frame cost equals native egui painting: 0.62–0.79 ms per 1000 filled rects here, dominated by egui tessellation.
- **Re-record on change**: about 1.0–1.1 µs per op (QuickJS, native recorder). That makes a 4-op knob about 5 µs and a 20-op widget about 20 µs. 100 widgets changing in one frame take about 2 ms.
- **Per-frame JS** (rAF physics, tweens): keep it under about 2k ops per frame in total, which is about 2 ms here.
- **Data**: `series` and other blob ops move per-point work to Rust. A 2000-point series costs about 14 µs to prepare natively, against 2.45 ms in JS.
- **With AOT paint (E-W2)**: recording costs about 6 ns per op. JS is then only the input snapshot.

## 8. Risks and open questions

- **One-frame latency for JS-driven feedback.** It is visible on drags of large widgets. Mitigations: slot previews (`drag_dx`), and, later, Rust interaction rules. Measure it with real widgets in E-W0.
- **`request_discard` budget.** egui allows two passes. Several widgets resizing in the same frame share one extra pass. More than one discard per frame is declined, so we fall back to one stale frame. Also check that a second pass does not double-fire events: queue them only from the pass that is not discarded (`ctx.will_discard()`).
- **Slot expressions are a mini-language.** Keep the node set closed and small, version the IR, and fuzz the decoder (blob bounds, register indices).
- **Recorder per engine.** §3.7 makes the painter engine-specific glue, about 25 functions. That is a cost each new `JsEngine` pays, so keep the table small and generated from one Rust list.
- **AOT drift.** Silent semantic differences between interpreted and compiled paint. The differential tests are mandatory, as is a runtime fallback on hash mismatch.
- **Text.** egui text shaping and fallback differ from Pango/Qt. `measureText` results are egui-specific, so widgets that hard-code text widths will not port. Document `measureText` as the only portable way.
- **Benchmark quality.** All numbers come from a heavily loaded shared machine. Before treating absolute budgets as commitments, rerun §9 on a quiet machine.
- **Open.**
  - Should `<canvas>` on GTK and Qt adopt svpaint too, replacing X8 JSON with BYTES? It would be 5x cheaper per op and give one recorder for all backends. This is likely worth doing, and cairo and QPainter can replay the same ops.
  - Region shapes beyond rect, circle and rounded rect.
  - Pointer capture semantics across regions.

## 9. Reproduce

```sh
# from the repo root; any target dir works
export CARGO_TARGET_DIR=/tmp/egui-target
cargo run --release --manifest-path docs/notes/scripts/egui-jsdraw-probe/Cargo.toml                 # stock rquickjs 0.14.0
cargo run --release --manifest-path docs/notes/scripts/egui-jsdraw-probe/Cargo.toml --features ndebug   # C asserts off
cargo run --release --manifest-path docs/notes/scripts/egui-jsdraw-probe/Cargo.toml \
  --no-default-features --features jit                                                              # quickjs-jit 0.14.1
# options: `-- quick` (5 samples), `-- only=<substring>` (filter rows), env PROBE_NO_INTERRUPT=1
```

- Output: a markdown table on stdout, with progress on stderr. The first build takes 7–9 minutes on a loaded machine. The JIT build adds Cranelift.
- Files:
  - `src/main.rs`: the harness, Rust baselines, the slot IR, the decoder and the egui frames;
  - `src/bench.js`: the JS workloads.
