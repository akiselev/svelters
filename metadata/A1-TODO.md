# A1 TODO: annotation batches

Inventory of the GTK4 and QtWidgets widget surface for A1 (`plans/milestones/A1-api-surface.md`),
grouped into batches a worker finishes in one session. Workers follow `metadata/A1-WORKER-gtk.md`
or `metadata/A1-WORKER-qt.md`. Framework gaps that block facts are in `metadata/A1-GAPS.md`;
a batch whose remaining facts all need a gap is marked `blocked: gap:<id>`.

Status values: `todo | in-progress (<worker>) | done | blocked: <why>`. The status line of a done
batch also lists facts that were tried and dropped because their generated test failed.

Changes since the inventory was written (2026-10-17):
- S1 (ABI v3 value model) is merged: enums travel as names, and flags, records, dates and
  string-form boxed types (colours, fonts, icons, key sequences, URLs, GFile) convert on both
  toolkits. Facts marked G4 below are now worth trying; Qt enum props need no `values` map unless
  the JS vocabulary differs from the Qt names. Re-check G4 items before marking them blocked.
- X2 (sync handlers) is merged: GTK signals with `handler_result: bool_stop` and Qt synthetic
  `close_request`/`key_press` let JS decide (`e.preventDefault()`). Annotate `handler_result` and
  `sync_required` (see `docs/notes/x2-sync-frame.md`, "What A1 should annotate"); items marked G5
  with bool results are now doable. Qt input widgets in your batch can get `synthetic: key_press`.

- X1 is merged: GTK methods are callable through GIRepository (`dispatch: dynamic` annotations
  with `args`/`expect`), and object getters on both toolkits return borrowed handles. Items marked
  G2 or G3 are now doable (except out params, callbacks and arrays: S2).

Common to every batch:
- **Facts to annotate**, by column: P = properties (JS name, sample, echo suppression), S =
  signals (trigger, handler result), M = methods, C = child slots (ops + effects = ownership),
  B = two-way bindings (each needs the echo check: an `echo <name>: N` label on the gallery page),
  E = enums (Qt: `values` maps), L = lifetime override (only when the derived block is wrong).
- **Test commands** are in the worker brief; `<re>` below is the batch's test filter.
- Every batch ends with the full gates and `ledger.py refresh`.

## GTK

Coverage now: `metadata/coverage/gtk.md`. GTK reaches every property and signal by reflection, so
GTK batches are about verified facts, slots, bindings, signal semantics and the gallery.

### gtk-base: GtkWidget, GtkEditable, GtkOrientable, GtkActionable

- Kinds: Widget (verify_subject `label`), Editable (subject `entry`), Orientable (`box`), Actionable (`button`).
- Depends on: nothing.
- P: Widget `visible`, `sensitive`, `tooltip-text`, `tooltip-markup`, `hexpand`, `vexpand`, `halign`, `valign`,
  `margin-start/end/top/bottom`, `width-request`, `height-request`, `opacity`, `can-focus`, `focusable`, `name`,
  `css-classes` (blocked G4: string list); Editable `editable`, `width-chars`, `max-width-chars`, `xalign`,
  `enable-undo`; Actionable `action-name` (string; actions themselves are out of scope).
- S: Widget `show`, `hide`, `map`, `unmap` (trigger: property `visible`), `destroy` (skip: lifetime owns it);
  Editable `insert-text`/`delete-text` (G5: run-last with params, annotate delivery only).
- M: Widget `grab_focus` (G2: returns bool), `queue_draw`, `queue_resize` (expect: true).
- Gallery: `01-widget.svelte` (sensitive/tooltip/align/margins/opacity on labels and buttons).
- Tests `<re>`: `_(widget|editable|orientable|actionable)_`.
- Status: done (54 facts in the gtk ledger touched by this batch incl. 19 P, 3 S, 4 M; dropped: Widget.css-classes (no list property test), Widget map/unmap (no mapped toplevel in harness); handler_result/sync_required left as derived, no triggers; note: gi_calls.rs asserts resolve_method(present) <= 1, brittle once more generated methods exist)

### gtk-buttons: buttons and toggles

- Kinds: Button (more), ToggleButton, CheckButton, Switch, LinkButton, LockButton, MenuButton, ScaleButton, VolumeButton.
- Depends on: gtk-base (soft: gallery uses base props).
- P: Button `has-frame`, `icon-name`, `use-underline`, `can-shrink`; ToggleButton `active`; CheckButton `active`,
  `inconsistent`, `label`, `use-underline`, `group` (reference prop, role: reference; test with `rust` trigger or leave);
  Switch `active`, `state`; LinkButton `uri`, `visited`; MenuButton `label`, `icon-name`, `direction`, `has-frame`,
  `always-show-arrow`, `primary`; ScaleButton/VolumeButton `value` (G7: adjustment 0..0, see gtk-ranges).
- S: ToggleButton/CheckButton `toggled` (trigger: property `active`), Switch `notify::active`, `state-set` (G5: veto),
  LinkButton `activate-link` (G5), MenuButton `activate`.
- C: MenuButton `popover` (one; `set_popover`), Button `child` (one; `set_child`, replaces the label).
- B: ToggleButton/CheckButton `bind:active` (signal `toggled`, value: prop, echo_suppress [toggled, notify::active]);
  Switch `bind:active` (signal `notify::active`, value: prop).
- M: MenuButton `popup`, `popdown` (zero-arg; expect on `active`/visibility of the popover).
- Gallery: `02-buttons.svelte`.
- Tests `<re>`: `_(button|togglebutton|checkbutton|switch|linkbutton|lockbutton|menubutton|scalebutton|volumebutton)_`.
- Status: done (~45 facts verified: props, signals incl. sync state-set/activate-link, 4 bindings, popup/popdown + 8 dynamic methods; not attempted: CheckButton.group, ScaleButton.icons, LockButton.permission; blocked: MenuButton `popover` slot (new gap: typed child slots, generator casts to gtk::Widget); see docs/notes/a1-gtk-buttons.md)

### gtk-text: text entry and display text

- Kinds: Entry (more), PasswordEntry, SearchEntry, Text, EditableLabel, Label (more), Inscription, TextView, ShortcutLabel.
- Depends on: gtk-base (Editable).
- P: Entry `visibility`, `max-length`, `has-frame`, `input-purpose`, `activates-default`, `overwrite-mode`,
  `placeholder-text` (done); PasswordEntry `show-peek-icon`, `placeholder-text`; SearchEntry `placeholder-text`,
  `search-delay`; EditableLabel `editing`; Label `use-markup`, `use-underline`, `wrap`, `wrap-mode`, `selectable`,
  `justify`, `ellipsize`, `xalign`, `yalign`, `lines`, `max-width-chars`; Inscription `text`, `min-lines`, `nat-chars`;
  TextView `editable`, `wrap-mode`, `monospace`, `cursor-visible`, `left-margin`, `top-margin`, `justification`;
  ShortcutLabel `accelerator`.
- S: Entry `activate` (trigger `rust`: `o.downcast_ref::<gtk::Entry>().unwrap().emit_activate()`), SearchEntry
  `search-changed`, `activate`, `stop-search`; Label `activate-link` (G5).
- B: PasswordEntry/SearchEntry/Text `bind:text` inherit from Editable (check the binding table lists them);
  EditableLabel `bind:text`.
- Blocked: TextView text (G3: buffer), Label `attributes` (G4).
- Gallery: `03-text.svelte`.
- Tests `<re>`: `_(entry|passwordentry|searchentry|text|editablelabel|label|inscription|textview|shortcutlabel)_`.
- Status: done (~64 facts verified: 40 props, 8 signals, 5 bindings, 8 methods, plus the 4 bind:text; dropped: Label.ellipsize and Label.wrap-mode (Pango enums not in metadata); blocked: TextView text and buffer methods (GtkTextBuffer has no kind; only get_buffer verified), Label.attributes (no value form for Pango AttrList), Label activate-link verified as bool_stop sync; see docs/notes/a1-gtk-text.md)

### gtk-ranges: ranges, progress and levels

- Kinds: ProgressBar, LevelBar, Scale, SpinButton, Scrollbar, Range (abstract).
- Depends on: gtk-base.
- P: ProgressBar `fraction`, `text`, `show-text`, `pulse-step`, `inverted`, `ellipsize`; LevelBar `value`,
  `min-value`, `max-value`, `mode`, `inverted`; Scale `digits`, `draw-value`, `has-origin`, `value-pos`;
  SpinButton `digits`, `numeric`, `wrap`, `snap-to-ticks`, `climb-rate`, `update-policy`; Range `inverted`,
  `show-fill-level`, `restrict-to-fill-level`, `fill-level`.
- S: LevelBar `offset-changed` (detailed), SpinButton `value-changed`, `wrapped`; Range `value-changed`.
- M: ProgressBar `pulse` (zero-arg).
- B: LevelBar `bind:value` (notify::value, value: prop).
- Blocked: Scale/SpinButton/Range `value` and the range itself (G7: the GtkAdjustment is a non-widget object;
  `set_range` needs G2); SpinButton `input`/`output` (G5).
- Gallery: `04-ranges.svelte`.
- Tests `<re>`: `_(progressbar|levelbar|scale|spinbutton|scrollbar|range)_`.
- Status: todo

### gtk-containers-a: single-child and two-pane containers

- Kinds: CenterBox, Paned, Frame, Expander, Revealer, AspectFrame, Overlay, WindowHandle, Viewport, ScrolledWindow (more), SearchBar.
- Depends on: nothing.
- C: CenterBox `start`/`center`/`end` (one; `set_start_widget` ...); Paned `start`/`end` (one; `set_start_child`/
  `set_end_child`); Frame `child`, `label` (one; `set_label_widget`); Expander `child`, `label`; Revealer `child`;
  AspectFrame `child`; Overlay `child` (one) and `overlays` (many; `add_overlay`/`remove_overlay`, contains: parent);
  WindowHandle `child`; Viewport `child`; SearchBar `child`.
- P: Paned `position`, `wide-handle`, `resize-start-child`, `shrink-end-child` ...; Frame `label`, `label-xalign`;
  Expander `expanded`, `label`, `use-markup`; Revealer `reveal-child`, `transition-type`, `transition-duration`;
  AspectFrame `ratio`, `obey-child`, `xalign`; ScrolledWindow `hscrollbar-policy`, `vscrollbar-policy`, `has-frame`,
  `min-content-width`, `propagate-natural-height`; SearchBar `search-mode-enabled`, `show-close-button`.
- S: Expander `activate`; ScrolledWindow `edge-reached` (G5? delivery only).
- B: Expander `bind:expanded` (notify::expanded); Revealer `bind:reveal-child`? (one-way: read_only `child-revealed`).
- Gallery: `05-containers.svelte`.
- Tests `<re>`: `_(centerbox|paned|frame|expander|revealer|aspectframe|overlay|windowhandle|viewport|scrolledwindow|searchbar)_`.
- Status: done (79 facts verified across gtk ledger incl. 63 batch tests; dropped: Viewport.scroll_to and SearchBar.connect_entry (need object/adjustment args, not useful), Revealer bind (child-revealed read-only), Overlay get-child-position (needs sync value result, G5), Paned/Frame/Expander child attributes (G1); edge-reached delivery-only via emit_by_name; one test flaky once, passed on rerun)

### gtk-containers-b: multi-child layouts and pages

- Kinds: Stack, StackSwitcher, StackSidebar, Notebook, FlowBox, FlowBoxChild, ListBox, ListBoxRow, ActionBar, Grid, Fixed.
- Depends on: gtk-containers-a (soft).
- C: Stack `children` (many; `add_child`/`remove`, order append_only); Notebook `pages` (many; `insert_page(child, null,
  index_opt)`/`remove_page(child_index_opt)`, index_of `page_num`); FlowBox `children` (many; `insert(child, index)`/
  `remove`, wrapped:FlowBoxChild, contains: ancestor); ListBox `children` (many; `insert(child, index)`/`remove`,
  wrapped:ListBoxRow); ActionBar `start`/`end` (many; `pack_start`/`pack_end`/`remove`), `center` (one).
- P: Stack `visible-child` (reference), `transition-type`, `hhomogeneous`; StackSwitcher/StackSidebar `stack` (reference);
  Notebook `page`, `show-tabs`, `show-border`, `tab-pos`, `scrollable`; FlowBox `selection-mode`, `min-children-per-line`,
  `max-children-per-line`, `homogeneous`, `column-spacing`, `row-spacing`; ListBox `selection-mode`, `show-separators`,
  `activate-on-single-click`; ActionBar `revealed`.
- S: Notebook `switch-page`; ListBox `row-activated`, `row-selected`; FlowBox `child-activated`, `selected-children-changed`.
- B: Notebook `bind:page` (notify::page, value: prop).
- Blocked: Grid and Fixed children (G1), Notebook tab labels (G1), Stack names/titles (G1),
  ListBox/FlowBox sort/filter (G8).
- Gallery: `06-layout.svelte`.
- Tests `<re>`: `_(stack|stackswitcher|stacksidebar|notebook|flowbox|flowboxchild|listbox|listboxrow|actionbar|grid|fixed)_`.
- Status: todo

### gtk-bars: header bars, window controls, status

- Kinds: HeaderBar (more), WindowControls, Statusbar, PopoverMenuBar, InfoBar.
- Depends on: nothing.
- P: HeaderBar `show-title-buttons`, `decoration-layout`, `use-native-controls`; WindowControls `side`,
  `decoration-layout`.
- Mark unexposed: Statusbar (deprecated 4.10; push/pop need G2), InfoBar (deprecated 4.10).
- Blocked: PopoverMenuBar, PopoverMenu (G7: GMenuModel).
- Gallery: `07-bars.svelte`.
- Tests `<re>`: `_(headerbar|windowcontrols)_`.
- Status: todo

### gtk-dialogs: windows, popovers and non-blocking dialogs

- Kinds: Window (props), Popover, AboutDialog, ShortcutsWindow (unexposed?), EmojiChooser.
- Depends on: gtk-buttons (MenuButton popover slot).
- P: Window `resizable`, `modal`, `decorated`, `deletable`, `default-width`, `default-height`, `maximized`,
  `fullscreened`, `hide-on-close`, `transient-for` (reference); Popover `autohide`, `has-arrow`, `position`,
  `cascade-popdown`; AboutDialog `program-name`, `version`, `comments`, `website`, `copyright`, `license-type`,
  `wrap-license`.
- C: Popover `child` (one).
- S: Popover `closed`, `activate-default`; EmojiChooser `emoji-picked` (string arg).
- M: Window `close`, `minimize`, `maximize`, `unmaximize`, `fullscreen`, `unfullscreen` (zero-arg; expect on the props);
  Popover `popup`, `popdown`.
- Never: `gtk_dialog_run` and other blocking calls. Deprecated dialogs (Dialog, MessageDialog, FileChooserDialog,
  ColorChooserDialog, FontChooserDialog, AppChooserDialog, PageSetupUnixDialog, PrintUnixDialog, Assistant) are
  marked unexposed with the reason.
- Gallery: `08-dialogs.svelte` (a button that opens an AboutDialog and a MenuButton with a popover).
- Tests `<re>`: `_(window|popover|aboutdialog|emojichooser)_`.
- Status: todo

### gtk-display: images, separators, spinners, calendar

- Kinds: Image, Picture, Separator, Spinner, Calendar, Video, MediaControls, DrawingArea, GraphicsOffload.
- Depends on: nothing.
- P: Image `icon-name`, `pixel-size`, `icon-size`, `file` (string path), `resource`; Picture `alternative-text`,
  `can-shrink`, `content-fit`; Spinner `spinning`; Calendar `day`, `month`, `year`, `show-heading`,
  `show-day-names`, `show-week-numbers`; DrawingArea `content-width`, `content-height`.
- S: Calendar `day-selected`, `next-month`, `prev-month`, `next-year`, `prev-year`.
- M: Spinner `start`, `stop` (expect on `spinning`).
- B: Spinner `bind:spinning`? (notify::spinning; only if it is useful), Calendar `bind:day` (notify::day).
- Blocked: Image/Picture paintable and gicon (G4), Calendar date (G4), DrawingArea drawing (G8), Video/MediaControls
  media streams (G7).
- Gallery: `09-display.svelte`.
- Tests `<re>`: `_(image|picture|separator|spinner|calendar|video|mediacontrols|drawingarea|graphicsoffload)_`.
- Status: done (31 generated tests: 18 props, 2 notify signals + day-selected, 2 bindings (spinning, day) with echo tests, 5 methods; dropped: Calendar.date (no generated test for boxed props), Image.file/resource/gicon/paintable, Picture.file/paintable (no verifiable generated test form), Calendar next/prev-month/year signals (no trigger); blocked: gap:G7 for Video/MediaControls, gap:G8 for DrawingArea drawing/resize; Separator has no own facts)

### gtk-leftovers: everything not worth exposing yet

- Kinds: ColumnView, ListView, GridView, TreeView, IconView, ComboBox, ComboBoxText, CellView, TreeExpander, DropDown
  (G6); ColorButton, FontButton, ColorChooserWidget, FontChooserWidget, AppChooserButton, AppChooserWidget,
  ColorDialogButton, FontDialogButton (deprecated or G4/G7); FileChooserWidget (deprecated 4.10); DragIcon,
  GLArea, ShortcutsGroup, ShortcutsSection, ShortcutsShortcut, PopoverBin, Text (internal), ListBase.
- Work: add `unexposed: {reason}` overrides (or a short note pointing at the gap) so the coverage report says why.
  `ComboBoxText` (deprecated 4.10) stays unexposed; `DropDown` waits for G6/G4.
- Gallery: none.
- Status: todo

## Qt

Coverage now: `metadata/coverage/qt.md`. Qt exposes only annotated facts, so Qt batches add
elements and facts; `inherit:` blocks on base classes carry facts to every subclass element.

Tag names: the GTK tag when the widget is the same concept and the shared props mean the same
(`button`, `entry`, `label`, `box`, `window`, `scrolledwindow`, `checkbutton`?) — check
`packages/renderer/types/gtk.d.ts` and `gen_bindings.py` (shared tags must agree on bindings);
otherwise the lower-case Qt class name without the Q (`checkbox`, `slider`, `spinbox`, `tabwidget`).

### qt-base: inherit blocks for QWidget, QAbstractButton, QAbstractSlider, QAbstractSpinBox, QFrame

- Kinds: QWidget, QAbstractButton, QAbstractSlider, QAbstractSpinBox, QFrame (inherit blocks only; elements come in later batches).
- Depends on: nothing.
- P: QWidget `enabled`, `toolTip`, `statusTip`, `whatsThis`, `minimumWidth`, `minimumHeight`, `maximumWidth`,
  `maximumHeight`, `visible`? (careful: window), `styleSheet`, `accessibleName`; QAbstractButton `text` (JS `label`),
  `checkable`, `checked`, `autoRepeat`, `autoExclusive`, `shortcut` (G4); QAbstractSlider `minimum`, `maximum`, `value`,
  `singleStep`, `pageStep`, `tracking`, `invertedAppearance`, `orientation` (enum map horizontal/vertical);
  QAbstractSpinBox `readOnly`, `wrapping`, `frame`, `specialValueText`, `buttonSymbols` (enum), `accelerated`;
  QFrame `frameShape`, `frameShadow` (enums), `lineWidth`, `midLineWidth`.
- S: QAbstractButton `clicked`, `toggled(bool)`, `pressed`, `released`; QAbstractSlider `valueChanged(int)`,
  `sliderMoved(int)`, `rangeChanged(int,int)`; QAbstractSpinBox `editingFinished`.
- M: QAbstractButton `click`, `toggle`, `animateClick`; QWidget `show`, `hide`, `setFocus`, `update`, `adjustSize`
  (careful: `close` deletes WA_DeleteOnClose widgets; leave it on window only).
- B: QAbstractButton `bind:checked` (toggled(bool), arg0); QAbstractSlider `bind:value` (valueChanged(int), arg0).
- Note: the existing `button` element declares `label`/`click`/`clicked` itself; inherited facts with the same JS
  name are skipped, so keep or drop those duplicates deliberately.
- Gallery: `01-widget.svelte` (enabled, toolTip, sizes on existing elements).
- Tests: the generated Qt suite (`cargo nextest run -p counter-qt`), filter output for `QWidget|button|...`.
- Status: done (inherit facts verified through existing elements: QWidget 10 props + 5 methods; QAbstractButton props label/checkable/autoRepeat/autoExclusive/shortcut, signals clicked/pressed/released/toggled, methods click/toggle/animateClick; QAbstractSpinBox 6 props + editingFinished; QFrame 4 props; QAbstractSlider 8 props + 3 signals declared but untested until a slider element exists; dropped: `visible` (by brief), `checked` excluded on button (not checkable by default; verified with checkbox batch); blocked: bind:checked/bind:value, inherit blocks have no bindings, subclass elements add them in their batches)

### qt-buttons: buttons and toggles

- Kinds: QPushButton (more), QCheckBox, QRadioButton, QToolButton, QCommandLinkButton.
- Depends on: qt-base.
- Elements: `checkbox` (QCheckBox), `radiobutton` (QRadioButton), `toolbutton` (QToolButton), `commandlinkbutton`.
- P: QPushButton `flat`, `autoDefault`, `default`; QCheckBox `tristate`, `checkState` (enum via getter/setter
  `checkState`/`setCheckState`); QRadioButton (inherits); QToolButton `autoRaise`, `toolButtonStyle`, `arrowType`,
  `popupMode` (enums); QCommandLinkButton `description`.
- S: QCheckBox `checkStateChanged(Qt::CheckState)` (G4 unless an int arg works) / `stateChanged(int)`.
- B: QCheckBox/QRadioButton `bind:checked` (inherited from QAbstractButton).
- Blocked: QToolButton menus (G7), icons (G4).
- Gallery: `02-buttons.svelte`.
- Status: todo

### qt-text: text entry and text display

- Kinds: QLineEdit (more), QLabel (more), QPlainTextEdit, QTextEdit, QTextBrowser, QKeySequenceEdit.
- Depends on: qt-base.
- Elements: `plaintextedit`, `textedit`, `textbrowser`.
- P: QLineEdit `readOnly`, `maxLength`, `echoMode` (enum), `clearButtonEnabled`, `frame`, `inputMask`, `dragEnabled`,
  `cursorPosition`; QLabel `wordWrap`, `textFormat` (enum), `openExternalLinks`, `indent`, `margin`,
  `textInteractionFlags`? (flags: G4); QPlainTextEdit `plainText`, `readOnly`, `placeholderText`, `lineWrapMode`
  (enum), `tabChangesFocus`, `overwriteMode`; QTextEdit `plainText`, `html`, `markdown`, `acceptRichText`, `readOnly`,
  `placeholderText`; QTextBrowser `openExternalLinks`, `openLinks`.
- S: QLineEdit `returnPressed`, `editingFinished`, `textEdited(QString)`, `selectionChanged`; QLabel
  `linkActivated(QString)`, `linkHovered(QString)`; QPlainTextEdit/QTextEdit `textChanged()`.
- M: QLineEdit `clear`, `selectAll`, `undo`, `redo`, `copy`?, `setSelection(int,int)`; QPlainTextEdit
  `appendPlainText(QString)`, `clear`; QTextEdit `append(QString)`, `clear`.
- B: QPlainTextEdit/QTextEdit `bind:plainText` (textChanged(), value: prop, suppress_signals on the prop).
- Blocked: QTextBrowser `source` (G4 QUrl), QKeySequenceEdit (G4 QKeySequence), `document()` (G3).
- Gallery: `03-text.svelte`.
- Status: done (see ledger; dropped: QTextEdit html (toHtml returns a full document); not tried: QTextBrowser QUrl signals, QKeySequenceEdit keySequenceChanged (boxed signal args), QTextBrowser readOnly/undoRedoEnabled (no accessors))

### qt-ranges: sliders, spin boxes, progress

- Kinds: QSlider, QDial, QScrollBar, QSpinBox, QDoubleSpinBox, QProgressBar, QLCDNumber.
- Depends on: qt-base.
- Elements: `slider`, `dial`, `scrollbar`, `spinbox`, `doublespinbox`, `progressbar`, `lcdnumber`.
- P: QSlider `tickPosition` (enum), `tickInterval`; QDial `notchesVisible`, `wrapping`, `notchTarget`; QSpinBox
  `value`, `minimum`, `maximum`, `singleStep`, `prefix`, `suffix`, `displayIntegerBase`; QDoubleSpinBox `value` (f64),
  `minimum`, `maximum`, `decimals`, `singleStep`; QProgressBar `value`, `minimum`, `maximum`, `format`, `textVisible`,
  `orientation`, `invertedAppearance`; QLCDNumber `digitCount`, `intValue`, `value`, `mode`, `segmentStyle`.
- S: QSpinBox `valueChanged(int)`, `textChanged(QString)`; QDoubleSpinBox `valueChanged(double)`; QProgressBar
  `valueChanged(int)`; QLCDNumber `overflow`.
- M: QProgressBar `reset`; QLCDNumber `display(int)`/`display(double)` (pick one).
- B: QSpinBox/QDoubleSpinBox `bind:value` (valueChanged, arg0, suppress_signals); sliders inherit QAbstractSlider's.
- Gallery: `04-ranges.svelte`.
- Status: todo

### qt-containers-a: frames, group boxes, splitters, stacks, tabs, tool boxes

- Kinds: QFrame, QGroupBox, QScrollArea (more), QSplitter, QStackedWidget, QTabWidget, QToolBox.
- Depends on: qt-base.
- Elements: `frame` (QFrame + composite QVBoxLayout like `box`), `groupbox` (composite layout), `splitter`,
  `stackedwidget`, `tabwidget`, `toolbox`.
- C: QGroupBox/QFrame `children` (many, layout host, like `box`); QSplitter `children` (many; `addWidget`/
  `insertWidget(int,QWidget*)`, remove: `setParent(nullptr)` on the child, index_of `indexOf`, list `count`/`widget(int)`);
  QStackedWidget `pages` (many; `addWidget`/`insertWidget`/`removeWidget` + setParent(nullptr)); QTabWidget `pages`
  (many; `addTab(QWidget*, const QString&)` with `const:QString()`, `insertTab`, remove `removeTab(index)` then
  setParent(nullptr) on the child, index_of `indexOf`, list `count`/`widget(int)`); QToolBox `pages` (`addItem`/`insertItem`/`removeItem`).
- P: QGroupBox `title`, `checkable`, `checked`, `flat`, `alignment`?; QSplitter `orientation` (enum), `childrenCollapsible`,
  `handleWidth`, `opaqueResize`; QStackedWidget `currentIndex`; QTabWidget `currentIndex`, `tabPosition`, `tabShape`
  (enums), `tabsClosable`, `movable`, `documentMode`, `usesScrollButtons`; QToolBox `currentIndex`;
  QScrollArea `widgetResizable`, `horizontalScrollBarPolicy`/`verticalScrollBarPolicy` (enums).
- S: QGroupBox `toggled(bool)`, `clicked(bool)`; QSplitter `splitterMoved(int,int)`; QStackedWidget `currentChanged(int)`;
  QTabWidget `currentChanged(int)`, `tabCloseRequested(int)`, `tabBarClicked(int)`; QToolBox `currentChanged(int)`.
- B: QTabWidget/QStackedWidget/QToolBox `bind:currentIndex` (currentChanged(int), arg0).
- Blocked: tab/item titles and icons (G1), QSplitter stretch factors (G1).
- Gallery: `05-containers.svelte`.
- Status: done (QFrame 6 facts incl. children slot, QGroupBox 4 props + 2 signals + children, QSplitter 4 props + 1 signal + children, QStackedWidget currentIndex + changed + bind + pages, QTabWidget 6 props + 3 signals, QToolBox changed, QScrollArea 3 props; dropped: QGroupBox checked (ignored unless checkable), QTabWidget/QToolBox pages + currentIndex + bind (parent-is-host test assumption, new gap in docs/notes/a1-qt-containers-a.md); blocked: gap:G1 for tab/item titles, icons, stretch factors)
### qt-containers-b: layouts and MDI

- Kinds: QHBoxLayout, QVBoxLayout, QGridLayout, QFormLayout, QStackedLayout, QBoxLayout, QMdiArea, QMdiSubWindow, QDockWidget.
- Depends on: qt-containers-a.
- Elements: `mdiarea` (QMdiArea; `subwindows` many slot via `addSubWindow` — wraps the child in a QMdiSubWindow:
  effect `wrapped:QMdiSubWindow`, needs a generator check), `dockwidget` (QDockWidget standalone: `widget` one slot,
  `features`, `floating`, `windowTitle`).
- Mark unexposed with reasons: QGridLayout/QFormLayout (G1), layouts as separate elements (the `box` element is the
  layout-managed widget), QStackedLayout (use `stackedwidget`).
- Blocked: QMainWindow docks/toolbars areas (G1).
- Gallery: `06-layout.svelte`.
- Status: todo

### qt-selection: combo boxes and list widgets

- Kinds: QComboBox, QFontComboBox, QListWidget, QTreeWidget, QTableWidget, QButtonGroup (not a widget).
- Depends on: qt-base.
- Elements: `combobox`, `listwidget`.
- P: QComboBox `editable`, `currentIndex`, `currentText`, `maxVisibleItems`, `placeholderText`, `insertPolicy` (enum),
  `duplicatesEnabled`; QListWidget `currentRow`, `sortingEnabled`, plus QAbstractItemView `selectionMode` (enum),
  `alternatingRowColors`.
- S: QComboBox `currentIndexChanged(int)`, `currentTextChanged(QString)`, `activated(int)`; QListWidget
  `currentRowChanged(int)`, `itemSelectionChanged()`.
- M: QComboBox `addItem(QString)`, `insertItem(int, QString)`, `removeItem(int)`, `clear`; QListWidget
  `addItem(QString)`, `insertItem(int, QString)`, `clear`, `takeItem(int)` (returns an item: G3; skip).
- B: QComboBox `bind:currentIndex` (currentIndexChanged(int), arg0, suppress_signals); QListWidget `bind:currentRow`.
- Blocked: QFontComboBox (G4), QTreeWidget/QTableWidget items (G7), item models (G6).
- Gallery: `07-selection.svelte`.
- Status: todo

### qt-bars: main window, status bar, tab bar, button box

- Kinds: QMainWindow (more), QStatusBar (more), QTabBar, QDialogButtonBox, QToolBar, QMenuBar, QMenu, QSizeGrip.
- Depends on: qt-base.
- Elements: `tabbar`, `dialogbuttonbox`.
- P: QMainWindow `documentMode`, `animated`, `dockNestingEnabled`, `tabShape`; QStatusBar `sizeGripEnabled`;
  QTabBar `currentIndex`, `tabsClosable`, `movable`, `shape`, `expanding`, `drawBase`, `documentMode`;
  QDialogButtonBox `orientation`, `centerButtons`, `standardButtons` (flags: enum map of common combinations, or G4).
- S: QStatusBar `messageChanged(QString)`; QTabBar `currentChanged(int)`, `tabCloseRequested(int)`;
  QDialogButtonBox `accepted`, `rejected`, `helpRequested`.
- M: QStatusBar `showMessage(QString)`, `showMessage(QString,int)`?, `clearMessage`; QTabBar `addTab(QString)`,
  `insertTab(int, QString)`, `removeTab(int)`.
- C: QStatusBar `widgets` (many; `addWidget`/`insertWidget`/`removeWidget` + setParent(nullptr)) and `permanent`.
- B: QTabBar `bind:currentIndex`.
- Blocked: QToolBar, QMenuBar items, QMenu (G7), QMainWindow toolbars/docks (G1).
- Gallery: `08-bars.svelte`.
- Status: todo

### qt-dialogs: non-blocking dialogs

- Kinds: QDialog, QMessageBox, QInputDialog, QProgressDialog, QErrorMessage, QFileDialog, QColorDialog, QFontDialog, QWizard, QWizardPage.
- Depends on: qt-base.
- Elements: `dialog` (QDialog + composite layout, `children` slot), `messagebox`, `inputdialog`, `progressdialog`,
  `errormessage`, `filedialog`, `wizard`, `wizardpage`.
- P: QDialog `modal`, `sizeGripEnabled`, `windowTitle`; QMessageBox `text`, `informativeText`, `detailedText`, `icon`
  (enum), `textFormat`; QInputDialog `labelText`, `textValue`, `intValue`, `intMinimum`, `intMaximum`, `doubleValue`,
  `inputMode` (enum), `okButtonText`, `cancelButtonText`; QProgressDialog `value`, `minimum`, `maximum`, `labelText`,
  `minimumDuration`, `autoClose`, `autoReset`; QFileDialog `fileMode`, `acceptMode`, `viewMode` (enums), `defaultSuffix`;
  QWizardPage `title`, `subTitle`; QWizard `wizardStyle` (enum), `currentId`?.
- S: QDialog `accepted`, `rejected`, `finished(int)`; QInputDialog `textValueChanged(QString)`, `intValueChanged(int)`;
  QProgressDialog `canceled`; QFileDialog `fileSelected(QString)`, `currentChanged(QString)`; QWizard `currentIdChanged(int)`.
- M: QDialog `open` (non-blocking), `accept`, `reject`, `done(int)`, `show`; QErrorMessage `showMessage(QString)`;
  QFileDialog `setDirectory(QString)`; QProgressDialog `reset`, `cancel`.
- C: QWizard `pages` (many; `addPage(QWizardPage*)`, `removePage(int)` by id — check), QDialog `children`.
- Never: `exec()`, `QMessageBox::information/warning/...` static helpers, `QFileDialog::getOpenFileName` (all block).
- Lifetime: dialogs are toplevel (generator `gen_is_toplevel`); check close behaviour on one dialog.
- Blocked: QColorDialog/QFontDialog values (G4).
- Gallery: `09-dialogs.svelte` (buttons that open non-blocking dialogs; the screenshot shows the main window only).
- Status: done (about 122 ledger facts incl. inherited QDialog facts on every dialog element; dropped: QInputDialog.doubleValue (ingest types it i32), QWizard.startId (reads -1 without pages), QWizard/QFileDialog/QFontDialog/QColorDialog/QErrorMessage accept/done methods (protected in subclass: generator gap, see docs/notes/a1-qt-dialogs.md); blocked: QWizard pages slot and gallery dialogs mounting/opening, new gap G9 (no toplevel non-window mount, no method calls from Svelte); QFileDialog fileSelected/currentChanged and QDialog.show not annotated)

### qt-display: calendar, date/time edits, misc display

- Kinds: QCalendarWidget, QDateEdit, QTimeEdit, QDateTimeEdit, QGraphicsView, QSplashScreen, QRhiWidget, QRubberBand,
  QFocusFrame, QUndoView, QHeaderView, QListView, QTreeView, QTableView, QColumnView.
- Depends on: qt-base.
- Elements: `calendar` (QCalendarWidget: `gridVisible`, `navigationBarVisible`, `firstDayOfWeek` (enum),
  `horizontalHeaderFormat`, `verticalHeaderFormat` (enums), `dateEditEnabled`; S `selectionChanged()`,
  `currentPageChanged(int,int)`; M `showToday`, `showNextMonth`, `showPreviousMonth`).
- Mark unexposed with reasons: QDateEdit/QTimeEdit/QDateTimeEdit values (G4: only `displayFormat`, `calendarPopup`
  work; expose if worthwhile), QGraphicsView (G7: scene), QSplashScreen (startup only), QRhiWidget (GPU API),
  QRubberBand/QFocusFrame/QSizeGrip/QSplitterHandle (internal), item views (G6).
- Gallery: `10-display.svelte`.
- Status: todo
