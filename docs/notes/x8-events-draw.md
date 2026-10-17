# X8 working notes: typed sync results, event families, canvas, ListBox headers, Gdk values

Lane X8, branch reset to master 66785c3, X7 merged at 3345b21. Plan: `plans/milestones/X8-events-draw.md`. ABI text: `plans/architecture/backend-abi.md`, "ABI v2.4 additive (X8)".

## 1. Typed sync results (G5)

- Host: a sync result may be a plain object (`MAP`) or an array (`LIST`) of primitives (`quickjs.rs::sync_result_value`). `write_out` already handled any `Value`; the renderer passes `e.result` through unchanged.
- Signals declare `out_params: [{index, name, type}]` (override schema, `SignalInfo.outs`). `deliver` leaves them out of the JS args and `typed_result` writes the result into the `gpointer` slots: `{ret?, name: v}`, a bare number for a single out, `null` = default. A no-decision result must still return a value for an int signal (`0`): returning `None` aborts inside the glib closure marshaller (found by the generated test).
- Generated tests: `sync_result: {setup, reply, expect}` on a signal makes `gen_backend.py` emit a second test (`syncresult_*`) that sets the harness reply and checks the native effect. `SpinButton::input` (out `newValue`) and `output` (consumed: GTK leaves the text the handler wrote) are annotated.
- The test harness host leaked the raw storage of a non-primitive reply (dangling list pointers): the reply is now kept alive (`testing.rs::h_call_sync`).
- Integration: `crates/svelters-gtk/tests/host_typed_sync.rs` (+ `tests/js/typed_sync.js`): hex display through `output`, parse through `input` (object, bare number, null, `ret: -1`). Use `updatePolicy="if-valid"` so an error keeps the value; with the default `always` GTK turns a failed conversion into 0.
- `GtkScale` has no `format-value` signal in GTK 4 (`gtk_scale_set_format_value_func`): open (needs a text-per-value push, X5-style, not a callback).
- Other non-bool returns (`format-entry-text` string, drag actions flags, `create-window` object): the machinery handles int/bool/string; none annotated, each would need an honest generated test.

## 2. Event families (G5)

Qt (`backend.cpp.in`, "event families"):

- One generic filter (`EventFamilyFilter`, per object, installed on the first listener) and an application-level `EventCaptureFilter` for the capture flag. Families: `key-release`, `mouse-press/release/move`, `double-click`, `wheel`, `context-menu` (consumable: `call_sync`, `true` consumes) and `focus-in/out`, `enter`, `leave`, `resize` (posted, never consumed). They are `inherit.signals` of `QWidget.yaml`, so every widget element has them; the generated `synthetic_listen` test runs on the first element only (an inherited fact is verified once).
- Payloads: `Point`/`Size` records via `Conv<QPointF/QPoint/QSize>`; buttons and modifiers as Qt enum masks. `mouse-move` turns on mouse tracking. `after` is refused for all event-filter signals.
- Tests: three runtime tests in `verify.cpp.in` (consume + propagate, payloads, capture), and `examples/counter-qt/tests/host_events.rs` with `hooks::send_mouse`: JS decides press/wheel, a capture handler on an ancestor runs first and consumes, `mouse-move`, posted `resize` with a Size record.

GTK: decision, controllers are declarative child elements (`keycontroller`, `clickgesture`, `motioncontroller`, `scrollcontroller`, `focuscontroller`) rather than `on<event>` sugar on widgets. Reasons: controllers have state (button, flags, propagation phase) that a sugar attribute would hide; the insert path already existed (X2) and `on<signal>capture` already maps to the capture phase (X3). Only the element tags and the annotated signals were missing. Event names stay toolkit-specific (GTK `pressed`, Qt `mouse-press`); a portable layer is possible later on top. `scroll` only fires for the directions in the controller's `flags` (`flags="vertical"`). `gen_kinds.py` skips non-widget kinds.

## 3. Canvas (G8 draw func)

- Design: recording canvas, as X7 suggested. JS records `[name, ...args]` ops (`@svelters/canvas`, `Canvas2D`, write-only) and sends one JSON string as `ops`. JSON (not a LIST value) keeps the property a plain string on both toolkits: GTK uses `serde_json`, Qt `QJsonDocument`. Cost is a stringify/parse per redraw, fine for thousands of ops.
- GTK: `DrawingArea` gets a virtual prop `ops` (`virtual: {op: canvas_ops}`, new generator op) and its existing `resize` signal is annotated. `canvas.rs` installs the draw func at create and replays with cairo (toy text API: no font fallback or shaping). `canvas` is an alias of the `drawingarea` tag (`lookup::kind_by_name`).
- Qt: hand-written kind `canvas` (`canvas.hpp.in`, like `virtuallist`): `SvCanvas : QWidget`, `paintEvent` replays with QPainter. Paths are kept in device space (points go through the current matrix when added, like canvas), clips are device paths, text goes through `addText`. Own members: `ops`, `contentWidth/Height`, `hexpand/vexpand`, plus the 12 event families through `listen_event`. Default size policy is Preferred x Fixed (like GtkDrawingArea, no expansion).
- The size is not a draw-callback argument: `resize` is posted (GTK `(w, h)`, Qt `(Size, oldSize)`) and the `draw()` attachment (`$state` size, `$effect` re-records when state or size changes) redraws. First draw happens at 0x0.
- Tests: GTK `tests/canvas.rs` (replay pixels through `render_image`; a real mapped drawing area snapshotted, ops replaced, invalid list is an error); Qt `runtime/canvas_replays_ops_into_pixels` and `host_events.rs` (pixels through the real host); gallery pages `13-canvas` on both toolkits check the three drawn colours in the screenshots (`target/gallery/{gtk,qt}-13-canvas.png`); `packages/canvas/test/recorder.test.mjs`.
- Not done: `drawImage` (needs the Gdk texture/QImage as an image source; item 5 made textures convertible, the canvas op is the follow-up), gradients, patterns, shadows, `measureText`, hit regions, `getImageData`.

## 4. ListBox/FlowBox (G8)

- Sort and filter are not native: `arrange(items, {filter, compare, group})` (`@svelters/lists/arrange`) returns the rows to render keyed (so GTK sees inserts, removes and moves) and the header of the first row of each group.
- Headers: child attribute `header` on the ListBox `children` slot (`<label parent:header="Fruit">`), through a new GTK attr setter form (`set: {class, expr}`). GtkListBox only places headers from its header func (setting `row.set_header` outside it leaves an unmapped widget: found by `tests/listbox_header.rs`), so the text is kept on the row and one native header func, installed on first use, turns it into a heading label (`util.rs::set_row_header`). FlowBox has no header concept; sorting and filtering are the same JS approach.
- Tests: generated `attr_listbox_children_header`, `tests/listbox_header.rs` (pixels), `packages/lists/test/arrange.test.mjs`, gallery page `14-arrange` (GTK).

## 5. G11 Gdk values

- `GdkCursor` from a cursor name string; `GdkPaintable`/`GdkTexture` from a path or `file:` URI, or from `BYTES` (encoded image). Needed the `v4_6` feature of gtk4-rs (`Texture::from_bytes`). `Widget.cursor` is annotated (round trip in the generated test, names in `tests/gdk_kinds.rs`); `picture`/`image` `paintable` take paths and bytes (`gdk_kinds.rs`).
- Not done: `PangoAttrList` (`Label.attributes`), Gdk objects as handles, Qt equivalents (`QCursor`, `QPixmap`).

## Gotchas

- The agent shell refuses compound commands (heredoc plus `&&`/`;`) in a worktree: scripts go through files.
- `Texture.download` on Xvfb needs the cairo renderer (`GSK_RENDERER=cairo` is set by the test helpers that need it); the `canvas`/`listbox_header` tests use the default renderer under `xvfb-run` and pass.
