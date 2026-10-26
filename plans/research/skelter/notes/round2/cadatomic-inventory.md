# Migration inventory: egui 0.37 -> tessel (cadatomic, berth, quire)

Read-only survey, 2026-10-25. No cargo was run. Checkouts:

| Repo | Path | Sha | Note |
|---|---|---|---|
| cadatomic | /home/user/cadatomic | 91ade11 on `research/own-gui-toolkit` (code = master 0a1c345) | design doc `docs/research/2026-10-25-own-gui-toolkit.md` |
| berth | /home/user/berth | 85938b2 "Merge claude/b10-tab-ids" | cadatomic pins 2bd2422 (`pins.toml`: "branch claude/b10-tab-ids, one commit on 3380aa0: tab author ids"). The checkout is **shallow (1 commit)**, so 2bd2422 is not present and no diff could be taken. 85938b2 is the merge of that same branch into master; the tab-id work (`TAB_AUTHOR_PREFIX`, `TAB_CLOSE_AUTHOR_PREFIX` in `egui_berth/src/draw.rs`) is visible. Any master-only commits between 3380aa0 and the merge are not identifiable here. |
| quire | /home/user/akiselev/quire | 3e78dcd | = cadatomic's pin (`Cargo.toml` rev 3e78dcd3bf74...) |
| svelters (context) | /home/user/svelters | 1d6656c2 | = cadatomic's svelters pin; svelters-egui src 65,916 LOC, 72 test files |

LOC are `wc -l` (comments and inline `#[cfg(test)]` included). "egui::" = occurrences of the literal `egui::`; "touch" = lines matching `egui::|\bui\.|\bUi\b|\bResponse\b|\bPainter\b|\bctx\.` (a proxy: `ctx.` also matches cadatomic's own `ViewContext`/tour contexts, so treat it as an upper bound).

---

## 1. berth

### 1.1 Crates

| Crate | src | tests | other | GUI deps |
|---|---|---|---|---|
| berth-core | 6,810 | 7,490 | bench 48 | none: indexmap, optional serde/serde_json(preserve_order)/schemars. "No egui, winit, wgpu or tokio; builds for wasm32-unknown-unknown; never spawns a thread" (`berth-core/src/lib.rs`) |
| egui_berth | 5,190 | 5,690 | examples 1,171 | `egui = "=0.37.0"` (re-exported), macOS-only core-graphics/objc2/objc2-app-kit/objc2-foundation (OS pointer, window-at-point, monitor frame during native drags); dev: egui_kittest =0.37.0, eframe =0.37.0 (glow, x11) |

### 1.2 berth-core modules (all egui-free)

| Module | LOC | What it does |
|---|---|---|
| solver.rs | 1271 | Layout + window size -> integer rectangles (`DockFrame`): min/max propagation, collapse to strips, overflow |
| apply.rs | 947 | The reducer: one `DockCommand` on a layout clone; availability = same code |
| model.rs | 539 | `Layout`, windows, areas, regions, floats, placements, slot memory (private fields) |
| drag.rs | 467 | Drag machine Idle->Armed->Dragging, hysteresis, `DockInput`, `DragHooks`, programmatic drags |
| query.rs | 371 | Read-only queries, pane-moving primitives |
| area.rs | 355 | Iterative area-tree ops |
| command.rs | 350 | `DockCommand`, `Target`, `FocusTarget` |
| dock.rs | 338 | `Dock`: layout + undo + queue + workspaces/profiles, `AreaHint` |
| check.rs | 280 | Invariants |
| drop.rs | 279 | Drop resolver: pointer -> `Target`; preview rect; needs renderer-measured `TabChips` |
| host.rs | 273 | Platform seam `DockHost` (windows_at, window_rect, pointer_screen_pos, monitor_rects, `DragCaps`); `ScriptedHost` for tests |
| file.rs | 254 | Versioned JSON layout file, migrations, goldens |
| avail.rs | 248 | Availability, refusals, `Policy` |
| workspace.rs | 162 | Workspaces/profiles as data |
| policy.rs | 147 | `PaneInfoPolicy` |
| ids.rs | 139 | Ids (strings / counters) |
| geom.rs | 136 | Integer geometry, `LogicalRect` |
| undo.rs | 96 | Snapshot undo |
| info.rs | 68 | `PaneInfo` (per-frame facts: title, closable, min/max size, icon) |
| lib.rs | 52 | re-exports |
| busy.rs | 38 | `BusyProbe` (Send+Sync) for the host idle barrier |

### 1.3 egui_berth modules

| Module | LOC | egui:: | Role | egui coupling |
|---|---|---|---|---|
| draw.rs | 938 | 8 (+`use egui::{...}`) | Chrome for one window: tab strips (scrolling, chips), splitters (sash drag, Increment/Decrement a11y actions), regions/stripes, floats; pane bodies through `PaneHost::pane_ui` once per pass; AccessKit nodes via `ctx.accesskit_node_builder`; context menu; strip scroll state in egui temp memory | heavy: `Ui::new_child`, `UiBuilder` (layer, clip, a11y role/label/parent), `Ui::interact` with `Sense`, `CursorIcon`, `smooth_scroll_delta`, `layer_id_at`, `Response` |
| chrome.rs | 747 | 4 | BD7 `ChromePainter` seam: berth lays out/hit-tests/a11y, painter only paints (frame, group, strip, tab, button, title_band, stripe, stripe_button, sash, drop_preview, drag_ghost, carry_badge); `EguiChrome` default; `ChromeMetrics` | trait methods take `&Ui`, `&Response`, `&Painter`, `egui::Rect/Pos2` |
| windows.rs | 693 | 8 | Secondary windows: placement memory, off-screen recovery (`recover_native`, `recover_on_monitors`), embedded-window chrome as `egui::Area` (Order::Middle, fixed_pos), native windows via `show_viewport_immediate`/`show_viewport_deferred`, `ViewportBuilder`, `ViewportCommand` (Focus, CancelClose, OuterPosition, InnerSize, Maximized, Fullscreen), move/resize gesture (8 handles) | heavy (viewports) |
| ui.rs | 597 | 5 | `DockUi`: `logic(ctx, dock, host) -> DockLogic` (between frames: intents, `Closed/MovedToViewport/Hidden/Shown` events) and `show(ui, dock, host) -> DockOutput` (the pass; never mutates the model) | `Context` frame/pass numbers, `ViewportId`, `request_repaint(_of)`, `embed_viewports()` |
| drag.rs | 564 | 6 | Tab drag/drop through the core drag machine, keys, overlay dismissal, ghost/preview on a Foreground `LayerId` via `ctx.layer_painter`, native cross-window via `ViewportInfo.inner_rect/outer_rect`, `ViewportCommand::Focus` on drop | heavy |
| carry.rs | 299 | 3 | Keyboard "carry mode": drop targets as clickable `Area`s (Order::Foreground) + preview layer painter | medium |
| frame.rs | 265 | 1 | `solve_window`: core `DockFrame` -> egui `Rect`s | light (Rect/Pos2 only) |
| native.rs | 245 | 15 | macOS OS pointer/foreign-window/monitor queries during native drags (egui-winit 0.37 drops releases) | platform; replaceable by toolkit pointer capture |
| style.rs | 185 | 1 | `DockStyle` metrics/flags, `DockText` strings, `GroupButtons` | light (colours from `egui::Visuals`) |
| event.rs | 134 | 2 | `PaneEvent`, `CloseRequest`, `ContextMenuRequest`, `DockLogic`, `DockOutput` | `egui::ViewportId`, `egui::Pos2` in public types |
| keys.rs | 128 | 16 | `DockKeys` as data (`egui::KeyboardShortcut`) | light |
| testing.rs | 114 | 4 | `RecordingHost` | light |
| deferred.rs | 110 | 2 | Thread-local `DeferredSlot` hand-off for deferred viewport closures (`Rc<RefCell<dyn DeferredHost>>`) | egui-architecture workaround; disappears with tessel |
| dockhost.rs | 106 | 0 | `EguiDockHost`: window rects from last pass, z-order = draw order; embedded vs native coordinates | none (berth-core types) |
| lib.rs | 37 | 0 | re-exports egui, berth_core | |
| host.rs | 28 | 4 | `PaneHost` trait | `&mut egui::Ui`, `&egui::Painter`, `egui::ViewportId` |

### 1.4 egui_berth public API

- `DockUi` (`ui.rs`): builders `new`, `with_salt`, `with_style(DockStyle)`, `with_painter(impl ChromePainter)`/`set_painter`, `with_text(DockText)`, `with_keys(DockKeys)`, `with_viewports(ViewportMode::{Immediate,Deferred})`, `auto_apply(bool)`, `with_policy(impl Policy)`; per frame `logic(&Context, &mut Dock, &mut dyn PaneHost) -> DockLogic`, `show(&mut Ui, &Dock, &mut dyn PaneHost) -> DockOutput`; queries `metrics`, `focus_direction`, `set_monitor_rects`, `note_layout_replaced`, `is_settled`, `anchor_rect(anchor) -> (WindowId, Rect)`, `viewport_of`.
- `PaneHost` (`host.rs`): `pane_ui(&mut Ui, &PaneId)`, `pane_info(&PaneId) -> Option<PaneInfo>`, `paint_icon(&Painter, Rect, &IconRef)`, `restore_focus(&PaneId, ViewportId)`, `viewport_done(&mut Ui, WindowId)`.
- `ChromePainter` + `ChromeMetrics`, `EguiChrome` (`chrome.rs`); `DockStyle`, `DockText`, `GroupButtons` (`style.rs`); `DockKeys` (`keys.rs`); `DeferredHost`, `DeferredParts`, `DeferredSlot`, `ViewportMode` (`deferred.rs`); `EguiDockHost` (`dockhost.rs`); `PaneEvent`, `HiddenReason`, `CloseReason`, `CloseRequest`, `ContextMenuRequest`, `DockLogic`, `DockOutput` (`event.rs`); `TAB_AUTHOR_PREFIX`, `TAB_CLOSE_AUTHOR_PREFIX` (AccessKit author ids `tab:<pane>`, `tab-close:<pane>`).

### 1.5 How egui_berth draws

- **One layer per window** for chrome and pane bodies: `ui.new_child(UiBuilder::new().max_rect(rect).accessibility_role(Role::Window))`; pane bodies are `Ui::new(ctx, id, UiBuilder::new().layer_id(ui.layer_id()).max_rect(body).accessibility_role(Role::TabPanel))` with a clip rect (`draw.rs:236-249`). Floats are painted in the same layer in model order (`draw_float`, raise on press if `layer_id_at(pointer) == ui.layer_id()`, `draw.rs:838-870`).
- **Overlays:** drop preview and drag ghost on `LayerId::new(Order::Foreground, "berth.overlay"+window)` via `ctx.layer_painter` (`drag.rs:261`); carry targets as `Area` Foreground (`carry.rs:214-226`).
- **Embedded secondary windows** (kittest default `embed_viewports()`): `Area::new(key).order(Order::Middle).fixed_pos(...)` with berth-owned title band and 8 resize handles (`windows.rs:284`).
- **Native secondary windows / tear-off:** `ctx.show_viewport_immediate(vp, ViewportBuilder, ...)` (default) or `show_viewport_deferred` + `DeferredSlot` thread-local (`windows.rs:414-438`); placement through `ViewportCommand`s; cross-window drop only where every window reports `inner_rect` (not Wayland); macOS asks the OS for pointer/foreign windows (`native.rs`).
- **Input:** `Ui::interact` with `Sense::click_and_drag` per tab/sash/title; press arms core `DragMachine`; `smooth_scroll_delta` consumed by strips; keys read in `show` (`handle_keys`, `carry_keys`); AccessKit actions Increment/Decrement on sashes.
- **a11y:** berth writes every node itself (`ctx.accesskit_node_builder`); painters must not call `ui.interact` (HANDOFF.md:665; DESIGN-DELTAS.md:275-293).
- **Repaint:** requests repaint while the drag machine or carry is active; deferred windows `request_repaint_of`.

### 1.6 What `tessel_berth` needs from the toolkit

1. **Layers / z-order:** per window a base layer for chrome + pane hosts; an overlay layer above everything in that window (drop preview, ghost, carry targets); floats as ordered sublayers (raise on press). Clip per pane body.
2. **Pane host slot:** a node whose rect the dock assigns, into which the host mounts arbitrary content (native tessel subtree, an `EguiIsland`, or a Svelte root) exactly once per visible pane; plus `viewport_done` equivalent (end-of-window-pass hook).
3. **Pointer capture** for tab drags that continue outside the source window and across windows (global pointer position in screen coordinates, release delivery even outside the window: replaces `native.rs`), plus hover-hysteresis timing (frame clock).
4. **Windows:** create/close/move/resize/focus/maximize/fullscreen OS windows with a frame clock each (§8.3); report outer/inner rects and monitor work areas (feeds `DockHost::window_rect`, `monitor_rects`, `windows_at` z-order including foreign windows where the OS can say).
5. **Hit regions with cursors** (resize cursors on sashes/handles), wheel deltas for tab-strip scroll, keyboard routing with the host's keymap (berth leaves `DockKeys` empty in cadatomic; dock chords come from the app keymap).
6. **Text measurement** for tab chips (the core resolver needs measured chip rects) and labels; icon painting (`paint_icon`).
7. **AccessKit:** berth-authored nodes with roles TabList/Tab/TabPanel/Window/Button/Splitter, author ids, Increment/Decrement actions, live announcements (`DockLogic.announcements`).
8. **Busy:** `berth_core::BusyProbe` stays; the renderer reports drags.
- Reusable unchanged: berth-core entirely; `dockhost.rs`, most of `frame.rs`/`style.rs` (swap egui geometry types), `DockText`, chrome *metrics*. Replaced: `deferred.rs` and `native.rs` (egui/winit workarounds).

### 1.7 berth rules relevant to a new renderer crate

berth has **no CLAUDE.md** (files: README.md, HANDOFF.md, docs/API-NOTES.md, docs/DESIGN-DELTAS.md, docs/BD7-chrome-painter.md, scripts/check.sh). Rules found:
- README: berth-core "No egui, winit, wgpu or tokio; builds for wasm32-unknown-unknown; never spawns a thread"; egui_berth depends on `egui = "=0.37.0"` and re-exports it (one copy in the graph). A `tessel_berth` must equally pin and re-export tessel.
- Workspace lints: `unsafe_code = "forbid"`, `missing_docs = "warn"`, clippy `dbg_macro`, `todo` warn (`Cargo.toml`). macOS AppKit calls are done safely through objc2-app-kit (HANDOFF.md:535,570).
- `scripts/check.sh`: fmt, `clippy --workspace --all-targets --all-features -D warnings`, `test --workspace --all-features`, berth-core `--no-default-features` and `--target wasm32-unknown-unknown`; run once per lane through the machine slot wrapper (`CARGO_SLOT`).
- Goldens: bless only when missing with `BERTH_BLESS=1`, never edit; refactors never re-bless (HANDOFF.md:29,663).
- `PaneEvent` `#[non_exhaustive]`, uses `berth_core::CloseReason`; `show` never mutates the model (intents only); the dock never requests focus inside a pane body (R7); `pane_ui` at most once per pass per visible pane; painters never call `ui.interact` (a11y ownership).
- berth never moves an OS window itself (no StartDrag/`start_system_move`, HANDOFF.md:701).
- cadatomic register: "The dock and the code editor are standalone **egui-only** crates (no svelters, cadatomic or kernel dependencies), each in its own repository" (`docs/plan/00-decisions.md:466`), restated in cadatomic `CLAUDE.md` ("the standalone egui-only dock and code-editor repositories"). berth gets no DS1 tokens and stays free of svelters (`00-decisions.md:1531`); the default chrome painter stays egui-only (`:1394`, `:1727`). **Adding `tessel_berth`/`tessel_quire` contradicts the literal "egui-only" wording of a user decision -> needs the user (or at least a register entry) before a lane starts.** The design doc's §9.4/§12 assumes the renderer crates sit "next to egui_berth and egui_quire" in the same repositories.

---

## 2. quire

### 2.1 Crates

| Crate | src | tests | other | GUI deps |
|---|---|---|---|---|
| quire-text | 8,175 | 472 | benches 122 | none: ropey, smallvec, unicode-segmentation, unicode-general-category, opt imara-diff/serde/schemars. "No egui, no tokio, no threads, no regex, no clock; must build for wasm32" (Cargo.toml) |
| quire-core | 18,379 | 8,529 | benches 181 | none: quire-text(diff), smallvec, regex-automata, fancy-regex, opt serde/schemars. "No egui, no tokio, no threads; must build for wasm32" |
| quire-syntax | 2,513 | 1,052 | benches 91 | none (tree-sitter 0.26 + grammars; quire-core) |
| egui_quire | 8,920 | 4,270 | benches 742, examples 136 | `egui =0.37.0` (default_fonts), quire-core(serde), serde, serde_json |

quire `CLAUDE.md` hard rules relevant to a new renderer: quire-core egui/eframe/winit/wgpu/tokio-free, no threads, wasm32; no crate spawns threads (host `Spawner`); `egui =0.37.0` exactly; `unsafe_code = "forbid"`; no dependency on svelters/cadatomic/cadabra; ropey never in public API (everything through `Text`); positions are UTF-8 byte offsets valid with their `Version`; one mutation path `Document::apply(Transaction)`; commands pure; licence MIT OR Apache-2.0 (Zed code GPL: designs only); master stays green; new user-facing commands or behaviour differing from VS Code / ARCH-QUIRE §3 need approval via macbook-cadatomic (the owner session).

### 2.2 quire-text: what it is

The shared **text-editing model** for quire's editor and (planned) svelters' text fields: `Text` (rope behind an API, LF in memory), `ChangeSet` (Helix model, CM6 mapping), multi-range `Selection`, `Transaction` (changes + selection + origin + author + grouping vs `Version`), `TextDocument` (per buffer) and `TextView` (per view: selection, focus, **IME preedit** `Preedit`/`PreeditKind`), `History` (Monaco grouping; per-toolkit native grouping models in `history/native.rs`), `DocRules`/`EditRules`, `EditCommand` (field commands incl. `PreeditUpdate`, `HostRequest`, `InputFilter`), `VisualLines` trait (caller supplies row layout for Up/Down/PageUp/Home/End on wrapped rows; `NoWrap` built in), word rules (VS Code, Pango, Qt), visual columns. **It is a model, not a layout or IME transport**: no glyphs, no geometry, no platform IME; egui-free and wasm-clean. svelters at 1d6656c2 does not depend on it yet (`svelters-egui/src/widgets/entry.rs:4`: "OW4's TextInput unit on TC0 (quire-text) replaces it").

quire-text modules: text.rs 1163, change.rs 851, history/native.rs 824, command.rs 758, history/mod.rs 614, document.rs 605, words/vscode.rs 429, ops/edit.rs 409, selection.rs 336, words/native.rs 296, transaction.rs 281, words/mod.rs 258, view.rs 217, ops/model.rs 207, recent.rs 194, ops/motion.rs 171, ops/mod.rs 159, visual.rs 120, rules.rs 106, columns.rs 93, lib.rs 58, version.rs 26.

### 2.3 quire-core and quire-syntax (egui-free)

quire-core: command.rs 641 (`EditorCommand`, VS Code ids), commands/* 3,527 (edit 1156, comment 629, mod 574, multi 318, indentation 301, lines 211, suggest 175, find 145, fold 85, motion 56, hints 53), completion/* 4,799 (snippet 1412, session 1024, filters 740, insert 702, model 476, mod 445), document.rs 889 (`Document`, one mutation path), hover.rs 892, view.rs 863 (`EditorView`: selection, folds, find, **scroll**, focus, IME), inlay_hints.rs 940, lang.rs 910, semantic_tokens/* 1,425, syntax.rs 750 (`Highlighter` trait, host `Spawner`, `CancelFlag`, packed tokens), parameter_hints.rs 502, folding.rs 440, injected.rs 363 (inlay hints, **IME preedit** as injected text), config.rs 352, find.rs 349, diagnostics.rs 208, large.rs 170, transaction.rs 114, lib.rs 69. (Grep hits for `Response`/`ctx.` in commands/edit.rs etc. are false positives: no egui in the dependency graph.)

quire-syntax: tree.rs 824 (layered incremental parse with injections), driver.rs 321 (`SyntaxDriver`: background parse jobs through the host spawner), highlight.rs 264, dev_tests.rs 273, highlighter.rs 230, config.rs 187, source.rs 148, lang.rs 91, style.rs 83, queries.rs 63, lib.rs 29.

### 2.4 egui_quire modules

| Module | LOC | egui touch | Role |
|---|---|---|---|
| editor.rs | 2802 | 57 | `CodeEditor<'a>` (`new(doc, view, id, a11y_name)`, `.id/.theme/.text/.key_handling/.author/.read_only`, `measure(ctx, avail)`, `show(ui)`, `show_in(ui, rect) -> EditorOutput`), `EditorOutput {response, applied, commands, events, caret_rect, settled, dirty, visible_lines}`, `EditorEvent` (Save/Undo/Redo requested, FocusChanged, HoverAt, SignatureHelpRequested, Completion*, InlayHint*, SemanticTokens*, LinkActivated, ContextMenu, TextSelectionSettled...), `KeyHandling::{Builtin(Keymap), Host}`; per-frame `Mem` in egui memory under the editor id; event loop (Text, Ime, Copy/Cut/Paste, Key, MiddleClickPaste, AccessKit actions), pointer (multi-click, drag select, box select), scroll, focus-lock filter, IME output |
| wrap.rs | 742 | 10 | Soft wrap and long-line prefixes: a line as one or more rows; `WrappedRows` |
| paint.rs | 512 | 9 | Paints background, current line, find matches, selections, text galleys, **preedit**, squiggles, carets, gutter, **own scrollbars** (`ui.interact(track, Sense::drag())`) |
| height.rs | 508 | 0 | `HeightIndex`: per-line heights in chunks with Fenwick trees (virtual scroll) |
| glue.rs | 507 | 4 | Every widget->core call (sync, find, preedit, scroll_px, folds) |
| layout.rs | 450 | 17 | Line layout: one egui `LayoutJob` section per token run, cached galleys, byte<->x mapping, monospace fast path |
| markdown.rs | 434 | 5 | Minimal Markdown for hovers/hints |
| keymap.rs | 434 | 3 | VS Code `keybindings.json` keymap with `when` contexts (`egui::Key`, `Modifiers`, `OperatingSystem`) |
| ascii.rs | 383 | 18 | E1 P2 fast path: builds an ASCII line's `Galley` from glyph templates per subpixel bin (bypasses harfrust; depends on egui's atlas internals) |
| suggest_widget.rs | 358 | 14 | Completion list + details, `Area` Order::Foreground, painter text/galley |
| hover_widget.rs | 262 | 8 | Content hover, `Area` Foreground + `ScrollArea::both` |
| a11y.rs | 252 | 8 | `Role::MultilineTextInput` + `TextRun` children for a page of lines around the caret (50, 500 with a screen reader), following egui's `accesskit_text.rs` |
| widths.rs | 245 | 0 | Longest line tracking (h-scroll extent) |
| theme.rs | 238 | 2 | `EditorTheme` (from `egui::Visuals`), `EditorText` |
| parameter_hints_widget.rs | 218 | 17 | Signature help, `Area` Foreground |
| find_bar.rs | 185 | 5 | Find/replace bar with **egui `TextEdit::singleline`** (two fields) |
| chord.rs | 160 | 1 | Chord notation (`egui::Key`) |
| galley_cache.rs | 117 | 10 | Per-line galley cache, key (content hash, style hash, font, ppp, wrap width), LRU sixteenths |
| command.rs | 69 | 0 | `KeyAction`, `WidgetAction` |
| lib.rs | 44 | 0 | re-exports `egui`, `quire_core` |

egui-free inside egui_quire already: height.rs, widths.rs, command.rs (822 LOC); nearly free: glue.rs, chord.rs, keymap.rs, wrap.rs logic (types `egui::Vec2/Key` only).

### 2.5 How egui_quire renders, and IME

- **Its own virtualised layout over egui galleys:** `HeightIndex` (Fenwick) gives y per line; only `visible_lines` are laid out; each line is one egui `Galley` from a `LayoutJob` (one section per token run, colours baked in) cached in `GalleyCache` keyed by content/style/font/ppp/wrap; ASCII lines skip shaping (`ascii.rs`); wrapped lines are `WrappedRows`; caches drop when egui rebuilds the font atlas (`AtlasState::rebuilt_by`, editor.rs:580). Overlays (selection, carets, squiggles, find matches) are cheap shapes rebuilt every frame (`paint.rs`). No `ScrollArea` for the text: scroll offset lives in `quire_core::EditorView`, wheel consumed from `smooth_scroll_delta`, scrollbars hand-drawn. Popups (suggest, details, hover, parameter hints) are `egui::Area` Order::Foreground; the find bar is a child Ui with two `TextEdit`s.
- **IME:** input `egui::Event::Ime(ImeEvent::{Preedit{text, active_range_chars}, Commit, DeleteSurrounding, Enabled/Disabled})` when `memory.owns_ime_events(id)` (editor.rs:1622-1640, 1930-1972): preedit replaces the selection, stored as the view's preedit (`glue::set_preedit`) and drawn as injected text (quire-core `injected.rs`); commit/delete-surrounding become `composition_cmd` transactions. Output: `ctx.output_mut(|o| o.ime = IMEOutput { purpose: Normal, rect: to_global * text_rect, cursor_rect: to_global * caret, should_interrupt_composition: false })` (editor.rs:768-781). `Widget::composing` in cadatomic's `<code-editor>` kind keeps Enter in the preedit.
- **Repaint scheduling:** caret blink, hover/hint/semantic-token deadlines -> `request_repaint_after`; `EditorOutput.settled` excludes blink (feeds the idle barrier via the kind's `busy`).

### 2.6 What `tessel_quire` needs

1. **Text layout API (tessel-text):** shape a line with per-range styles (colour, underline/squiggle, weight) into glyph runs with cluster->x and x->byte mapping (affinity), line height/ascent, monospace advance probe; rewrap at a width without reshaping; an LRU line cache keyed like `GalleyKey` (the design's two-generation shaped-line cache + "quire's per-line cache ... laying out only the viewport", §6.5). The ASCII fast path becomes a tessel-text concern.
2. **Display-list primitives:** glyph runs, rects (selection, current line, find), carets, wavy underlines, gutter text; a **scroll layer** whose offset changes without re-recording lines (compositor-side scrolling of a clipped content layer, with the editor's own virtualisation by `HeightIndex`).
3. **Overlay/popup layers** anchored to the caret rect with placement (suggest, hover, parameter hints), above other panes, with their own scroll.
4. **Input:** key events with logical+physical key and modifiers (VS Code chords, two-step chords), text input, clipboard (copy/cut/paste, Linux PRIMARY / middle-click paste), pointer with click count/timing, drag capture, wheel (pixel + line deltas).
5. **IME:** platform composition events (preedit text + cursor range, commit, delete-surrounding), "owns IME" per focused node, and an IME composition rect + cursor rect in window coordinates every frame; purpose hint; interrupt composition.
6. **Focus** with a focus-lock filter (Tab/Escape kept while preedit, find bar or popups are open); `consume_key`/shortcut-override ordering with the app keymap.
7. **AccessKit:** MultilineTextInput with TextRun children, text selection, SetValue/ReplaceSelectedText actions, live region announcements.
8. **Timers:** repaint-at deadlines on the host clock; a waker callable from the syntax spawner threads.
9. A plain native text field for the find bar (quire-text-backed) instead of `TextEdit`.

---

## 3. cadatomic

### 3.1 Crates

| Crate | LOC (.rs) | files | GUI deps (Cargo) |
|---|---|---|---|
| cadatomic-app | 89,310 (src 64,261 / tests 23,966 / examples 1,083) | 215 | egui, eframe(wgpu, accesskit, default_fonts, wayland, x11, clipboard), egui-wgpu, wgpu 30.0.1, egui_berth, egui_quire, svelters-egui, svelters-automation, svelters-host; opt egui_inspection, egui_mcp, accesskit_consumer (feature `inspection`); dev egui_kittest (wgpu, eframe) |
| cadatomic-ui | 13,685 | 45 | egui, svelters, svelters-egui, svelters-host, svelters-compiler, svelters-abi, quire-core, quire-syntax, egui_quire. `check-boundaries.sh`: no wgpu/egui-wgpu/eframe, no kernel seam |
| cadatomic-core | 44,292 | 121 | none (cadabra optional `kernel`); boundary-checked: no egui/eframe/egui-*/wgpu/winit/tokio |
| cadatomic-model | 4,467 | 19 | serde, thiserror only; boundary-checked |
| cadatomic-cli | 6,718 | 31 | rmcp, tokio; boundary-checked: no egui/eframe/wgpu/winit (`cadatomic-headless`) |
| cadatomic-ecad | 23,594 | 33 | copperfield, earcutr (no GUI) |
| agent-api/worker, embed-api/worker, link-probe | 1,450 total | | no GUI |

egui:: totals: cadatomic-app/src 1,320 in 79 files (95 of 155 files mention egui in any form, incl. `svelters_egui`, `egui_wgpu`), cadatomic-ui/src 240 in 18 files, tests 652 in 47 files, examples 40.

### 3.2 Per-module table, cadatomic-app/src

Columns: LOC, `egui::` count, touch lines. Grouped by migration category (section 3.3 uses these sums).

**A. Window and app glue** (6,117 LOC; egui:: 90; touch 189)
| File | LOC | egui:: | touch | Notes |
|---|---|---|---|---|
| app.rs | 5878 | 88 | 187 | `ShellApp`: `logic(ctx)` (drain channels, dock logic, embed logic, intents, commands, pumps), `ui(ui)` (shell_regions 5009-5294, central_view 5295, overlays), `impl eframe::App` (5866-5878); 13 svelters_egui |
| main.rs | 90 | 2 | 2 | `eframe::run_native`, `NativeOptions` |
| crash.rs | 106 | 0 | 0 | |
| lib.rs | 43 | 0 | 0 | |

**B. Shell chrome** (5,982 LOC; egui:: 174; touch 386)
| File | LOC | egui:: | touch |
|---|---|---|---|
| shell/chrome.rs | 1097 | 59 | 201 | title bar (`egui::MenuBar`), ribbon, activity rail, panel headers, bottom tab strip, status bar, `ScrollArea`s |
| shell/help.rs | 128 | 9 | 29 | Help and dev-tools windows |
| shell/start.rs | 180 | 4 | 18 | start page |
| shell/mod.rs | 40 | 3 | 7 | |
| shell/layout.rs, ribbon.rs, shutdown.rs, stripes.rs, tour.rs, commands.rs | 562 | 0 | 0 | data/state machines (shutdown uses `ViewportCommand::CancelClose` via app.rs) |
| surfaces.rs | 1295 | 9 | 12 | task panel host, palette, action bar, keymap, context store |
| shortcuts/mod.rs | 471 | 68 | 77 | egui key events -> `KeyStroke` -> layered keymap in a begin-pass hook; `disable_defaults`; dock chords |
| files.rs | 564 | 14 | 23 | file dialogs, recovery overlay (6 svelters_egui) |
| task_panel/mod.rs | 563 | 0 | 3 | session host (panel itself is Svelte) |
| tool_search.rs | 426 | 0 | 0 | |
| bottom.rs | 350 | 0 | 0 | Svelte bottom panel side |
| action_bar/mod.rs | 172 | 8 | 16 | |
| status_line.rs | 134 | 0 | 0 | |
| (app.rs `shell_regions`, ~290 lines, counted in A) | | | | `Panel::top("title_bar")`, `top("ribbon")`, `bottom("status_bar")`, `left("activity_rail")`, `left("sidebar")`, `right("right_stripe")`, `right("properties")`, `bottom("bottom_panel")`, `CentralPanel` |

**C. Dock seam** (1,094 LOC; egui:: 20; touch 36)
| File | LOC | egui:: | touch |
|---|---|---|---|
| dock/seam.rs | 794 | 11 | 25 | the only module naming berth; `Dock` wraps `berth_core::Dock` + `DockUi`; Save/Discard/Cancel `Modal`; `Adapter` implements `egui_berth::PaneHost` |
| dock/mod.rs | 216 | 9 | 11 | `PaneId`, cadatomic `PaneHost` (`ui(&PaneId, &mut egui::Ui)`, `viewport_done(&mut Ui)`), `DockEvent` (carries `egui::ViewportId`), `DockKeys` (`egui::KeyboardShortcut`, `egui::Modifiers`) |
| dock/layout_file.rs | 84 | 0 | 0 | |
(dock/viewport_tabs.rs 166 is counted under F.)

**D. Svelte embedding, app side** (2,649 LOC; egui:: 22; touch 41)
| File | LOC | egui:: | touch |
|---|---|---|---|
| palette/svelte.rs | 417 | 12 | 14 |
| agent/{hub,chat,view,mod} | 1158 | 3 | 7 |
| learn/* | 794 | 0 | 2 |
| tree/svelte.rs | 154 | 0 | 7 |
| tour_player/views.rs | 68 | 6 | 7 |
| views/svelte_view.rs | 58 | 1 | 4 |

**E1. Rust egui panels, general** (5,279 LOC; egui:: 126; touch 494)
| File | LOC | egui:: | touch |
|---|---|---|---|
| tree/mod.rs (model tree, Rust) | 725 | 9 | 56 |
| designer.rs (Designer pane + pick-mode adorners overlay) | 655 | 32 | 99 |
| problems/mod.rs + rows.rs | 799 | 1 | 41 |
| palette/mod.rs (command palette modal) | 568 | 42 | 78 |
| inspector/mod.rs, evidence.rs, measure.rs | 843 | 11 | 140 |
| inspection.rs + inspection/bridge.rs | 338 | 2 | 8 |
| project/pane.rs, tour.rs, mod.rs | 391 | 13 | 45 |
| views/mod.rs, function_keys.rs, inspection_views.rs, pick_link.rs, scene_link.rs | 720 | 16 | 27 |
| view_seam.rs | 240 | 0 | 0 |

**E2. ECAD panes and logic (non-canvas)** (18,703 LOC; egui:: 300; touch 487; 22 of 42 files touch egui, 11,121 LOC in those)
- egui-heavy panes: board/routing.rs 909 (56/56), board/lib_pane.rs 631 (48/89), board/schematic.rs 1697 (34/40), board/outline.rs 455 (26/29), board/erc.rs 365 (20/21), board/library tree library/tree.rs 338 (16/31), board/stack.rs 240 (14/33), board/tour.rs 496 (11/8), board/fab.rs 377 (10/40), board/focus.rs 100 (10/13), board/panels.rs 751 (9/37), library/pins.rs 263 (8/23), footprint/tour.rs 279 (7/6), board/new_design.rs 190 (6/10), board/lists.rs 364 (5/16), board/pending.rs 259 (5/20), board/sch_tour.rs 440 (5/4), library/tour.rs 284 (5/3), board/hub.rs 902 (3/3), library/hub.rs 803 (1/2), footprint/mod.rs 598 (1/2), library/tool.rs 380 (0/1).
- egui-free logic (no migration work): board/route.rs 1205, board/live.rs 606, board/document.rs 372, board/editing.rs 316, board/probe.rs 295, board/sch_flow.rs 226, board/tool.rs 219, board/frame.rs 122, board/waivers.rs 79, board/library.rs 383, board/mod.rs 61, footprint/props.rs 806, footprint/tool.rs 459, footprint/flow.rs 456, footprint/decode.rs 234, library/props.rs 508, library/flow.rs 429, library/document.rs 367, library/part_flow.rs 364, library/mod.rs 75.

**F. 3D viewport** (6,567 LOC; egui:: 84; touch 116; 4,875 LOC are pure wgpu/math with no egui)
| File | LOC | egui:: | wgpu:: | Notes |
|---|---|---|---|---|
| viewport/gpu.rs | 1121 | 0 | 138 | pipelines, shared document buffers, formats (COLOR Rgba8Unorm, DEPTH Depth32Float, ID R32Uint, MSAA 4) |
| viewport/view.rs + view/support.rs | 1025 | 0 | 83 | `ViewGpu` per view: targets, render key, pick pass + readback, screenshot readback |
| viewport/camera.rs | 624 | 0 | 0 | f64 camera |
| viewport/scene_in.rs | 548 | 0 | 0 | |
| viewport/widget.rs | 461 | 13 | 1 | `Viewport::show(ui, &ViewportModel) -> ViewportOutput`; input -> camera intents; adds the paint callback |
| viewport/env.rs | 340 | 0 | 49 | environment/background pipelines |
| viewport/orientation.rs | 289 | 40 | 0 | orientation gizmo painted with egui `Painter` |
| viewport/cpu_raster.rs, pick.rs, math.rs, screen_point.rs, mod.rs | 792 | 0 | 0 | |
| viewport/headless.rs | 178 | 0 | 13 | `HeadlessGpu` (own instance/adapter/device) for tests and tools |
| viewport/theme.rs | 185 | 5 | 2 | |
| viewport/adapter.rs | 131 | 0 | 31 | adapter selection + limits for eframe |
| viewport/composite.rs | 116 | 0 | 35 | blit pipeline into the host pass |
| viewport/callback.rs | 108 | 1 | 6 | `egui_wgpu::CallbackTrait` |
| viewport/layout.rs | 74 | 9 | 0 | `PixelRect` rounding |
| views/viewport_view.rs | 409 | 15 | 0 | the viewport view class (tab) |
| dock/viewport_tabs.rs | 166 | 1 | 0 | viewport panes sharing one upload |

**G. 2D canvases** (7,075 LOC; egui:: 370; touch 521)
| File | LOC | egui:: | touch | Notes |
|---|---|---|---|---|
| canvas2d/gpu.rs | 1951 | 0 | 0 | 250 wgpu::; instance buffers per primitive kind, tiles, flags, layers, scratch texture, motion frames |
| canvas2d/mod.rs | 488 | 1 | 1 | `Canvas2dCallback: CallbackTrait`, `Canvas2dResources`, scene retire/patch, `OffscreenCanvas` |
| board/pane.rs | 738 | 74 | 100 | board canvas pane (navigation, selection, HUD, layer tabs) |
| board/sheet.rs | 994 | 64 | 95 | schematic sheet pane + navigator |
| footprint/pane.rs | 816 | 84 | 106 | footprint canvas |
| library/part.rs | 1182 | 39 | 98 | part pane (two canvas callbacks) |
| library/symbol.rs | 649 | 72 | 90 | symbol canvas |
| board/overlay.rs | 257 | 36 | 31 | hover/selection/rubber-band etc. painted with egui after the canvas |

**H. Code editor surface** (251 LOC; egui:: 4; touch 11): ide/editor_view.rs 102 (egui_quire `CodeEditor::show_in`, sets AccessKit author id, repaint deadlines), ide/source_view.rs 141, ide/mod.rs 8.

**I. Tours and automation** (10,544 LOC; egui:: 130; touch 184)
| File | LOC | egui:: | touch |
|---|---|---|---|
| tour/runner.rs | 1827 | 10 | 8 |
| tour/registry.rs | 1761 | 0 | 13 |
| tour_player/player.rs | 1231 | 1 | 1 |
| tour/generic/overlay.rs (ring, callout, keycaps, spotlight; layer painters) | 787 | 25 | 36 |
| tour/model.rs | 752 | 9 | 9 |
| tour_player/tours.rs | 718 | 1 | 4 |
| pixel.rs (feature `inspection`) | 462 | 33 | 58 |
| tour/flow.rs | 397 | 0 | 0 |
| tour_player/verdicts.rs | 329 | 0 | 0 |
| tour/generic/ghost.rs (ghost cursor) | 314 | 4 | 4 |
| tour/generic/input.rs (synthetic egui::Events) | 259 | 9 | 8 |
| tour/live.rs (egui Plugin: input/output hooks, AccessKit nodes) | 236 | 23 | 26 |
| tour/coverage.rs | 235 | 0 | 0 |
| waits.rs | 205 | 0 | 0 |
| tour/area.rs | 201 | 4 | 4 |
| tour_player/search.rs, words.rs, commands.rs, mod.rs | 414 | 0 | 3 |
| tour/mod.rs, commands.rs, dialogs.rs | 307 | 4 | 4 |
| tour/generic/mod.rs | 109 | 7 | 6 |

### 3.3 Per-module table, cadatomic-ui/src

| Category | Files | LOC | egui:: | touch | svelters_egui |
|---|---|---|---|---|---|
| S. Svelte embed host + exports modules (app_embed 509, panels 247, live 312, designer 535, svelte/{model 1252, panes 601, dx 476, tour_player 487, problems 356, panel 335, exports 324, palette 320, tree 308, pending 216, mod 230, lists 190, learn 184, agent 157, pcb 168, output 137, jobs 115, tours 89, source 43}) | 23 | 7,591 | 32 | 52 | 20 |
| K1. Rust kinds and behaviours for svelters (property_grid 955, pick 449 [`PickOverlay: ScenePass`, `Presents: Behavior`], quantity_field 303, empty_state 158, status 112 [`status-text`], commands/behavior.rs 103 [`use:command`], op/behavior.rs 97 [`OpMark`, `LiveRegion`]) | 7 | 2,177 | 148 | 156 | 20 |
| K2. `<code-editor>` kind (code_editor/core 770, mod 416, host 339, tests 173, spawn 132) | 5 | 1,830 | 22 | 27 | 3 |
| T. Theme and icons (theme/mod.rs 394, theme/icons.rs 107) | 2 | 501 | 33 | 48 | 2 |
| N. Toolkit-neutral (i18n 330, gallery 317, text 224, op/mod 340, commands/mod 136, commands/exports 138, op/exports 69, lib 32) | 8 | 1,586 | 5 | 6 | 5 |

Rust kinds registered with `#[svelters::widget]`: `cad:empty-state`, `cad:property-grid` (PREPARE, CUSTOM_LAYOUT, VIRTUAL, FOCUSABLE), `cad:status-text`, `cad:code-editor` (FOCUSABLE, EDITABLE), `cad:quantity-field` (FOCUSABLE, EDITABLE); behaviours `Command`, `OpMark`, `LiveRegion`, `Presents`; one `ScenePass` (`PickOverlay`). (The doc's "cadatomic's 8" counts slightly differently: 5 widgets + 4 behaviours + 1 pass here.)

Svelte panels as data (`cadatomic-ui/src/panels.rs`): 18 `PanelSpec`s: `panel.task_svelte`, `panel.problems`, `panel.jobs`, `panel.output`, `status.prompt`, `status.selection`, `status.units`, `overlay.palette`, `panel.tree`, `ecad.layers`, `ecad.pending`, `ecad.properties`, `ecad.nets`, `ecad.components`, `panel.tours`, `panel.tour_player`, `agent.chat`, `panel.learn`. Sources: `ui/` 22 `.svelte` panels, 17,387 LOC of `.svelte`+`.ts` (incl. 96 `*.tour.ts`, 1 `*.lesson.ts`).

### 3.4 Category detail: minimal toolkit API, and whether it can sit in an egui island

| Category | LOC (app+ui) | egui:: | Minimal tessel API it needs | Island unchanged? |
|---|---|---|---|---|
| Shell chrome (B + app.rs shell_regions) | ~6,270 | 174 (+~40 in app.rs) | Edge panels with resizable splitters (or berth regions), menu bar + nested menus/popovers, toolbar buttons with icon+label+tooltip, status-bar sections, start page list, modal dialogs (unsaved changes, file recovery), keyboard routing hook *before* widgets (shortcuts begin-pass: `KeyStroke` resolution in Rust, consume matched keys), text-focus query (`text_edit_focused` equivalent), close-request interception (`CancelClose`) | Technically yes (it is all egui today), but the doc's step 1 moves it first, natively; it is also what makes the islands composable (chrome sits outside them). `ShellApp::ui` lays out with `egui::Panel` around a `CentralPanel` holding the dock; in island mode the whole of `ui()` would be one island = no gain. |
| Dock seam (C) | 1,094 + berth 5,190 | 20 | `tessel_berth` (section 1.6); `PaneHost` re-typed to host a pane slot node instead of `&mut egui::Ui`; `DockEvent`/`DockKeys`/`pane_rect`/`open_floating` re-typed off `egui::ViewportId/Rect/KeyboardShortcut` | No: it is the frame that hosts the islands. But it can be switched pane-by-pane: a tessel dock whose pane slots are `EguiIsland`s runs every existing `PaneHost::ui` body unchanged. |
| Svelte embedding (D + S + K1 + K2) | ~14,250 (app 2,649; ui 11,598) | ~224 | `svelters-tessel` backend ABI (create, insert, set_prop, listen, call_sync) on the tessel tree; `AppEmbed` slots become tessel subtrees mounted into dock pane slots / fixed slots (`panel.task`, `status.*`, `overlay.palette`); per-window root passes (`show_viewport_passes`, `set_viewport`); busy probe; the 5 Rust kinds re-hosted as tessel `Kind`s (measure/record/event/a11y), behaviours as native behaviour units, `PickOverlay` as an overlay layer pass | Yes as a whole (they render through svelters-egui into an egui Ui today), but per the doc they move together with the backend. Within a tessel Svelte panel, an egui kind (`<code-editor>`, `<property-grid>`) could only survive via a node-level island (an `EguiIsland` leaf); recommended only as a stopgap for `property-grid`. |
| Rust egui panels (E1 + E2 non-canvas) | ~24,000 (of which ~15,300 LOC in egui-touching files) | 426 | Per panel: list/tree rows (virtual list), labels, buttons, checkboxes, combo boxes, text fields (quire-text), scroll areas, collapsing headers, tooltips, context menus, modal/palette overlay, drag in lists; most use `egui::Grid`/`ScrollArea`/`ui.horizontal`. The inspector, palette and library are the doc's first ports. | Yes: each pane body is a `PaneHost::ui(id, &mut egui::Ui)` call today; an island per pane (or one island context shared by all island panes) runs them unchanged. Caveats: designer.rs and palette/mod.rs paint context-wide overlays (`ctx.layer_painter`, `Area`); problems/tree link selection with the viewport; tour anchors read their AccessKit ids. |
| 2D canvases (G) | 7,075 | 370 | A `Surface`/external-texture primitive (a texture the app renders into on the shared wgpu device, composited by rect, with a per-frame "prepare" hook that gets `&Device, &Queue`), an overlay layer above it for hover/selection/rubber band (today `board/overlay.rs` with egui `Painter`), pointer history (coalesced samples) and capture for drags/routing, wheel/pinch zoom, key focus ownership (`board.focus`/`sheet.focus`). Later: native canvas layers (§6.6). | Yes: `egui_wgpu::Callback` keeps working inside an island because the island renders through egui-wgpu on *our* device; the canvas renders to its own target in `prepare` and blits in `paint`. Cost: an extra copy (canvas texture -> island texture -> window). Better: lift the canvas out to a tessel `Surface` early since its seam is already "render to my texture, then blit". |
| 3D viewport (F) | 6,567 | 84 | Same `Surface` primitive + device access; later an OS compositor layer with its own present (§6.6) and a texture fallback; raw-input/pointer capture while orbiting; the orientation gizmo (egui Painter, 289 LOC) as overlay primitives; screenshot of the window (pixel tier) and of the target (already its own). | Yes, same as canvases. |
| Code editor surface (H + K2 + egui_quire) | 251 + 1,830 + quire 8,920 | 26 + quire | `tessel_quire` (section 2.6); text layout and IME are M2 gates | Yes: `EditorView::ui` is an egui pane body; `<code-editor>` lives inside a Svelte panel and moves with the Svelte backend (needs `tessel_quire` by then, or a node-level island). |
| Theme (T) | 501 | 33 | Theme tokens consumed by tessel styling (DS1 `Theme` already lives in svelters); font registration (Inter regular/semibold, symbols, Phosphor icon font as last fallback family and an `icons` family); text styles; zoom factor | Must do both during migration: `theme::apply` keeps filling egui `Style` for islands while also feeding tessel. |
| Icons | theme/icons.rs 107; `PaneHost::paint_icon` | | Glyph-from-font rendering (Phosphor 2.1.0 codepoints) now; the `ui/icons` Python kit (`tools/iconkit`) is an experiment with no runtime use in Rust yet (no Rust source reads `ui/icons`); colour icons later need SVG/colour-glyph raster to the atlas | Island: unchanged (egui font). |
| Tours and automation (I) | 10,544 | 130 | See section 4.3 | The runner and model are toolkit-neutral except input synthesis (`egui::Event`), node discovery (AccessKit `TreeUpdate` from egui output) and overlay painting. |

### 3.5 The 3D viewport callback (`crates/cadatomic-app/src/viewport/callback.rs`)

- `install(render_state)`: once at startup (eframe `cc.wgpu_render_state`, kittest `create_render_state`), inserts `ViewportResources { renderer: Renderer::new(device, target_format), docs: GpuDocuments, views: HashMap<ViewId, ViewGpu> }` into egui-wgpu's `renderer.write().callback_resources` (type-map). `forget_document`/`forget_view` remove entries through the same `RenderState`.
- `ViewportCallback { id: ViewId, handle: ViewportHandle, frame: RenderFrame }`, added each frame by `Viewport::show` with `ui.painter().add(egui_wgpu::Callback::new_paint_callback(rect, ViewportCallback{..}))` (`widget.rs:448`).
- **`prepare(device, queue, screen, egui_encoder, resources) -> Vec<CommandBuffer>`**: gets or creates the view's `ViewGpu` and calls `ViewGpu::prepare(device, queue, renderer, docs, frame)` (`view.rs:500-760`), which: (1) collects finished pick/screenshot readbacks (polls the device only while something is in flight; blocks only if `block_on_readback`); (2) brings the shared `GpuDocuments` for the `DocId` up to date (range patches on a new scene revision); (3) `ensure_targets`: colour `Rgba8Unorm` (RENDER_ATTACHMENT | TEXTURE_BINDING | COPY_SRC), MSAA colour (4 samples), MSAA depth `Depth32Float`, and a composite bind group over the colour view; (4) if the render key changed, writes uniforms/highlight flags and records the colour pass (+ environment pass) into **its own** encoder; (5) if a cursor pixel is given and pick inputs changed, renders the ID pass into a `PICK_SIDE`^2 `R32Uint` id texture + depth and copies it to one of two `MAP_READ` readback buffers (one-frame-late pick); (6) optionally copies the colour target into a screenshot buffer; (7) `queue.submit` once only if anything was recorded, then `map_async` the readback buffers. It returns **no** command buffers to egui and never uses `egui_encoder`. A UI-only frame submits nothing (stats track idle prepare cost).
- **`paint(info, pass, resources)`**: looks up the view's composite bind group and draws one full-rect blit (`Composite::draw`, `composite.rs`) into egui's render pass at `PixelRect::from_points(info.viewport, ppp)`. The blit pipeline is built for egui's `target_format` with no depth and 1 sample.
- Owned GPU state: per view colour/MSAA/depth targets, pick id/depth textures and 2 readback buffers, screenshot buffer, uniforms; shared per document: vertex/index/flag buffers and `Pipelines`; global: `Renderer` (pipelines, env pipelines, composite pipeline/sampler).
- canvas2d (`canvas2d/mod.rs:243-322`) uses the identical seam: `Canvas2dCallback{view, scene_id, scene: Arc<dyn TiledScene>, frame, key, flags, patch}`; `prepare` uploads or patches the scene (`SceneGpu`), applies queued forgets/retires (a static `FORGET` mutex, since the app has no render state), writes flags, `CanvasTarget::render` (colour + scratch textures, motion frames); `paint` draws the motion blit or the composite blit. Used by board/pane.rs:266, board/sheet.rs:266, footprint/pane.rs:334, library/symbol.rs:210, library/part.rs:741,811.

### 3.6 Code editor surface (`crates/cadatomic-app/src/ide/editor_view.rs`, 102 LOC)

Rust pane body over `egui_quire::CodeEditor` for host-owned documents (`DocRef`, shared with `<code-editor doc=...>`): `ui(&mut egui::Ui, salt)` updates the syntax driver (waker = `ctx.request_repaint`), applies host selection, `CodeEditor::new(...).id(id).show_in(ui, rect)`, sets the AccessKit author id (`ide.source/<path>`) via `ctx.accesskit_node_builder`, notes output on the binding, schedules repaints for syntax deadlines and the undo-group pause. egui touch is tiny; the real work is `tessel_quire`.

---

## 4. Tests and automation

### 4.1 egui_kittest test files (cadatomic-app)

45 of 57 files in `crates/cadatomic-app/tests` import egui_kittest (the 12 without: autosave, board_canvas, board_save, canvas_motion, canvas_partial, canvas_patch, palette_metadata, pixel, tool_search, view_seam, viewport_a5c, viewport_fit; several of these drive `HeadlessGpu`/`OffscreenCanvas` directly). Plus `examples/e4_frame_budget.rs` (Harness). Counts per file (Harness = lines naming `Harness`; run = `.run/.run_ok/.run_steps/.run_until*`; get/query = `.get_by*`/`.query_by*`; key = `key_press*`/`key_down`; render = `.render/.snapshot`; event = `.event(`/`input_mut`):

| File | LOC | Harness | run | step | get_by | query_by | click | key | type_text | hover/drag | render | event |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| agent_chat.rs | 140 | 4 | 4 | 1 | 4 | 5 | 3 | 0 | 1 | 0 | 1 | 0 |
| board_content.rs | 723 | 12 | 27 | 2 | 1 | 6 | 0 | 9 | 0 | 0 | 1 | 4 |
| board_drc.rs | 265 | 3 | 8 | 2 | 2 | 4 | 2 | 0 | 0 | 0 | 1 | 0 |
| board_edit.rs | 640 | 12 | 19 | 6 | 2 | 0 | 0 | 7 | 0 | 0 | 1 | 3 |
| board_live.rs | 403 | 6 | 9 | 2 | 1 | 0 | 0 | 0 | 0 | 0 | 4 | 2 |
| board_route.rs | 741 | 13 | 8 | 4 | 2 | 1 | 1 | 1 | 0 | 0 | 1 | 2 |
| board_view.rs | 451 | 5 | 29 | 3 | 3 | 4 | 1 | 5 | 0 | 1 | 1 | 9 |
| board_waivers.rs | 387 | 7 | 8 | 2 | 1 | 6 | 3 | 1 | 1 | 0 | 1 | 0 |
| bottom_panels.rs | 485 | 7 | 11 | 9 | 5 | 5 | 7 | 2 | 2 | 0 | 0 | 2 |
| code_editor.rs | 746 | 16 | 2 | 11 | 1 | 2 | 1 | 11 | 10 | 0 | 0 | 6 |
| cross_probe.rs | 509 | 2 | 7 | 1 | 12 | 2 | 7 | 0 | 0 | 0 | 1 | 5 |
| designer.rs | 499 | 9 | 26 | 0 | 2 | 1 | 1 | 4 | 0 | 0 | 1 | 6 |
| docking.rs | 911 | 9 | 31 | 6 | 29 | 6 | 13 | 0 | 1 | 0 | 1 | 7 |
| files.rs | 346 | 6 | 4 | 2 | 2 | 0 | 1 | 1 | 0 | 0 | 1 | 0 |
| footprint_editor.rs | 244 | 2 | 5 | 2 | 7 | 0 | 6 | 0 | 1 | 0 | 2 | 2 |
| gallery.rs | 566 | 17 | 1 | 5 | 1 | 0 | 1 | 7 | 0 | 3 | 0 | 5 |
| i18n.rs | 151 | 2 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| inspection.rs | 645 | 3 | 15 | 0 | 11 | 13 | 1 | 0 | 0 | 0 | 2 | 0 |
| learn.rs | 369 | 6 | 0 | 7 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 2 |
| library.rs | 287 | 2 | 3 | 2 | 8 | 0 | 5 | 1 | 3 | 0 | 1 | 0 |
| library_pane.rs | 468 | 2 | 7 | 2 | 4 | 3 | 3 | 0 | 0 | 0 | 2 | 4 |
| live_edit.rs | 406 | 7 | 4 | 2 | 4 | 6 | 4 | 3 | 2 | 0 | 0 | 0 |
| measure.rs | 436 | 2 | 13 | 3 | 0 | 3 | 5 | 0 | 0 | 2 | 5 | 0 |
| new_design.rs | 351 | 2 | 5 | 2 | 8 | 0 | 7 | 0 | 0 | 1 | 1 | 3 |
| op_states.rs | 382 | 9 | 0 | 7 | 3 | 0 | 0 | 0 | 0 | 1 | 1 | 0 |
| palette_tree.rs | 568 | 13 | 6 | 3 | 7 | 5 | 8 | 6 | 2 | 0 | 2 | 3 |
| part_editor.rs | 145 | 3 | 1 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 2 | 0 |
| picking.rs | 357 | 4 | 7 | 0 | 4 | 4 | 5 | 0 | 0 | 1 | 0 | 0 |
| project_workspace.rs | 119 | 3 | 1 | 2 | 0 | 3 | 0 | 0 | 0 | 0 | 1 | 0 |
| regions.rs | 288 | 8 | 2 | 5 | 3 | 5 | 4 | 0 | 0 | 0 | 0 | 2 |
| scene_m1.rs | 467 | 2 | 8 | 2 | 0 | 1 | 0 | 0 | 0 | 1 | 9 | 0 |
| schematic_edit.rs | 771 | 2 | 13 | 3 | 6 | 0 | 4 | 0 | 0 | 0 | 1 | 7 |
| schematic_view.rs | 252 | 3 | 7 | 1 | 3 | 5 | 2 | 0 | 0 | 0 | 2 | 2 |
| shell.rs | 371 | 7 | 32 | 0 | 20 | 27 | 15 | 3 | 0 | 0 | 0 | 0 |
| surfaces.rs | 1194 | 8 | 91 | 0 | 9 | 14 | 10 | 36 | 2 | 0 | 0 | 0 |
| svelte_ui.rs | 211 | 2 | 7 | 0 | 2 | 3 | 3 | 1 | 1 | 0 | 0 | 0 |
| t2_settle.rs | 204 | 6 | 1 | 2 | 2 | 0 | 1 | 0 | 1 | 0 | 0 | 0 |
| task_forms.rs | 885 | 18 | 9 | 0 | 17 | 13 | 22 | 13 | 2 | 0 | 1 | 3 |
| tour_player.rs | 535 | 9 | 1 | 4 | 4 | 0 | 2 | 0 | 1 | 0 | 0 | 3 |
| tours.rs | 1295 | 3 | 0 | 8 | 0 | 0 | 0 | 0 | 0 | 0 | 2 | 1 |
| ui_missing.rs | 82 | 2 | 0 | 1 | 5 | 2 | 2 | 0 | 1 | 0 | 0 | 0 |
| viewport.rs | 873 | 1 | 2 | 0 | 2 | 0 | 0 | 0 | 0 | 0 | 2 | 0 |
| viewport_ui.rs | 502 | 7 | 6 | 11 | 0 | 0 | 0 | 0 | 0 | 1 | 2 | 6 |
| workspace_ecad.rs | 280 | 3 | 5 | 2 | 1 | 0 | 1 | 0 | 0 | 0 | 1 | 0 |
| zoom.rs | 265 | 5 | 14 | 0 | 4 | 3 | 4 | 1 | 0 | 0 | 0 | 0 |
| examples/e4_frame_budget.rs | 803 | 2 | 1 | 2 | 1 | 0 | 0 | 0 | 0 | 0 | 4 | 12 |

Harness API frequency across `tests/*.rs` (receiver `h.`/`app.`): `state` 651+8, `run` 323(+4), `step` 126, `state_mut` 124+5, `get_by_label` 96, `event` 89, `run_steps` 82, `key_press` 71, `get_by_role_and_label` 67+13, `query_by_label` 47, `key_press_modifiers` 41, `query_all_by_label` 38, `display` 18, `hover_at` 15, `check_accessibility` 15, `render` 13 (+37 `.render(` overall), `query_by_label_contains` 12, `get_by_label_contains` 11, `query_by_role_and_label` 8, `query_all_by_role` 8(+2), `remove_cursor` 6, `query_all_by_label_contains` 6, `get_by` 6, `drop_at` 5, `drag_at` 5, `query_all_by_value` 5, `event_modifiers` 3, `root` 3, `set_size` 2, `run_ok` 2. Node methods: `.click()` 100, `.type_text()` 20, `.rect()` 20, `.focus()` 19, `.label()` 16, `.accesskit_node()` 7, `.value()` 5, `.is_disabled()` 5, `.hover()` 3, `.children_recursive()` 2. Builder: `with_size` 49, `with_max_steps` 31, `renderer(WgpuTestRenderer::from_render_state(rs))` 21, `with_step_dt` 6, `with_pixels_per_point` 1, `build_ui_state(|ui, app| { app.logic(ctx); app.ui(ui) }, app)`; GPU setup `egui_kittest::wgpu::{create_render_state(default_wgpu_setup(), RendererOptions::PREDICTABLE)}` then `viewport::install(&rs)` and `canvas2d::install(&rs)`; `h.input().predicted_dt`. Snapshots (`.snapshot(`) in inspection, measure, op_states, palette_tree, viewport; PNG goldens in `tests/snapshots/` (a5a-box-cylinder, eu-next-sheet, eu-next-silk, eu1-board). `CADATOMIC_SHOTS` screenshot helpers (`shot`) in agent_chat.rs, designer.rs, footprint_editor.rs, tours.rs.

berth and quire kittest suites (to port alongside their renderers): egui_berth/tests 15 files, 5,690 LOC (deferred.rs and native.rs drive eframe-style viewports without Harness); egui_quire/tests 9 files, 4,270 LOC (editor.rs 2,031 LOC: 25 Harness, 161 run/step, 150 event injections - IME, keys, text).

### 4.2 Scenarios

- `scenarios/*.ndjson`: 10 files (a8a-smoke, board-edit, board-push-route, board-route, file-roundtrip, m1-prep-scene, mcad-measure, schematic-e5, wait-idle-job) + `scenarios/golden/{a8a-smoke,file-roundtrip}.tree.json`. Step kinds used: command 52, wait_idle 12, start 9, commands_list 7, tree 6, fixture 4, jobs 3. **No UI steps** ("Steps that need the selector ladder ... arrive with the `ui.*` methods", `cadatomic-cli/src/scenario.rs`). Header names pins and environment (dpi, ui_scale, locale, theme, os, build).
- Runner: `crates/cadatomic-cli/src/scenario.rs` (597), drive layer `drive.rs` (222): `InProcessDrive` (core `Dispatcher` directly) and `RpcDrive` (socket to a running app). Tests: `crates/cadatomic-cli/tests/scenarios.rs` (421) - note CLAUDE.md says `cargo test -p cadatomic-app --test scenarios`, but the file lives in cadatomic-cli and uses `cadatomic_cli::drive` with core `IdleBarrier`/`Jobs`. Live: `scripts/xrun.sh cadatomic-headless scenario run --live`.
- Toolkit impact: none today (command level + wait_idle). They port for free as long as the barrier semantics hold.

### 4.3 Tours

- Sources: `ui/tours/**` 96 `*.tour.ts`, 1 `*.lesson.ts` (101 .ts incl. concepts/courses), built with `ui/lib/tour/index` (`click(anchor)`, `invoke(command, {via: ['palette']})`, `expect`, `board.is(...)`); `tours/coverage.toml` (ratchet not enforced).
- Rust runtime: `tour/registry.rs` (loads modules reported by the Svelte loader `svelters:app/tours`), `tour/model.rs`, `tour/runner.rs` (`TourRun`, `run_to_end`, locators: author id, role+label, position; settle = 3 idle polls), `tour/generic/*` (generic half meant to move to `svelters-tour`): `Driver` trait = `nodes() -> Vec<UiNode{role,label,value,author_id,focused,disabled,rect}>`, `ctx() -> egui::Context`, `screen()`, `take_user_input()`, plus `svelters_automation::Driver` (`scene()`, `busy()`, `step()`, `send(egui::Event)`, `frame_ms()`); `AppDriver: Driver` adds `app()`, `app_mut()`, `restart()`.
- Drivers: `tour/live.rs` `LiveDriver` = an `egui::Plugin` whose input hook appends synthetic `egui::Event`s after real ones (and detects real user input) and whose output hook keeps the AccessKit `TreeUpdate` nodes (with MultilineTextInput text from TextRuns); kittest driver `Kittest` in `tests/tours.rs:113-187` (nodes from `h.root().children_recursive()`, `h.event`, `h.step`).
- Overlay: `tour/generic/overlay.rs` (ring, callout, keycaps, spotlight; context layer painters), `ghost.rs` (ghost cursor, eased path, ripple), `input.rs` (click/chord/typing/drag frames as `egui::Event` PointerMoved/PointerButton/Key/Text).

### 4.4 egui_mcp, egui_inspection, pixel tier

- `crates/cadatomic-app/src/pixel.rs` (462, feature `inspection`, refused in release, plus env `EGUI_INSPECTION=1`): registers `egui_inspection::InspectionPlugin` on the context (no TCP listener); `PixelTier::call(method, params)` serves RPC methods `ui.tree` (AccessKit `TreeUpdate` -> `accesskit_consumer::Tree` -> `egui_mcp::tree::{Locator, Query, QueryFilter}` projection), `ui.click`, `ui.key`, `ui.type`, `ui.drag` (`Request::ApplyEvents { events: Vec<egui::Event> }` with selectors resolved to window positions), `ui.inspect` (raw `egui_inspection` request: GetInfo, GetTree, Settle{max_steps}), `screenshot.window` (plugin PNG), `screenshot.viewport` (`ViewportHandle::request_screenshot` readback). Errors `ui.selector_no_match/_ambiguous/_no_bounds`, `ui.frame_timeout`, `ui.no_tree`.
- `crates/cadatomic-cli/src/pixel.rs` (380): toolkit-neutral `PixelHost` seam and method names (`METHOD_UI_*`, `METHOD_SCREENSHOT_*`); `cadatomic-cli/src/mcp/{server,tools,call}.rs` (931) expose them as MCP tools (`cadatomic mcp` sidecar over stdio; `rmcp =3.5.0`); one tool forwards a raw egui_inspection request (`tools.rs:295`). Tests: `cadatomic-cli/tests/{mcp,pixel_rpc}.rs`, `cadatomic-app/tests/pixel.rs`.

### 4.5 wait-idle / IdleBarrier

- `crates/cadatomic-core/src/jobs/barrier.rs` (447): `IdleBarrier::new(jobs, required_settled_frames)` (`DEFAULT_SETTLED_FRAMES = 3`); idle = no job open or unpublished, every foreground `BusySource` quiet, no repaint requested, and N consecutive settled frames. `note_frame(repaint_requested)` at the end of every frame; `note_repaint_requested()`; `set_frame_waker(f)` (how `wait_idle` asks for a frame when frames run on demand); `wait_idle(timeout) -> Result<IdleReport, IdleTimeout>`; `report()` lists busy sources longest-first. `BusySource { id, SourceKind::{Foreground,Background}, SourceClock::{Wall,Virtual}, probe }`.
- App wiring (`crates/cadatomic-app/src/app.rs`): barrier at 690; sources `source-tabs`, `pool.jobs`, `pool.syntax` (quire syntax pool), `ecad.lib.jobs`, `ecad.jobs`, `agent.chats` (694-756), RPC calls in flight (1264), `embed` (svelters busy probe, 4187); frame waker `request_repaint_after(1ms)` (4024: "a zero-delay request_repaint from outside a frame runs two frames"); `note_frame(ui.ctx().has_requested_repaint())` at the end of `ui()` (4451; `note_frame(true)` on the theme-not-ready first frame, 4427); cosmetic wakes (op marks, tour pacing) requested *after* `note_frame` so they never hold the barrier. `waits.rs` mirrors every source into svelters' `BusyRegistry` and keeps one `Journal`. The dock's `is_settled()` (berth drag in flight) is read by `tour_busy` (app.rs:1796) but is not a barrier source.
- Transports: `cadatomic-cli/src/wait.rs` (`wait_idle`, `wait_idle_pumped` for in-process drives; `stub-wait-idle` mutation feature, compile error in optimised builds).
- Coupling to egui: only `has_requested_repaint()` and the `request_repaint_after` waker. tessel must provide "is another frame scheduled" (any dirty layer, running animation, pending timer) and a cross-thread frame request; idle animations of the compositor must count as cosmetic, not as repaint.

### 4.6 Headless and xrun

- `scripts/xrun.sh` (25): `xvfb-run` with the pinned lavapipe (Mesa 26.1.3, `VK_DRIVER_FILES`); the pinned build has no WSI, so it serves offscreen rendering only; `CADATOMIC_XRUN_SYSTEM_GPU=1` for a windowed app. `scripts/setup-lavapipe.sh` installs it. WSL: `WGPU_BACKEND=vulkan` (GL backend crashes).
- `cadatomic-headless` = the `cadatomic-cli` binary (`batch`, `serve` = the real dispatcher on a frame thread behind the RPC server, `mcp`, `scenario`); no GUI stack.
- `viewport::headless::HeadlessGpu` / `canvas2d::OffscreenCanvas`: own wgpu instance/adapter/device (`Backends::from_env`), drive the same `ViewGpu::prepare`/`CanvasTarget::render` without egui.

### 4.7 What `tessel-test::Harness` must provide for a mechanical port

1. Builder: size, pixels-per-point, max steps, fixed step dt (virtual clock), a renderer built from an existing render state (shared device so `viewport::install`/`canvas2d::install` equivalents can register their resources), and a state-owning constructor taking a closure `|cx, app| { app.logic(..); app.ui(..) }` or the app type directly.
2. `run()` (until no repaint requested, bounded by max steps, panicking on overrun), `run_ok()`, `run_steps(n)`, `step()`, `state()`, `state_mut()`, `input()` (predicted dt), `set_size`.
3. Queries over the **AccessKit** tree with the same semantics and names: `get_by_label`, `get_by_label_contains`, `get_by_role_and_label`, `get_by(predicate)`, `get_all_by_role_and_label`, `query_by_label`, `query_by_label_contains`, `query_by_role_and_label`, `query_all_by_label(_contains)`, `query_all_by_role(_and_label)`, `query_all_by_value`, `root()`, `children_recursive()`; node `click()`, `focus()`, `type_text()`, `hover()`, `rect()`, `label()`, `value()`, `is_disabled()`, `accesskit_node()`.
4. Input injection: `event(Event)`, `event_modifiers`, `key_press(Key)`, `key_press_modifiers(Modifiers, Key)`, `hover_at(pos)`, `drag_at`/`drop_at`, `remove_cursor`; IME events for quire. Ideally an adapter that accepts `egui::Event`-shaped values (tour `input.rs`, pixel tier, e4 example) so call sites change only imports.
5. `render()` -> RGBA image (wgpu on lavapipe/WARP, deterministic text), `snapshot(name)` with the existing PNG compare, `check_accessibility()`.
6. Automation parity: a plugin-equivalent hook to (a) append synthetic events after real ones at the event-time dispatch point and detect real input, (b) read each frame's AccessKit tree with bounds and author ids (tour live driver, pixel tier `ui.tree`), (c) window screenshots on demand (pixel tier), (d) `Settle{max_steps}`.
7. Barrier parity: `has_requested_repaint()` equivalent after each frame and a frame waker.

---

## 5. Window, event loop, viewports, wgpu device

- **Window/event loop:** `crates/cadatomic-app/src/main.rs:39-90`: `eframe::NativeOptions { viewport: ViewportBuilder::default().with_title("cadatomic").with_inner_size([1440, 900]).with_min_inner_size([960, 600]), wgpu_options: viewport::adapter::wgpu_options(), ..Default }`; `eframe::run_native("cadatomic", options, creator)`. The creator installs `viewport::install(rs)` and `canvas2d::install(rs)` from `cc.wgpu_render_state`, checks the Linux primary selection (`svelters_egui::primary::use_system`), builds `ShellApp`, handles `--tour <id>`, `attach_rpc(runtime_dir, seed, &cc.egui_ctx)`, `attach_document(seed, &cc.egui_ctx)`. Non-window modes (`mcp`, `batch`) exit before eframe. `impl eframe::App for ShellApp` delegates `logic`/`ui`/`on_exit` (app.rs:5866-5878); `eframe::Frame` is unused. No present-mode/latency options are set (eframe defaults); the doc's M0 items (a) `SurfaceConfig::LOW_LATENCY` and (b) DX12-first are not in `adapter.rs` yet.
- **Multi-viewport:** cadatomic itself never calls `show_viewport_*` and never sets `ViewportMode::Deferred` or `embed_viewports`; all secondary windows come from berth's default **immediate** viewports (`egui_berth/src/windows.rs:431-438`; under kittest they are embedded `Area`s). Dock tear-off = berth `NewWindow`/drag to empty space; the Svelte embed follows panes into viewports (`AppEmbed::set_viewport` on `DockEvent::Shown/MovedToViewport`, `svelters_egui::embed::show_viewport_passes(ui)` in `ShellPanes::viewport_done`, app.rs:174-177). `ViewportId` appears in `dock/mod.rs` (`DockEvent`), `views/pick_link.rs` (placed rect per viewport), app.rs (root filters at 1756/1770, `CancelClose`/`Close` at 3983/4001/4411). Deferred-viewport workarounds live in berth (`deferred.rs`) and svelters (`SlotPainter`, ARCH-EMBED §5.2), not in cadatomic.
- **wgpu device/adapter** (`crates/cadatomic-app/src/viewport/adapter.rs`): `wgpu_options()` returns `egui_wgpu::WgpuConfiguration` with `WgpuSetupCreateNew::without_display_handle()`, `native_adapter_selector = select` (drops Noop and adapters without fragment storage buffers; prefers Vulkan/Metal/DX12 over GL, then discrete > integrated > virtual > other > CPU) and `device_descriptor` with `required_limits` (wgpu defaults, or the adapter's own limits on GL; `max_texture_dimension_2d` 8192). eframe creates the instance/adapter/device/queue and surface.
- **Sharing with egui-wgpu:** the 3D viewport and canvas2d never create a device in the app; they receive egui-wgpu's `RenderState` (`device`, `queue`, `target_format`, `renderer: Arc<RwLock<egui_wgpu::Renderer>>`), store their resources in `renderer.callback_resources`, and run inside egui-wgpu's frame through paint callbacks (`prepare` submits its own encoder on the shared queue; `paint` blits into egui's pass, which must be 1-sample, no depth, in `target_format`). Tests: `egui_kittest::wgpu::create_render_state(default_wgpu_setup(), RendererOptions::PREDICTABLE)` + `WgpuTestRenderer::from_render_state`. Only `HeadlessGpu` makes its own device.
- **For tessel/EguiIsland:** tessel owns instance/adapter/device (port `adapter::select`/`required_limits` into tessel-platform setup); the island's egui-wgpu `Renderer` must be created on tessel's device so existing `CallbackTrait`s keep working inside islands; viewport/canvas resources should move from `callback_resources` to a tessel-side resource registry with a "prepare(device, queue)" hook per frame and a `Surface` primitive referencing their colour texture.

---

## 6. cadatomic-core / cadatomic-model boundaries and GUI-facing seams

- **No GUI deps (confirmed):** cadatomic-core `[dependencies]` = optional cadabra, cadatomic-model, serde, serde_json, sha2, thiserror, schemars, toml, inventory, serde_path_to_error; cadatomic-model = serde, thiserror. The only `egui` mentions in core sources are doc comments (barrier.rs:295,304; jobs.rs:9; palette.rs:3; scene/mod.rs:3; inspect/mod.rs:1; lib.rs:1) and **key names**: `keymap.rs:582` "Key names are egui's `Key` names", `keylayout.rs:48,160-162` (physical/logical key names as egui spells them). `scripts/check-boundaries.sh` enforces core/model/cli/ui limits with `cargo tree`. The coupling-by-name means tessel's key enum must round-trip egui's `Key::name()` strings (or core's names get a new table).
- **Seams the toolkit must integrate with:**
  - **One command path:** `command/registry.rs` (typed `CommandKey<Args, Output>`, inventory-collected, sorted by id), `dispatch.rs` (`CommandDispatcher`: the one path from RPC/CLI/scenario/app; `run(id, args, CommandSource::Gui|ActionBar|RepeatLast|Rpc|Cli|Script|Replay, ...)`), `invocation.rs` (`PartialInvocation`, `ArgSource`, `DraftStore`: one record for keybinding args, palette drafts, action-bar slots, repeat-last), `log/` (command log), `host_command.rs`, `ui_request.rs` (`UiRequest`: what a command asks of the surface; applied by the app host), `view.rs` (`ViewState` trait the app implements over the viewport: camera, selection, screen point), `interaction.rs` (`InteractionLogic`/`ToolHost`: custom pointer interactions that end in at most one invocation - move tool, router). Rule: GUI, shortcuts, palette, action bar, RPC, MCP, CLI, macros produce the same invocation record; widgets send `Intent`s which `ShellApp::logic` applies after the frame (`views/mod.rs`).
  - **Keymaps/shortcuts:** `keymap.rs` (`KeyStroke`, `KeySequence`, `KeyModifiers`, `Layer`, `KeyBinding`, `Keymap`, `ChordState`, `Offer`), `keylayout.rs` (which strokes a press may mean on non-US layouts; never steal a key that types), `context.rs` (`ContextKey`, `ContextExpr` `when`, `ContextStore`). App side: `shortcuts/mod.rs` resolves in a begin-pass hook before widgets, asks the focused Svelte widget (`shortcut_override`), yields to IME composition and text focus, removes matched events; dock chords are handed to berth as `DockKeys`. The design doc §7.1 keeps this: "Keymaps resolve in Rust through the command layer before JS sees the key".
  - **Jobs and the barrier:** `jobs/registry.rs` (`Jobs`, waker, events), `jobs/pool.rs` (`JobPool`, `StdThreadPool`), `jobs/barrier.rs` (above); "every executor call is a job; the engine never runs on the UI thread"; results drained in `App::logic`. Toolkit needs: a thread-safe "wake the UI" (`request_repaint`), a place to run `logic` before the pass, and the end-of-frame `note_frame`.
  - **Events/journal:** `events.rs` + svelters `Journal` (tours wait on it).
  - **Op/waiting states:** `op.rs` (presentation timing for pending/stale/failed), consumed by `cadatomic-ui/src/op` behaviours.
  - **Palette model:** `palette.rs` (rows, fuzzy ranking, filter job off the UI thread).
  - **Scene:** `scene/`, `scene_pipeline` (headless scene the viewport uploads).
  - **Designer:** `designer.rs` (`DesignerState` read queries over the app's Svelte UI).

## Addendum (from the agent's hand-back summary; not in the original write)

**Hardest seams.** (1) IME and text: egui_quire builds galleys per line with its own virtualisation (Fenwick
`HeightIndex`, `GalleyCache`, an ASCII fast path that depends on egui atlas internals); IME arrives as
`ImeEvent::Preedit/Commit/DeleteSurrounding` and the editor reports composition and cursor rects each frame through
`IMEOutput`. quire-text is a pure model (rope, ChangeSet, multi-selection, history, preedit state in `TextView`), no
layout, no IME transport, not used by svelters yet. tessel-text must provide a shaped line cache, x↔byte mapping, rewrap
without reshaping, and IME plumbing. (2) Svelte Rust kinds inside a tessel panel (`<code-editor>`, `<property-grid>`)
are egui widgets: re-host as tessel Kinds with the backend, or support a node-level island. (3) Multi-window and
tear-off: berth immediate viewports, macOS OS-pointer workarounds (`native.rs`), the deferred-viewport thread-local
(`deferred.rs`); tessel_berth needs global pointer capture across windows, window rects, monitor work areas and z-order,
a per-window overlay layer, per-window frame clocks. (4) Automation parity: the tour live driver is an egui `Plugin`
that appends synthetic `egui::Event`s and reads AccessKit `TreeUpdate`s; the pixel tier is egui_inspection plus the
egui_mcp tree projection; kittest verb counts: `state` ~650, `run` 323, `step` 126, `get_by_label` 96, `.click()` 100,
`event` 89, `key_press` 71+41, `get_by_role_and_label` 80. (5) The idle barrier depends only on
`ctx.has_requested_repaint()` and a cross-thread `request_repaint_after(1ms)` waker; the dock's drag-in-flight is not a
barrier source (read only by `tour_busy`, `app.rs:1796`). (6) Key names: cadatomic-core keymap.rs/keylayout.rs store egui
`Key` name strings; tessel's key enum must round-trip them. (7) Policy: the register calls berth and quire "standalone
egui-only crates" (`00-decisions.md:466`); adding tessel renderers there needs a user decision. Both forbid `unsafe`, pin
egui exactly and re-export it; quire requires approval for new behaviour.

**Core seams:** registry and dispatcher with `CommandSource`, `PartialInvocation`, `UiRequest`, `ViewState`,
`InteractionLogic`, keymap/context `when`, Jobs and IdleBarrier, the Journal. Widgets send `Intent`s applied by `logic`
after the frame. Scenarios (10 NDJSON) are command-level only and port for free; their test is
`cadatomic-cli/tests/scenarios.rs` (CLAUDE.md says cadatomic-app, which is wrong).

**Suggested order:** M3a tessel owns window/device/frame loop; port `adapter::select` and limits; EguiIsland on tessel's
device; island the whole app first as a smoke test; re-type the dock seam off `egui::ViewportId/Rect/KeyboardShortcut`.
M3b tessel_berth and native shell chrome (menus, title bar, ribbon, status, edge panels, palette modal), keyboard
pre-dispatch through the core keymap, `CancelClose`; every pane an island. M3c tessel-test Harness with kittest verbs,
injection hook and AccessKit snapshot; port `tour/live.rs` and the pixel tier. M4-early: lift canvas2d and the 3D
viewport into Surface layers (seam is already texture + blit); overlays become overlay layers. M2/M3: tessel-text +
tessel_quire (IME gate), then svelters-tessel and the Svelte panels with the 5 Rust kinds (code-editor, property-grid
first). M6: port Rust egui panels by traffic (inspector, palette, library, ECAD panes board/sheet first); tour and
designer overlays last; delete egui.
