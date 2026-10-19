# Svelters v2 status

The global progress ledger. States: `QUEUED`, `IN PROGRESS`, `BLOCKED`, `DONE`. Record the master commit and a one-line evidence summary when a milestone completes. Detailed evidence goes in the milestone file under `## Completion record`.

| Milestone | State | Commit | Notes |
| --- | --- | --- | --- |
| P0 setup | DONE | e64b97c | tag `v1-legacy`, move 2021 tree to `legacy/`, new workspace skeleton |
| P1 JS lane | DONE | 3f94c1a | 4 fixtures (19/87/35/9 ops) pass in Node TestHost |
| P1b renderer fork | DONE | 75e6ff4 | fork vendored @ f3de3f2; renderer + shim produce identical op logs; conformance node==qjs |
| P2 host lane | DONE | 5ebc7aa | 21/21 tests; 4 P1 fixtures byte-identical in QuickJS; engine-agnostic core + JsEngine trait |
| P3 GTK lane | DONE | 8f5dfe1 | 443 Gtk kinds ingested; 298 kinds/1304 props/349 signals in tables; 22 tests + P1 oplog replay pass |
| P4 Qt lane | DONE | dd6c28c | 192 classes ingested; 8 elements annotated; 41 generated tests pass; leaf-crate rebuild verified |
| F1 fork bindings | DONE | 32fe00a | element bind:this (call/getProp/StaleHandleError) + renderer bind hook in fork 27b7dfb70 (pushed to akiselev/svelte master + svelters/renderer); entry.text two-way on GTK+Qt with native echo check |
| D1 docs generator | DONE | 77addd6 | per-toolkit docs IR frontends → Starlight site (881 pages, link-checked); docs ingestion pass; GFDL-safe Qt `brief` policy; `pnpm docs:build` / `docs:test` |
| S1 serdes main types | DONE | 8071bb2 | ABI v3 value model (tags, descriptors, records, boxed, ERROR, CALLBACK, async, variants, dates) on GTK+Qt; 114/114 nextest; remainder → S2 |
| S2 ABI v3 remainder | DONE | e5a208d | out/in-out params (host+GTK+Qt), Qt callbacks + async (QMenu::addAction functor, QDialog::open Promise), Qt UTF-16 results (1 KiB wide getProp 32.6→1.8 µs), typed GVariant, GValue props, VALUE-class handle_info; 167/167 nextest |
| X3 child attrs + listen phases | DONE | 7310c77 | `parent:<attr>` child attributes (GTK grid/fixed/notebook/stack/overlay; Qt grid/box/tabwidget/splitter), listen_ex after/capture, ABI v2.2 additive; 453/453 nextest; open: QMainWindow toolbar/dock areas, Qt tabIcon |
| X5 lists (M5) | DONE | 013f593 | portable `virtuallist` (GTK GListModel+ListView/GridView, Qt QAbstractListModel+viewport-pooled rows), JS-owned ids + LIS keyed diff → minimal splices, `@svelters/lists` ListView; 10k/100k rows e2e on both; 422/422 nextest |
| M2a ABI v2 | DONE | 417093a | schedule_wake native timers (no host thread), lifetime classes/handle_info/dead-handle ring, cascading on_destroyed reasons, onRequestClose + host-decided quit; 74/74 nextest |
| M2b dev loop | DONE | 79a423f | in-process hot reload on GTK+Qt (edit → new UI ~1.7–2 s, mostly Vite); errors mapped to .svelte:L:C; svelte-check in pnpm test; 66/66 nextest |
| A1 API surface + galleries | DONE | c184bb0 | 22/22 batches; GTK 420 / Qt 528 verified facts (431 / 1571 tests, 0 failing); elements GTK 66/102, Qt 52/77; galleries GTK 10 / Qt 11 pages with screenshot tests; 607/607 nextest; open gaps → X4/X7 |
| X4 menus + actions | DONE | 157bcaf | menubar/menu/action/menuseparator/toolbar/contextmenu; GTK GMenuModel+GSimpleAction+PopoverMenu(Bar)+ShortcutController, Qt QAction/QActionGroup/popup; bind:checked check+radio; 653/653 nextest |
| X6 styling | DONE | 43ae1a1 | class/class:/style:/style + scoped `<style>` -> GTK CSS / QSS with diagnostics, `@native`, `--sv-*` tokens + dark scheme at runtime, ABI v2.3 `style`; 638/638 nextest (1 unrelated flaky), styling gallery pages + screenshot tests, docs guide; details in `plans/milestones/X6-styling.md` |
| X7 toplevels + non-widget kinds | DONE | 3345b21 | dialogs/secondary windows declared anywhere (transient, modal via open/show/present); TextBuffer/Adjustment forwarding (bind:text on textview, bind:value on scale), QTextDocument kind; QMainWindow toolbars/docks with parent:area; 706/706 nextest; G5/G8/Gdk-Pango → X8 |
| X8 events, typed results, draw funcs | DONE | 8e5fcbd | typed sync results (e.result objects → out params), Qt event families + GTK controller elements, native canvas (JS display list → cairo/QPainter), ListBox headers + `arrange`, Gdk cursor/texture values; 742/742 nextest; open: canvas drawImage/gradients/measureText, PangoAttrList, Qt QCursor/QPixmap, GtkScale format func |
| ST1 PR #3 merge + style unification | DONE | 8f7427f | PR #3 merged; one style table per toolkit (metadata/style: css / setter / child-attr, native kebab-case names) feeding semantics json, props.generated.js and the docs reference; renderer setter rules (class:/id, getProp restore); compiled path: compile-time setter matching, classes/id/style via host.style, CSS to X6 via Svelte-scoped sheets, style= / style:; emphasis and whole cssClasses writes gone; Codex P1/P2 fixes; 889/889 nextest; open: compiled dynamic style= setters ignore explicit attrs |
| MAC1 AppKit backend slice | DONE (unverified on macOS) | 92dd775 | svelters-apple + svelters-appkit (window, view, stack-view, text-field, secure-text-field, button, slider, progress-indicator, box; KVC/NSInvocation dynamic fallback; CFRunLoop wake/timers, CADisplayLink frames, window-should-close call_sync), tools/metadata/appkit pipeline, examples/counter-mac with 76 generated + 16 backend tests; links Mach-O from Linux; user to test per docs/notes/macos-setup.md |
| WIN1 WinUI 3 backend slice | DONE (unverified on Windows) | 72936f7 | svelters-winui (window, stack-panel, grid + row/column/row-span/column-span, text-block, button, check-box, slider, text-box; 48 props, 8 events, sync before-text-changing/key-down; DispatcherQueue wake/timers, CompositionTarget.Rendering frames), winmd pipeline with fetch script, examples/counter-win --selftest via UIA, 60 generated tests; links for gnu+msvc; WinUI does not run under Wine; user to test per docs/notes/windows-setup.md |
| FX1 compiled style + text_prop + mac/win wiring | DONE | 3ad42e3c | compiled style precedence (attr > style=/style: > rules > default, undefined falls back, dynamic #id), text_prop per kind for all 4 toolkits, AppKit/WinUI bindings, style tables + docs, semantics specs for appkit/winui (svelters-compile --backend appkit/winui); 929/929 nextest; open: GTK verify method_textview_get_buffer / method_textbuffer_undo SEGV once under load (passed on retry), WinUI is-checked/TextChanged echo ordering unverified on Windows |
| N1 native kebab-case rename | DONE | 78e137a8 | tags/props/events/child-attrs from native names via tools/metadata/common/naming.py (acronym run = one word); Qt box-layout+direction, grid-layout, menu (submenu/context by parent), action separator; onclicked etc.; Qt twins fixtures/<name>-qt; strict TestHost schemas; GUI tests killed after 6 min; 929/929 nextest, ledgers GTK 489 / Qt 595 facts; map docs/notes/n1-rename-map.md; open: methods not renamed, GTK stack bind:visible-child-name has no binding, schema.qt.json freshness unchecked |
| CI1 GitHub Actions per platform | DONE | 12bde1e2 | js, linux-gtk (Arch GTK 4.24 required + Ubuntu GTK 4.14 allowed-failure), linux-qt (Qt 6.11.3), macos-appkit, windows-winui; all run on push/PR, contents:read; green: js/gtk/qt; red: macOS backend tests (2) and WinUI init → lanes FG1/FA1/FW1 |
| FW1 WinUI startup fix | DONE | 9fea0b98 | islands init order (STA, bootstrap, DispatcherQueue, composed Application, WindowsXamlManager, then XamlControlsResources), single init per thread, WinUiBackend::shutdown(); windows-latest green: lib 12/12, generated 60/60, selftest, counter; open: generated tests skip shutdown(), keyboard routing untested |
| FA1 AppKit CI fix + Xcode metadata | DONE | cd670afd | slot-backed props (subviews etc.) refused before conversion, insert restores old child on refusal, display link added to the real NSRunLoopCommonModes + 64 ms backup timer, panic-guarded test cleanup; metadata/appkit regenerated with Xcode 26.1.1 / macOS 26.1 SDK on the macOS runner (byte-identical), appkit-metadata CI freshness job; macos-latest green; open: ProMotion/multi-display/real interaction only verifiable on a real Mac |
| FG1 GTK >= 4.14 support | DONE | 7c07fc87 | newer kinds resolve `*_get_type` via dlsym at runtime, version-gated props with compat.rs errors, Ubuntu GTK 4.14 CI job now required (614 tests) alongside Arch 4.24 (822); local gates 931/931 (4 flaky under load: qt toplevels close_dialog/close_parent ABRT, gtk verify echo_editable* SEGV); limitation: `--sv-*` CSS variables need GTK >= 4.16 |
| AI1 annotation inference (GTK → Qt → WinUI) | PAUSED (GTK slice code-complete, not merged) | fc01aa9e on `ci/ai1-gtk` | probe.py + infer.py + metadata/rules/gtk.yaml + metadata/inferred/gtk; ingested < inferred < manual; +937 inferred verified GTK facts (565 methods, 220 bindings, 109 props, 40 signals, 3 slots), 1 parked, 45 mid / 90 low unreviewed in `_review.json`; full ledger refresh 1451/1451 green, but full gates + 4.14 CI not run; Qt/WinUI slices not started; handoff `docs/notes/ai1-handoff.md` |
| NT0 non-GUI types pilot (GTK, N0) | DONE | 49bc8290 | ABI v2.5 additive (SHARED, construct, call_static, SVL_T_TYPE, STATIC/CTOR flags); every GObject a handle; gi.rs over 8 typelibs; types_facts.py classifies 301 types; packages/types ES modules (file per class, sideEffects:false) + signature-closure manifest; curated 10 types: 50 types_verify tests in the GTK ledger (564/564); Qt oplog replay skips construct/static logs (Qt types are N2); local 996/996 nextest, pnpm test, test:meta, docs:test; all 5 CI workflows green incl. gtk-4-14; handoff `docs/notes/nt0-handoff.md` |
| X1 object returns + GTK GI calls | DONE | 5ea82a6 | borrowed handles with stable identity + weak refs (GTK qdata, Qt by_obj map); GTK dynamic calls via GIRepository (scalars, strings, enums/flags, objects, GStrv, boxed); ~110 Qt object getters; 156/156 nextest; out params wait on S2 |
| X2 sync handlers + frame clock | DONE | ba5bb65 | call_sync with event-object results (e.result / preventDefault), close-request per-event veto, key handled on GTK+Qt, rAF on GdkFrameClock / QWindow::requestUpdate; ABI v2.1 additive; 127/127 nextest; listen phases deferred (listen_ex proposal) |
| P5 integration | DONE | 1b22e76 | counter on GTK+Qt; 57/57 nextest incl. 12 integration; first frame GTK ~1.2 s / Qt ~0.2 s; binaries 3.4 / 2.6 MB; JS dominates the 1000-row split (host/ABI < 20 ms) |
| NC1 native compiler (experiment) | MERGED via ST1 | 8f7427f | Rust Svelte compiler (verbatim ports of the Svelte parser, acorn, scope analysis) + per-framework semantic specs, now the only planned frontend (fork + stock renderer path to retire); plans/milestones/NC1-native-compiler.md |
| M2 dev loop | QUEUED | | |
| M3 metadata + codegen | QUEUED | | |
| M4 native semantics | QUEUED | | |
| M5 lists / model-view | QUEUED | | |
| M6 release binding linker | QUEUED | | |
| M7 egui | QUEUED | | |

## Handoff (2026-10-18, paused for weekly quota)

`master` is green and usable: local gates 931/931 at 7c07fc87 (FG1), and all five CI workflows were green after FW1/FA1/FG1. Nothing unverified has been merged. Two lanes are paused on pushed branches. Each has a handoff note, and its worktree is kept under `.claude/worktrees/`.

**Resume order:**
1. **NT0** (`ci/nt0-types`, worktree `agent-ab68746002833b005`): follow `docs/notes/nt0-handoff.md`.
   - `cargo check -p svelters-gtk --tests`.
   - `python3 tools/metadata/ledger.py refresh --toolkit gtk`, then commit `metadata/verified/gtk.json`.
   - Full gates: `pnpm -r build`; `tools/dev/cargo-slot xvfb-run -a cargo nextest run --workspace --no-fail-fast`; `pnpm test`; `pnpm docs:test`.
   - Check the CI results on `ci/nt0-types`, in particular the GTK 4.14 job and the Qt build with the new nullable ABI members.
   - Merge.
2. **AI1** (`ci/ai1-gtk`, worktree `agent-ae50fa4f78edc518f`): follow `docs/notes/ai1-handoff.md`.
   - Merge master, after NT0 lands. Both touch the ledger and the generated GTK tests.
   - Full gates and the 4.14 CI job, then merge.
   - Then the Qt slice: accept-by-pattern, table-driven tests. Then WinUI: rules only, verified via CI.
   - Watch: `bindings.generated.js` grew from 11KB to 60KB.
   - The 135 mid/low facts in `metadata/inferred/gtk/_review.json` await a decision on agent review.

**Decided, not started:**
- Reactivity research for the ABI: notify / NOTIFY / KVO / DP callbacks (`plans/research/NON-GUI-TYPES.md` §10 Q2).
- NT N1–N4 (GTK async, Qt with runtime overload dispatch, native models, WinUI/AppKit).
- CLI C0 (`plans/research/CLI.md`).
- Retiring the stock renderer and Svelte fork in favour of the Rust compiler.
- Runtime phases 0–1 (`plans/research/RUNTIME.md`).
- The dev module loader (`plans/research/QUICKJS-RUNTIME.md`).
- The libadwaita widget-pack pilot (`plans/research/CUSTOM-WIDGETS.md`).

**Open user questions:** the custom-widgets questions, the runtime research questions, and the QuickJS decisions (CSS scoping port order, alias table location, CJS, release format, debugger, Intl, Rust CLI). See each research doc.

**Known flakes under load:** qt toplevels close_dialog/close_parent (ABRT); gtk verify echo_editable*, textview_get_buffer, textbuffer_undo (SEGV).

**Untracked in the main checkout, not ours:** `.playwright-mcp/`, `docs/notes/scripts/winui_proto_filter.txt`.

## Research (docs/notes/)

| Topic | File | State |
| --- | --- | --- |
| Svelte 5 custom renderer + DOM surface | svelte5-custom-renderer.md | done |
| 2021 GTK version archaeology | gtk-v1-archaeology.md | done |
| GIR / Qt metadata ingestion | widget-metadata-ingestion.md | done |
| Signals, return values, child slots | signals-and-slots-semantics.md | done |
| Custom renderer PR hands-on trial | custom-renderer-trial.md | done |
| Node / Deno runtime integration | node-deno-integration.md | done |
| QuickJS JIT (quickjs-jit, Cranelift) | quickjs-jit-cranelift.md | done: no JIT for now; V8 if perf demands |
| Windows GUI backend (Win32 / WinUI 3) | windows-gui-backend.md | done: WinUI 3 via windows-rs; Win32 fallback |
| Apple GUI backends (AppKit / UIKit / SwiftUI) + JavaScriptCore | apple-gui-backend.md | done: AppKit+UIKit via objc2; JSC optional engine |
| JS ↔ native serdes | js-native-serdes.md | done: extend svl_value in place (ABI v3, M3) |
| Ownership and lifetimes | ownership-and-lifetimes.md | done: explicit tree-driven lifetime, 4 lifetime classes, ABI v2 proposal |
| Svelte feature survey (lists, snippets, bindings, modifiers) | svelte-feature-survey.md | done |
| Styling: GTK CSS and Qt QSS | styling-gtk-css-qss.md | done: 4-layer model, portable vocabulary + native escape hatch |
