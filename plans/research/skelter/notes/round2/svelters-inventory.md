# svelters-egui porting inventory for tessel (svelters master 1d6656c2)

Read-only investigation, 2026-10-25. Nothing was built or run. `S/` = /home/user/svelters,
`C/` = /home/user/cadatomic. LOC are `wc -l` (comments and tests included). The proposal is
C/docs/research/2026-10-25-own-gui-toolkit.md §4–§12, and the first-round map is
C/docs/research/notes/2026-10-25-own-gui-toolkit/local-pipeline.md.

Legend:
- **Port action**
  - **M**: move. The code moves to a tessel crate or svelters-tessel unchanged, or with only its
    value types swapped (`egui::Rect/Vec2/Pos2/Id/ViewportId/Order/Key/Modifiers` become tessel
    types).
  - **R**: rewrite against tessel. The logic can often be kept and only the egui edge rewritten;
    the notes say which.
  - **D**: drop. These are egui widget wrappers that the OW plan already deletes ("OW-DONE").
  - **F**: frozen. The code stays only in the frozen egui backend, and tessel needs no port.
- **egui coupling**
  - **none**: no egui types.
  - **value**: egui is used only as plain data (Rect, Vec2, Id, Order, Key, Color32).
  - **deep**: the code uses `Ui`, `Response`, `Sense`, `Painter`/epaint shapes, `Context`
    memory or data, galleys, `Area`, viewports, egui's AccessKit builder, or egui widgets.

Target crates follow proposal §12: `tessel-core`, `tessel-text`, `tessel-render`,
`tessel-widgets`, `tessel-platform`, `tessel-test`, and `svelters-tessel` (the ABI backend, which
lives in svelters).

---------------------------------------------------------------------------------------------------
## 0. Totals

crates/svelters-egui (S/crates/svelters-egui):

| Item | Files | LOC |
|---|---|---|
| src `*.rs` | 173 | 65,916 |
| src non-Rust (theme/base.css, theme/data/*.json) | 8 | 598 |
| tests `*.rs` (70 files + common/mod.rs) | 71 | 18,817 |
| Cargo.toml + clippy.toml | 2 | 175 |

LOC by port action, all 181 src files (the per-file assignment is in §1):

| Action | Files | LOC | Share |
|---|---|---|---|
| **M** move (as-is or value-type swap) | 69 | 23,247 | 35% |
| **R** rewrite against tessel | 100 | 40,462 | 61% |
| **D** drop (egui wrappers) | 5 | 1,088 | 2% |
| **F** frozen egui backend only | 7 | 1,717 | 3% |
| total | 181 | 66,514 | |

Of the R total, about 8.5k is "re-host, logic kept". That covers:
- the interaction units (2,986);
- the provider traits (2,547);
- the M5 and drag-and-drop parts of `virtuallist`, `treeview` and `dnd` (about 1.5k of 2,926);
- the cascade glue in style.rs (about 1.5k of 2,562).

About 32k is genuine redraw or re-substrate work:
- widgets (12.1k), views (4.5k) and paint (1.4k);
- text, fonts, rich text and icons (2.1k);
- focus (1.1k), the frame and walk (0.75k), embed (1.8k), a11y, windows and zoom.

These are estimates; the per-file actions are in §1.

Outside svelters-egui, these crates also need a tessel side:

| Crate / file | LOC | egui coupling | Action |
|---|---|---|---|
| svelters-automation src | 7,095 | `Driver::send(egui::Event)`, `screenshot -> egui::ColorImage`, 91 `egui` mentions in session.rs | R (driver and synthetic input); selector, journal and RPC are M |
| svelters-testkit src | 1,373 | `egui.rs` 152 (TestApp runner), ratchet and sched | R (the egui runner only) |
| svelters-conformance tests/egui_measure.rs, egui_scroll.rs | 880 + 487 | kittest | R (they become tessel_measure and tessel_scroll) |
| svelters-testkit tests/vis1.rs, sched_egui.rs | 731 + 155 | kittest + PaintTrace | R |
| svelters-macros src/lib.rs | 2,681 | generated code names `::svelters_egui::{provider, interact, egui}` (49 refs) | R (crate path parameter) |
| examples/egui-kinds | 2,116 | provider kinds on egui | R or F |
| cadatomic-ui provider kinds (5 Widgets, 4 Behaviors, 2 StateProviders, 1 ScenePass, 1 VirtualChildren) | about 3.5k | `egui::Ui`/`Response`/`Painter` | R (cadatomic lane) |

---------------------------------------------------------------------------------------------------
## 1. Module table: crates/svelters-egui/src

Coupling counts were taken per file with `grep -c egui`; the levels below come from reading the
egui lines.

### 1.1 Top-level files (28 files, 14,904 LOC)

| File | LOC | What it does | egui | Action → target | Notes |
|---|---|---|---|---|---|
| lib.rs | 124 | crate root, re-exports, lint denies | value | R → svelters-tessel lib.rs | |
| backend.rs | 221 | `EguiBackend` state (scene cell, outbox, flags, wake), split by concern | value | R → svelters-tessel | `checkpoint_pending`, `frame_pending` and `shared.wake` carry over |
| ops.rs | 495 | `impl Backend for EguiBackend`: create, insert, set_prop, listen_ex, style, bounds, observe, start, describe, wake, schedule_wake, request_frame | value | M → svelters-tessel | Swap three things: `TextEditState` cleanup (:194), `request_repaint_of`/`_after_for` (:383-416) and `ViewportCommand::Close` (:393) |
| frame.rs | 450 | `before_pass` (zoom, deferred writes, native closes via call_sync, checkpoint gate :210-221, rAF, list binds, state providers, restyle, transitions, remeasure) and `after_pass` (outbox flush → post_event/prop_changed, repaint) | deep | R → svelters-tessel (UI phases 1–5 of proposal §5.2) | The order of steps carries over; it is the heart of the event-time rework |
| walk.rs | 296 | lays out a stale window, paints visible nodes as egui widgets, culls by clip, outbox, pass guard | deep | R → tessel-core record phase | `culled` predicate and pass guard kept |
| layout.rs | 1,407 | taffy 0.15 on the scene's columns (no TaffyTree); leaf measure via `ctx.fonts_mut` (:290-322) | value (+text measure) | M → tessel-core | Replace `Metrics` (egui FontId/Style) with a tessel-text measurer; drop the per-kind egui size copies |
| events.rs | 1,166 | EV1: DOM capture/target/bubble over the scene, `EventFilter`, `Overlay`, `input_hook` on egui RawInput, coalesced pointermove, call_sync dispatch | value (input types), deep (`input_hook` plugin, OverlayCx holds `egui::Ui`) | M (propagation) + R (input source) → tessel-core | `egui::{Event, RawInput, Pos2, Modifiers, PointerButton}` become tessel events |
| focus.rs | 1,073 | EV3: focus scopes, tab chains, autofocus/restore, R11 key claims, TextCommand, default/cancel buttons, IME composing detection (:442-443) | deep (egui `Memory` focus: `m.focused()`, `request_focus`, `move_focus`, temp data) | R → tessel-core focus | Own focus model needed (proposal §7) |
| a11y.rs | 60 | accessible-name rules | deep (`accesskit_node_builder`) | R → tessel-core a11y projection | Small file, but egui's AccessKit integration is spread over 31 files (§6.8) |
| anim.rs | 367 | TR2 native animator: `SVL_STYLE_ANIMATE` keyframes sampled into Visual channels, `animationend` | none | M → tessel-compositor (Web Animations seed) | |
| assets.rs | 474 | AS1: `asset://` and `data:` loaders installed on an egui Context (bytes, SVG via resvg, PNG) | deep (`add_bytes_loader`/`add_image_loader`/`load_texture`) | R → tessel-render resources | The asset registry is svelters-modules (egui-free) |
| dialogs.rs | 259 | WG4b rfd file and message dialogs as `#[svelters::export(module = "egui")]`, `DialogBackend` | none | M → svelters-tessel | The module name `svelters:app/egui` is user-visible and needs an alias |
| fonts.rs | 297 | SVP6: font weights mapped to egui families (egui has no weights), icon fonts | deep (`FontDefinitions`) | R → tessel-text | parley/fontique has real weights, so the weight→family mapping shim goes away |
| icons.rs | 840 | ICR1 icon registry (keys, optical masters, colour slots) + drawing (resvg raster → egui texture; icon-font glyphs) | deep (textures, fonts) | R: registry M, paint R → tessel-widgets/render | |
| ids.rs | 48 | how egui ids are derived for nodes | deep | D | tessel uses NodeId (and the AccessKit id = NodeId) |
| leaf.rs | 503 | the built-in leaf wrappers (egui Label/Button/Checkbox/ComboBox/Separator/Image) **and `text-edit` = `egui::TextEdit` (:232)**, container chrome, `invalid_outline` | deep | D (wrappers); text-edit becomes a NEW own widget on quire-text | The highest-risk gap (§6.6) |
| menus.rs | 624 | WG2a menu payload `MenuW`, props, shortcuts, pick logic (check/radio/activate); egui MenuBar/menu wrappers (:395-569) | deep | R: data model M (about 300), egui drawing D | The own drawing is widgets/menu.rs |
| overlays.rs | 409 | popup/modal/inner-window payloads, out-of-flow dispatch; egui Popup/Modal/Window wrappers (:261-360) | deep | R: dispatch M, wrappers D | |
| overrides.rs | 85 | DZ0 designer prop overrides | none | M | |
| numeric.rs | 49 | IN1 locale number display/parse | none | M | |
| platform.rs | 162 | DX2 SystemClipboard (arboard) + shell | none | M → svelters-tessel / tessel-platform | |
| primary.rs | 166 | W5 X11/Wayland primary selection, hooked on `TextEditOutput` | deep | R → tessel-platform + text input | `PrimarySelection` trait itself is egui-free |
| rich.rs | 503 | RT2 label runs as one egui `LayoutJob`, one galley | deep | R → tessel-text (parley rich layout) | |
| spec.rs | 1,111 | generates `metadata/semantics/egui.json` + `packages/native-runtime/types/egui.d.ts` from the kind registry; style table = taffy props of KINDS + `style::ENGINE_PROPS` | value (paths, provenance strings) | M → svelters-tessel | Paths `SPEC_PATH`/`TYPINGS_PATH` (:30-33) need a tessel decision (§8) |
| style.rs | 2,562 | CS5e cascade on the scene: `NodeRef: svelters_style::Element`, Stylist, restyle, `ENGINE_PROPS`, utilities (U3), transitions, matched rules (DZ0), computed style → `egui::Color32`/`WidgetVisuals` for the stock wrappers (:1112-1264) | deep (wrapper visuals), value (rest) | R: about 1.5k M (Element impl, restyle, utilities, transitions), Visuals mapping D | |
| style/shadow.rs | 163 | `box-shadow` via epaint blurred RectShape | deep | R → tessel Shadow prim (closed-form blur) | |
| use_units.rs | 863 | OW1a4: 16 `use:` actions (`UseUnit<U>: Behavior`) exported by `svelters:behaviors/sv` | deep (via Behavior/Unit) | R → tessel-widgets | Follows Behavior and Unit |
| windows.rs | 114 | `window` kind as egui viewport (`show_viewport_immediate`, `send_viewport_cmd_to`), close-request | deep | R → tessel-platform windows | |
| zoom.rs | 176 | ZM1 Ctrl+=/-/0 via egui `zoom_factor` | deep | R → tessel-core (scale factor) | |

### 1.2 scene/ (19 files, 8,764 LOC): moves to tessel-core almost whole

| File | LOC | What it does | egui | Action | Notes |
|---|---|---|---|---|---|
| scene/mod.rs | 576 | arena, column layout, phases | value | M | |
| scene/node.rs | 548 | `NodeCore` (holds `egui_id: egui::Id`, `rect`/`drawn: egui::Rect` :355-359), `LayoutBox`, `W` payload enum, flags, Dirty | value | M (types swapped) | **Bug to carry over:** `SCROLL_CENTER = 1<<21` (:290) collides with `INK_OVERFLOW = 1<<21` (:296) (local-pipeline §3.1) |
| scene/tree.rs | 753 | the one edit path: create/insert/remove/destroy, owner checks, root table (`TreeRoot::Overlay { layer: egui::Order }` :37, `base: egui::Id`) | value | M | |
| scene/access.rs | 1,034 | SceneHandle/SceneRef/SceneMut/NodeRef/NodeMut, `set_value`; `hook_passes` installs egui pass hooks (:212-233) | value (+one egui hook) | M (hook R) | |
| scene/dirty.rs | 133 | dirty bits, layout boundaries, per-root queues | none | M | |
| scene/ext.rs | 572 | `ExtKey<T>` typed per-node columns | none | M | |
| scene/keys.rs | 232 | `PropKey<T>` statics | none | M | |
| scene/compact.rs | 236 | SG6 interner | none | M | |
| scene/id.rs | 274 | NodeId (gen<<24\|idx), VNodeId, small ids | none | M | |
| scene/journal.rs | 589 | change journal, observers, Damage | value (5 mentions) | M | Feeds the damage computation (proposal §6.4) |
| scene/query.rs | 589 | indexes, cursors, Query | value | M | |
| scene/hit.rs | 485 | per-viewport hit lists, `hit_test`, `PaneHit` | value (Rect/Pos2/Order/ViewportId) | M | Becomes the committed hit list (proposal §7.1) |
| scene/virt.rs | 701 | `RowHeights`, `RowPool`, `RowSource`, `VirtualChildren`, `VScroll` (reads egui ctx for hover/wheel :258-285), `a11y_row` (egui accesskit builder) | value + some deep | M (VScroll input and a11y_row R) | |
| scene/paint_cache.rs | 164 | SG6 opt-in `Arc<[Shape]>` per node | deep (epaint Shape) | R → per-node DisplayList cache | Retained display lists make it native |
| scene/props.rs | 415 | built-in prop get/set over typed fields | none | M | |
| scene/source.rs | 81 | DZ0 source-loc | none | M | |
| scene/dump.rs | 235 | text dump, SceneStats | none | M | |
| scene/tests.rs | 564 | SG1 internal tests | value | M | |
| scene/journal_tests.rs | 583 | SG2 internal tests | value | M | |

### 1.3 embed/ (6 files, 2,570 LOC)

| File | LOC | What it does | egui | Action | Notes |
|---|---|---|---|---|---|
| embed/mod.rs | 1,825 | `Embed` API: EmbedConfig, Budgets (warn 8 ms / interrupt 50 ms / mount 500 ms :78-81), load/mount/unmount/move_root, `logic(ctx)`, `show_slot(ui)`, `create_overlay`/`show_overlay` (egui::Area :1163-1210), `show_windows`, focus_info, shortcut_override, text_command, settle | deep (signatures take `egui::Context`/`Ui`/`ViewportId`/`Order`/`Key`) | R → svelters-tessel | The API shape is renderer-agnostic; slots become tessel layers or panes |
| embed/current.rs | 184 | deferred-viewport registry, `SlotPainter` thread-local hack, begin-pass plugin | deep | **F** | Proposal §8.3 removes the need |
| embed/roots.rs | 160 | root table, faults | none | M | |
| embed/slots.rs | 137 | slot table | value | M | |
| embed/settle.rs | 153 | Busy, Settle, BusyProbe | none | M | Add `COMMIT_IN_FLIGHT` (proposal §5.4) |
| embed/panes.rs | 111 | CV1 host-painted panes answering SG3 hit tests | none | M | |

### 1.4 provider/ (5 files, 3,007 LOC)

| File | LOC | What it does | egui | Action |
|---|---|---|---|---|
| provider/mod.rs | 1,494 | `Widget` trait, Caps, KindEntry/PropEntry/EventEntry/PartEntry, WidgetCx/MeasureCx/A11yCx/LayoutCx, ErasedWidget, `with_taken` | deep | R → tessel-core `Kind` (proposal §9.4) |
| provider/behavior.rs | 613 | `Behavior` trait (sense/before_ui/after_ui/paint_over/detached/filter), BehaviorCx | deep | R |
| provider/pass.rs | 440 | `StateProvider`, `ScenePass`, PrepareCx, ProviderCx (has `ctx()`) | deep (ScenePass::overlay(ui), order) | R (StateProvider is nearly M) |
| provider/state.rs | 235 | StateBits, registry, `state_bits!` | none | M |
| provider/methods.rs | 225 | RA3 kind methods, MethodCx | none | M |

### 1.5 kinds/ (19 files, 7,243 LOC): the vocabulary tables

| File | LOC | What it does | egui | Action |
|---|---|---|---|---|
| kinds/mod.rs | 547 | KindDef/PropDef/SignalDef/MethodDef, DOM event signal macro | value | M |
| kinds/builtin.rs | 408 | `KINDS` (order = kind ids), Widget/Window/Box/Label/Button/Checkbox/TextEdit/Slider rows | none (`native:` strings name egui types) | M (update `native:` strings) |
| kinds/registry.rs | 579 | KindRegistry: built-ins + `#[svelters::widget]` providers, `replaces` routing, id scheme | none | M |
| kinds/docs.rs | 798 | per-kind reference semantics for docs/spec | none | M |
| kinds/containers.rs | 53 | window, box tables | none | M |
| kinds/controls.rs | 63 | toggle-button, combo-box | none | M |
| kinds/decor.rs | 74 | separator, icon | none | M |
| kinds/inputs.rs | 172 | hyperlink, radio-button, switch, range-slider, drag-value | none | M |
| kinds/layouts.rs | 89 | grid, collapsing-header, scroll-area | none | M |
| kinds/media.rs | 119 | image, progress-bar, spinner, canvas | none | M |
| kinds/menus.rs | 275 | menu family tables | none | M |
| kinds/panes.rs | 130 | panel, sides, frame, stack | none | M |
| kinds/pickers.rs | 201 | date-picker, color-picker, calendar, color-editor | value | M |
| kinds/popups.rs | 215 | popup, modal, inner-window | value (`egui::Order`) | M |
| kinds/rich.rs | 85 | span, b, i, code, a | none | M |
| kinds/views.rs | 509 | scrollbar, table, table-column, tab-view, tab, split-view, toast, dialog-buttons, status-bar, toast-host | value | M |
| kinds/virtuallist.rs | 1,749 | M5 model-view protocol (`splice`, `setRow`, setup/bind/unbind/teardown via call_sync), recycled slots, RowSource rows, drawing and scroll | deep | R (M5 and RowPool logic about 1k M; draw and scroll R) |
| kinds/treeview.rs | 667 | tree-view = VirtualList + Tree | deep | R |
| kinds/dnd.rs | 510 | WG4b drag/drop props on any node; payload in **`egui::DragAndDrop`**, OS file drops (`hovered_files`/`dropped_files`), cursor | deep | R → tessel-platform DnD (§6.4) |

### 1.6 interact/ (18 files, 3,701 LOC): behaviour units

| File | LOC | Unit | egui | Action |
|---|---|---|---|---|
| interact/mod.rs | 375 | `Unit` trait, `InputCx { resp: &egui::Response, input: &egui::InputState, … }`, Intents, PartResponses, FocusCx | deep | R (the InputCx adapter over tessel Event) |
| interact/hover.rs | 39 | Hoverable | none | M |
| interact/press.rs | 179 | Pressable ("follows egui's click semantics (what `egui::Button` did, so every button test holds)" :4-6) | value (Response) | R |
| interact/focus.rs | 94 | Focusable | value | R |
| interact/toggle.rs | 103 | Toggleable | value | R |
| interact/drag.rs | 119 | Draggable | value | R |
| interact/scrub.rs | 243 | DragToValue | value | R |
| interact/step.rs | 345 | Steppable | value (+accesskit) | R |
| interact/scroll.rs | 706 | Scrollable (wheel, momentum, bars, keys, AccessKit) | value (InputState wheel) | R |
| interact/resize.rs | 305 | Resizable | value | R |
| interact/expand.rs | 70 | Expandable | value | R |
| interact/roving.rs | 81 | Roving (focus across siblings) | value (Key) | R |
| interact/roving_list.rs | 173 | RovingList (index inside one widget) | value | M |
| interact/dismiss.rs | 85 | Dismissable | value | R |
| interact/anchor.rs | 230 | Anchored (Floating UI flip and shift) | value | M |
| interact/tooltip.rs | 154 | Tooltipable | none | M |
| interact/typeahead.rs | 119 | Typeahead | none | M |
| interact/dnd.rs | 281 | DragSource, DropTarget | deep | R |

(views/selection.rs `KeySelect` is the `Selectable` unit, 374 LOC, none, M.)

### 1.7 paint/ (8 files, 1,392 LOC): R → tessel-widgets on DisplayList prims

| File | LOC | What it does | egui |
|---|---|---|---|
| paint/mod.rs | 500 | `PaintCx { painter: &egui::Painter, ppp, clip, trace }`, `StyleView`, UA tables, `paint_box/paint_outline/paint_text/paint_icon/paint_image/text_galley/measure_text` | deep |
| paint/boxes.rs | 205 | box primitive: outer shadows, bg, inset, border (per-side, per-corner, gradient), outline | deep (epaint RectShape/PathShape) |
| paint/text.rs | 167 | text primitive: galley from computed style, elision | deep (galley) |
| paint/icon.rs | 172 | icon primitive | deep |
| paint/image.rs | 26 | image primitive, object-fit | deep |
| paint/painters.rs | 129 | `PartPainter` registry (CSS `paint()`) | deep (signature) |
| paint/parts.rs | 85 | P3 paint parts laid out by CSS | value |
| paint/trace.rs | 108 | PaintTrace side table for VIS1 and tests | value |

### 1.8 widgets/ (38 files, 12,493 LOC): R → tessel-widgets (details in §2)

button 283, calendar 725, checkbox 277, collapsing 273, color_editor 1,112, color_picker 262,
date_picker 285, dialog 532, drag_value 245, entry 226 (**D**: the interim one-line entry; it is
replaced by the quire-text TextInput), frame 247, glyph 70, hyperlink 115, image 327, label 341,
list_rows 374, menu 1,108, mod 236, numfmt 155 (**M**, no egui), panel 179, parts 108,
picker_glyphs 45, picker_popup 243, popup 215, progress 270, radio 242, rail 292, range_slider 466,
scroll_area 534, scrollbar 159, select 511, separator 57, slider 487, split_view 461, switch 140,
toggle_button 194, tooltip 189, value_box 508.

`widgets/mod.rs` holds the own-widget switch: `KindSet`, `DEFAULT_OWN` (22 tags) and `OWN_ONLY`
(12 tags), plus `SVELTERS_OWN_WIDGETS`. All of it is dropped: tessel has one implementation per
kind. The Len, invalid and pick helpers move.

### 1.9 views/ (8 files, 4,847 LOC)

| File | LOC | What it does | egui | Action |
|---|---|---|---|---|
| views/table.rs | 2,544 | own `table`/`table-column` (OW5b): virtual rows and columns, frozen header, sticky columns, Resizable columns, KeySelect; **cell editor is `egui::TextEdit::singleline` (:2092-2133)** | deep | R |
| views/toast.rs | 861 | `toast-host`/`toast`, Rust `show`; drawn in a foreground `egui::Area` | deep | R |
| views/status_bar.rs | 295 | OW7c `status-bar`, status-tip | deep | R |
| views/item_keys.rs | 243 | reads navigation keys/typed text from egui input for KeySelect | deep | R (small) |
| views/mod.rs | 239 | Tier 1 views dispatch | value | R |
| views/selection.rs | 374 | `KeySelect` (selection by id) | none | M |
| views/tabs.rs | 149 | `tab-view`/`tab`: **still egui `ScrollArea::horizontal` + `Button::selectable` (:62-112), the own version (OW5c) is not built** | deep | R (new own widget) |
| views/dialog_buttons.rs | 142 | OW7b platform button order | value | R (small) |

### 1.10 elements/ (6 files, 1,631 LOC)

| File | LOC | What it does | egui | Action |
|---|---|---|---|---|
| elements/mod.rs | 313 | WG2b payloads, prop get/set, measure hooks | value | M |
| elements/canvas.rs | 722 | `canvas` X8 display list (`@svelters/canvas` ops JSON) replayed with egui's Painter | deep | R → tessel Path/Quad/Mask prims (a direct mapping) |
| elements/images.rs | 100 | file:// PNG ImageLoader | deep | R → tessel-render image decode |
| elements/panes.rs | 185 | sides/frame/stack taffy style, `stack_layout`, chrome via `egui::Frame::*` (:22-30) | value + deep chrome | R (the style and stack layout are M) |
| elements/inputs.rs | 149 | hyperlink/radio/switch egui wrappers behind the switch | deep | D |
| elements/media.rs | 162 | image/progress/spinner wrappers + canvas measure | deep | D (move canvas measure into canvas.rs) |

### 1.11 native/ (3 files, 1,593 LOC)

| File | LOC | What it does | egui | Action |
|---|---|---|---|---|
| native/mod.rs | 702 | FX1 `Native` + `Policy` platform rows (lines about 1-397: kinetic, scrollbars, ButtonOrder, focus_on_click, tooltip delays, …) + TextEdit word/key glue (`TextEditState`, `CCursorRange`) | deep (glue) | R: Policy M (about 400 LOC; `for_os(egui::os::OperatingSystem)` swapped), TextEdit glue D |
| native/undo.rs | 671 | FX3 GTK/Qt undo grouping models (unused in crate; spec for quire-text) | none | M → quire-text TC0 |
| native/words.rs | 220 | FX1 Pango/Qt word stops | none | M → quire-text TC0 |

### 1.12 theme/ (11 files, 1,840 LOC incl. 598 css/json)

| File | LOC | What it does | egui | Action |
|---|---|---|---|---|
| theme/mod.rs | 1,052 | DS1 token map (ROLES, COMPONENT_*), three outputs (CSS vars, egui Style, Rust tokens) | value (`Color32`, `Shadow`) | M |
| theme/backend.rs | 70 | install Theme on the backend at run time | deep | R (small) |
| theme/egui_style.rs | 120 | `Theme::egui_style` for stock egui widgets and egui-native Rust | deep | **F** (and the EguiIsland needs it: proposal §10) |
| theme/base.css | 126 | UA/base sheet | none | M |
| theme/data/*.json (7) | 472 | affordance, density, shape, 4 VS Code themes | none | M |

### 1.13 imm/ (4 files, 1,289 LOC): OW-RUST immediate-mode own widgets for egui-native Rust (berth `ChromePainter`, shell chrome)

imm/mod.rs 437, imm/widgets.rs 577, imm/chrome.rs 155, imm/el.rs 120 (el.rs has no egui).

Action **F**. cadatomic's crates do not reference `svelters_egui::imm` (grep found 0 hits). Its
consumer is berth's egui `ChromePainter` (BD7). `tessel_berth` will need an equivalent retained
chrome. The virtual element of imm/el.rs (cascade without a node) is reusable.

### 1.14 runner/ (5 files, 832 LOC) and bin/ (2 files, 245 LOC)

| File | LOC | What it does | Action |
|---|---|---|---|
| runner/mod.rs | 49 | runner overview | R |
| runner/eframe.rs | 124 | eframe glow window runner | **F** |
| runner/kittest.rs | 264 | `TestApp`, `EmbedHarness { run, step, run_until_settled }` over `egui_kittest::Harness`; re-exports `kittest::{NodeT, Queryable}` | R → tessel-test |
| runner/viewports.rs | 238 | headless multi-viewport runner (EM2) | R → tessel-test |
| runner/dock.rs | 157 | dock seam test helper | R → tessel-test |
| bin/svelters-egui-run.rs | 135 | eframe window runner binary | R → svelters-tessel-run |
| bin/svelters-bundle.rs | 110 | .svb bundle build/inspect/precompile (backend-agnostic) | M → svelters-cli |

---------------------------------------------------------------------------------------------------
## 2. Element kinds

**The "76 kinds" figure is a grep artefact.** `grep -rn 'svelters::widget' crates` gives 76 hits,
but those include doc comments, macros and tests (local-pipeline §3.1 took the number from there).

The real vocabulary is in `metadata/semantics/egui.json`:
- **57 tags**;
- **60 KindDefs**, which adds the abstract `Widget`, `MenuEntry` and `Run`;
- **26 own-widget structs** (`#[svelters::widget(namespace = "sv", replaces = "<tag>")]` in
  `src/widgets/`).

cadatomic adds 5 element kinds and 4 behaviours, 9 provider kinds in all:
- the element kinds are `cad.empty-state`, `property-grid`, `status-text`, `code-editor` and
  `quantity-field`;
- the behaviours are `op`, `live`, `command` and `presents`.

Tier sources:
- EGUI.md §5 (S/plans/research/EGUI.md:248-250): Tier 0 is egui core plus small composites, and
  Tier 1 adds table, pickers, dialogs, DnD, tab-view, split-view, tree-view and toast;
- ARCH-WIDGETS §6.1 table (S/plans/research/ARCH-WIDGETS.md:770-830) for units and parts.

Kinds added after EGUI.md are marked "T1+":
- scrollbar, calendar and color-editor;
- dialog-buttons, status-bar and toast;
- the RT2 runs.

Columns:
- **Own**:
  - **DO**: own by default (`DEFAULT_OWN`), with the egui wrapper still behind
    `SVELTERS_OWN_WIDGETS=none`;
  - **OO**: own only (`OWN_ONLY`);
  - **eg**: still an egui implementation;
  - **lay**: layout only.
- **Units**: as grepped from each file.
- **Paint**: `box` = paint_box/paint_outline, `text` = paint_text/text_galley/measure_text,
  `icon` = paint_icon, `parts` = paint parts.
- **egui widgets used directly**: today's implementation. Wrapper code is marked "(wrapper)".

| # | Tag | Rust file(s) (LOC) | Tier | Own | Units | Paint | egui widgets used directly |
|---|---|---|---|---|---|---|---|
| 1 | window | windows.rs (114), kinds/containers.rs | T0 | own chrome (OW2) | — | box (WINDOW_UA) | `show_viewport_immediate`, `ViewportBuilder`, `send_viewport_cmd_to` |
| 2 | box | layout.rs, style.rs | T0 | lay | (Hoverable via walk) | box via walk | — |
| 3 | grid | layout.rs, kinds/layouts.rs | T0 | lay | — | — | — |
| 4 | sides | elements/panes.rs (185) | T0 | lay | — | — | (`egui::Frame` chrome) |
| 5 | stack | elements/panes.rs `stack_layout` | T0 | lay | — | — | — |
| 6 | panel | widgets/panel.rs (179) | T0 | OO | Resizable | box | — |
| 7 | frame | widgets/frame.rs (247) | T0 | DO | Hoverable, Focusable, Pressable, Toggleable (checkable) | box, text, parts | wrapper `egui::Frame` (elements/panes.rs) |
| 8 | scroll-area | widgets/scroll_area.rs (534) | T0 | OO | Scrollable | box | — (`egui::ScrollArea` wrapper deleted) |
| 9 | collapsing-header | widgets/collapsing.rs (273) | T0 | DO | Expandable, Focusable, Hoverable, Pressable | box, text, icon, parts | — |
| 10 | label | widgets/label.rs (341) + rich.rs (503) | T0 | DO | Hoverable, Pressable (if `clicked` listened) | text | wrapper `egui::Label` (leaf.rs:202); `LayoutJob` galley (rich.rs) |
| 11 | hyperlink | widgets/hyperlink.rs (115) | T0 | DO | Hoverable, Focusable, Pressable | text, outline | wrapper `egui::Link/Hyperlink` (elements/inputs.rs:44) |
| 12 | button | widgets/button.rs (283) | T0 | DO | Hoverable, Focusable, Pressable | box, text, icon | wrapper `egui::Button` (leaf.rs:210) |
| 13 | toggle-button | widgets/toggle_button.rs (194) | T0 | DO | Hoverable, Focusable, Pressable, Toggleable, Roving | box, text | wrapper `Button::selected` (leaf.rs:250) |
| 14 | checkbox | widgets/checkbox.rs (277) | T0 | DO | Hoverable, Focusable, Pressable, Toggleable | box, text, icon, parts | wrapper `egui::Checkbox` (leaf.rs:218) |
| 15 | radio-button | widgets/radio.rs (242) | T0 | DO | Hoverable, Focusable, Pressable, Toggleable, Roving | parts | wrapper `egui::RadioButton` (elements/inputs.rs:68) |
| 16 | switch | widgets/switch.rs (140) | T0 | DO | Hoverable, Focusable, Pressable, Toggleable | parts | wrapper (elements/inputs.rs) |
| 17 | slider | widgets/slider.rs (487) + rail.rs (292) + value_box.rs (508) + entry.rs (226) + numfmt.rs (155) | T0 | OO | DragToValue, Steppable, Hoverable, Focusable (+value box: Pressable) | text, box (rail) | value_box sets `IMEOutput` (:423); entry.rs is an interim text buffer |
| 18 | range-slider | widgets/range_slider.rs (466) + rail/value_box | T0 | OO | DragToValue, Steppable, Hoverable, Focusable | text | — |
| 19 | drag-value | widgets/drag_value.rs (245) + value_box/entry/numfmt | T0 | OO | (via ValueBox) DragToValue, Steppable, Hoverable, Focusable, Pressable | box, text, icon | IMEOutput (value_box) |
| 20 | **text-edit** | leaf.rs:232 (503 shared) + native/mod.rs (702) + primary.rs (166) + focus.rs | T0 | **eg** | none (egui's) | none (egui's) | **`egui::TextEdit::singleline/multiline`**, `TextEditState`, `TextEditOutput`, `CCursorRange` |
| 21 | image | widgets/image.rs (327 shared) + elements/images.rs (100) + assets.rs + paint/image.rs | T0 | DO | Pressable, Hoverable, Focusable (when clickable) | image, box | wrapper `egui::Image` (leaf.rs:489); egui ImageLoader/textures |
| 22 | icon | widgets/image.rs + icons.rs (840) + paint/icon.rs (172) | T0 | DO | Hoverable, Focusable, Pressable | icon, box | wrapper `Label/Button` (leaf.rs:289-293); textures, icon fonts |
| 23 | separator | widgets/separator.rs (57) | T0 | DO | — | box | wrapper `egui::Separator` (leaf.rs:276) |
| 24 | progress-bar | widgets/progress.rs (270) | T0 | DO | — | box, text, parts | wrapper `egui::ProgressBar` (elements/media.rs:76) |
| 25 | spinner | widgets/progress.rs | T0 | DO | — | path/parts | wrapper `egui::Spinner` (media.rs:87) |
| 26 | combo-box | widgets/select.rs (511) + popup.rs (215) | T0 | DO | Anchored, Focusable, Hoverable, Pressable, RovingList, Typeahead | box, text | `egui::Area` (popup surface); wrapper `egui::ComboBox` (leaf.rs:262) |
| 27 | menu-bar | menus.rs (624) + widgets/menu.rs (1,108) + glyph.rs (70) | T0 | DO | Anchored, Dismissable, RovingList, Typeahead | box, text | `egui::Area`; wrapper `egui::MenuBar` (menus.rs:505) |
| 28 | menu | same | T0 | follows root | same + submenu hover delay | box, text, icon | `egui::Area` |
| 29 | action | same | T0 | follows root | Pressable/Toggleable semantics inside menu | box, text, icon | wrappers `Button/Checkbox/RadioButton` (menus.rs:446-458) |
| 30 | menu-separator | same | T0 | follows root | — | box | — |
| 31 | context-menu | same | T0 | DO | Anchored (at pointer), Dismissable, RovingList, Typeahead | box | `egui::Area` |
| 32 | menu-button | same | T0 | DO | Pressable, Focusable, Dismissable, Anchored | box, text, icon | `egui::Area`; wrapper `containers::menu::MenuButton` |
| 33 | popup | overlays.rs (409) + widgets/popup.rs (215) | T0 | DO | Anchored, Dismissable (+dialog focus scope) | box | `egui::Area` + `AreaState` (popup.rs:69-73); wrapper `egui::Popup` (overlays.rs:307) |
| 34 | modal | widgets/dialog.rs (532) + overlays.rs | T0 | DO | Dismissable, Resizable (+dialog scope) | box, text | `egui::Area` + egui modal layer; wrapper `egui::Modal` (overlays.rs:338) |
| 35 | inner-window | widgets/dialog.rs + overlays.rs | T0 | DO | Draggable/Resizable (edges), Pressable (close, collapse) | box, text | `egui::Area`; wrapper `egui::Window` (overlays.rs:354) |
| 36 | canvas | elements/canvas.rs (722) + elements/media.rs | T0 | own (self-painted) | — (allocate_rect) | X8 display list → `egui::Painter` paths | `egui::Painter` |
| 37 | virtual-list | kinds/virtuallist.rs (1,749) + widgets/list_rows.rs (374) + scene/virt.rs (701) | T0 | own rows (OW5a) | Scrollable, KeySelect (Selectable), Typeahead, DragSource/DropTarget | box, parts | egui ctx for wheel/hover (VScroll), AccessKit rows |
| 38 | tree-view | kinds/treeview.rs (667) + list_rows | T1 | own rows | Expandable, KeySelect, Scrollable | box, parts | as virtual-list |
| 39 | table | views/table.rs (2,544) | T1 | own (OW5b) | KeySelect, Resizable, RovingList, Scrollable | box, text, icon | **`egui::TextEdit::singleline` cell editor (:2133)** |
| 40 | table-column | views/table.rs, kinds/views.rs | T1 | own | — | — | — |
| 41 | tab-view | views/tabs.rs (149) | T1 | **eg** | — | — | **`egui::ScrollArea::horizontal` + `egui::Button::selectable`** (:90-112) |
| 42 | tab | views/tabs.rs | T1 | lay | — | — | — |
| 43 | split-view | widgets/split_view.rs (461) | T1 | OO | Resizable | box | — |
| 44 | date-picker | widgets/date_picker.rs (285) + calendar.rs + picker_popup.rs (243) + picker_glyphs.rs (45) | T1 | OO | Anchored, Dismissable, Focusable, Hoverable, Pressable | box, text | `egui::Area` (popup) |
| 45 | color-picker | widgets/color_picker.rs (262) + color_editor.rs + picker_popup.rs | T1 | OO | Anchored, Dismissable, Focusable, Hoverable, Pressable | box | `egui::Area` |
| 46 | toast-host | views/toast.rs (861) | T1 | OO | (Dismissable-like timeout, Pressable close) | box, text | **`egui::Area` foreground** |
| 47 | toast | views/toast.rs | T1+ (OW7b) | own | — | box, text | (in host's Area) |
| 48 | scrollbar | widgets/scrollbar.rs (159) | T1+ (OW2) | OO | Scrollable | — | — |
| 49 | calendar | widgets/calendar.rs (725) | T1+ (OW6) | OO | Focusable, Hoverable | box, text | — |
| 50 | color-editor | widgets/color_editor.rs (1,112) | T1+ (OW6) | OO | (own drag code; no unit names) | box, text | **`egui::TextEdit::singleline` hex field (:908-911)** |
| 51 | dialog-buttons | views/dialog_buttons.rs (142) | T1+ (OW7b) | own | — | — | — |
| 52 | status-bar | views/status_bar.rs (295) | T1+ (OW7c) | own | — | box | — |
| 53-57 | span, b, i, code, a | kinds/rich.rs (85) + rich.rs (503) | T0+ (RT2) | own | (a: link activation) | text runs | `LayoutJob` |

Abstract and common:
- **`Widget` base props:** tooltip (widgets/tooltip.rs 189, Tooltipable + Anchored, its own
  `egui::Area` at `Order::Tooltip`), DnD props `drag-type`/`drop-types` (kinds/dnd.rs), `status-tip`,
  `invalid` outline, accessibility props.
- **`MenuEntry`** and **`Run`** are abstract bases.
- **Tier 2** packs (EGUI.md §5: plot, tiles, dock, node-graph, markdown, code-editor, icon fonts,
  native-menus, inspector) have no built-in kinds. code-editor exists as cadatomic's
  `cad.code-editor` provider over quire.

**Direct egui widget use still live in the library.** The burn-down count of
`#[allow(clippy::disallowed_types)]` is 14 lines:

| File:line | Wrapper |
|---|---|
| leaf.rs:148, :470 | button, label, checkbox, toggle-button, separator, icon wrappers, plus **text-edit** |
| elements/inputs.rs:33, :62 | hyperlink, switch, radio |
| elements/media.rs:64 | progress, spinner |
| menus.rs:19, 21, 395, 493, 569 | menus |
| overlays.rs:261 | popup, modal, window |
| views/tabs.rs:62 | **tab-view** |
| views/table.rs:2092 | **table cell editor TextEdit** |
| widgets/color_editor.rs:908 | **hex TextEdit** |

Only text-edit, the table cell editor, the colour hex field and tab-view lack an own replacement.
The rest sit behind the own-widget switch and can be dropped.

**`egui::Area` users** (layers): widgets/popup.rs:73, widgets/tooltip.rs:176, widgets/dialog.rs:463,
widgets/menu.rs (via popup), views/toast.rs, embed/mod.rs:1189 (overlay slots),
kinds/views.rs:499 (toast-host), kinds/dnd.rs (ghost `LayerId`).

---------------------------------------------------------------------------------------------------
## 3. Provider and extension traits

All in S/crates/svelters-egui/src unless noted.

### 3.1 `Widget` (provider/mod.rs:330-401)

```rust
pub trait Widget: 'static {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, avail: AvailableSpace) -> egui::Vec2;
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response;
    fn a11y(&self, _cx: &A11yCx<'_>, _node: &mut egui::accesskit::Node) {}
    fn busy(&self) -> bool { false }
    fn hit_test(&self, _cx: &HitCx, _local: egui::Pos2) -> Hit { Hit::Node }
    fn virtual_children(&self) -> Option<&dyn crate::scene::VirtualChildren> { None }
    fn layout(&mut self, cx: &mut LayoutCx<'_>, inputs: crate::taffy::LayoutInput) -> crate::taffy::LayoutOutput { cx.default_layout(inputs) }
    fn taffy_style(&self, _style: &mut crate::taffy::Style) {}
    fn prepare(&mut self, _cx: &mut PrepareCx<'_>) {}
    fn consume_key(&self, _key: egui::Key, _modifiers: egui::Modifiers) -> bool { false }
    fn composing(&self) -> bool { false }
    fn text_input(&self) -> Option<bool> { None }
    fn text_command(&mut self, _cmd: &crate::focus::TextCommand, _cx: &mut WidgetCx<'_>) -> bool { false }
}
```

- **Capabilities.** `Caps` (provider/mod.rs:86-106): FOCUSABLE, EDITABLE, HEIGHT_FOR_WIDTH,
  PAINTS_BOX, CONTAINER, CUSTOM_LAYOUT, PREPARE, VIRTUAL, HIT_TEST.
- **Registration.** `KindEntry` (:206-238) holds namespace, name, id, tag, rust, docs, caps,
  props, events, `new: fn() -> Box<dyn ErasedWidget>`, attaches_to, replaces, style_as and parts.
  It is collected with `inventory::collect!(KindRef)`.
- **Other types:** `KindDecl` (:288-304), `ErasedWidget` (:1066), `Event<T>`, `Part`, `RowTy`,
  `BindSourceDef` (CX3 bind sources).
- **WidgetCx helpers** (all egui-typed):
  - `interact(ui, sense) -> egui::Response`;
  - `run_units(ui, resp, …)`;
  - `sizing_pass(avail, |ui| …)`;
  - `text_size`, `font() -> egui::FontId`;
  - `cached_shapes`, `record_hit`, `focus_stop`.
- **Implementers:**
  - svelters-egui src: 26 own widgets in `src/widgets/`;
  - svelters tests and examples: 16 (examples/egui-kinds plus tests: focus_keys, ow1a3, ow_css,
    own_widgets);
  - cadatomic: 5 (C/crates/cadatomic-ui/src/svelte/empty_state.rs:47, property_grid.rs:543,
    status.rs:69, code_editor/mod.rs:168, quantity_field.rs:145).
- **The proposal's `Kind`** (§9.4) replaces it with `measure/record/event/a11y/native_state`.
  `layout`, `taffy_style`, `prepare`, `hit_test`, `virtual_children`, `consume_key`, `composing`,
  `text_input` and `text_command` are missing from the sketch and must be added.

### 3.2 `Behavior` (provider/behavior.rs:56-94)

```rust
pub trait Behavior: 'static {
    fn sense(&self) -> egui::Sense { egui::Sense::hover() }
    fn after_ui(&mut self, host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>);
    fn before_ui(&mut self, _host: NodeRef<'_>, _cx: &mut BehaviorCx<'_>) {}
    fn paint_over(&mut self, _host: NodeRef<'_>, _painter: &egui::Painter, _cx: &mut BehaviorCx<'_>) {}
    fn detached(&mut self, _host: &mut NodeMut<'_>) {}
    fn filter(&mut self, _host: NodeRef<'_>, _ev: &crate::events::SceneEvent, _cx: &mut BehaviorCx<'_>) -> crate::events::Flow { crate::events::Flow::Pass }
}
```

- `BehaviorCx` holds `id: egui::Id`, `rect: egui::Rect`, state, emits and writes.
- **Implementers:**
  - svelters-egui src: 16 (`UseUnit<U>` instantiations in use_units.rs);
  - tests and examples: 3;
  - cadatomic: 4 (`Presents` svelte/pick.rs:399, `OpMark` op/behavior.rs:57, `LiveRegion`
    op/behavior.rs:82, `Command` commands/behavior.rs:61).

### 3.3 `StateProvider` and `ScenePass` (provider/pass.rs:41-54, :147-159)

```rust
pub trait StateProvider: 'static {
    fn name(&self) -> &'static str;
    fn update(&mut self, scene: &mut SceneMut<'_>, cx: &ProviderCx<'_>);   // ProviderCx::ctx() -> Option<&egui::Context>
    fn wants(&self) -> bool;
    fn busy(&self) -> bool { false }
}
pub trait ScenePass: 'static {
    fn scope(&self) -> PassScope;
    fn order(&self) -> egui::Order { egui::Order::Foreground }
    fn overlay(&mut self, _ui: &mut egui::Ui, _scene: SceneRef<'_>, _cx: &mut PassCx<'_>) {}
    fn after_layout(&mut self, _scene: SceneRef<'_>, _damage: &Damage) {}
}
```

- **StateProvider implementers:** 0 in src, 1 in tests, 2 in cadatomic (`PickProvider`
  svelte/pick.rs:259, `OpProvider` op/mod.rs:112).
- **ScenePass implementers:** 1 in src (`OverlayPass<O>` events.rs:452), 1 in tests, 1 in
  cadatomic (`PickOverlay` svelte/pick.rs:324).
- StateProvider is nearly egui-free; ScenePass paints through `egui::Ui`.

### 3.4 Behaviour units: `Unit` (interact/mod.rs:66-88)

```rust
pub trait Unit: Default + 'static {
    type Params: Default;
    type Intent;
    const OWNS: StateBits;
    fn sense(&self, _p: &Self::Params) -> egui::Sense { egui::Sense::hover() }
    fn input(&mut self, p: &Self::Params, cx: &mut InputCx<'_>, out: &mut Intents<Self::Intent>);
    fn a11y(&self, _p: &Self::Params, _node: &mut accesskit::Node) {}
    fn claims(&self, _p: &Self::Params, _key: egui::Key, _m: egui::Modifiers) -> bool { false }
    fn next_frame(&self) -> Option<std::time::Duration> { None }
}
```

18 implementers, all in svelters-egui src: Hoverable, Pressable, Focusable, Toggleable, Draggable,
DragToValue, Steppable, Scrollable, Resizable, Expandable, Roving, RovingList, Dismissable, Anchored,
Typeahead, Tooltipable, DragSource and DropTarget. There are none in cadatomic.

### 3.5 Virtualisation: `RowSource`, `RowPool`, `VirtualChildren` (scene/virt.rs:323, :456-489, :491-503)

```rust
pub trait RowSource: std::any::Any {
    fn len(&self) -> usize;
    fn key(&self, row: usize) -> u64;
    fn template(&self, _row: usize) -> u32 { 0 }
    fn uniform_height(&self) -> Option<f32> { None }
    fn build(&mut self, s: &mut SceneMut<'_>, template: u32) -> NodeId;
    fn bind(&mut self, s: &mut SceneMut<'_>, node: NodeId, row: usize);
    fn anchor(&self, _key: u64) -> Option<String> { None }
    fn find_anchor(&self, _anchor: &str) -> Option<u64> { None }
    fn version(&self) -> u64 { 0 }
    fn label(&self, _row: usize) -> Option<String> { None }
}
pub trait VirtualChildren {
    fn len(&self) -> usize;
    fn key(&self, row: usize) -> u64;
    fn row_of(&self, key: u64) -> Option<usize>;
    fn rect(&self, row: usize) -> Option<egui::Rect>;
    fn anchor(&self, key: u64) -> Option<Cow<'_, str>>;
    fn find_anchor(&self, anchor: &str) -> Option<u64>;
    fn row_a11y(&self, _key: u64, _node: &mut egui::accesskit::Node) {}
}
```

- `RowPool` is a struct (:323): realised rows recycled by key and template.
- `VirtualList::set_row_source(list, source: impl RowSource, pool: RowPool)` is at
  kinds/virtuallist.rs:977.
- **RowSource implementers:** 2 in tests, 0 in cadatomic.
- **VirtualChildren implementers:** 1 in src, 1 in tests, 1 in cadatomic (`RowsView`
  svelte/property_grid.rs:160).
- These are egui-free except for `Rect` and `accesskit::Node`.

### 3.6 Extension storage, keys and other traits

- `ExtKey<T>` (scene/ext.rs:66-90): `const fn new(name)` (sparse) and `const fn dense(name)`;
  `id()` is assigned on first use. There are 8 `static … ExtKey` declarations in svelters crates
  and 2 in cadatomic. **No egui.**
- `PropKey<T>` / `PropValue` (scene/keys.rs:14). No egui.
- `PaneHit` (scene/hit.rs:64): `fn hit_test(&self, cx: &HitCx, local: egui::Pos2) -> Hit`,
  `part_rect -> Option<egui::Rect>`, `part_anchor`. 2 implementers in tests.
- `EventFilter` (events.rs:278): `fn filter(&mut self, ev: &SceneEvent, scene: SceneRef) -> Flow`
  and `after`. 1 in tests.
- `Overlay` (events.rs:380): `viewport() -> Option<ViewportId>`, `order() -> egui::Order`,
  `paint(&mut self, cx: &mut OverlayCx)` (OverlayCx holds `&mut egui::Ui`). 1 in tests.
- `PartPainter` (paint/painters.rs:67): `fn paint(&self, cx: &mut PaintCx, style: &StyleView,
  rect: egui::Rect, part: &PartInfo, args: &PaintArgs)`. 1 in src (closure blanket impl).
- `DialogBackend` (dialogs.rs:102): `files(mode, options) -> DialogFuture<Option<Vec<PathBuf>>>`,
  `message(options) -> DialogFuture<String>`. 1 in src (rfd), 1 in tests, 2 in cadatomic
  (tour/dialogs.rs:78 `TourDialogs`, tests/files.rs:273). **No egui.**
- `PrimarySelection` (primary.rs:30): `set(&self, text)`, `get(&self) -> Option<String>`. **No egui.**

### 3.7 Kinds registry

- `KindRegistry` (kinds/registry.rs) merges the built-in `KINDS` with the inventory `KindRef`s,
  sorted by spec id (pack-62 ids). Duplicates and tag collisions panic.
- It does the `replacement()` routing for `replaces = "<tag>"` and checks that props and events
  cover the row.
- It feeds `spec.rs`: the `kinds.json` fragment for the compiler (`--spec-fragment`) and the
  `IntrinsicElements` typings.

### 3.8 Macros (S/crates/svelters-macros/src/lib.rs, 2,681 LOC; re-exported from S/crates/svelters/src/lib.rs, 168)

- `#[svelters::widget(namespace, tag?, caps?, replaces?, style_as?)]` on a struct (lib.rs:1665-1697).
  - Field attributes: `#[prop(bind|default=|measure|rename=|enumeration=)]`, `#[event] x: Event<T>`,
    `#[state]`, `#[behavior] u: Unit` (generates `run_units`, `units_sense`, `units_a11y`,
    `units_claims`, `units_next_frame` and a `<Name>Fired` struct), `#[part]` / `#[part(node)]`.
  - On an `impl` block it generates RA3 methods; there is also a fn form.
- `svelters::wrap_widget! { … |p, ui| ui.add(egui::…) }` (:1699-1790) wraps any ecosystem egui
  widget. **F**: egui-only by definition.
- `#[svelters::behavior(namespace, name?)]` (:2116).
- **The generated code hard-codes `::svelters_egui::provider::…`, `::svelters_egui::interact::Unit`,
  `::svelters_egui::egui::{Ui, Response, Sense, Vec2}`** (49 `svelters_egui` refs, e.g. :1729,
  :1771-1785, :2027-2083, :2375-2470).
  - So tessel needs a `crate = "svelters_tessel"` argument, or a facade path. Today's doc comment
    says: "The generated code names `::svelters_egui`, so the crate depends on `svelters-egui`
    too" (:1648).
  - The rest of the macro (prop tables, JsType/FromJs plumbing, inventory) is backend-neutral.

---------------------------------------------------------------------------------------------------
## 4. Backend C ABI

### 4.1 `svl_backend` vtable (S/crates/svelters-abi/src/lib.rs:292-352)

`SVL_ABI_VERSION = 3` (lib.rs:8). Tail members carry v2.1–v2.8 additions as nullable function
pointers. All backends are in-tree and rebuilt together (backend-abi.md:305).

| Member | Signature | Since | Required? | egui implements |
|---|---|---|---|---|
| abi_version, self_ | `u32`, `*mut c_void` | v1 | yes | yes |
| resolve | `(self, what: u32 /*SVL_KIND/PROP/SIGNAL/METHOD/SLOT/ATTR*/, kind: svl_id, name: *const c_char, len) -> svl_id` | v1 | yes | yes |
| create | `(self, kind) -> svl_handle` | v1 | yes | yes |
| insert | `(self, parent, child, before /*0=end*/, slot) -> c_int` | v1 | yes | yes |
| remove | `(self, parent, child) -> c_int` | v1 | yes | yes |
| destroy | `(self, h)` | v1 | yes | yes |
| set_prop / get_prop | `(self, h, prop, *const svl_value)` / `(…, *mut svl_value) -> c_int` | v1 | yes | yes |
| call | `(self, h, method, args, n, out) -> c_int` | v1 | yes | yes (kind methods, M5 calls) |
| listen | `(self, h, signal, on: bool) -> c_int` | v1 | yes | yes |
| handle_info | `(self, h, *mut svl_handle_info) -> c_int` | v2 | yes | yes |
| run | `(self, *const svl_host) -> c_int` (owns the loop) | v1 | yes | yes (eframe runner; an embedder uses `start`) |
| wake | `(self)` thread-safe → schedule a checkpoint | v1 | yes | AtomicBool + `request_repaint_of` (ops.rs:377-388) |
| schedule_wake | `(self, ms: i64)` one-shot, `<0` cancels | v2 | yes | deadline + `request_repaint_after_for` (ops.rs:398-409) |
| quit | `(self)` | v1 | yes | `ViewportCommand::Close` |
| last_error | `(self) -> *const c_char` | v1 | yes | yes |
| caps | `u32` (`SVL_CAP_UTF16_IN/OUT`, `BYTES_RETAIN`) | v3 | yes (0 ok) | |
| describe | `Option<(self, what, kind, member) -> *const svl_member_desc>` | v3 | optional | yes (ops.rs:470) |
| describe_type | `Option<(self, type_id) -> *const svl_type_desc>` | v3 | optional | no |
| cancel | `Option<(self, u64) -> c_int>` | v3 | optional | no |
| request_frame | `(self)` one `host.frame` on the next frame | v2.1 | yes (no-op default in safe trait) | flag + repaint (ops.rs:411-416) |
| set_child_attr / get_child_attr | `Option<(self, parent, child, attr, v) -> c_int>` | v2.2 | optional | no |
| listen_ex | `Option<(self, h, signal, on, flags /*AFTER, CAPTURE*/) -> c_int>` | v2.2 | optional | yes: capture on DOM events (ops.rs:336) |
| style | `Option<(self, op: u32 /*SVL_STYLE_TARGET…ANIMATE*/, h, v) -> c_int>` | v2.3 | optional | yes: `SVL_STYLE_TARGET` answers the engine dialect (4, Rust CSS in process); `SVL_STYLE_ANIMATE` = native animator (ops.rs:418, style.rs:2282) |
| construct / call_static | `Option<…>` | v2.5 | optional | no |
| observe | `Option<(self, h, prop, on) -> c_int>`, `SVL_OBSERVE_UNSUPPORTED` | v2.6 | optional | yes, exact for every kind (ops.rs:447-458) |
| start | `Option<(self, *const svl_host) -> c_int>` non-blocking start, embedder drives the loop | v2.7 | optional | yes (ops.rs:460-468) |
| bounds | `Option<(self, h, *mut f64 /*x,y,w,h*/) -> c_int>` | v2.8 | optional | yes (synchronous layout, ops.rs:423-445) |

Style ops (lib.rs:102-122): TARGET 0, CLASSES 1, NAME 2, INLINE 3, OPACITY 4, SHEET 5, SCHEME 6,
DECLS 7, TRANSFORM 8, CLIP 9, INERT 10, ANIMATE 11.

### 4.2 `svl_host` callbacks (lib.rs:263-280)

```rust
pub struct svl_host {
    pub self_: *mut c_void,
    pub post_event: unsafe extern "C" fn(*mut c_void, svl_handle, svl_id, *const svl_value, usize),
    pub checkpoint: unsafe extern "C" fn(*mut c_void),
    pub call_sync: unsafe extern "C" fn(*mut c_void, svl_handle, svl_id, *const svl_value, usize, *mut svl_value) -> c_int, // 0 ok, -1 failed, -2 busy
    pub on_destroyed: unsafe extern "C" fn(*mut c_void, svl_handle, u32),
    pub invoke_callback: unsafe extern "C" fn(*mut c_void, u64, *const svl_value, usize, *mut svl_value) -> c_int,
    pub release_callback: unsafe extern "C" fn(*mut c_void, u64),
    pub frame: unsafe extern "C" fn(*mut c_void),                        // v2.1
    pub prop_changed: Option<unsafe extern "C" fn(*mut c_void, svl_handle, svl_id)>, // v2.6
}
```

- The safe Rust side is `svelters_abi::safe::Backend` (safe.rs:1360-1480+), with defaults for
  every optional member, and `HostRef` (safe.rs:1240-1305): `post_event`, `checkpoint`,
  `call_sync`, `frame`, `prop_changed`, `as_raw`.
- svelters-abi is 2,845 LOC: lib.rs 364, safe.rs 2,082 and buffer.rs 399, plus a 369-line C
  header.

### 4.3 Semantics a new backend must honour (S/plans/architecture/backend-abi.md, 892 lines)

**Checkpoint** (:84-94):
1. settle offloads;
2. deliver the changed-props batch;
3. deliver events;
4. run timers;
5. drain jobs;
6. apply ops through the vtable;
7. wake if jobs remain.

Ops are applied directly, with no batching.

**Wake delivery** (:96-105):
- it must not be starvable;
- wakes coalesce;
- on egui, a wake is "a flag read at the start of the next pass plus `request_repaint`". tessel's
  event-time model (proposal §5.1) replaces this with an immediate checkpoint after dispatch.

**Frame clock and call_sync** (v2.1, :303-324):
- exactly one `host.frame` per `request_frame`;
- call_sync is only for listened signals, inside the toolkit emission;
- `-2` busy means the event is queued and the deferred default applies (`close-request` vetoes);
- no job drain inside call_sync.

**M5 model-view protocol** (:361-391): the `virtuallist` kind with `splice`, `count`, `scrollTo`,
`setRow` and `rowAt`; sync `setup/bind/unbind/teardown`; async `activate` and `select`.

**Observe** (v2.6, :621-738): announce on native change only, never echo host writes.

**JS-side ops** (:107-131): `create/insert/remove/destroy/setProp/getProp/call/listen` plus
`construct/callStatic/typeChain/observe`; `__svelters_dispatch` and `__svelters_prop_changed`.

**Minimum for `svelters-tessel`:**
- the 15 required members;
- `request_frame`, `listen_ex` (EV1 capture), `style` (engine target and ANIMATE), `observe`,
  `start` (cadatomic embeds), `bounds` and `describe`.

That is the same set as egui. The egui ops.rs (495 LOC) is the template, and most of it moves.

### 4.4 Other backends as size references

Hand-written Rust, excluding `generated/` and WinUI's `bindings.rs`:

| Backend | Hand-written | Generated | Structure |
|---|---|---|---|
| svelters-gtk | 10,766 | 35,699 Rust (+ json) | `backend.rs` 2,110 (vtable over GObject, signals → post_event / call_sync inside emission), `backend/{menus 1,124, vlist 662, canvas 378, observe 360, style 289, wake}` (wake: G_PRIORITY_DEFAULT idle with redraw guard), `gi.rs` 2,046 (introspection), `convert.rs` 976, `closures.rs` 308, `testing.rs` 869 |
| svelters-winui | 6,257 | 85,488 (`bindings.rs` 68,721 + generated) | `backend.rs` 1,095 (DispatcherQueue wake, CompositionTarget::Rendering rAF), `types/*` (ops/rt/model/value/invoke), `parse.rs`, `table.rs`, `testing.rs` 841 |
| svelters-appkit | 4,954 | 2,012 | `backend.rs` 1,569 (CFRunLoopSource wake, CADisplayLink frames), `style.rs` 842, `observe.rs` 465, `testing*.rs` 1,190 |
| svelters-apple | 2,322 | — | shared Apple helpers |
| Qt | C++ generated by tools/metadata/qt (Python 8,361 LOC + templates, e.g. verify.cpp.in 1,332) | — | leaf-crate build via svelters-qt-build (279) |
| svelters-backend-log | 1,099 | — | op-log backend (tests, TestHost parity) |
| **svelters-egui** | **65,916** | — | owns the tree, layout, CSS cascade and every widget |

The native backends are thin adapters, 5–11k LOC each, because the toolkit owns the tree and the
widgets. The egui backend is 6× GTK's hand-written size, because it is the toolkit. svelters-tessel
shrinks to backend glue of about the GTK size (≈ ops.rs + frame.rs + embed + kinds + spec ≈ 10–12k)
only once scene, widgets and paint move into tessel-*.

---------------------------------------------------------------------------------------------------
## 5. svelters-style, svelters-sched, svelters-host as the backend uses them

### 5.1 svelters-style (S/crates/svelters-style, 8,244 src + 1,977 tests; deps cssparser =0.38, selectors =0.41, cssparser-color, smallvec; "It links no toolkit", lib.rs:11)

Files: values.rs 1,546, utility.rs 1,276, cascade.rs 695, utilities.generated.rs 554, media.rs 539,
sheet.rs 493, selector.rs 393, tables.generated.rs 376, view.rs 301, computed.rs 287,
transition.rs 244, invalidation.rs 231, shorthand.rs 220, utility_table.rs 214, table.rs 206,
element.rs 185, state.rs 153, var.rs 132, atom.rs 103, lib.rs 56, diag.rs 40.

Public API (lib.rs:42-56). svelters-egui imports (style.rs:48-51 and 27 other files):

| Concern | API used by svelters-egui |
|---|---|
| Sheets | `Stylesheet`, `ParseOptions`, `DeclBlock`, `Diagnostic`, `Severity`, `SheetId`, `Origin` |
| Selector matching and cascade | `Stylist::new(&svelters_style::tables::EGUI)` (style.rs:378), `MatchCaches`, `Element` (implemented by the scene's `NodeRef` and by imm's virtual element), `PseudoElement`, `StateMask`, `StateNames`, `STATE_PSEUDO_CLASSES` |
| Computed style | `ComputedStyle` (35 uses), `ComputeContext`, `ComputedValue`, `Length`, `Color`, `Shadow`, `Gradient`, `Content` (`Content::Icon`, `Content::None`), `StyleView` + `StyleView::with_overlay`, `Overlay`, `overlay_get` |
| Media | `MediaEnv`, `ColorScheme`, `Contrast`, `Platform::{Linux, Windows, Macos, Other}`, `MEDIA_FEATURES` |
| Change tracking | `Reach`, `for_each_reached` (Invalidation sets) |
| Transitions | `Transitions` (`Transitions::sample` per logic step) |
| Utilities | `UtilityConfig`, `UtilityTokens`, `UtilityIssue`, `UtilityStats` |
| Tables | `tables::EGUI` (generated: the egui table comes from egui.json's `style` section, which spec.rs builds from KINDS' layout props + `style::ENGINE_PROPS`) |

No svelters-style file needs changing for tessel. A `TESSEL` table (or a renamed neutral table)
is generated the same way (§8).

### 5.2 svelters-sched (888 LOC: lib.rs 115, test_scheduler.rs 649, seeds.rs 64, rng.rs 60; zero deps)

- `pub type Job = Box<dyn FnOnce() + Send + 'static>` and `enum JobKind`.
- `pub trait Spawner: Send + Sync { fn spawn(&self, job: Job); fn spawn_with(&self, meta: JobMeta, job: Job) }`.
- `pub trait Clock: Send + Sync { fn now(&self) -> Duration; fn wake_at(&self, key: u64, at: Option<Duration>, waker: &Waker) }`.
- `TestScheduler`, `run_seeds`.
- svelters-egui does not use it directly. The host takes it through `HostConfig.sched`
  (svelters-host core.rs:86-90), and svelters-testkit/tests/sched_egui.rs exercises it with egui.
- tessel's compositor and workers should take a `Spawner`/`Clock`, so deterministic tests keep
  working.

### 5.3 svelters-host (20,425 src; lib.rs 8,317 incl. core) as the egui backend uses it

- **Through the ABI** (`HostRef`):
  - `checkpoint()` (frame.rs:81-85, gated :210-221);
  - `call_sync(h, sig, args)` (frame.rs:374-375 close-request, list binds, table editors, EV1
    `dispatch_input` events.rs:1054-1088);
  - `post_event` (frame.rs:295, :414) and `prop_changed` (frame.rs:123, :418);
  - `frame()` (frame.rs:225).
- **Through the Rust API** (`Host<'static>` owned by `Embed`, embed/mod.rs:446, :500
  `Host::new_owned(backend, cfg)`):
  - `set_turn_budget` (10 uses: the per-entry interrupt budget, armed around each JS entry,
    embed/mod.rs:502, 578, 588);
  - loading: `load_entry`, `load_entry_from`, `load_from`, `load_bundle`;
  - roots: `mount_root`, `unmount_root`, `root_props`, `root_info`, `retarget_root`, `close_root`,
    `take_root_messages`;
  - `take_errors`, `busy`, `ops_in_flight` (settle);
  - also `exports::{CallCx, FromJs, IntoJs, JsType, NativeError, Value, schema, dump}`,
    `platform::{MimeData, type_matches, file_uri}` (clipboard and DnD), `rows`, `dx::FrameMonitor`,
    `bytecode::{precompile, stamp}`, `modules::svb`, `ModuleProvider`, `JsError`, `HostConfig`.
- **Budgets:**
  - `HostConfig`: memory 256 MiB, stack 1 MiB, `turn_budget` 10 s default,
    `max_jobs_per_checkpoint` 10,000 (core.rs:94-112);
  - Embed `Budgets { warn: 8 ms, interrupt: 50 ms, mount: 500 ms }` (embed/mod.rs:66-81);
  - interrupt via `interrupt_flag` or deadline (quickjs.rs:211-235).
- **Other Host methods available:**
  - `is_idle`, `drain_jobs`, `collect_garbage`, `interrupt_flag`, `last_interrupt`, `take_timing`;
  - `start`/`start_with`/`on_started`, `run`/`run_with`, `stop`;
  - `set_locale`, `watch`/`apply_changes`/`request_reload`/`on_reload` (HMR).

tessel's event-time JS (proposal §5.1) needs a cheap "dispatch then drain microtasks" entry. Today
call_sync does not drain jobs (backend-abi.md:320-321), so leftovers go to `wake` and then the
checkpoint. A host change is therefore needed: call_sync followed by an immediate `checkpoint()`,
or a new host entry point.

---------------------------------------------------------------------------------------------------
## 6. Subsystems today and their egui dependency

| # | Subsystem | Where (LOC) | How it works today | egui dependency | tessel port |
|---|---|---|---|---|---|
| 6.1 | **Focus** | focus.rs 1,073; interact/focus.rs 94; native/mod.rs Policy; widgets' `focus_stop` | EV3: scopes, tab chains recorded during the walk in egui temp data keyed by egui ids; `begin` acts on the last pass's record; autofocus and restore; R11 key claims; default/cancel buttons; TextCommand routing | **deep**: `ctx.memory(|m| m.focused())`, `request_focus`, `move_focus(FocusDirection::None)` (focus.rs:449-980); 109 focus-API hits in 36 files | own focus model in tessel-core (focused NodeId, focus-visible reason); recorded tab chains become scene order |
| 6.2 | **Overlays, popovers, menus, dialogs** | widgets/popup.rs 215, menu.rs 1,108, select.rs 511, tooltip.rs 189, dialog.rs 532, picker_popup.rs 243; overlays.rs 409; menus.rs 624; views/toast.rs 861; embed `create_overlay`/`show_overlay` | each open surface is an **`egui::Area`** at `Order::Foreground` (tooltips `Order::Tooltip`), placed by `Anchored` (flip and shift), closed by `Dismissable`; modal on egui's modal layer with backdrop; menus keep the WG2a data model (entries are out-of-flow scene children); overlay slots are click-through Areas that take input only over an interactive node | **deep**: Area, AreaState, Order, LayerId, egui layer picking | tessel layers (z-raised content layers); Anchored and Dismissable logic is M |
| 6.3 | **Windows and viewports** | windows.rs 114; embed/current.rs 184; runner/viewports.rs 238; embed `set_viewport` | `window` kind = root viewport or `show_viewport_immediate`; deferred viewports through the `SlotPainter` thread-local and a begin-pass plugin (cadatomic disables `prepass_plugin`); off-thread wakes repaint ROOT | **deep** (eframe and egui viewports) | tessel-platform windows, one compositor for all windows (proposal §8.3); current.rs is F |
| 6.4 | **Drag and drop** | kinds/dnd.rs 510; interact/dnd.rs 281 | `drag-type`/`drop-types` props on any node run DragSource/DropTarget; the payload rides in **`egui::DragAndDrop`** (so cadatomic's Rust egui panels interoperate); OS file drops via `hovered_files`/`dropped_files`; ghost on a layer; auto-scroll | **deep** | tessel-platform DnD + an island bridge (egui panels in islands still use egui::DragAndDrop: needs a shim in EguiIsland) |
| 6.5 | **Clipboard and primary selection** | platform.rs 162 (arboard `SystemClipboard`, `svelters:clipboard`); primary.rs 166 (W5, Linux); Cargo.toml forces `egui-winit` `clipboard` | text fields copy and paste through egui-winit's arboard clipboard; `Embed::text_command` routes Cut/Copy/Paste via the host clipboard (DX2) and `ctx.copy_text`; primary selection is hooked on `TextEditOutput` | value (platform.rs none; primary deep; egui OutputCommand::CopyText) | platform.rs M; primary R onto quire-text selection |
| 6.6 | **IME and text editing** | leaf.rs:232 `egui::TextEdit`; native/mod.rs 702 (FX1 GTK/Qt word rules and keys over `TextEditState`); native/undo.rs 671 + words.rs 220 (salvaged spec, unused); widgets/entry.rs 226 (interim buffer); value_box.rs sets `IMEOutput` (:423); focus.rs reads `Event::Ime(Preedit/Commit)` (:442-443); table and color_editor use TextEdit | **egui's TextEdit is the editing core; quire-text is NOT used anywhere in svelters** (no dependency; TC0 is "QUEUED" in S/STATUS.md:95; OW4 "text-edit on TC0 (macbook)" not merged, STATUS.md:94); IME is egui-winit's | **deep** | **the biggest new build**: text-edit, the table cell editor and the colour hex field on quire-text (Document/View), IME via tessel-platform (TSF, macOS, text-input-v3), caret and selection paint, AccessKit text runs; pins 1–7 conformance (svelters-conformance/tests/egui_measure.rs 880) gate it |
| 6.7 | **Virtual lists** | kinds/virtuallist.rs 1,749; kinds/treeview.rs 667; widgets/list_rows.rs 374; scene/virt.rs 701 | M5 protocol (splice; setup/bind via call_sync in `before_pass` frame.rs:228, at this frame's offset, R22); RowHeights (uniform until measured); RowPool recycling; RowSource Rust models (LS2); VScroll reads egui wheel and hover; own rows (OW5a) on Scrollable; KeySelect; AccessKit list/tree roles through `a11y_row` | value (logic), deep (VScroll input, a11y_row, drawing) | M5/RowPool/RowHeights M; scrolling becomes compositor scroll layers (proposal §6.4, §7.2) |
| 6.8 | **Accessibility (AccessKit)** | a11y.rs 60; `Widget::a11y(&mut egui::accesskit::Node)`; `Unit::a11y`; `VirtualChildren::row_a11y`; `a11y_row` | egui owns the AccessKit tree: nodes are filled through `ctx.accesskit_node_builder(id, …)` (63 hits in 31 files) and `UiBuilder::accessibility_role/label` (embed/mod.rs:936-940); **node ids are egui Ids** | **deep** | tessel-core projection from scene props (proposal §9.5); the per-kind fill code mostly carries over if the builder API matches `accesskit::Node` |
| 6.9 | **Zoom** | zoom.rs 176; theme/mod.rs; `Embed::set_zoom` | `pixels_per_point = zoom × native ppp` via `ctx.set_zoom_factor`; everything in points; Ctrl+=/-/0 (Cmd on mac) | deep (egui zoom_factor) | a scale factor in tessel-core/platform; logic M |
| 6.10 | **Icons and assets** | icons.rs 840 (ICR1 registry: keys, optical-size masters, colour slots `--icon-subject/accent/badge`, resvg raster → egui texture, icon-font glyph fallback); assets.rs 474 (`asset://` and `data:` loaders on the egui Context, SVG via resvg =0.45.1, PNG via image 0.25); elements/images.rs 100 | egui's image loader pipeline (`add_bytes_loader`, `add_image_loader`, `load_texture`, `try_load_image`) | **deep** | tessel-render resources (atlas, Mask/Color sprites); the registry and slot rewriting (roxmltree) are M |
| 6.11 | **Fonts and text** | fonts.rs 297; paint/text.rs 167; rich.rs 503; layout.rs `Metrics` (:290-322) | egui families stand in for weights (`register_weight`: cadatomic maps 600 → a family); galleys from epaint (no bidi, no real weights); text measured with `fonts_mut(layout_no_wrap)`; elision by binary search over galley glyphs | **deep** | tessel-text (parley, fontique, harfrust, skrifa) changes every size: TK1/VIS1 snapshots and test pixel positions get regenerated |
| 6.12 | **Theme** | theme/mod.rs 1,052 (DS1 token map → CSS vars, egui `Style`, Rust tokens); data/*.json (4 VS Code themes, density, shape, affordance); base.css; theme/egui_style.rs 120; theme/backend.rs 70 | one token map, three outputs; `Theme::egui_style` keeps stock egui widgets and egui-native Rust (cadatomic shell, berth, quire) in step | value (`Color32`, `Shadow`) / deep (egui_style) | tokens and CSS M; egui_style is F but is still needed to theme EguiIslands |
| 6.13 | **Animation and transitions** | anim.rs 367 (TR2, `SVL_STYLE_ANIMATE`, Visual channels opacity/transform/clip); style.rs transitions (`Transitions::sample`) | sampled every logic step on the UI thread | none | M → compositor animations (proposal §9.2) |
| 6.14 | **Hit testing and input** | scene/hit.rs 485; events.rs 1,166 | per-viewport hit lists from the last pass; egui `Plugin::input_hook` sees RawInput first, runs DOM listeners via call_sync and can remove consumed events; egui's `Ui::interact` stays the picking, capture and focus substrate for widgets (ARCH-WIDGETS W2) | value (hit lists) / deep (interact, input_hook) | committed hit list + native capture (proposal §7.1) |

---------------------------------------------------------------------------------------------------
## 7. Tests

### 7.1 crates/svelters-egui/tests (70 files + common/mod.rs = 18,817 LOC; 59 use the kittest harness)

`k` = lines naming `Harness`, `kittest`, `TestApp`, `EmbedHarness` or `Rig::`. Files with k=0 do
not use kittest.

| File | LOC | k | Subject |
|---|---|---|---|
| animator.rs | 170 | 13 | TR2 native animator, virtual clock |
| assets.rs | 204 | 5 | AS1 egui loaders |
| bind_targets.rs | 144 | 9 | CX3 bind targets |
| clipboard.rs | 93 | 6 | DX2 clipboard and shell |
| counter.rs | 169 | 14 | counter fixture end to end |
| css_parse_cost.rs | 15 | 0 | measurement helper (ignored) |
| cv1.rs | 264 | 6 | CV1 canvas seams, panes |
| eframe_idle.rs | 30 | 0 | eframe window goes idle (F) |
| embed.rs | 482 | 9 | EM1 many roots |
| events.rs | 521 | 13 | EV1 capture/bubble |
| focus_keys.rs | 811 | 21 | EV3 scopes, key claims, edit commands, default/cancel |
| gallery.rs | 148 | 7 | gallery fixture |
| gallery_views.rs | 160 | 6 | WG4a gallery pages |
| hmr.rs | 158 | 6 | H1 HMR |
| hmr_sites.rs | 172 | 7 | DZ0 call-site state |
| hmr_state.rs | 153 | 5 | H3 $state across HMR |
| icon_loader.rs | 156 | 0 | IC1 SVGs via egui_extras SvgLoader |
| icons.rs | 255 | 4 | ICR1 registry |
| imm.rs | 593 | 3 | OW-RUST imm (F) |
| intl.rs | 86 | 4 | IN1 locale numbers |
| live_styles.rs | 132 | 4 | LD2 live `<style>` |
| ls2.rs | 502 | 11 | LS2 Rust row models, benches |
| menus.rs | 623 | 6 | WG2a menus and overlays |
| menus_app.rs | 118 | 6 | menus gallery page |
| motion.rs | 90 | 9 | TR1 transitions |
| native_focus.rs | 121 | 5 | FX1 focus policy and words |
| observe.rs | 140 | 6 | observe exactness |
| oplog_replay.rs | 194 | 3 | egui op-log replays |
| os_clipboard.rs | 64 | 0 | CB1 egui-winit clipboard |
| overrides.rs | 148 | 4 | DZ0 overrides |
| ow1a.rs | 222 | 2 | OW1a own controls |
| ow1a2.rs | 310 | 3 | OW1a2 |
| ow1a3.rs | 286 | 12 | OW1a3 own label/RT2 |
| ow1a4.rs | 539 | 3 | `use:` actions |
| ow1b.rs | 372 | 11 | value controls |
| ow2.rs | 336 | 5 | scroll, panels, windows |
| ow3.rs | 414 | 2 | menus, popup, select, tooltips |
| ow5b.rs | 189 | 7 | table header |
| ow6.rs | 315 | 7 | pickers |
| ow7.rs | 277 | 6 | modal, inner-window, toasts |
| ow7b.rs | 263 | 8 | declarative toast, dialog-buttons |
| ow7c.rs | 131 | 4 | status-bar |
| ow_css.rs | 355 | 3 | OW-CSS stage 2 |
| own_widgets.rs | 435 | 5 | OW0 foundation |
| passes.rs | 387 | 6 | SG4 providers, passes, overlays |
| queries.rs | 316 | 3 | SG3 queries and hit-test |
| rt2_rich.rs | 136 | 6 | RT2 rich label |
| scene.rs | 313 | 5 | scene public API |
| scene_memory.rs | 118 | 3 | SG6 memory and paint cost |
| source_loc.rs | 245 | 5 | DZ0 source locations |
| spec.rs | 173 | 0 | egui.json/egui.d.ts generation check |
| startup.rs | 119 | 4 | Q4b startup timings (not CI) |
| style.rs | 178 | 4 | CS5e cascade |
| style_shadow.rs | 151 | 4 | CS5g box-shadow |
| style_source.rs | 122 | 3 | DZ0 matched rules |
| style_text.rs | 201 | 4 | CS5f typography |
| theme.rs | 185 | 6 | DS1 theme |
| tick_profile.rs | 82 | 3 | TR3 transition cost |
| tier0.rs | 603 | 20 | WG2b one test per kind |
| treeview.rs | 187 | 10 | WG5b tree-view |
| utilities.rs | 329 | 6 | U3 utility classes |
| viewports.rs | 409 | 5 | EM2 multi-viewport |
| views.rs | 338 | 11 | WG4a Tier 1 views |
| vlist.rs | 928 | 27 | LS1/SG5 virtual-list |
| wg4b.rs | 336 | 11 | pickers and DnD |
| wg5a.rs | 494 | 7 | list/table keyboard, type-ahead |
| widgets.rs | 466 | 19 | WG1 SC1 kinds |
| widgets_panel.rs | 105 | 6 | SC1 panel from Svelte |
| window.rs | 94 | 4 | close-request |
| zoom.rs | 243 | 9 | ZM1 zoom |
| common/mod.rs | 186 | 5 | `Rig`: backend behind real vtable + recording host, `click`, `type_into`, `drawn` |

### 7.2 egui_kittest API usage

The counts below cover tests/*.rs, tests/common/mod.rs and src/runner/kittest.rs, with hits and
files from grep. Note that `.run()` and `.step()` also count svelters' own `EmbedHarness` and
`TestApp` wrappers.

| API | Hits | Files |
|---|---|---|
| `Harness::new` | 28 | 19 |
| `Harness::builder` (`.with_size` 11/7) | 11 | 7 |
| `Harness<` (type) | 8 | 6 |
| `get_by_role(Role::…)` | 198 | 29 |
| `get_by_label(` | 151 | 33 |
| `get_by_label_contains` | 4 | 3 |
| `get_all_by*` | 6 | 5 |
| `query_by_label(` | 120 | 37 |
| `query_by_label_contains` | 2 | 2 |
| `query_by_role` | 36 | 10 |
| `query_all_by*` | 13 | 8 |
| `.click()` | 135 | 32 |
| `click_accesskit` | 3 | 2 |
| `.hover()` | 1 | 1 |
| `.focus()` | 9 | 6 |
| `type_text(` | 7 | 4 |
| `key_press(` | 75 | 25 |
| `.run()` | 1,089 | 66 |
| `run_steps(` | 38 | 10 |
| `.step()` | 137 | 29 |
| `run_until*` (incl. `run_until_settled`) | 8 | 3 |
| `render()` | 1 | 1 |
| `snapshot(` | 5 | 1 |
| `input_mut(` | 8 | 4 |
| `event(` (pushes `egui::Event`) | 152 | 34 |
| `drag(` | 26 | 8 |
| `.state()` | 28 | 8 |
| `accesskit_node()` (`.data()` 33, `.toggled` 7, `.value` 6, `.role` 4, …) | 84 | 26 |
| `use egui_kittest::kittest::Queryable` | 27 | 27 |

- Raw `egui::` in tests: 1,472 references. The most frequent are `egui::Key` 185,
  `egui::Event` 153, `egui::Modifiers` 118, `egui::PointerButton` 107, `egui::vec2` 100,
  `egui::accesskit` 57, `egui::Rect` 51 and `egui::pos2` 50.
- So a mechanical port needs `tessel-test` to accept the same event vocabulary, through a
  compatibility module re-exporting `Key/Modifiers/PointerButton/Event`-shaped types, and to
  return AccessKit nodes.
- svelters' own test API (runner/kittest.rs): `TestApp::{load, load_stepped, load_with,
  with_config, from_source*, mount_source, embed_host, with_loader, host, run, errors}` and
  `EmbedHarness::{new, set_ui, run, step, run_until_settled}`. These wrap `egui_kittest::Harness`
  and re-export `kittest::{NodeT, Queryable}`.

### 7.3 svelters-automation (7,095 src + 1,503 tests) and svelters-testkit (1,373 + 956)

**svelters-automation** (lib.rs:1-30) has six parts:
- **Query:** selectors `role= label= text= anchor= root= pos= nth=` and CSS via svelters-style,
  chained with `>>`.
- **Read:** `Session::{query, node, snapshot, roots, a11y_tree}`.
- **Act:** `Tier::Semantic` sends AccessKit action requests and `Tier::Synthetic` sends synthetic
  egui input, both after actionability checks.
- **Wait:** `wait_settled`, `wait_for`, `wait_event`, `never`, `arm_seen`, busy sources, clock
  domains, `WaitReport`.
- **Capture and journal:** `screenshot`, `journal_subscribe`/`journal_take`.
- **Out of process:** `server::Server` speaks NDJSON JSON-RPC 2.0, with `client::{Client, App}` and
  `svelters-run`.

The **`Driver` trait** (driver.rs:22-66) is the seam:

```rust
pub trait Driver {
    fn scene(&self) -> SceneHandle;
    fn busy(&self) -> Option<String>;
    fn step(&mut self);
    fn send(&mut self, event: egui::Event);
    fn frame_ms(&self) -> f64;
    fn a11y_nodes(&self) -> Vec<A11yNode> { … }
    fn screenshot(&mut self) -> Option<egui::ColorImage> { … }
    fn clock_ms(&self) -> Option<f64> { None }
    fn seed(&self) -> Option<u64> { None }
    fn hold_clock(&mut self, _held: bool) {}
}
```

- The egui coupling sits in session.rs (91 mentions), driver.rs 16, engine.rs 5 and clock.rs 4.
- Selectors, the journal, RPC, replay and busy logic are egui-free (M).
- `SceneHandle` comes from svelters-egui, so it moves with the scene.

**svelters-testkit** (TK1) runs one `.svelte` case, with inline expectations, on every backend:
- runners:
  - `log` (log backend);
  - `egui` (`TestApp` + kittest + AccessKit tree; egui.rs 152 LOC);
  - gtk/qt via examples/counter;
- results go through a per-backend `ratchet`;
- the cases are tests/cases (5 `.svelte` files + snapshots); `packages/testkit/kit/egui` holds the
  portable kit components (Window, Column, Row, Button, TextInput, Check).

tessel needs a `tessel.rs` runner, a kit, and a ratchet entry.

### 7.4 Conformance and CI suites that run against the egui backend

- **svelters-conformance/tests/egui_measure.rs** (880): the WG3 pins 1–7, text-field behaviour
  measured against GTK and Qt references. ARCH-WIDGETS §6.1 gives the `FLOOR` as "today 141/139 on
  pins 1–3, 7; 232/235 on all 18 scripts with FX3". STATUS.md:98 gives egui_measure GTK 193/243
  and Qt 200/243. This is the text-edit gate.
- **svelters-conformance/tests/egui_scroll.rs** (487): scroll physics against recorded references
  (conformance/scroll).
- **svelters-testkit/tests/vis1.rs** (731): VIS1c, own widgets against VS Code reference captures
  through PaintTrace. The ratchet is `conformance/visual/baseline-{windows,macos}.json` (213/354
  on Windows refs). CI job `binary(vis1)`; macos-appkit.yml:111.
- **svelters-testkit/tests/sched_egui.rs** (155): seeded schedules on egui.
- **svelters-egui/tests/oplog_replay.rs**: op logs of fixtures with toolkit egui (3 fixtures
  name egui).
- CI (.github/workflows):
  - linux-gtk.yml nextest includes `-p svelters-egui -p svelters-automation -p egui-kinds-example
    -p svelters-testkit -p svelters-conformance` (:61-75);
  - `egui-benches` (`ls2` bench_*, :111-127);
  - `egui-snapshots` (TK1 cases rendered through wgpu on lavapipe, `cases_on_egui`, gallery-egui
    build, :133-181);
  - clippy on svelters-egui with the disallowed lists (:65-68);
  - js.yml checks the generated egui widget reference (`tools/docs/egui/gen_reference.py --check`,
    :108-112) and the utility tables against egui.json (:115).
- The JS conformance job (js.yml `conformance`, against `conformance/baseline.json`) runs Svelte's
  tests on the Node TestHost, so it is not egui-specific.

---------------------------------------------------------------------------------------------------
## 8. Backend-specific items elsewhere in svelters that need a `tessel` entry (or a neutral name)

**The toolkit string `"egui"` is a semantic switch in several places, not just a label.** The
cheapest path is a choice between two options:
- (a) tessel ships as backend name `egui` with the same vocabulary;
- (b) better: rename the egui spec to a neutral "scene" spec and key behaviour on the spec's
  capabilities (`style.engine: "backend"`, `inline_text`), not on the name.

| Item | Path | What is egui-specific |
|---|---|---|
| Semantic spec | metadata/semantics/egui.json (562 KB: 60 kinds, 57 tags, `style` section, `theme` tokens, `bind_sources`, `inline_text`, `event_aliases`, `method_lookup`) | generated by svelters-egui `spec.rs` from `kinds/*.rs` + `style::ENGINE_PROPS`; `source.generator` names svelters-egui |
| Typings | packages/native-runtime/types/egui.d.ts (1,604) | generated by spec.rs (`TYPINGS_PATH`, spec.rs:30) |
| Style table | crates/svelters-style/src/tables.generated.rs `pub static EGUI` (:240), `ALL` (:376) | generated by tools/metadata/style/gen_props.py (`TOOLKITS = ["gtk","qt","appkit","winui","egui"]`, :21) from metadata/semantics/egui.json's `style` section; **there is no metadata/style/egui.yaml** (the other four toolkits have one) |
| Utility tables | crates/svelters-style/src/utilities.generated.rs, packages/style/src/utilities.generated.{js,json}, docs/egui/reference/utilities.md | tools/metadata/style/gen_utilities.py reads `metadata/semantics/egui.json` (:40) |
| Compiler built-in specs | crates/svelters-compiler/src/semantics/backend.rs:13 `EGUI_JSON = include_str!(…egui.json)`, :59 `"egui" => …`, :60 error list | needs `"tessel"` (or the neutral spec) |
| Compiler kind listing | semantics/kinds.rs:141 `for tk in ["gtk","qt","appkit","winui","egui"]` | |
| Compiler CLI | bin/svelters-compile.rs:1, :34 `--backend gtk\|qt\|appkit\|winui\|egui` | |
| Compiler lowering | lower/engine_css.rs, lower/mod.rs:45 (D9: `<style>` passed as CSS when `style.engine == "backend"`), :293 inline text runs; utilities/mod.rs (egui part names :63, EmbedConfig utilities :67) | keyed on spec fields; mostly name-neutral already |
| Vite plugin | packages/native-runtime/vite.js:376 `if (toolkit === 'egui')` (raw scoped CSS), :451 `backend !== 'egui'`, :464-465 `@apply` expansion; :172 behaviours from `svelters_egui::dump` | **hard-coded name** |
| Runtime JS | packages/native-runtime/src/style.js:51 (`SVL_STYLE_TARGET` engine answer), :123 "utility classes render on egui only"; motion.js:14, 261 (backend answers OP_ANIMATE); index.js:283-304 (anchor scoping is "egui's prop"); lists.js:578 (`tree-view` native on egui) | capability-based (style target answer), messages name egui |
| TestHost | packages/testhost/src/index.js:437 `styleDialect = toolkit === 'egui' ? 'engine' : …` | hard-coded name |
| Module loader | crates/svelters-modules/src/source/transform.rs:81 `self.sem.spec().backend == "egui"` (live `<style>` path), :122 message; source/mod.rs:57, 61, 390, 636 | **hard-coded name** |
| Build package | packages/build/index.js:19-23 (kinds.json fragments, embed.js bundle for "svelters-egui's Embed") | |
| Testkit | packages/testkit/kit/egui/* (Window, Column, Row, Button, TextInput, Check), packages/testkit/bin/svelters-cases.mjs, crates/svelters-testkit/src/egui.rs | new `kit/tessel` + runner |
| Dialog exports | dialogs.rs `#[svelters::export(module = "egui")]` → JS `svelters:app/egui` | user-visible module name |
| Behaviour exports | `svelters:behaviors/sv` (use_units.rs) | neutral |
| Docs | docs/egui/ (73 files, 10,415 lines: START-HERE, css/, rust/, icons.md, reference/ generated by tools/docs/egui/gen_reference.py 646 + tools/docs/test_egui_reference.py) | a tessel copy or rename |
| Fixtures | fixtures/{counter,embed,timers,if-toggle,motion,keyed-list,lists,gallery,gallery-egui-menus}-egui (9 dirs) | tessel variants or shared |
| Examples | examples/egui-kinds (2,116 LOC Rust) | provider-kind example |
| Compiler tests | crates/svelters-compiler/tests/{g1_egui_spec.rs, cs5g_egui_css.rs, wg2a_egui_menus.rs, fw1/egui-element, fw1/capture-egui} | |
| Conformance | crates/svelters-conformance/tests/egui_{measure,scroll}.rs; conformance/visual baselines | §7.4 |
| CI | .github/workflows/linux-gtk.yml egui jobs, js.yml reference and utilities checks | new tessel jobs (lavapipe already used) |
| Lints | crates/svelters-egui/clippy.toml (ZU3 blocking-call list + OW `disallowed-types`) | copy the ZU3 half to svelters-tessel |

---------------------------------------------------------------------------------------------------
## 9. Riskiest pieces

Ordered by risk × size.

1. **Text editing and IME (text-edit, table cell editor, colour hex field).**
   - These are still `egui::TextEdit`. quire-text (TC0) is QUEUED and OW4 is unmerged (S/STATUS.md:94-95).
   - Nothing in svelters links quire.
   - IME, caret and selection painting, the PRIMARY selection, the FX1 word and key rules
     (native/mod.rs) and undo models (native/undo.rs, unused) must all be built new.
   - The pins 1–7 conformance (egui_measure.rs) and 10+ test files gate it.
2. **Text shaping and measurement swap** (epaint galleys → parley/skrifa).
   - layout.rs `Metrics`, paint/text.rs, rich.rs `LayoutJob`, fonts.rs's weight→family shim and
     elision all change.
   - Every own-widget size changes, so TK1 snapshots, VIS1 ratchets (213/354) and the
     pixel-position asserts in tests have to be regenerated.
3. **The interaction substrate.**
   - Units take `&egui::Response` and `&egui::InputState`.
   - Pressable deliberately copies egui's click semantics "so every button test holds"
     (interact/press.rs:4-6).
   - Focus is egui `Memory` (109 hits in 36 files).
   - Picking and capture are `Ui::interact`.
   - tessel must reproduce click, drag-threshold, capture and focus semantics exactly, or about
     1,000 kittest assertions drift.
4. **AccessKit and test locators.**
   - 63 `accesskit_node_builder` sites in 31 files, with egui Ids as AccessKit ids.
   - Tests locate everything by role and label (198 + 151 + 120 + 36 hits).
   - automation and the MCP tools also read AccessKit.
   - Names, roles and the tree shape must be identical.
5. **Layers, overlays and multi-window:** `egui::Area`/`Order` per floating surface, egui's modal
   layer, click-through overlay slots, viewports plus the deferred-viewport `SlotPainter` hack.
6. **The extension API is egui-typed.**
   - `Widget::ui(&mut egui::Ui) -> egui::Response`, `Behavior::after_ui(&egui::Response)`,
     `paint_over(&egui::Painter)`, `ScenePass::overlay(&mut egui::Ui)`, `Unit::sense()`.
   - The macros hard-code `::svelters_egui` (49 refs).
   - The 26 own widgets and cadatomic's 5 widgets + 4 behaviours + 2 providers + 1 pass +
     1 VirtualChildren all need rewriting.
   - The proposal's `Kind` sketch (§9.4) lacks `layout`, `taffy_style`, `prepare`, `hit_test`,
     `virtual_children` and the key and text hooks.
7. **Drag and drop interop** rides on `egui::DragAndDrop` with cadatomic's Rust egui panels; once
   those panels live in EguiIslands, tessel↔island DnD needs a bridge.
8. **The toolkit name `"egui"` is load-bearing** in vite.js, testhost, svelters-modules' transform,
   compiler backend tables and the style and utility generators (§8). Renaming late breaks builds
   across repos.
9. **Event-time JS needs a host change.** call_sync does not drain microtasks
   (backend-abi.md:320-321); the widget-signal outbox (frame.rs:402-431) is how `onclicked` works
   today, and cadatomic uses `onclicked` 66 times against `onclick` 2 times.
10. **Small carry-over bug:** the scene flag bit collision `SCROLL_CENTER` = `INK_OVERFLOW` = 1<<21
    (scene/node.rs:290/296).

---------------------------------------------------------------------------------------------------
## 10. Suggested porting order (work breakdown seed)

1. **Prep in svelters while still on egui** (no behaviour change):
   - neutralise the `"egui"` name checks: switch vite.js, testhost and the modules transform to the
     spec's `style.engine`, and decide between the shared or renamed spec (§8);
   - give the macros a `crate =` path;
   - put the scene's value types behind a `geom` module (type aliases);
   - fix the bit collision;
   - add a host entry "dispatch + drain microtasks" for event-time JS;
   - split native/mod.rs into Policy and TextEdit glue, and move native/undo.rs + words.rs to
     quire-text.
2. **tessel-core: the M set, about 23k.**
   - scene/ (8.6k), layout.rs (with a text-measure trait), events.rs propagation, embed
     roots/slots/settle/panes, kinds tables and registry, provider state and methods, ExtKey and
     PropKey, journal → damage, hit lists.
   - Bring up svelters-tessel's ops.rs/backend.rs/frame.rs with the ABI on the M1 counter demo
     (proposal M1).
3. **tessel-text + tessel-render primitives**:
   - paint/ → DisplayList (box, text, icon, image, shadow);
   - StyleView glue from style.rs and theme tokens;
   - fonts with real weights;
   - icons and assets into the atlas.
4. **Interaction and focus:**
   - the `InputCx` adapter over tessel events, then the 13 R units (logic kept);
   - the own focus model (focus.rs);
   - the AccessKit projection (a11y);
   - the hit test and capture.
5. **Tier 0 simple kinds:**
   - box, grid, sides, stack, window, label + rich runs;
   - button, checkbox, switch, radio, toggle, separator, progress, spinner, icon, image,
     hyperlink, frame, collapsing-header, canvas;
   - at the same time, `tessel-test` Harness with kittest verbs and an egui-shaped event shim, so
     tier0.rs, widgets.rs, ow1a*.rs, own_widgets.rs, focus_keys.rs and events.rs port mechanically.
6. **Scroll and layers:**
   - scroll-area, scrollbar, panel, split-view (Scrollable, Resizable) as compositor scroll layers;
   - virtual-list and tree-view (M5, RowPool), vlist.rs and wg5a.rs;
   - popup, tooltip, menus, combo-box, modal, inner-window, toast(-host), dialog-buttons and
     status-bar on tessel layers.
7. **Text input** (the M2 gate):
   - text-edit on quire-text plus IME per OS, the PRIMARY selection and clipboard;
   - then the value controls' entry (slider, range-slider, drag-value value boxes), the table cell
     editor, color-editor hex;
   - port egui_measure → tessel_measure pins.
8. **Tier 1 rest:**
   - table (2.5k);
   - date-picker, color-picker, calendar, color-editor;
   - tab-view (new own widget; today's is still egui);
   - drag and drop (+ the island bridge), zoom, windows and multi-window, dialogs.
9. **Tooling:**
   - svelters-automation `Driver` for tessel, testkit `tessel` runner and kit, VIS1 on tessel;
   - CI jobs, docs/egui → docs/tessel (or shared), fixtures.
10. **Extension API:**
    - the provider traits as tessel `Kind` and behaviour traits, and the macros;
    - then cadatomic re-hosts its 5 widgets, 4 behaviours, 2 providers and 1 pass (cadatomic
      lane);
    - `imm` stays frozen until tessel_berth needs retained chrome.
