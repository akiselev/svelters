# QT-SURVEY: what Qt, KDE and the Qt IDE libraries have that we don't

Status: research, 2026-10-22. Docs only; nothing was built or run. The question (user): "a survey of Qt and its
libraries: what else are we missing, UI feature or affordance wise (also devex like the QSettings type)". This doc
answers it for a desktop CAD/EDA IDE (cadatomic) on svelters-egui. It lists only what matters there, and says who
should own each item, so that svelters does not duplicate what cadatomic, berth or quire already plan.

Read at:

- **svelters** origin/master `90fc9d6d`, plus the in-flight lanes: `ci/wg1` `9449c9ae`, `ci/wg2a` `3eb3b2e2`,
  `ci/wg2b` `c2f077e5`, `ci/wg4a` `b707c1b5`, `ci/wg4b` `f90ea71e`, `ci/sg4` `7203a4fd`, `ci/lists` `bacc6242`,
  `ci/cx1` `e1e98848`, `ci/fx1` `5681e01a`, `ci/h1` `73f2ae5a`, `ci/h3` `c89e9ac4`, `ci/cs5f` `fcc7c761`.
  There is no `ci/int-widgets`: the widget lanes are separate branches, each merging `ci/wg1`. `ci/wg3b` has no
  commits beyond master. There is no `ci/cs5g`.
- **Docs:** EGUI.md (§5 catalog, §8 decisions), STATUS.md, TRACKS.md, CADATOMIC.md, ARCH-EMBED.md,
  ARCH-SCENE.md, QUICKJS-RUNTIME.md (item 11, Intl).
- **cadatomic** working tree, 2026-10-22: `docs/plan/00-decisions.md` (U1–U6, D1–D11, the berth and quire
  answers, "egui element catalog"), `docs/plan/04-kernel-requests.md`, and `docs/research/2026-10-22-*` (ide-shell §2,
  §4, §6, §8; widget survey §1.10 and §2.2), plus `2026-10-16-command-architecture.md`.
- **berth and quire** are not checked out on this machine. Their scope comes from ARCH-CRATES §2–§3 and cadatomic's
  register.

Qt facts are from memory of Qt 6 and KDE Frameworks 6. Anything marked **verify** should be checked against the docs
before a lane relies on it (§4 lists them).

## Summary

1. **The widget catalog is in good shape. The gaps are in *behaviour around* widgets.** The Tier 0/1 lanes (WG1,
   WG2a/b, WG4a/b, LS1) cover most Qt Widgets classes cadatomic needs. What Qt has and we lack is mostly the glue
   that makes an IDE feel finished: key routing between focused widgets and app shortcuts (Qt's `ShortcutOverride`),
   keyboard navigation and editing inside item views, default and cancel buttons, tooltips on disabled widgets, and
   multi-format clipboard and drag data (`QMimeData`).
2. **Item views are the biggest P0 widget gap.** Qt's model/view gives a tree, keyboard navigation, type-ahead,
   selection ranges and in-cell editors (delegates, edit triggers) together. We have virtual rows (LS1, WG4a `table`),
   but no tree, no keyboard navigation and no cell editing. cadatomic's P0 PropertyGrid, model tree, Problems list
   and BOM all sit on these.
3. **Key routing is the main P0 seam.** cadatomic owns the keymap (layers `Modal > TextFocus > Session > Viewport >
   Global`). svelters must tell it, before the shortcut fires, whether the focused Svelte node claims the key. Ctrl+Z
   in a text field must undo the text, not the document. Qt solves this with `ShortcutOverride` and shortcut
   contexts. EV3 (focus) is planned, but this contract is not written down anywhere.
4. **Most devex APIs Qt bundles in QtCore are cadatomic's by decision:** commands, keymap and palette (command
   architecture), layered settings and the U5 policy layer, document undo over the command log, Fluent catalogs,
   jobs and processes (outboard), and the crash hook. svelters should add only the **binding layer**: traits that
   cadatomic implements, with simple defaults for other apps (§2).
5. **svelters devex gaps that cadatomic does not cover:**
   - a reactive settings and persisted-UI-state binding (`svelters:settings`);
   - standard paths and an app identity;
   - multi-format clipboard (`svelters:clipboard`);
   - `Intl`, which QuickJS-NG lacks entirely, so `toLocaleString` and plural rules don't work in Svelte;
   - opening paths and revealing files (`svelters:shell`);
   - JS `console` routed to `tracing` (today it goes to `eprintln!` and a buffer).
6. **cadatomic owns gaps that nobody has designed yet.** They are listed in §1.3 for main-cadatomic: recent files,
   single instance with open-in-running-instance, file-changed-on-disk handling, an About/licences page, PDF and
   print output of drawings, the settings UI and the keymap editor.
7. **Recommended next svelters lanes (§3):**
   - EV3+ keys and focus;
   - WG5a item-view keyboard, selection and editing;
   - WG6 affordance polish;
   - WG5b tree-view;
   - DX1 settings, UI state, paths and logging;
   - DX2 clipboard and shell;
   - IN1 Intl;
   - TK3+ accessibility announcements;
   - then PK1, the plot pack.

## 0. Legend

- **Status:**
  - **have**: on master;
  - **flight (lane)**: on a `ci/*` branch;
  - **planned (where)**: in a research doc or milestone, not started;
  - **partial**: some of it exists;
  - **missing**: nobody plans it.
- **Owner:**
  - **core**: `svelters`/`svelters-host`, engine- and backend-neutral;
  - **egui**: `svelters-egui`;
  - **pack**: a feature-gated svelters pack (EGUI.md §5 Tier 2);
  - **berth**, **quire**, **cad** (cadatomic).
- **Size:** lane-days for the owner, in the style of CADATOMIC.md §2. A dash means it is already sized elsewhere.
- **Prio:** importance for cadatomic.
  - **P0**: needed before the first usable IDE;
  - **P1**: needed for MCAD/ECAD work;
  - **P2**: later, or never.

## 1. Gap table

### 1.1 UI widgets

| # | Qt / KDE feature | What it gives | Status | Owner | Size | Prio |
|---|---|---|---|---|---|---|
| QW1 | `QTreeView` + `QAbstractItemModel` tree: lazy children (`hasChildren`/`canFetchMore`), expand/collapse all, uniform row heights | model tree, outline, libraries, layer manager | planned (`tree-view` after LS1/LS2; SV44) | egui (view); cad MODELS (`TreeFlatten`) | 6–8 | P0 |
| QW2 | `QAbstractItemView` keyboard: arrows, Home/End, PgUp/PgDn, Shift/Ctrl ranges, type-ahead `keyboardSearch`, Enter = activate, Menu key = context menu | keyboard-only lists, tables and trees; Problems, BOM | missing (deferred in WG4a and LS1) | egui | 3–4 | P0 |
| QW3 | Delegates and editors in cells: `createEditor`/`setModelData`, edit triggers (double-click, F2, any key, selected-click), persistent editors, Tab to the next cell, Esc reverts | property grid, BOM, constraint spreadsheet, stackup | missing (WG4a "rich cells" waits for `RowPool`, which is in `ci/lists`) | egui (edit protocol, focus); cad (editors such as `QuantityField`) | 4–6 | P0 |
| QW4 | `QHeaderView`: sort indicator, resize-to-contents, hide/move sections, header context menu, `saveState` | sortable, configurable tables that remember their layout | partial, flight (WG4a: column resize, `header-clicked`; reorder deferred) | egui | 2–3 | P1 |
| QW5 | `QItemSelectionModel`: current vs selected, ranges, selection flags | multi-select that survives filtering; cross-probing | planned (SV46, key-based with deltas) | core + egui | — (3–4) | P0 |
| QW6 | `QSortFilterProxyModel` (recursive filter, dynamic sort), `QAbstractItemModelTester` | filter/sort without copying rows; model invariant checks | proxies: planned (cad MD1–MD3). Tester: missing (the widget survey §1.10 asks for one) | proxies cad; a `RowModelTester` for the SV41 trait in core | 1–2 | P1 |
| QW7 | `QSpinBox`/`QDoubleSpinBox`: step buttons, wheel, PgUp ×10, `keyboardTracking=false` (commit only), special-value text, wrap, prefix/suffix | numeric entry with predictable commits | partial, flight (`drag-value`, WG2b; drag scrub, no step buttons, no commit-only mode) | cad (`QuantityField`, decided); egui: a `commit` mode on `drag-value` | 1 | P1 |
| QW8 | `QLineEdit` + `QValidator`: Invalid/Intermediate/Acceptable, `fixup`, input masks, `editingFinished`/`returnPressed`, clear button, read-only selectable | forms that commit on Enter/blur and show why input is wrong | partial: `hint-text`, `char-limit`, `password` (master); `invalid` flag (WG1). No commit event, no pattern; `\|commit` planned (SV3, CX) | egui (`submit`/`commit` events, `pattern`, `read-only`); compiler (`\|commit`) | 2–3 | P1 |
| QW9 | `QCompleter`, editable `QComboBox`, combo type-ahead | pickers that scale past a few dozen items | completer: planned in cad (decided W1 list); combo type-ahead: missing in WG1 `combo-box` | cad (completer); egui (type-ahead) | 0.5–1 | P1 |
| QW10 | `QKeySequenceEdit` | capture a shortcut for the keymap editor | missing (widget survey P1, "build") | egui (generic) | 1 | P1 |
| QW11 | `QDialogButtonBox` + default buttons (`QPushButton::setDefault`/`autoDefault`): platform button order, Enter = default, Esc = reject | confirmations behave the same as in native apps | missing. egui has no default button; WG2a `modal` closes on Esc | egui | 1–2 | P1 |
| QW12 | `QMessageBox` extras: detailed text, "don't ask again" (`setCheckBox`), standard icons; `QErrorMessage` | quiet, dismissable confirmations | partial, flight: native `messageBox` via rfd (WG4b), with no checkbox | cad (Svelte component; the flag lives in settings) | 1 | P1 |
| QW13 | `QLabel`/`QFontMetrics::elidedText` (ElideLeft/Middle/Right), rich text with links, buddy | long paths and names that stay readable | partial: egui truncates at the end only (**verify**); `hyperlink` (WG2b) | egui (`elide="middle"` on text kinds) | 1 | P1 |
| QW14 | `QIcon` modes (Normal/Disabled/Active/Selected, On/Off), SVG at any DPI, standard icons (`QStyle::SP_*`) | crisp, state-aware icons | partial: font-glyph `icon` (WG1), `image` PNG only (WG2b) | egui (SVG through `egui_extras` loaders; disabled tint from the cascade) | 1–2 | P1 |
| QW15 | `QTabBar` inside panels: movable, context menu, elide, overflow menu | tabbed panel content | partial, flight (WG4a `tab-view`, closable). Document tabs are berth's | egui | 1–2 | P2 |
| QW16 | `QSplitter`: collapsible children, double-click handle restore | collapsible inner splits | partial, flight (WG4a `split-view`, `bind:position`, keyboard) | egui | 0.5–1 | P2 |
| QW17 | `QListView` sections / Qt Quick `ListView.section` (sticky group headers) | Problems grouped by file; BOM grouped by part | missing | egui (on LS1 `RowPool`) | 1–2 | P1 |
| QW18 | `QProgressDialog.minimumDuration` (no dialog for short work; default 4 s, **verify**) + cancel | no flicker for fast operations | partial: progress and `AbortSignal` (E1); delay-to-show is cad's `Op<T>` waiting states (FW2) | cad | — | P1 |
| QW19 | `QtPropertyBrowser` (typed editors, groups, mixed values, reset to default, modified marker, compound point/size/rect) | the CAD inspector | planned: cad FW4 `PropertyGrid` (P0) on SG5 + QW3 | cad (grid); egui (QW2, QW3) | — | P0 |
| QW20 | QCustomPlot / Qwt / Qt Graphs: log axes, several y-axes, cursors with delta readout, decimation of 1M+ points, colour maps, polar/Smith, PNG/SVG/PDF export, live scrolling | waveforms, Bode, FFT, sweeps, field results | planned (Tier 2 pack `plot` on `egui_plot`; survey P1). Decimation, cursors, Smith and export are beyond `egui_plot` (log axes: **verify**) | pack `plot` (wrap, `Shared<[f32]>` series, min/max decimation in Rust); cad (waveform viewer, Bode, Smith composites) | 5–7 | P1 |
| QW21 | QtNodes (node editor) | dataflow and simulation graphs | planned (Tier 2 pack `node-graph`; egui-snarl lags at 0.36) | pack / cad | — | P2 |
| QW22 | `QWizard`: pages, mandatory fields (`"name*"`), branching `nextId`, commit page | new project, import and export flows | missing; `stack` + TR2 transitions are the parts | cad (Svelte component) | 2 | P2 |
| QW23 | `QPrintDialog`, `QPrintPreviewDialog`, `QPdfWriter` | printed and PDF drawings and schematics | missing (no printing in egui or rfd) | cad (vector export first, OS print later) | 5+ | P2 |
| QW24 | `QDateTimeEdit`/`QTimeEdit`, `QFontDialog`/`QFontComboBox` | dates, times, fonts | date: flight (WG4b `date-picker`); time and font: missing | egui | 1 each | P2 |
| QW25 | `QPlainTextEdit` as a log (append-only, `maximumBlockCount`, ANSI colours) | the Output panel | missing | cad (CH3, on LS1 rows or a read-only quire) | — | P1 |
| — | Already covered: `QToolBox` (collapsing-header, WG1), `QStackedWidget` (`stack`, WG2b), `QColorDialog` (`color-picker`, WG4b), `QFileDialog` (rfd, WG4b), `QProgressBar`/busy (WG2b), `QGroupBox` (`frame`), `QRadioButton` groups (WG2b), `QScrollArea` (WG1) | | have/flight | | | |

### 1.2 Affordances and interaction

| # | Qt / KDE feature | What it gives | Status | Owner | Size | Prio |
|---|---|---|---|---|---|---|
| QA1 | `QEvent::ShortcutOverride`, `Qt::ShortcutContext` (Widget, WidgetWithChildren, Window, Application), `QShortcut::activatedAmbiguously` | the focused widget claims a key before app shortcuts: Ctrl+Z, Del and arrows act on the text field, not the document | partial: WG2a `action` shortcuts are active while their menu's owner is drawn; ARCH-EMBED's `consume_key` exists; nothing else is specified | egui (claims and `focus_info`); cad (keymap layers) | 2–3 on top of EV3 | P0 |
| QA2 | Focus: `setTabOrder`, focus proxies and chains, `FocusScope` (QML), focus restored after a popup or modal closes, initial focus in dialogs, `Qt::FocusPolicy` | keyboard-only forms; menus that hand focus back | partial: flight (FX1 click focus and `:focus-visible`); planned (EV3: scopes, restore, traversal) | egui | — (EV3 4–5) | P0 |
| QA3 | Edit actions that reach the focused widget: Cut/Copy/Paste/Undo/Redo/Select All in the Edit menu operate on the focused `QLineEdit` | Rust-owned menus work in Svelte text fields | missing (no `text_command` seam) | egui (`Embed::text_command`); cad (Edit menu dispatch) | 1 | P0 |
| QA4 | `QKeySequence::StandardKey` (Redo is Ctrl+Y on Windows, Ctrl+Shift+Z elsewhere, **verify**; Preferences is Cmd+, on macOS), multi-chord sequences (up to 4 chords) | platform-correct defaults; chords such as Ctrl+K Ctrl+C | partial: WG2a parses `Primary+S`; quire has `KeySeq`; cad owns `KeyBinding` and presets | cad (table, presets); egui (`action` accepts chords and `std:` names) | 0.5 | P1 |
| QA5 | Tooltips: wake-up delay (~700 ms, **verify**), instant follow-on within the fall-asleep window, **tooltips on disabled widgets**, rich (HTML) tooltips, `statusTip` to the status bar, `whatsThis` | disable-with-reason (ide-shell rule 3); live tooltips (ide-shell §6) | partial, flight: a string `tooltip` on every kind (WG1), through `on_hover_text`, which egui suppresses on disabled widgets (**verify**). No snippet tooltip, no status tip | egui | disabled: 0.5 (P0); snippet tooltip: 2 (P1); status tip: 0.5 (P1) | P0/P1 |
| QA6 | `QUndoStack`: push runs `redo`, `mergeWith` ids (coalescing drags), macros, clean index → dirty flag, undo limit; `QUndoGroup` (one active stack per document); "Undo Fillet" text | cheap, labelled undo | planned, cad: document undo over the command log (A3a), layout undo in berth (D8), text undo in quire / egui `TextEdit` | cad, berth, quire | — | P0 (cad) |
| QA7 | `QUndoView` (history, jump to an index) | see and jump through history | planned, cad (widget survey P1) | cad | — | P1 |
| QA8 | Context menus from the keyboard (Menu key, Shift+F10) | keyboard-only context menus | partial, flight (WG2a: secondary click only, **verify**) | egui | 0.5 | P1 |
| QA9 | `QDrag`/`QMimeData`: several formats per drag, drop actions (copy/move/link by modifier), drag pixmap, auto-scroll near edges, tree auto-expand on hover, external file drops, dragging out to the OS | drag parts between trees, tables and viewports; drop a STEP file on a panel | partial, flight (WG4b: in-app `drag-type`/`drop-types`, `DragPayload`, feedback). Missing: file drops on nodes, actions, auto-scroll, auto-expand, drag-out (winit has no drag source, **verify**) | egui (berth has its own pane drag) | 2–3 (drag-out P2) | P1 |
| QA10 | `QClipboard` modes (Clipboard, X11 Selection, macOS FindBuffer) + `QMimeData` with several formats (text, HTML, image, URLs, app MIME types) + `dataChanged` | copy a table as TSV and HTML; copy refs between documents; paste images | partial: egui text only; Linux PRIMARY, flight (WG2b, arboard); quire has its own. No JS API, no multi-format | core (trait + `svelters:clipboard`); egui (backend) | 2–3 | P1 |
| QA11 | `QAccessible`: roles, states, relations (labelled-by, described-by), value interface, announcements (`QAccessibleAnnouncementEvent`, Qt 6.8, **verify**) | screen-reader use of panels; toasts and status read out | partial: AccessKit projection (G1), names (WG1), set-size (LS1). Missing: live regions or `announce()`, relations. Audit planned (TK3) | egui | 2 (+TK3 3–4) | P1 |
| QA12 | Input methods: preedit, composing state, `inputMethodHints` (digits only) | CJK input; no Enter to the default button while composing | partial: egui IME; composing flag planned (ARCH-EMBED E-7, pin 6) | egui | 1–2 | P1 |
| QA13 | `QScrollArea::ensureVisible`/`ensureWidgetVisible`, scroll on focus, `QScroller` kinetic, auto-scroll during drag | Tab to a field below the fold; reveal the selected row | partial: `bind:scroll-x/y` (WG1), `scrollTo` (LS1), `scroll-to-row` (WG4a). Missing: a generic `scrollIntoView`; scroll-on-focus (**verify** egui) | egui | 0.5–1 | P1 |
| QA14 | `QWidget::setCursor` / QML `HoverHandler.cursorShape` | resize, text and link cursors on custom widgets | partial: `cursor` is in the style tables (`svelters-style`); egui application **verify**; WG4b sets DnD cursors | egui | 0.5 | P1 |
| QA15 | `QLocale` + `tr()`: plurals (`%n`), number and date formats (decimal comma), collation, runtime language switch, RTL | units shown as "1,5 mm" in de-DE; sorted names; plural labels | partial: cad has Fluent (`catalog.t`) and ICU4X (PB10); RTL is out (CE10). **QuickJS-NG has no `Intl`**: `toLocaleString`, `PluralRules` and `Collator` don't work, and widget numbers ignore locale | core (`svelters:intl` over ICU4X, widget formatting); cad (catalog, literal-text lint through SV8) | 3–4 | P1 |
| QA16 | Mnemonics (`&File`, buddy labels Alt+N, KDE's `KAcceleratorManager`) | Alt-key access on Linux and Windows | missing in egui | cad (Rust menus); egui (Svelte dialogs) | 2 | P2 |
| QA17 | `QPalette` colour groups (Active/Inactive/Disabled): dimmed selection in inactive windows | which window has focus, at a glance | partial: CS5e state flags; no `:window-inactive`/`:backdrop` pseudo-class | egui | 0.5 | P2 |
| QA18 | High DPI, multi-monitor: per-monitor scale, fractional scaling, window restore clamped to a visible screen | crisp UI, no lost windows | have (egui per-viewport ppp; berth placement memory and off-screen recovery) | — | — | — |
| QA19 | `QGesture` pinch, pan, swipe; touchpad pinch | zoom plots and viewports from a touchpad | partial: egui `zoom_delta`, multi-touch; the viewport is cad's | cad, pack `plot` | — | P2 |

### 1.3 App shell (cross-checked against berth, quire and cadatomic)

| # | Qt / KDE / third-party | What it gives | Status | Owner | Size | Prio |
|---|---|---|---|---|---|---|
| QS1 | `QMainWindow::saveState`/`restoreState`, `saveGeometry` | layout and windows come back | have (berth layout file v2, versioned, migrations, skip-on-restore, host data per pane) | berth | — | — |
| QS2 | Qt Advanced Docking System / KDDockWidgets: drop overlays, auto-hide sidebars, perspectives, floating multi-pane windows, tab overflow, lazy content, focus highlight | an IDE dock | have/flight (berth BD0–BD6; 416-row KDDW catalogue cross-checked). To check: a tab-list dropdown (ADS), active-pane highlight, a Ctrl+Tab switcher over berth's MRU | berth; cad (switcher UI) | 1–2 | P1 |
| QS3 | `QToolBar` (overflow chevron, icon/text style, `createPopupMenu`), `QToolButton` popup modes (split, instant, delayed) | toolbars that degrade when narrow | planned, cad (fixed-geometry command strip CH1, split button in cad; no ribbon, so SARibbon is n/a) | cad | — | P0 |
| QS4 | `QStatusBar`: `showMessage` with a timeout, permanent widgets, status tips | tool prompts, units, solve state | planned, cad (status sections as `HostSlot::Section` roots, CH3); the status-tip feed is QA5 | cad (+egui QA5) | — | P0 |
| QS5 | Native menubar on macOS; Help-menu search | platform menus | planned (Tier 2 pack `native-menus`, muda); Linux first | pack | — | P2 |
| QS6 | KXmlGui: user-editable toolbars and menus, action collections merged from plugins | customisation | planned, cad ("customisable cursor surfaces", command registry) | cad | — | P2 |
| QS7 | `KShortcutsDialog`: conflicts, alternate keys, schemes import/export | keymap editor with presets (SolidWorks, Altium, KiCad, Blender) | planned in cad (ide-shell §4); needs QW10 | cad | — | P1 |
| QS8 | `KCommandBar` (palette over all actions with their shortcuts) | find any command | planned, cad (CH4, U4 metadata) | cad | — | P0 |
| QS9 | `KRecentFilesAction` (MRU, max count, missing files pruned, clear) + OS recent documents (freedesktop `recently-used.xbel`, macOS, Windows jump lists) | reopen work | **missing: not designed anywhere** (only berth's per-window tab MRU and recent commands in the palette) | cad (WORKSPACE) | 1–2 | P1 |
| QS10 | `QSessionManager`/KDE session restore; reopen documents after a crash | resume work | planned, cad (docking §7.4, autosave A1b) | cad, berth | — | P1 |
| QS11 | Single instance (`QLocalServer`, KDBusService unique): a second launch opens the file in the running instance | double-click a file while the IDE is open | **missing** | cad | 1–2 | P1 |
| QS12 | `KAboutData`: name, version, authors, licences, bug address; third-party licences page | legal and support basics | **missing** | cad | 0.5–1 | P2 |
| QS13 | KCrash/DrKonqi: crash report, autosave recovery, restart | recover from crashes | partial: cad's A1a panic hook writes a crash file. svelters half missing: source-mapped JS stacks + last N ops in the bundle (EGUI.md §4, unscheduled) | cad; core (JS context) | 1–2 (core) | P1 |
| QS14 | `KNotifications` / `QSystemTrayIcon::showMessage`; `QApplication::alert` (taskbar attention) | "simulation finished" while the IDE is unfocused | partial: in-app toast, flight (WG4a). OS notifications missing. Attention: egui `ViewportCommand::RequestUserAttention`, not exposed to JS | pack (`svelters:notify`), core (attention) | 1–2 | P2 |
| QS15 | `KMessageWidget` (inline info/warning/error bar with actions) | stale state, failed rebuild, read-only document | planned, cad (survey "info bar, banner", P0; `StaleBanner` in FW2) | cad | — | P0 |
| QS16 | `KPageDialog` / KCModules: page tree, search, Defaults/Apply/Reset, changed tracking | settings UI | planned, cad (layered settings A1b with provenance; generate it from the schema with PropertyGrid) | cad | — | P1 |
| QS17 | `KJob`/`KUiServer`: progress, speed, suspend/kill, one job tracker | Jobs panel | have/planned (E1 `WorkCx` progress and cancel; cad Jobs panel CH3) | core (have), cad | — | — |
| QS18 | QScintilla / KTextEditor vs quire | code and spec editing | quire has the rope, multi-cursor, folding, find, tree-sitter, diagnostics, IME and a11y paging. Not yet: completion popup, hover, signature help, inlay hints (P1); minimap, diff/merge, vi mode, snippets (P2). Brace matching and bookmarks: **verify** | quire, cad (`cadatomic-lsp`) | — | P1 |
| QS19 | Sonnet (spell-check highlighter) | spelling in notes and descriptions | missing | quire / egui text kinds | 2–3 | P2 |
| QS20 | `QSplashScreen` | startup feedback | missing; Rust startup is fast. Kernel loading can show in the status bar | cad | 0.5 | P2 |

### 1.4 Devex APIs

| # | Qt / KDE feature | What it gives | Status | Owner | Size | Prio |
|---|---|---|---|---|---|---|
| QD1 | `QSettings` (org/app scope, user/system fallback, native storage, typed `QVariant`, groups), KConfigXT (`.kcfg` → typed class with defaults, ranges, labels; `KConfigDialogManager` auto-binds widgets), KConfig's state/settings split (`openStateConfig`, **verify**), QML `Settings { property alias … }` | typed persistent settings in one line; UI state that survives restarts | partial: E3 stores and `persist(key)` (hot-reload only, not disk). cad owns the layered store and U5. **Missing:** a svelters binding (`svelters:settings`) and persisted UI state (column widths, splits, collapsed sections, last tab) | core (traits + JS module); cad (backend over A1b layers, U5 locks) | 3–4 | P1 |
| QD2 | `QStandardPaths` (config, data, cache, state, runtime, documents, temp) + `QCoreApplication` org/app names | correct per-platform locations | missing | core (`directories` crate) | 0.5 | P1 |
| QD3 | `QFileSystemWatcher` | react to files changed on disk | partial: dev watcher in `svelters-modules` (`notify`). App-level: cad's workspace (file changed externally) | cad; core (`svelters:fs.watch`, P2) | 1 | P2 |
| QD4 | `QTimer`, `QElapsedTimer`, `QDeadlineTimer` | timers | have (`setTimeout`/`setInterval`/rAF/`performance.now`, `schedule_wake`, virtual clock) | — | — | — |
| QD5 | `QProcess` (args, env, cwd, streamed stdout/stderr, exit code, kill) | run simulators, KiCad tools, scripts | cad: outboard and switchyard (OB1–OB11, SY1–SY6); E1 `Sink<T>` streams suffice for UI | cad; pack `svelters:process` for other apps | 2 | P2 |
| QD6 | `QDesktopServices::openUrl` (URLs, local files), reveal in file manager (KIO `highlightInFileManager`) | open docs, datasheets, output folders | partial, flight: `hyperlink` opens URLs (WG2b, egui `OpenUrl`). Missing: `openPath`, `revealInFolder` from JS | core (`svelters:shell`) | 0.5–1 | P1 |
| QD7 | `QLoggingCategory` (named categories, runtime rules such as `QT_LOGGING_RULES`, message handler) | filterable logs from UI code | missing: JS `console` goes to `eprintln!` (when echo is on) and a buffer (`core.rs` `console`) | core (route to `tracing`; `logger('cat')`) | 0.5–1 | P1 |
| QD8 | `QMetaObject`/QML introspection, Designer live preview | tooling, inspectors | have (kind specs, `svelters-exports/1`, SG3 queries, egui_mcp (D5), CX1 diagnostics, H1/H3 HMR in flight) | — | — | — |
| QD9 | Signals/slots, `QFuture`/`QPromise` progress, `QThreadPool` | async UI | have (Svelte reactivity, E1 workers with progress and abort, E3 `Emitter`/stores) | — | — | — |
| QD10 | `QDataWidgetMapper` (model columns ↔ widgets, Auto/Manual submit) | forms over records | partial: `bind:` on E3 store fields (have); `\|commit` and draft bindings planned (SV3, CX) | core, compiler | — | P1 |
| QD11 | `QAction` as a shared data object (text, icon, shortcut, checkable, enabled, statusTip) across menu, toolbar and palette | one source for every surface | cad: `CommandKey`, U4 metadata, registry; svelters' `action` kind (WG2a) only renders it | cad | — | P0 (cad) |
| QD12 | `QCommandLineParser` | CLI options | cad (clap) | cad | — | — |
| QD13 | QtTest (`QTest::keyClick`, `QSignalSpy`) | UI tests | have/planned (kittest, TK1; TK2 locators) | — | — | — |

### 1.5 Not worth doing

- `QMdiArea` (berth is the answer), `QSystemTrayIcon` (QS14 covers notifications), `QWhatsThis` (cad's live
  tooltips replace it), `QLCDNumber`, `QDial`, `QCommandLinkButton`, `QColumnView`, KIO, KActivities, Kirigami, and
  SARibbon (cad decided on a command strip).
- Qt Quick `SwipeView`, `Tumbler` and `PageIndicator` are mobile idioms. `Drawer` is berth's overlay panels.
  `ScrollIndicator` is egui's floating scroll bars (have). `Popup.closePolicy` and `Overlay.modal` are WG2a (flight).
  `StackView` is `stack` + TR2. `DelayButton` (press and hold to confirm) is a P2 idea for destructive operations.

## 2. Devex APIs: proposed svelters shapes

**Rule:** svelters defines a **trait plus a `svelters:*` module**, with a small default backend so that a non-cadatomic
app works out of the box. cadatomic plugs in its own backend wherever it already owns the concept. Everything rides
the existing machinery:

- E0 modules;
- E3 stores for reactive values;
- Promises with `AbortSignal` (E1);
- `NativeError` codes;
- LD1 native module providers, which serve the modules.

Each backend is set once on the `Embed` / `App` builder. Policy hooks follow the "deny-by-default, content
deferred" decision: a `principal` argument is accepted and ignored for now.

| Area | cadatomic already owns | svelters adds |
|---|---|---|
| Settings | layered store (defaults < team policy < user < project < document), provenance, U5 locks (A1b, ide-shell §4) | `SettingsBackend` trait + reactive `setting()`; `UiStateBackend` + `uiState()` |
| Paths | — | `AppId` + `Paths` (`directories`) |
| Undo | document undo over the command log (A3a); layout undo (berth D8); text undo (quire) | **no undo stack.** Only `focus_info`/`text_command` (QA1, QA3), so cad's Edit menu reaches Svelte text fields |
| Actions, keys | `CommandKey`, keymap layers, presets, palette (command architecture §3.5–3.7, CH4) | key-claim contract (`shortcut_override`), `use:keys` for component-scope keys, chords and `std:` names on `action` |
| i18n | Fluent catalog, `catalog.t`, pseudo-locale, literal-text tests, ICU4X (PB10) | `Intl` subset in QuickJS + locale-aware widget numbers + a reactive `locale` |
| File watching | workspace "changed on disk" | `svelters:fs.watch` (P2), behind a policy hook |
| Process | outboard / switchyard | nothing for cad; a P2 pack for other apps |
| Clipboard, DnD data | quire's clipboard | `MimeData` shared by clipboard and DnD; `svelters:clipboard` |
| Notifications | in-app toasts dispatch commands | `svelters:shell` (openUrl/openPath/reveal/attention); `svelters:notify` pack (P2) |
| Logging | trace, execution and parent ids on invocations | JS console → `tracing`, `logger(category)` |

### 2.1 Settings and persisted UI state (QD1)

```rust
// svelters-host::settings — engine- and backend-neutral
pub trait SettingsBackend: Send + Sync + 'static {
    fn get(&self, key: &str) -> Option<serde_json::Value>;
    fn set(&self, key: &str, v: serde_json::Value, who: Principal) -> Result<(), SettingsError>; // Locked { by }, Invalid
    fn source(&self, key: &str) -> Option<SettingSource>;            // { layer: "project", locked: bool }
    fn subscribe(&self, on_change: Box<dyn Fn(&[String]) + Send + Sync>) -> Subscription; // any thread; folds into R1's batch
}
pub trait UiStateBackend: 'static {                                     // KConfig "state", not settings
    fn load(&self, root: &RootKey) -> serde_json::Map<String, serde_json::Value>;
    fn save(&self, root: &RootKey, state: serde_json::Map<String, serde_json::Value>); // debounced by the host
}

// KConfigXT without XML: a typed group, defaults and ranges in one place; schemars gives typings and the settings-page schema
#[derive(svelters::Settings, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[settings(prefix = "viewport")]
pub struct ViewportSettings {
    #[setting(default = 45.0, min = 10.0, max = 120.0, label = "settings.viewport.fov")] pub fov: f32,
    #[setting(default = "orbit")] pub navigation: NavStyle,
}

svelters_egui::Embed::builder()
    .app_id(AppId::new("dev", "akiselev", "cadatomic"))
    .settings(cad_settings)            // default: TomlSettings at paths.config()/settings.toml (atomic rename, watched)
    .ui_state(cad_ui_state)            // default: JSON per root under paths.state(); cad: berth's per-pane host data string
```

```svelte
<script lang="ts">
  import { setting, uiState } from 'svelters:settings';
  import { viewport } from 'svelters:settings/viewport';      // generated from #[derive(Settings)]
  const fov = setting(viewport.fov);                           // { value, source, locked }, reactive
  const ui = uiState({ split: 0.4, open: { placement: false }, columns: [200, 80, 120] }); // keyed by root + anchor
</script>
<slider bind:value={fov.value} enabled={!fov.locked} tooltip={fov.locked ? `Locked by ${fov.source.layer}` : ''} />
<split-view bind:position={ui.split}> … </split-view>
<collapsing-header bind:open={ui.open.placement} text="Placement"> … </collapsing-header>
```

- A write to a locked key rejects with `NativeError { code: 'settings/locked' }`, and the widget restores the old
  value. U5's lockable keys and visible provenance arrive through `source` with no svelters policy.
- `uiState` is per root and anchored (SV16 anchors), so it survives HMR and restarts. It never goes into the command
  log. cadatomic stores it in berth's opaque per-pane host data. That data is already a string and is skipped on
  restore for temporary panes.
- The settings *page* (QS16) is cadatomic's PropertyGrid over the schema. svelters only emits the schema.

### 2.2 Paths and app identity (QD2)

```rust
pub struct AppId { qualifier: String, org: String, app: String }       // QCoreApplication org/app names; set once
pub struct Paths { /* directories::ProjectDirs */ }
impl Paths { pub fn config(&self) -> &Path; pub fn data(&self) -> &Path; pub fn cache(&self) -> &Path;
             pub fn state(&self) -> &Path; pub fn runtime(&self) -> Option<&Path>; pub fn documents() -> Option<PathBuf>; }
```

```ts
import { paths, app } from 'svelters:paths';   // read-only strings; app.name, app.version (KAboutData-lite)
```

The same `AppId` names the default settings file, the UI-state directory and notifications. cadatomic's single-instance
socket (QS11) can use it too.

### 2.3 Undo: a seam, not a stack (QA1, QA3, QA6)

cadatomic's undo is decided and complete. Svelte panels need only one thing: Edit-menu commands and undo keys must
reach a focused Svelte text field before the document.

```rust
pub struct FocusInfo { pub node: Option<NodeId>, pub root: Option<RootId>, pub text_input: bool, pub composing: bool,
                       pub claims: KeyClaims /* text, navigation, explicit chords from use:keys */ }
impl Embed {
    pub fn focus_info(&self) -> FocusInfo;                               // cad's TextFocus layer reads this
    pub fn shortcut_override(&self, chord: &KeyChord) -> bool;           // Qt's ShortcutOverride: true = let the widget have it
    pub fn text_command(&self, cmd: TextCommand) -> bool;                // Undo|Redo|Cut|Copy|Paste|SelectAll on the focused field
}
```

A generic `svelters:undo` (`UndoStack` with merge ids and a clean index) is a P2 pack for non-cadatomic apps. It is
not scheduled.

### 2.4 Actions and keys (QA1, QA4, QD11)

cadatomic's registry, keymap and palette stay the source of commands. svelters adds:

- **Claims.** Built-in kinds declare which keys they consume (`text-edit`: text and editing keys; `table`/`tree-view`:
  navigation keys; `slider`: arrows). `shortcut_override` answers from them, and cadatomic's layers decide the rest.
- **Component-scope keys**, like QML `Shortcut` with a scope:

  ```svelte
  <script> import { keys } from 'svelters:behaviors/keys'; </script>
  <tree-view use:keys={{ 'Delete': removeSelected, 'Primary+D': duplicate, 'F2': rename }} scope="focus-within" …/>
  ```

  It is a native behaviour (EV2b): matching runs in Rust and only the handler runs JS. Its chords are reported as
  claims, so cadatomic sees them as `TextFocus`-layer bindings and can show conflicts. cadatomic's own `shortcut`
  behaviour (SV2) can sit on the same router.
- **`action` shortcuts** accept chords (`"Primary+K Primary+C"`) and standard names (`"std:redo"`). They are resolved
  through a host `StandardKeys` table: cadatomic's keymap provides it, and svelters ships per-platform defaults.

### 2.5 i18n and Intl (QA15)

```rust
pub trait Locale: Send + Sync + 'static {
    fn tag(&self) -> &str;
    fn format_number(&self, x: f64, o: &NumberOpts) -> String;   // grouping, min/max fraction digits, sign, compact
    fn parse_number(&self, s: &str) -> Option<f64>;               // accepts "1,5" in de-DE
    fn plural(&self, n: f64, ordinal: bool) -> PluralCategory;
    fn compare(&self, a: &str, b: &str) -> std::cmp::Ordering;    // collation for sorted lists
    fn format_date(&self, t: jiff::Timestamp, o: &DateOpts) -> String;
}
// default: ICU4X compiled data behind feature `intl` (same icu4x version as cad's PB10; binary cost **verify**)
```

- **JS side.** The prelude installs a subset of `Intl` (`NumberFormat`, `PluralRules`, `Collator`, `DateTimeFormat`,
  `RelativeTimeFormat`) and `Number.prototype.toLocaleString` over the host `Locale`, so stock libraries work.
  `import { locale } from 'svelters:intl'` is reactive: changing the language re-renders, like KDE's "switch
  application language".
- **Widgets.** `drag-value`, `slider` and `table` format numbers with the host locale. Text kinds offer
  `inputmode="decimal"`.
- **Not in svelters.** Catalogs and `t()` stay cadatomic's (`catalog.t`, Fluent). A literal-text lint for `.svelte`
  templates is a cadatomic SV8 provider check, which svelters' CX1 codes and fix-its already support.

### 2.6 File watching and process (QD3, QD5)

```ts
import { watch } from 'svelters:fs';                     // P2, behind FsPolicy (deny for isolates)
for await (const ev of watch(dir, { recursive: true, debounceMs: 100, signal })) { … }  // notify, already in the tree
import { spawn } from 'svelters:process';                 // P2 pack; cadatomic uses outboard instead
const p = spawn('ngspice', ['-b', deck], { cwd, signal }); for await (const line of p.lines('stdout')) { … } await p.exited;
```

### 2.7 Clipboard and drag data (QA9, QA10)

```rust
pub struct MimeData(Vec<(String /* mime */, Bytes)>);   // QMimeData: one type for clipboard and DnD (WG4b DragPayload grows into it)
pub enum ClipboardKind { Clipboard, Primary }
pub trait ClipboardBackend: Send + Sync + 'static {
    fn read(&self, k: ClipboardKind, accept: &[&str]) -> Result<MimeData, ClipError>;
    fn write(&self, k: ClipboardKind, data: MimeData) -> Result<(), ClipError>;
    fn formats(&self, k: ClipboardKind) -> Vec<String>;  // + a change signal (dataChanged)
}
// default: arboard for text/plain, text/html, image/png; app MIME types kept in-process with a text/plain fallback
// (arboard support for custom formats **verify**; clipboard-rs if cross-process app formats are needed)
```

```ts
import { clipboard } from 'svelters:clipboard';
await clipboard.write({ 'text/plain': tsv, 'text/html': html, 'application/x-cadatomic-refs+json': refs });
const got = await clipboard.read(['application/x-cadatomic-refs+json', 'text/plain']);   // first available wins
```

quire, Rust panels and Svelte share the one backend, so a table copied in Svelte pastes into quire, and a Rust tree
drags into a Svelte table with the same `MimeData`.

### 2.8 Shell, notifications, logging (QD6, QS14, QD7)

```ts
import { openUrl, openPath, revealInFolder, requestAttention } from 'svelters:shell';
// openPath: default app; reveal: FileManager1.ShowItems over D-Bus / `open -R` / `explorer /select,` (**verify**)
import { notify } from 'svelters:notify';                 // P2 pack: notify-rust / macOS / Windows toasts
const action = await notify({ title: 'Sweep done', body: '42 points', actions: ['Open'], signal });
import { logger } from 'svelters:log';
const log = logger('cad.task');                           // tracing target `js::cad.task`; console.* → target `js`
log.debug('rebased', { slot });                           // fields kept; RUST_LOG / EnvFilter rules (QLoggingCategory)
```

The console buffer stays for tests and devtools. Each record carries the root (`RootKey`) and the source-mapped
`file:line`. cadatomic's subscriber adds its trace and execution ids.

## 3. Recommended order: the next svelters lanes

The ordering criterion: P0 gaps first, then the lanes that unblock most cadatomic surfaces. Lane names follow
TRACKS.md where a track exists. The egui lanes touch different files from the core/DX lanes, so the two run in
parallel (TRACKS §4.3).

| # | Lane | Content (gap ids) | Needs | Size | Acceptance |
|---|---|---|---|---|---|
| 1 | **EV3+ keys and focus** | EV3 as planned (scopes, restore after popup/modal, traversal, sync key handlers) + QA1 claims and `shortcut_override`/`focus_info` + QA3 `text_command` + QW11 default/cancel buttons and platform button order + QA12 composing flag + QA8 Menu key | WG2a, FX1 merged | 7–9 | kittest: Tab cycles a dialog and returns focus to the opener; Ctrl+Z in a `text-edit` reaches the field while cad's keymap stub sees `shortcut_override = true`; Enter activates the default button except while composing |
| 2 | **WG5a item-view interaction** | QW2 keyboard nav + type-ahead on `virtual-list` and `table`; QW5/SV46 key-based selection; QW3 edit protocol (edit triggers, commit/revert, Tab to the next cell, a provider-kind editor in a cell); QW17 sticky sections | LS1 (`ci/lists`) + SG4 merged | 8–10 | a 100k-row table is fully usable from the keyboard; F2 edits a cell with a `QuantityField`-shaped test kind; selection survives a filter |
| 3 | **WG6 affordance polish** | QA5 tooltips on disabled widgets (first, it is P0 and tiny) + `{#snippet tooltip()}` + `status-tip`; QW13 middle elision; QA13 `scrollIntoView` + scroll on focus; QW14 SVG; QA14 `cursor`; QW10 `key-sequence-edit`; QW9 combo type-ahead; QA9 file drops on nodes, drop actions, auto-scroll | WG1, WG2b, WG4b merged | 5–7 | gallery page per item; the disabled button's reason shows on hover; a dropped file arrives as `drop` with `text/uri-list` |
| 4 | **WG5b tree-view** | QW1 on `RowPool`/`TreeFlatten`-shaped rows; lazy children; expand/collapse all; DnD auto-expand; set-size and level a11y | WG5a | 6–8 | cad MD3's tree drives it with no JS per scrolled row (counted) |
| 5 | **DX1 settings, state, paths, logs** | QD1 (`SettingsBackend`, `#[derive(Settings)]`, `setting()`, `UiStateBackend`, `uiState()`), QD2 (`AppId`, `Paths`), QD7 (console → `tracing`, `logger`), QS13 core half (JS stacks + last N ops in a crash snapshot) | E3, LD1 (have) | 4–6 | a locked key rejects and shows its source; split position and column widths survive a restart through a test backend; `RUST_LOG=js::cad=debug` filters |
| 6 | **DX2 clipboard and shell** | QA10 (`MimeData`, `ClipboardBackend`, `svelters:clipboard`, PRIMARY through the same trait), WG4b `DragPayload` → `MimeData`, QD6 (`svelters:shell`), attention | WG2b, WG4b merged | 3–4 | multi-format copy from a Svelte table pastes as HTML into a test sink and as TSV into `text-edit` |
| 7 | **IN1 Intl** | QA15: `Locale` trait, ICU4X backend behind `intl`, `Intl` subset + `toLocaleString`, reactive `locale`, locale-aware numbers in `drag-value`/`slider`/`table` | — | 3–4 | de-DE shows and accepts `1,5`; `PluralRules('en').select(1)` is `one`; switching locale re-renders with no reload |
| 8 | **TK3+ accessibility** | TK3 as planned + QA11 `announce(text, politeness)` and live regions for toast/status/progress + labelled-by relations | WG4a merged | 4–6 | AccessKit tree audit in kittest; a toast is announced |
| 9 | **PK1 plot pack** (after the eight above, or in parallel if a slot is free) | QW20: `plot` pack over `egui_plot` with `Shared<[f32]>` series, min/max decimation, cursors with delta readout, several y-axes, PNG/SVG export | E2 (have) | 5–7 | 10M-sample series pans at frame budget; cursor delta matches the data |

- **Not scheduled in svelters:**
  - QA6/QA7 undo (cadatomic);
  - QS9 recent files, QS11 single instance, QS12 About, QW23 printing, QS16 settings UI, QS7 keymap editor. The
    svelters coordinator relays these to main-cadatomic as gaps found, not as requests.
- **Later:** QW15, QW16, QA16, QA17, QS5 and QS14 (packs and P2 items).

## 4. Qt behaviours and facts to verify before relying on them

| Item | Claim | Why it matters |
|---|---|---|
| QA5 | Qt tooltip wake-up ≈ 700 ms, instant follow-on within a fall-asleep window (`QStyle::SH_ToolTip_WakeUpDelay`/`FallAsleepDelay`) | default tooltip timing for `tooltip-delay` tokens |
| QA5 | egui `Response::on_hover_text` does not show on disabled widgets (needs `on_disabled_hover_text`) | WG1's tooltip may hide disable-with-reason |
| QW18 | `QProgressDialog::minimumDuration` defaults to 4000 ms | the "show busy only after N ms" default |
| QA4 | `QKeySequence::Redo` is Ctrl+Y on Windows and Ctrl+Shift+Z on Linux/macOS (KDE may differ) | the `std:redo` table |
| QA11 | `QAccessibleAnnouncementEvent` arrived in Qt 6.8 | prior art for `announce()` |
| QD1 | `KSharedConfig::openStateConfig` (KF 5.67+) splits state from settings; `QStandardPaths::StateLocation` arrived in Qt 6.7 | settings vs `uiState` split |
| QW13 | egui labels elide only at the end (`Label::truncate`) | middle elision is ours to build |
| QA13 | whether egui scrolls a focused widget into view when focus moves by Tab | scope of `scrollIntoView` |
| QA9 | winit has no drag-source API (no drag-out to the OS) | drag-out stays P2 |
| QA10 | arboard has no arbitrary custom formats | in-process app MIME vs `clipboard-rs` |
| QW20 | `egui_plot` log axes (native or by transform plus a formatter) | pack scope |
| QA15 | ICU4X compiled-data binary cost for number, plural, collation and date | the `intl` feature default |
| QD6 | reveal-in-folder mechanics per platform | `revealInFolder` |
| QS18 | quire brace matching and bookmarks | the QScintilla cross-check |
| QA8 | WG2a opens context menus from the Menu key or Shift+F10 | QA8 may already be done |

## Sources

- Qt 6 documentation (from memory): Qt Widgets (`QMainWindow`, `QAction`, `QShortcut`, `QKeySequence`, `QUndoStack`,
  `QCompleter`, `QValidator`, `QDataWidgetMapper`, model/view, `QHeaderView`, `QWizard`, dialogs, `QToolTip`,
  `QScrollArea`, spin boxes), Qt Quick Controls (`Popup`, `Drawer`, `ToolTip`, `ScrollIndicator`, `Overlay`,
  `SplitView`, `StackView`), QtCore/QtGui (`QSettings`, `QStandardPaths`, `QFileSystemWatcher`, `QProcess`,
  `QDesktopServices`, `QClipboard`, `QMimeData`, `QDrag`, `QTranslator`, `QLocale`, `QLoggingCategory`,
  `QAccessible`, `QSessionManager`).
- KDE Frameworks 6 (from memory): KXmlGui, KConfig/KConfigXT, KStandardAction, KShortcutsDialog, KCommandBar,
  KNotifications, KMessageWidget, KPageDialog, KRecentFilesAction, KAboutData, KCrash, KJob/KUiServer, Sonnet,
  KTextEditor.
- Third party (from memory): Qt Advanced Docking System, KDDockWidgets, QScintilla, QCustomPlot, Qwt,
  QtPropertyBrowser, QtNodes, SARibbon.
- svelters: `plans/research/{EGUI,TRACKS,CADATOMIC,ARCH-EMBED,ARCH-SCENE,QUICKJS-RUNTIME}.md`, `STATUS.md`,
  `metadata/semantics/egui.json`, `crates/svelters-host/src/{prelude.js,core.rs,quickjs.rs}`,
  `crates/svelters-style/src/table.rs`; branch milestone files on the `ci/*` tips listed at the top.
- cadatomic: `docs/plan/00-decisions.md`, `docs/plan/04-kernel-requests.md`,
  `docs/research/2026-10-22-{ide-shell-and-ux,widget-survey-and-native-acceptance,component-framework,models-filtering-and-dynamic-data,code-editor-and-lsp,docking-windows-and-svelters-roots}.md`,
  `docs/research/2026-10-16-command-architecture.md`.
