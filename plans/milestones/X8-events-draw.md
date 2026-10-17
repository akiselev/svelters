# X8: typed sync results, event families, canvas drawing, ListBox funcs

Lane X8 on master 66785c3. Gaps: G5 (typed results, Qt events), G8 (draw func, ListBox/FlowBox funcs), G11 (Gdk/Pango kinds).
Notes: `docs/notes/x8-events-draw.md`. ABI text first in `plans/architecture/backend-abi.md` ("ABI v2.4 additive (X8)").

## 1. Typed sync results (G5)

- Host: a sync handler result may now be a plain object (`{ret, name: primitive}`, becomes a `MAP`) or an array of primitives
  (`LIST`), besides null/bool/number/string. Still never read from the handler's return value; `e.result = {...}`.
- Signal metadata: `out_params: [{index, name, type}]` (positions among the signal params; the `gpointer` parameter of GTK
  signals such as `GtkSpinButton::input`). The tables carry them; the backend does not pass out-params to JS and writes them from
  the result. The result shape follows S2's out-param object: `{ret?, <name>: v}`; a bare number is shorthand for
  `{ret: 1, <first out>: n}` when the signal has one out param. `null` = toolkit default.
- GTK annotations: `SpinButton::input` (`handler_result: value`, out `newValue: f64`, `ret` 1 ok / -1 error / 0 not handled) and
  `output` (bool_stop; the handler formats through `text`). `GtkScale` format is a function in GTK 4
  (`gtk_scale_set_format_value_func`), not a signal; it is covered by item 4's callback-free approach (JS pushes the text,
  `Scale.formatValue` as a draw-value text list is out of scope; recorded as open).
- Generated test: signal annotation `sync_result: {reply, expect}` makes `gen_backend.py` emit a second test that sets the harness
  reply and checks the native effect.
- Integration: a SpinButton shows hex text from JS (`output` handler) and parses it back (`input` handler).

## 2. Event families (G5)

- Qt: synthetic signals through one event-filter family, declared in the override YAML (`synthetic: <family>`): `key_release`,
  `mouse_press|release|move|double_click`, `wheel`, `focus_in|out`, `enter`, `leave`, `resize`, `context_menu`. Payloads are v3
  values: positions are `Point` records, sizes `Size` records, buttons/modifiers integer masks. Consume/propagate by the
  event-object result (`e.preventDefault()` consumes). `capture` via listen_ex for every family (application filter, ancestors
  outermost first), as `key_press` does today.
- GTK: declarative controller children (element tags `keycontroller`, `clickgesture`, `motioncontroller`, `scrollcontroller`,
  `focuscontroller`). Decision: elements, not `on<event>` sugar, because controllers carry state (button filter, flags) and the
  insert path already exists (X2). Event names stay toolkit-specific; the docs table maps them.

## 3. Canvas (G8 draw func)

- Recording canvas: JS records ops into a display list (flat array: opcode, args...), set as one `ops` value. The native widget
  replays it in the draw func (cairo) / `paintEvent` (QPainter). No JS runs during paint.
- Elements: GTK `drawingarea` (prop `ops`, signal `resize`), Qt generated `canvas` (QWidget subclass). Both post `resize [w, h]`
  after allocation. JS package `packages/canvas`: `Canvas2D` recorder (subset), attachment `draw(fn)`.
- Tests: ops replayed into an image, pixel assertions (both toolkits), gallery screenshots.

## 4. ListBox / FlowBox sort, filter, header (G8)

JS sorts and filters and pushes the result (X5 approach). Headers: `parent:header` attribute.

## 5. G11 Gdk/Pango kinds, if time allows.
