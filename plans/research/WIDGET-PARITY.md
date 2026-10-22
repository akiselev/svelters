# WIDGET-PARITY: Qt-level configuration and styling for svelters' own widgets

Lane research/widget-parity (docs only; nothing built or run). Owner: svelters-agent. Date: 2026-10-22.

The user's requirement (2026-10-22): **"We need to make our widgets configurable and stylable as much as Qt and
more than egui's current set."**

This document is the acceptance list for the OW lanes of ARCH-WIDGETS §7. For each Qt widget an IDE uses, it maps
every Qt property that matters for cadatomic, and every QSS sub-control and pseudo-state, onto our props, signals,
methods, `::part()` names and pseudo-classes. Where nothing covers an item it says **MISSING** and names the OW lane
that adds it, with a size.

Inputs read:

- `plans/research/ARCH-WIDGETS.md` (master `d242d83c`): layers, units, parts, state bits 0–22, §6.1 inventory, §7
  lanes. Q1–Q5 decided by the user as recommended.
- `plans/research/QT-SURVEY.md` (QW/QA/QS rows), `DESIGN.md` (R1–R18), `CSS.md` (engine, `@container` later),
  `ARCH-SCENE.md` §7 (`Behavior::paint_over`, `StateProvider`).
- `metadata/semantics/egui.json` (master): 48 kinds with their props, signals and methods. This is what "have"
  means below.
- `crates/svelters-style/src/{selector,state}.rs`. The engine already matches attribute selectors against props
  (`[frame="false"]`, booleans as `"true"`/`"false"`), all four combinators, `:is/:where/:not/:has`, and the
  structural pseudo-classes.

Qt facts come from memory of Qt 6.8 (Widgets, Qt Style Sheets reference, "Customizing Qt Widgets Using Style
Sheets"). egui facts are for 0.37 from memory. Items marked **verify** should be checked before a lane relies on
them.

---


**Decided (user, 2026-10-22):** Q1 the QStyle equivalent is Rust part painters selected in CSS (`background-image: paint(cad.knurl)` on a `::part()`), plus `style_as` and Policy overrides; Q2 scope is the IDE-relevant set now (all 223 MISSING items as OW acceptance, ~53 agent-days; the 25 OW-P2 items later); Q3 table cells: both per-column Rust cell renderers (default for large tables) and Svelte `{#snippet}` cells; Q4 inline rich text through child elements (`<b>`, `<i>`, `<code>`, `<a>`, `<span class>`) styled by CSS, no HTML/Markdown parser.

**Decided (coordinator, 2026-10-22):** svelters ships generic `toolbar` and `status-bar` kinds, cadatomic builds its command strip and status sections on them; the completer is cadatomic's, svelters' half (caret-anchored popup + key forwarding) is OW4b; lane splits adopted: OW3b (toolbar, editable combo), OW4b (masks, validators, adornments, completer popup), OW5a (lists, tree incl. WG5b's tree-view, DnD), OW5b (table, header), OW5c (tabs), OW6b optional; new kinds `scrollbar`, `calendar`, `color-editor`, `dialog-buttons` in their lanes; P1-P10 styling rules go to OW-CSS/OW0b; container queries CS-CQ after OW-CSS. The **verify** items are checked by the lane that implements them. The state-bit gaps DOC1 found on today's egui wrappers (`:invalid`, `:open`, `:expanded`, `:read-only`, `:dragging`, `:busy`, `:checked` beyond checkbox, combo `:focus`) are acceptance for the own-widget replacements.

## 0. Summary

1. **Configuration.** Today's kinds cover the core of every Qt widget, but this document finds 223 MISSING items
   (192 configuration and 29 styling in the widget tables, 2 in the QStyle section), plus 25 P2 items. The gaps cluster in four places:
   - **spin box** (`drag-value`): special-value text, acceleration, adaptive steps, formatter and parser;
   - **line edit** (`text-edit`): masks, validators, clear button, adornments, echo modes, read-only;
   - **combo box**: editable, insert policy, rich options;
   - **item views and header**: selection behaviour, edit triggers, drag modes, header resize modes, movable
     sections, a multi-column tree.

   Most of these are props and signals on kinds whose OW lane is already planned. They grow the lanes but need no
   new architecture.
2. **Styling.** ARCH-WIDGETS' `::part()` plus pseudo-classes already reaches most QSS sub-controls. It falls short
   of QSS in seven places, which this document adds as parity rules (§1.3, P1–P10, of which these seven matter
   most):
   - **P1** repeated parts (tabs, rows, header sections, menu items) need Qt's `:first`/`:last`/`:only-one`/
     `:alternate`. Structural pseudo-classes must work after `::part()` and in pooled rows, by **logical**
     (model) index.
   - **P2** QSS swaps sub-control images (`image: url(...)`). We need `content: icon(<key>)`/`url(...)` on glyph
     parts, so a theme can change a check mark or chevron without Rust.
   - **P3** QSS moves sub-controls (`subcontrol-position`). Our node parts must be flex items that `order`,
     `width`, `margin` and `display: none` act on.
   - **P4** gradients with any number of stops, and radial gradients; QSS has both. `border-image` is P2.
   - **P5** one more state bit, `has-children` (bit 23).
   - **P6** per-item styling through the item element: `tab.dirty::part(header)`,
     `table-column.numeric::part(header)`.
   - **P10** multi-name parts for per-item variants: `::part(day today)`, `::part(cell negative)`.
3. **The QStyle equivalent is missing from ARCH-WIDGETS.** It offers own kinds in a new namespace, `paint_over`
   decorations and the OW-RUST layer. `replaces=` is crate-private. Nothing lets cadatomic redraw one part of a
   built-in, such as a knurled slider thumb or a custom tree twistie. §4 recommends **CSS-selected Rust part
   painters**: `background-image: paint(cad.knurl)` and `content: paint(...)`, after CSS Painting API's `paint()`.
   They are registered on the builder and called with the part's `StyleView`, rect, state and widget-specific
   `PaintArgs`. Size: S in OW0b and S in OW-CSS. This is Q1.
4. **Beyond Qt**, we already have or plan things QSS and egui lack:
   - tokens (`var()`) with light, dark and high-contrast media;
   - transitions;
   - `:has()`, sibling combinators and `:focus-visible`;
   - `:state()` provider bits, which cadatomic drives in bulk from Rust;
   - outer and inset shadows and outline offsets;
   - `use:` behaviour units on any element;
   - hot-reloaded CSS and Svelte;
   - AccessKit and MCP queries per part;
   - VIS1-measured looks;
   - `@container`, as a later CSS item.
5. **Per-lane growth** (§5):
   - OW0 +2.4 d (painters, part layout, icon parts);
   - OW-CSS +2.8 d (P1, P2, P4–P6, P10, `paint()`, keyframes);
   - OW1a +3.9 d (+2.9 without Q4);
   - OW1b +3.75 d (M→L);
   - OW2 +3.65 d;
   - OW3 +5.2 d, plus a new **OW3b** (toolbar, editable combo) of 3 d;
   - OW4 +2.1 d, plus a new **OW4b** (masks, validators, adornments, clear button, highlights) of 3 d;
   - OW5 +15.6 d, split into **OW5a** lists/tree/DnD (+7 d), **OW5b** table/header (+5.2 d) and **OW5c** tabs
     (+3.4 d);
   - OW6 +4.75 d (M→XL; OW6b optional);
   - OW7 +2.5 d.

   About 53 agent-days of parity work in total, spread over the lanes as they run. The unscheduled **OW-P2** ledger
   holds 25 more items (about 12 d).
6. **Three kinds Qt has and we lack**:
   - `toolbar` (QToolBar) and `status-bar` (QStatusBar). QT-SURVEY assigned both to cadatomic; this document
     proposes svelters ships the generic kinds and cadatomic composes its command strip and status sections on
     them. The coordinators settle this seam (§6, "for the coordinators").
   - `dialog-buttons` (QDialogButtonBox).

   Further new kinds: `scrollbar` (standalone QScrollBar for custom views), `calendar` (inline QCalendarWidget),
   `color-editor` (inline QColorDialog body), `key-sequence-edit` (WG6 QW10, already queued after OW4) and
   `time-edit` (P2).
7. **`tree-view` is not in ARCH-WIDGETS §6.1.** WG5b merged it after the inventory was taken. OW5a takes it.
8. **Open questions for the user** (§6):
   - Q1: the painter mechanism;
   - Q2: parity scope (the IDE subset now, P2 items later);
   - Q3: table cell rendering (Qt delegates);
   - Q4: inline rich text in labels, tooltips and read-only text.

MISSING counts per widget. Each count is one distinct item; rows that only depend on another MISSING row ("MISSING
with …") are not counted again. P2 items are listed separately and are not acceptance. Counts for new kinds
understate the work, because the whole kind is one item.

| § | Qt widget(s) | Our kind(s) | Config | Styling | Total | P2 |
|---|---|---|---|---|---|---|
| 2.1 | QPushButton, QToolButton | `button`, `icon`, `toggle-button`, `menu-button` | 9 | 2 | 11 | 1 |
| 2.2 | QCheckBox | `checkbox`, `switch` | 3 | 1 | 4 | 0 |
| 2.3 | QRadioButton | `radio-button` | 1 | 1 | 2 | 0 |
| 2.4 | QLineEdit | `text-edit` | 15 | 2 | 17 | 4 |
| 2.5 | QTextEdit, QPlainTextEdit | `text-edit multiline` | 7 | 0 | 7 | 1 |
| 2.6 | QSpinBox, QDoubleSpinBox | `drag-value` | 9 | 2 | 11 | 1 |
| 2.7 | QSlider (QDial N/A) | `slider`, `range-slider` | 7 | 1 | 8 | 2 |
| 2.8 | QComboBox | `combo-box` | 13 | 2 | 15 | 0 |
| 2.9 | QProgressBar | `progress-bar`, `spinner` | 5 | 3 | 8 | 0 |
| 2.10 | QLabel | `label`, `hyperlink` | 3 | 0 | 3 | 2 |
| 2.11 | QScrollArea, QScrollBar | `scroll-area`, new `scrollbar` | 9 | 3 | 12 | 2 |
| 2.12 | QTabWidget, QTabBar | `tab-view`, `tab` | 14 | 3 | 17 | 0 |
| 2.13 | QToolBar | new `toolbar` | 2 | 0 | 2 | 0 |
| 2.14 | QMenu, QMenuBar, QAction | `menu-bar`, `menu`, `action`, `menu-separator`, `context-menu` | 15 | 0 | 15 | 1 |
| 2.15 | QToolTip | `tooltip` (common) | 5 | 1 | 6 | 0 |
| 2.16 | QGroupBox, QToolBox | `frame`, `collapsing-header` | 4 | 0 | 4 | 0 |
| 2.17 | QSplitter | `split-view`, `panel` | 6 | 1 | 7 | 0 |
| 2.18 | QAbstractItemView, QListView, QTreeView, QTableView, QHeaderView | `virtual-list`, `tree-view`, `table`, `table-column` | 44 | 1 | 45 | 5 |
| 2.19 | QDateEdit, QCalendarWidget | `date-picker`, new `calendar` | 11 | 5 | 16 | 2 |
| 2.20 | QColorDialog | `color-picker`, new `color-editor` | 5 | 1 | 6 | 2 |
| 2.21 | QDialog, QDialogButtonBox | `modal`, `inner-window`, new `dialog-buttons` | 3 | 0 | 3 | 0 |
| 2.22 | QStatusBar | new `status-bar` | 2 | 0 | 2 | 0 |
| 2.23 | QDockWidget | berth | 0 | 0 | 0 | 0 |
| §4 | QStyle / QProxyStyle | `style_as`, Policy override (painters: rule-sized) | 2 | 0 | 2 | 0 |
| | **Total** | | **194** | **29** | **223** | **23** (+2 cross-cutting) |

---

## 1. Legend and the cross-cutting mappings

### 1.1 Status values

| Status | Meaning |
|---|---|
| **have** | on master: a prop, signal or method in `metadata/semantics/egui.json`, or a selector the engine matches today |
| **ARCH (lane)** | designed in ARCH-WIDGETS; that OW lane delivers it |
| **MISSING → *lane* (d)** | in neither; the lane adds it; size in agent-days |
| **P2 → OW-P2 (d)** | real Qt feature, not needed for cadatomic's first IDE; parked in the unscheduled OW-P2 list |
| **CSS** | no prop by design: a rule in the theme does it (the selector or property is named) |
| **N/A** | not taken, with the reason |
| **cad / berth / quire / DX1 / IN1** | owned elsewhere (QT-SURVEY owner column or a decided lane) |

Props are written in their Svelte form (`kebab-case` attributes, `bind:` where two-way). Signals are the
`on<name>` callbacks. "Common" means the `Widget` base kind (`enabled`, `visible`, `tooltip`,
`accessible-name`, `autofocus`, sizes, flex and drag props), which every kind has.

### 1.2 Qt pseudo-states → ours (applies to every widget below)

| Qt pseudo-state | Ours | Status |
|---|---|---|
| `:hover` | `:hover` (bit 0; node parts: `::part(x):hover`) | have; parts ARCH (OW-CSS) |
| `:pressed` | `:active` (bit 1) | have |
| `:focus` | `:focus`; keyboard-only ring with `:focus-visible` (bit 14) | have; ARCH (OW0, OW-CSS) |
| `:edit-focus` | N/A (Qt for Embedded keypad navigation) | N/A |
| `:disabled`, `:enabled` | same | have |
| `:checked`, `:unchecked`, `:on`, `:off` | `:checked`, `:not(:checked)` | have |
| `:indeterminate` | `:indeterminate` (bit 15) | ARCH (OW0, OW-CSS) |
| `:open` (combo popup, menu button) / `:closed` | `:open` (bit 7), `:not(:open)` | have (bit); prop mirror for menu-button/select ARCH (OW3) |
| `:open`/`:closed` on tree branches | `:expanded`/`:not(:expanded)` (bit 8) on the row | have |
| `:has-children` | `:has-children` (new bit 23, P5) | rule P5 (OW-CSS, OW5a) |
| `:has-siblings`, `:adjoins-item` (branch lines) | N/A: VS Code-style indent guides instead, `tree-view::part(guide)` | N/A (§2.18) |
| `:default` | `:default` (bit 17) | ARCH (OW0) |
| `:flat`, `:no-frame` | `[frame="false"]` | have (attribute selector on the prop) |
| `:read-only` | `:read-only` (bit 10), `:read-write` | have |
| `:editable` | `[editable="true"]` | per widget (combo §2.8) |
| `:selected` | `:selected` (bit 6) | have |
| `:alternate` | `:nth-child(even)` by logical row index (P1) | rule P1 (OW-CSS) |
| `:first`, `:last`, `:middle`, `:only-one` | `:first-child`, `:last-child`, `:not(:first-child):not(:last-child)`, `:only-child`, after `::part()` and on pooled rows, by logical index (P1) | rule P1 (OW-CSS) |
| `:previous-selected`, `:next-selected` | real nodes: `:has(+ :selected)`, `:selected + *`; repeated parts: N/A (Qt needs them for overlapping tab shapes, which we do not draw) | have (nodes) / N/A |
| `:horizontal`, `:vertical` | `[orientation="horizontal"]`, `[orientation="vertical"]` | per widget (P6 props) |
| `:top`, `:bottom`, `:left`, `:right` | `[tab-position="…"]`, `[side="…"]` | per widget |
| `:closable`, `:movable`, `:floatable` | `[closable="true"]`, `[movable="true"]`; docks are berth's | per widget |
| `:exclusive`, `:non-exclusive` | `action:not([group=""])`, `action[group=""]` | have |
| `:active` (window active) / inactive | `:not(:backdrop)` / `:backdrop` (bit 20) | ARCH (OW0) |
| `:maximized`, `:minimized`, `:window` | inner windows: `[collapsed="true"]`; no maximise (berth owns real panes) | P2 → OW-P2 (0.25) |
| `:!state` (negation) | `:not(:state)` | have |

### 1.3 Parity rules (cross-cutting additions to ARCH-WIDGETS)

| # | Rule | Why (Qt) | Lane, size |
|---|---|---|---|
| **P1** | **Logical position of repeated parts.** For repeated node parts (`tab`, `header`, `item`, `option`, `day`) and pooled rows, structural pseudo-classes (`:first-child`, `:last-child`, `:only-child`, `:nth-child(An+B [of S])`) match the widget's **logical** index and count (model order), not shadow or pool order. The widget sets them with `cx.set_logical_index(part, i, n)`, and the engine's `Element` reads that override when one is present. Our dialect allows structural pseudo-classes after `::part()`, a documented deviation from CSS Shadow Parts, which allows only user-action ones. | `:first`/`:last`/`:middle`/`:only-one` on tabs, sections and toolbar items; `:alternate` rows. `:nth-child` over a pool of 40 recycled rows would stripe by slot, not by row | OW-CSS 0.5 (engine); each widget sets indices in its lane (≤ 0.1 each) |
| **P2** | **Glyphs from CSS.** Paint parts that draw a glyph (`mark`, `chevron`, `twistie`, `close`, `inc`/`dec` arrows, `sort`, `check`, `submenu`) read `content: icon(<key>)` or `content: url(asset://…)`, and later `content: paint(<name>)` (§4). The base sheet sets the defaults (`checkbox:checked::part(mark) { content: icon(check) }`, `:indeterminate` → `icon(dash)`), so no glyph key is hard-coded in Rust. | `QCheckBox::indicator:checked { image: url(…) }`, `::down-arrow { image: … }`, `::branch:closed { image: … }` | OW-CSS 0.25 (parse, computed `Content`); OW0b 0.25 (icon primitive reads it) |
| **P3** | **Part layout from CSS.** Node parts are flex items of the host's content box: `order`, `width`/`height`, `min-*`/`max-*`, `margin`, `align-self` and `display: none` apply. Paint parts honour `width`/`height`, `margin` and `display: none`. A widget may fix some properties of a part (a slider thumb's main-axis position) and documents which. | `subcontrol-origin`/`subcontrol-position` (spin buttons on the left, the combo arrow on the left, macOS-style tab close on the left); `QToolButton` `toolButtonStyle` (icon above text) | OW0b 0.5 |
| **P4** | **Gradients**: `linear-gradient` with any number of stops and angles, `radial-gradient`. `border-image` (nine-slice) is P2. | `qlineargradient`/`qradialgradient` with any stops; `border-image` | OW-CSS 0.5; border-image P2 → OW-P2 (1) |
| **P5** | **Bit 23 `has-children`** (built-in, prop mirror on tree rows). The free bits become 24–31. | `QTreeView::branch:has-children` | OW-CSS 0.1 + OW5a 0.1 |
| **P6** | **Per-item styling through the item element.** A node part that stands for a child element (the tab header of a `tab`, the header cell of a `table-column`, the row of a menu `action`) has that element as its **style host**: it matches the element's classes, attributes and `:state()` bits, and the containing widget still lays it out. Written `tab.dirty::part(header)`, `table-column[align="end"]::part(header)`. | `setTabTextColor`, per-section header styles through a delegate, per-action fonts | OW5c 0.25 (first user), OW-CSS 0.25 (style-host indirection) |
| P7 | **Orientation and placement props**, so `[orientation=…]` and `[tab-position=…]` give Qt's `:horizontal`/`:vertical`/`:top`/…: `orientation` on `slider`, `range-slider`, `progress-bar`, `toolbar`; `tab-position` on `tab-view` | QSlider/QProgressBar orientation, QTabWidget tabPosition | per widget (OW1a, OW1b, OW3b, OW5c) |
| P8 | **Press signals and repeat on every pressable**: `press`, `release` signals (Pressable `Down`/`Up`) and a `repeat` prop (`true` or `[delay, interval]` ms) on `button`, `icon`, `toggle-button` | QAbstractButton `pressed`/`released`, `autoRepeat*` | OW0 0.25 |
| **P10** | **Multi-name parts** (CSS Shadow Parts' `::part(a b)` matches a part that has both names). A repeated part carries extra names for its variants, either from the widget (`::part(day today)`, `::part(day weekend)`) or from the data (`{ value, part: "negative" }` cells, `{ label, part: "recent" }` options, `highlights` ranges). | Qt's per-item `ForegroundRole`/`BackgroundRole`, `setDateTextFormat`, `setWeekdayTextFormat`, `extraSelections` formats | OW-CSS 0.1; widgets set names in their lanes |
| P9 | **Metric tokens instead of QStyle pixel metrics.** Every size a widget needs and Qt reads from `QStyle::pixelMetric` (indicator size, scrollbar extent, slider thumb length, tab overlap, menu item padding) is CSS on a part, with defaults in the base sheet. No Rust constant decides a size (ARCH-WIDGETS §1.1 `BoxGeom`). Behaviour timing (`SH_ToolTip_WakeUpDelay`, drag distance, double-click interval) stays in `native::Policy` and is overridable on the builder (§4.3). | `QStyle::pixelMetric`, `styleHint` | built into each lane; checked by VIS1c (no size constant in `widgets/`) |

The engine already gives `qproperty-*`'s useful half: style-table "setter" properties (layout props) can be set
from CSS today. Other configuration stays props by design: a stylesheet does not set `maxLength`.

---

## 2. Per widget

Each section has three tables: **Configuration** (Qt property, signal or method → ours), **Styling** (QSS
sub-control or pseudo-state → `::part()`/pseudo-class), and a short **Beyond Qt** list. Common props, and the
pseudo-states of §1.2, are not repeated unless a widget gives them a twist.

### 2.1 Push and tool buttons: QPushButton, QToolButton (QAbstractButton) → `button`, `icon[frame]`, `toggle-button`, `menu-button`

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| `text` | `text` (`button`, `toggle-button`), `label` (`menu-button`) | have |
| `icon` | `icon` (icon key or `asset://` ref); the `icon` kind with `frame` for icon-only buttons | `button icon`: ARCH (OW0, §1.3 struct); `icon[frame]`: have |
| `iconSize` | CSS `width`/`height` on `::part(icon)` | **MISSING → OW0 (0.25)** (the part, see Styling) |
| `toolButtonStyle` (IconOnly, TextOnly, TextBesideIcon, TextUnderIcon) | CSS: `button { flex-direction: column }` (text under), `::part(label) { display: none }` (icon only), `::part(icon) { display: none }` (text only) (P3) | **MISSING → OW0 (0.25)** (content laid out as parts) |
| `shortcut` (activates the button) | `shortcut` prop, with the same chord grammar as `action` (`Primary+S`, `std:` names, QT-SURVEY QA4) | **MISSING → OW3 (0.25)** |
| mnemonic (`&Save`) | QA16 (cadatomic for Rust menus) | P2 → OW-P2 (1) |
| `checkable`, `checked`, `toggled` | `toggle-button checked`, `changed` | have |
| `autoRepeat`, `autoRepeatDelay`, `autoRepeatInterval` | `repeat` (`true` or `[delay, interval]`), Pressable `repeat` (P8) | **MISSING → OW0 (0.1)** (the unit param is ARCH) |
| `autoExclusive`, QButtonGroup (`exclusive`, ids) | `toggle-button radio` + `group` (an exclusive set, as `action group`), with Roving arrows within the group | **MISSING → OW1a (0.25)** (`radio` alone keeps a button on; there is no group) |
| `down` (programmatic pressed look) | N/A: JS never sets state bits (ARCH-WIDGETS §2.1); use a class | N/A |
| `pressed`, `released` | `press`, `release` (P8) | **MISSING → OW0 (0.1)** |
| `clicked` | `clicked`; also `double-clicked`, `secondary-clicked` | have |
| QPushButton `default` | `default` | have |
| `autoDefault` (the focused push button becomes the default in a dialog; Windows and KDE, not macOS) | `Policy::auto_default`; the dialog scope (EV3) moves `:default` to the focused button | **MISSING → OW7 (0.25)** |
| reject on Escape | `cancel` | have |
| `flat` | `frame={false}` | have |
| `setMenu` (push button with a menu) | `menu-button` | have |
| QToolButton `popupMode` InstantPopup | `menu-button` | have |
| `popupMode` MenuButtonPopup (split button) | `menu-button split`: `clicked` on the main part, the menu on the `arrow` part (DESIGN R14) | **MISSING → OW3 (0.5)** |
| `popupMode` DelayedPopup (press and hold) | `menu-button popup-mode="delayed"` (the hold delay comes from `Policy`, about 600 ms, **verify**) | **MISSING → OW3 (0.25)** |
| `autoRaise` (the frame shows on hover only) | CSS: `button[frame="false"]:hover { background-color: var(--sv-toolbar-hover) }` in the base sheet | CSS |
| `arrowType` | `icon="chevron-down"` (any icon key) | have (via `icon`) |
| `defaultAction` (QAction-backed button) | cadatomic `CommandKey` (QD11) | cad |
| `small` (an egui-ism) | becomes a density class (`.small`) in the base sheet; the prop stays as an alias until the docs pass | CSS |
| QCommandLinkButton | N/A (QT-SURVEY §1.5) | N/A |

**Styling**

| QSS | Ours | Status |
|---|---|---|
| box model on the button (background, border, radius, padding, min-size) | `button { … }` (box primitive) | ARCH (OW0) |
| icon (no QSS sub-control; `icon-size`, `qproperty-icon`) | `::part(icon)` (P) | **MISSING → OW0 (0.1)** (ARCH lists no parts for `button`) |
| text (`color`, `text-align`, `font`) | `::part(label)` (P), or `color`/`font-*` on the host | **MISSING → OW0 (0.1)** |
| `QPushButton::menu-indicator` | `menu-button::part(chevron)` | ARCH (OW3) |
| `QToolButton::menu-button` (the split half) | `menu-button::part(arrow)` (N) with its own `:hover`/`:active` | **MISSING → OW3** (with split) |
| `QToolButton::menu-arrow` | `menu-button::part(chevron)` (glyph from `content`, P2) | ARCH (OW3) + P2 |
| `:default` | `:default` | ARCH (OW0) |
| `:flat` | `[frame="false"]` | have |
| `:checked` | `toggle-button:checked`; `button[selected="true"]` | have |
| `:pressed` | `:active` | have |
| `:open` (menu shown) | `menu-button:open` | ARCH (OW3: prop mirror of `active`) |

**Beyond Qt.**
- R1 lip, R2 pressed sink (`transition: none` on `:active`) and R3 hover ease, as token-driven CSS.
- R8 focus ring at offset +1, keyboard only (`:focus-visible`).
- R9 disabled with a reason: hoverable, tooltip shown, `cursor: not-allowed`.
- `:state(cad.pending)` on a command button, driven from Rust with no JS.
- `use:pressable` gives any element the same press semantics.
- egui 0.37 has `Button::selected`, `frame`, `small`, `min_size`, `shortcut_text`, `image` and `right_text`. It has
  no default or cancel button, one global `Visuals.widgets` look per state (inactive, hovered, active, open), no
  focus-visible distinction and no per-instance style beyond `fill`/`stroke` in code.

### 2.2 QCheckBox → `checkbox` (and `switch`)

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| `text` | `text` | have |
| `checked` | `checked` (`bind:checked`) | have |
| `tristate`, `checkState` (Unchecked, PartiallyChecked, Checked) | `indeterminate` prop (written by the app; a click clears it and checks, as in Qt and GTK) + `tristate` (the user cycles through partial) | **MISSING → OW1a (0.25)** (bit 15 is ARCH; the props are not) |
| `checkStateChanged` (Qt 6.7), `stateChanged` | `changed` (carries `checked`; with `tristate`, carries `"partial"`) | have; the tristate payload comes with the row above |
| `clicked`, `toggled` | `clicked`, `changed` | have |
| `icon` | `icon` | **MISSING → OW1a (0.1)** |

**Styling**

| QSS | Ours | Status |
|---|---|---|
| `::indicator` (size, border, image) | `checkbox::part(indicator)` (P) | ARCH (OW1a) |
| `::indicator:checked`/`:unchecked`/`:indeterminate` `image` | `checkbox:checked::part(mark) { content: icon(check) }`, `:indeterminate` → `icon(dash)` (P2) | ARCH parts (OW1a) + P2 (OW-CSS, OW0b) |
| `::indicator:hover`/`:pressed`/`:disabled` | `checkbox:hover::part(indicator)` (the host state comes before the part) | ARCH (OW-CSS) |
| `spacing` (indicator to text) | `gap` on the host | have (CSS) |
| text | `::part(label)` (P) | **MISSING → OW1a (0.1)** |

**Beyond Qt.**
- `switch` is a toggle with an animated knob (R17: the knob moves through a `transition` on `--knob`). Qt Widgets
  has no switch.
- `accent-color` (CSS UI 4) as a one-line theme hook for indicator fills: **MISSING → OW-CSS (0.1)**. It also
  serves `radio-button`, `switch`, `progress-bar` and `slider`.
- egui's `Checkbox` has `indeterminate` (**verify**: since 0.27), with one global look and no indicator styling.

### 2.3 QRadioButton (+ QButtonGroup) → `radio-button`

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| `text`, `checked` | `text`, `checked` | have |
| `autoExclusive` (same parent) / QButtonGroup | `group` (exclusive by group name) + `value` | have |
| the group's `checkedId` / selected value | Svelte `bind:group` | have (`tier0.rs::radio_buttons_bind_group_and_checked`) |
| QButtonGroup `exclusive=false` | use checkboxes | N/A |
| keyboard: arrows move within the group, one tab stop | Roving | ARCH (OW1a) |
| `toggled`, `clicked` | `changed`, `clicked` | have |
| `icon` | `icon` | **MISSING → OW1a (0.1)** |

**Styling**

| QSS | Ours | Status |
|---|---|---|
| `::indicator` | `radio-button::part(indicator)` (P) | ARCH (OW1a) |
| `::indicator:checked` image | `radio-button:checked::part(dot)` (P; a circle by default, and `content` may set an icon, P2) | ARCH (OW1a) + P2 |
| text | `::part(label)` | **MISSING → OW1a (0.05)** |

**Beyond Qt.**
- DESIGN R17 (state by fill, not colour alone) is a checked VIS1 rule.
- Segmented groups are `toggle-button radio group`, styled with P1 (`:first-child`/`:last-child` radii).
- egui's `RadioButton` has a single look and no group keyboard handling.

### 2.4 QLineEdit → `text-edit` (single line)

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| `text` | `text` (`bind:text`; `bind:text|commit` through CX3) | have / ARCH (OW4, CX3) |
| `placeholderText` | `hint-text` (alias `placeholder`, added for familiarity) | have; alias **MISSING → OW4 (0.05)** |
| `maxLength` | `char-limit` (alias `maxlength`) → `FieldRules::max_chars` | have; the alias comes with the row above |
| `readOnly` | `read-only` → `FieldRules::read_only`, `:read-only` | **MISSING → OW4 (0.1)** (bit 10 exists; `text-edit` has no prop) |
| `echoMode` Normal / Password | `password` → `FieldRules::secret` | have |
| `echoMode` NoEcho | `echo="none"` | **MISSING → OW4 (0.1)** |
| `echoMode` PasswordEchoOnEdit | `echo="password-on-edit"` (plain while focused and edited) | P2 → OW-P2 (0.25) |
| a reveal-password toggle (Qt has none; GTK `show-peek-icon`) | `revealable` → `::part(reveal)` (N) | P2 → OW-P2 (0.25) |
| password mask character (QSS `lineedit-password-character`) | CSS custom property `--sv-password-char` (default U+25CF, ARCH-WIDGETS §4.6) | **MISSING → OW4 (0.05)** |
| `inputMask` (`>AAAA-9999;_`) | `input-mask`, in Qt's grammar (`A a N n X x 9 0 D d # H h B b > < ! ; \`), as a TC0 `InputFilter` plus motion that skips fixed literals | **MISSING → OW4b (1.0)** |
| `validator` (QIntValidator, QDoubleValidator, QRegularExpressionValidator; Invalid / Intermediate / Acceptable; `fixup`) | `pattern` (regex: Acceptable vs Intermediate); `validator="<name>"`, a named Rust validator from a registry (`fn(&str) -> Validity { Invalid, Intermediate, Acceptable(fixup?) }`), sync, with no JS per keystroke; the `oncommit` reject (have, WG5a) | `pattern`: ARCH (OW4, §2.4); `validator`: **MISSING → OW4b (0.5)** |
| `hasAcceptableInput` | `:invalid`/`:valid` (bit 9), `:user-invalid` (bit 22) | ARCH (OW4) |
| `inputRejected` | `rejected` signal (TC0 `Executed::rejected`: MaxChars, Filtered, ReadOnly) | **MISSING → OW4 (0.1)** |
| `completer` (QCompleter: popup, inline or unfiltered popup; case sensitivity; filter mode) | cadatomic's completer (QW9, decided). svelters gives the seam: a `popup` anchored to the caret rect (`placement="caret"`), with Up/Down/Enter/Escape claimed by the popup while it is open | cad; the seam **MISSING → OW4b (0.25)** |
| `clearButtonEnabled` | `clearable` → `::part(clear)` (N; pointer only, as in Qt) | **MISSING → OW4b (0.25)** |
| `addAction(action, LeadingPosition / TrailingPosition)` | adornments: children with `slot="start"` / `slot="end"` (icons, icon buttons), laid out inside the field's border box (P3) | **MISSING → OW4b (0.5)** |
| `alignment` | CSS `text-align` | ARCH (OW0b text primitive) |
| `frame` | CSS (`border: none`) | CSS |
| `cursorMoveStyle` (logical vs visual for bidi) | N/A: RTL is out for cadatomic (CE10) | N/A |
| `cursorPosition`, `selectionStart`, `selectedText`, `setSelection`, `selectAll`, `deselect` | `bind:selection-start`, `bind:selection-end`; methods `select-all`, `set-selection(a, b)`; signals `selection-changed`, `cursor-moved` | **MISSING → OW4 (0.25)** |
| `undo`, `redo`, `cut`, `copy`, `paste`, `insert`, `del`, `isUndoAvailable`, `isRedoAvailable` | methods of the same names over TC0 commands; read-only props `can-undo`/`can-redo`; the Edit menu reaches the field through `Embed::text_command` (QA3) | **MISSING → OW4 (0.25)** |
| `isModified`, `setModified` | `modified` (read-write: set on edit, cleared on `commit` or by the app) | **MISSING → OW4 (0.1)** |
| `dragEnabled` (drag selected text out) | `text-drag` (TC0's `Drop` input already exists for drops in) | P2 → OW-P2 (0.5) |
| `textEdited` (user only) vs `textChanged` (also programmatic) | `input` (user only) vs `changed`; programmatic writes never echo (observe sweep) | ARCH (OW4, §2.4) |
| `editingFinished` (Enter or focus loss, only if acceptable) | `commit` | ARCH (OW4) |
| `returnPressed` | `submit` | ARCH (OW4) |
| focus | `gained-focus`, `lost-focus`, `request-focus` | have |
| context menu (Undo, Redo, Cut, Copy, Paste, Delete, Select All) | a default context menu built from TC0 commands, replaceable with a `context-menu` child | **MISSING → OW4 (0.25)** |
| `inputMethodHints` (digits only, no prediction) | `inputmode` (`text`, `decimal`, `numeric`, …) → IME purpose (`egui::ImePurpose`, **verify**) | **MISSING → OW4 (0.1)** |

**Styling**

| QSS | Ours | Status |
|---|---|---|
| box model; `:focus`, `:read-only`, `:disabled` | `text-edit:focus`, `:read-only`, `:disabled` (R4 sunken field, R5 read-only as text) | ARCH (OW4, DS3) |
| `selection-color`, `selection-background-color` | `::selection` (alias of `::part(selection)`) | ARCH (OW-CSS) |
| placeholder colour (palette `PlaceholderText`) | `::placeholder` | ARCH (OW-CSS) |
| caret (`QStyle::PM_TextCursorWidth`) | `caret-color`; width from `--sv-caret-width` (P9) | `caret-color`: **MISSING → OW-CSS (0.05)**; width ARCH (§4.6) |
| `lineedit-password-character`, `lineedit-password-mask-delay` | `--sv-password-char` (above); the mask delay is P2 | MISSING (counted above) / P2 → OW-P2 (0.25) |
| clear button (`QLineEdit QToolButton`, no named sub-control) | `::part(clear)` (N) | MISSING with `clearable` (OW4b) |
| leading and trailing action icons | slotted children: plain CSS on them | MISSING with the adornments (OW4b) |
| invalid look | `:invalid`, `:user-invalid` (`outline: 1px solid var(--sv-invalid-border)`) | ARCH (OW4) |
| IME preedit underline | `::part(preedit)` (P; `text-decoration-*`) | **MISSING → OW4 (0.05)** |

**Beyond Qt.**
- `:user-invalid`: red only after the first commit.
- Live `bind:` with no JS per keystroke under `|commit`.
- Undo rules per platform (GTK, Qt, Monaco).
- PRIMARY selection and GTK hex entry.
- AccessKit text runs.
- egui's `TextEdit` has `hint_text`, `char_limit`, `password`, `interactive`, `desired_width`, `font`, `margin`,
  `clip_text` and a custom `layouter`. It has no masks, validators, adornments or clear button, no `commit` vs
  `input`, and no per-instance styling of selection and caret beyond `Visuals`.

### 2.5 QTextEdit / QPlainTextEdit → `text-edit multiline` (code and logs: quire, cadatomic CH3)

Everything in §2.4 that applies to multi-line text carries over: read-only, placeholder, undo, selection, the
signals and the styling. Additions:

| Qt | Ours | Status |
|---|---|---|
| `plainText`, `toPlainText` | `text` | have |
| `html`, `markdown`, `acceptRichText` | N/A for an editable field; read-only rich text is Q4 | N/A / Q4 |
| `lineWrapMode` (NoWrap, WidgetWidth, FixedColumnWidth), `wordWrapMode` | `wrap="none" \| "word" \| "char"` (CSS `white-space`/`overflow-wrap` on the host) | **MISSING → OW4 (0.25)** |
| `tabChangesFocus` | `tab-focus` (Tab moves focus instead of inserting `\t`; Ctrl+Tab always moves focus) | **MISSING → OW4 (0.1)** |
| `tabStopDistance` | CSS `tab-size` | **MISSING → OW-CSS (0.05)** |
| `overwriteMode` (the Insert key) | `overwrite` (bindable; Insert toggles it; the caret is drawn as a block, `::part(caret)`) | **MISSING → OW4 (0.25)** |
| `undoRedoEnabled` | `undo={false}` (TC0 history off) | **MISSING → OW4 (0.05)** |
| `maximumBlockCount` (logs) | cadatomic's Output panel (QW25), on quire or rows | cad |
| `centerOnScroll` | N/A | N/A |
| `extraSelections` (highlighted ranges with formats) | `highlights`: `[{ start, end, part }]`, each painted as `text-edit::part(highlight <part>)` (P10), e.g. `::part(highlight search-match)` | **MISSING → OW4b (0.5)** |
| `cursorWidth` | `--sv-caret-width` | ARCH |
| `textInteractionFlags` (read-only selectable, links clickable) | `read-only`; read-only rich text with links: Q4 | MISSING with `read-only` (§2.4) |
| `copyAvailable`, `undoAvailable`, `redoAvailable`, `modificationChanged` | `can-copy`, `can-undo`, `can-redo`, `modified` (observable) | **MISSING → OW4 (0.05)** (`can-copy`; the rest is §2.4's) |
| scroll bars (QAbstractScrollArea) | a multi-line `text-edit` is a Scrollable (`::part(scrollbar-y)`, `::part(thumb)`) | ARCH (OW4 on OW2) |
| spell check (Sonnet) | QS19 | P2 → OW-P2 (2) |
| line numbers, folding, syntax | quire | quire |

**Styling** adds `::part(highlight)`, `::part(caret)` (the block caret in overwrite mode) and the scroll parts of
§2.11 to §2.4's.

### 2.6 QSpinBox / QDoubleSpinBox (QAbstractSpinBox) → `drag-value`

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| `value`, `minimum`, `maximum` | `value`, `min`, `max` | have |
| `singleStep` | `step` (Steppable), `speed` (drag) | `speed`: have; `step`: ARCH (OW1b) |
| `decimals` (QDoubleSpinBox) | `decimals` | have |
| `prefix`, `suffix` | `prefix`, `suffix` | have |
| step buttons (`buttonSymbols`: UpDownArrows, PlusMinus, NoButtons) | `buttons` (shows `inc`/`dec`, FM1 QW7); glyphs from CSS (`content: icon(add)` for PlusMinus, P2); `display: none` for NoButtons | ARCH (OW1b) + P2 |
| `wrapping` | `wrap` | ARCH (OW1b, FM1 QW7) |
| PgUp/PgDn ×10 | Steppable `page` | ARCH (OW1b) |
| wheel | `wheel="hover" \| "focused" \| "off"` (Policy default) | ARCH (OW1b) |
| `keyboardTracking=false` (emit only on commit) | `commit-mode="release"` / `bind:value|commit` | ARCH (OW1b, CX3) |
| `editingFinished` | `commit` | ARCH (OW1b) |
| `specialValueText` (shown at `minimum`, e.g. "Auto") | `special-value-text` | **MISSING → OW1b (0.25)** |
| `accelerated` (holding a step button speeds up) | `accelerated` (Pressable repeat with an acceleration curve from Policy) | **MISSING → OW1b (0.25)** |
| `stepType` AdaptiveDecimalStepType (steps one decade below the value) | `step-type="adaptive"` | **MISSING → OW1b (0.25)** |
| `correctionMode` (CorrectToPreviousValue, CorrectToNearestValue) | `correction="previous" \| "nearest"` (what an out-of-range typed value commits to) | **MISSING → OW1b (0.1)** |
| `readOnly` | `read-only` (motion and copy only; Steppable and DragToValue stop, ARCH §2.5) | **MISSING → OW1b (0.1)** |
| `alignment` | CSS `text-align` on the host | ARCH (OW0b) |
| `frame` | CSS | CSS |
| `showGroupSeparator` | `group-separator` (IN1 `Intl.NumberFormat` `useGrouping`) | **MISSING → OW1b (0.1)** |
| locale-aware display and parsing (`QLocale`) | IN1 `Locale` (`1,5` in de-DE) | IN1 is merged; the wiring is **MISSING → OW1b (0.25)** |
| `displayIntegerBase` (hex, binary) | `base` (2, 8, 16) | P2 → OW-P2 (0.25) |
| `textFromValue` / `valueFromText` (subclass hooks: units, fractions, expressions) | `formatter="<name>"` / `parser="<name>"`: named Rust functions from a registry (cadatomic's `QuantityField` units and expressions); a JS `format` callback only for low-frequency display | **MISSING → OW1b (0.5)** |
| `validate`/`fixup` | as §2.4 `validator` | MISSING (counted in §2.4, OW4b) |
| `stepBy`, `stepUp`, `stepDown`, `selectAll`, `clear` | methods `step(n)`, `select-all` | **MISSING → OW1b (0.1)** |
| `valueChanged`, `textChanged` | `input` (alias `changed`), `drag-started`, `drag-stopped` | have |
| drag-to-scrub on the field (not in Qt) | DragToValue (`speed`, Shift ×0.1, Ctrl ×10) | have / ARCH |

**Styling**

| QSS | Ours | Status |
|---|---|---|
| box model on the field | `drag-value { … }` (R4 sunken) | ARCH (OW1b) |
| `::up-button`, `::down-button` (+ `:hover`, `:pressed`, `:disabled` at a limit) | `::part(inc)`, `::part(dec)` (N) with their own states; `:disabled` when at the limit without `wrap` | ARCH parts (OW1b); the limit `:disabled` **MISSING → OW1b (0.05)** |
| `::up-arrow`, `::down-arrow` | `::part(inc-glyph)`, `::part(dec-glyph)` (P; `content`, P2) | **MISSING → OW1b (0.1)** |
| button position (`subcontrol-origin: border; subcontrol-position: top right`) | P3: `::part(inc) { order: -1 }` etc. | P3 (OW0b) |
| text | `::part(text)` (P) | ARCH (OW1b) |
| `:read-only` | `:read-only` | MISSING with `read-only` (above) |
| scrubbing state | `:dragging` (bit 11) | have |

**Beyond Qt.**
- Scrub-to-change, and commit-on-release drafts that Escape restores.
- Steppable as `use:steppable` on any element, so a cadatomic quantity field gets the same keys.
- Locale numbers through IN1.
- egui's `DragValue` has `speed`, `range`, `clamp_existing_to_range`, `prefix`/`suffix`, `fixed_decimals`,
  `custom_formatter`/`custom_parser` (closures) and `update_while_editing` (**verify** names). It has no step
  buttons, wrap, special-value text or commit mode.

### 2.7 QSlider (QAbstractSlider) → `slider`, `range-slider` (QDial: N/A)

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| `value`, `minimum`, `maximum` | `value`, `min`, `max` (`range-slider`: `low`, `high`) | have |
| `singleStep` | `step` (0 = continuous) | have |
| `pageStep` | `page-step` (PgUp/PgDn, click in the track with `track-click="page"`) | **MISSING → OW1b (0.1)** (Steppable `page` is ARCH; no prop) |
| `orientation` | `orientation` (P7) | **MISSING → OW1b (0.5)** |
| `tickPosition` (NoTicks, TicksAbove, TicksBelow, TicksBothSides) | `ticks="none" \| "before" \| "after" \| "both"` | **MISSING → OW1b (0.5)** (with the `tick` part) |
| `tickInterval` | `tick-interval` (0 = `page-step`, as in Qt) | **MISSING → OW1b** (with ticks) |
| tick labels (Qt has none; QML and GTK `add_mark` have) | `marks`: `[{ value, label? }]`, snapping optional (`snap-to-marks`) | **MISSING → OW1b (0.25)** |
| `invertedAppearance` | `inverted` (max at the start; for a vertical slider, max at the top is the default) | **MISSING → OW1b (0.1)** |
| `invertedControls` (wheel and keys reversed) | `inverted-controls` | P2 → OW-P2 (0.05) |
| `tracking=false` (value emitted on release only) | `commit-mode="release"` / `bind:value|commit` | ARCH (OW1b) |
| `sliderPosition` (the drag position while not tracking) | the draft (ARCH §2.4); exposed as the `input` payload while dragging | ARCH (OW1b) |
| `sliderPressed`, `sliderReleased`, `sliderMoved` | `drag-started`, `drag-stopped`, `input` | have |
| `valueChanged` | `input` / `changed` | have |
| `actionTriggered` (SliderSingleStepAdd, PageStepAdd, …) | `step` signal with `{ n, page }` | P2 → OW-P2 (0.1) |
| click in the track: jump to the point vs page step (`SH_Slider_AbsoluteSetButtons`, GTK `gtk-primary-button-warps-slider`) | `track-click="jump" \| "page"` (Policy default) | **MISSING → OW1b (0.1)** |
| wheel | `wheel` (as `drag-value`) | ARCH (OW1b) |
| value box (`show-value`, egui) | `show-value` | have |
| logarithmic scale (not in Qt) | `scale="linear" \| "log"` (DragToValue `log`) | **MISSING → OW1b (0.1)** (the unit param is ARCH) |
| QDial (`notchesVisible`, `wrapping`) | N/A (QT-SURVEY §1.5: not worth doing). A cadatomic knob is a provider kind on DragToValue | N/A |

**Styling**

| QSS | Ours | Status |
|---|---|---|
| `::groove` | `slider::part(track)` (N) | ARCH (OW1b) |
| `::handle` (+ `:hover`, `:pressed`) | `slider::part(thumb)` (N) with its own `:hover`/`:active`/`:dragging` | ARCH (OW1b) |
| `::sub-page` (filled part before the handle) | `slider::part(fill)` (P) | ARCH (OW1b) |
| `::add-page` (part after the handle) | `slider::part(rest)` (P) | **MISSING → OW1b (0.05)** |
| tick marks (no QSS sub-control; drawn by QStyle) | `slider::part(tick)` (P, repeated; P1 gives `:first-child`/`:last-child`); `::part(mark-label)` | **MISSING → OW1b** (with ticks) |
| `:horizontal`, `:vertical` | `[orientation="…"]` | MISSING with `orientation` |
| range: the selected span | `range-slider::part(range)`, `::part(low)`, `::part(high)` | ARCH (OW1b) |
| the value box | `::part(value)` | ARCH (OW1b) |
| `handle` `margin` trick (a handle larger than the groove) | P3: `::part(thumb) { width; height; margin }`; the widget fixes only the thumb's main-axis position | P3 (OW0b) |

**Beyond Qt.**
- `accent-color` and `transition` on `::part(thumb)` (DS2).
- R17 contrast checks.
- Two-handle `range-slider` (Qt has none) and `log` scale.
- egui's `Slider` has `vertical`, `logarithmic`, `smart_aim`, `step_by`, `trailing_fill`, `handle_shape`,
  `show_value`, `prefix`/`suffix`, `clamping` and `drag_value_speed` (**verify**). It has no ticks, marks, page
  step, commit mode or per-part styling.

### 2.8 QComboBox → `combo-box`

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| items (`addItem(icon, text, data)`, `insertSeparator`, disabled items through the model) | `options`: strings, or objects `{ label, value?, icon?, disabled?, separator?, tooltip? }` | strings: have; objects: **MISSING → OW3 (0.5)** |
| `currentIndex` | `selected` | have |
| `currentText`, `currentData` | `value` (bindable, the option's `value` or label) | **MISSING → OW3 (0.1)** |
| `placeholderText` | `hint-text` | have |
| `editable` (QLineEdit inside) | `editable`: the field is an OW4 text field; `bind:text` for the edit text | **MISSING → OW3b (0.75)** |
| `insertPolicy` (NoInsert, InsertAtTop, InsertAtBottom, InsertAtCurrent, InsertAfterCurrent, InsertBeforeCurrent, InsertAlphabetically) | `insert-policy` (Enter in an editable combo adds the text as an option per policy; `insert` signal, and the app owns `options`) | **MISSING → OW3b (0.25)** |
| `duplicatesEnabled` | `allow-duplicates` | **MISSING → OW3b (0.05)** |
| `completer` (inline completion on an editable combo) | prefix completion over `options` (Typeahead in the field); a richer completer is cadatomic's (QW9) | **MISSING → OW3b (0.25)** |
| `validator` | `pattern` / `validator` (§2.4) on the editable field | MISSING (counted in §2.4) |
| `maxVisibleItems` | `max-visible` (rows before the popup scrolls; default 30 as Qt, **verify**) | **MISSING → OW3 (0.1)** |
| `maxCount` | N/A (the app owns `options`) | N/A |
| `sizeAdjustPolicy` (AdjustToContents, AdjustToContentsOnFirstShow, AdjustToMinimumContentsLengthWithIcon), `minimumContentsLength` | `size-to="contents" \| "current" \| "min-chars"` + `min-chars`; the default measures the widest option (Qt's default AdjustToContentsOnFirstShow) | **MISSING → OW3 (0.25)** |
| `iconSize` | CSS on `::part(option-icon)` | MISSING with option objects |
| `frame` | CSS | CSS |
| `modelColumn`, custom `setView` (a tree or table popup) | N/A: popups with custom content are `menu-button` or `popup` + `virtual-list`/`tree-view` (Svelte composite) | N/A |
| `showPopup`, `hidePopup` | `popup`, `popdown` methods; `bind:open` | **MISSING → OW3 (0.1)** |
| type-ahead in the closed combo (keyboardSearch) | Typeahead | ARCH (OW3, WG6 QW9) |
| wheel over the closed combo changes the selection (Qt; often disabled in IDEs) | `wheel="off" \| "hover" \| "focused"` (default `off`) | **MISSING → OW3 (0.05)** |
| `activated` (user pick, even the same item), `textActivated` | `activate` (carries index and value) | **MISSING → OW3 (0.1)** |
| `currentIndexChanged` | `changed` (`input` alias), and `commit` (§2.4: a pick is a commit) | have / ARCH |
| `highlighted`, `textHighlighted` (keyboard or hover in the popup) | `highlight` | **MISSING → OW3 (0.05)** |
| `editTextChanged` | `input` on the edit text | MISSING with `editable` |

**Styling**

| QSS | Ours | Status |
|---|---|---|
| box model; `:editable`, `:!editable` | `combo-box[editable="true"]` (field look, R4) vs button look (R1) | MISSING with `editable` |
| `::drop-down` (the button area) | `combo-box::part(button)` (N; the arrow area, hoverable on its own in editable combos) | **MISSING → OW3 (0.1)** |
| `::down-arrow` (+ `:on` when open) | `combo-box::part(chevron)`; `combo-box:open::part(chevron)` | ARCH (OW3) |
| `:on`/`:off` (popup open) | `:open` | ARCH (OW3) |
| popup list (`QComboBox QAbstractItemView`) | `combo-box::part(popup)` (N) | ARCH (OW3) |
| popup items (`QComboBox QAbstractItemView::item:selected`) | `::part(option)` (N, repeated; `:selected`, `:hover`, `:current`, `:disabled`, P1 positions) | ARCH (OW3) + P1 |
| separator item | `::part(option-separator)` | MISSING with option objects |
| the current text in the closed combo | `::part(label)` | **MISSING → OW3 (0.05)** |
| the edit field in an editable combo | `::part(field)` (N; a `text-edit` part, inherits `::placeholder`, `::selection`) | MISSING with `editable` |

**Beyond Qt.**
- `match_anchor_width` popups.
- Per-option styling through extra part names (`{ label, part: "recent" }` → `::part(option recent)`, P10):
  **MISSING → OW3 (0.1)**.
- R14 chevron rule, and R7 shadows on the popup.
- egui's `ComboBox` has `selected_text`, `width`, `height` (popup), `icon` (custom arrow painter) and
  `wrap_mode`. It is not editable and has no type-ahead, rich items or popup styling.

### 2.9 QProgressBar → `progress-bar`, `spinner`

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| `value`, `minimum`, `maximum` | `fraction` (0–1); add `value`/`min`/`max` as an alternative form | `fraction`: have; `value/min/max`: **MISSING → OW1a (0.1)** |
| busy (`minimum == maximum == 0`) | `indeterminate` (prop mirror of bit 15); `animate` stays for the egui look until OW1a | **MISSING → OW1a (0.1)** (bit ARCH; prop not) |
| `orientation` | `orientation` (P7) | **MISSING → OW1a (0.25)** |
| `invertedAppearance` | `inverted` | **MISSING → OW1a (0.05)** |
| `textVisible` | `show-percentage` / `text` | have |
| `format` (`%p%`, `%v`, `%m`) | `format` (same placeholders, numbers through IN1) | **MISSING → OW1a (0.1)** |
| `alignment` (text) | CSS `text-align` on `::part(label)` | MISSING with the label part |
| `textDirection` (vertical text) | N/A | N/A |
| QProgressDialog `minimumDuration` | cadatomic `Op<T>` waiting states (QW18) | cad |
| spinner `size`, `spinning` | have | have |

**Styling**

| QSS | Ours | Status |
|---|---|---|
| box model (the groove is the widget) | `progress-bar::part(track)` (P) | ARCH (OW1a) |
| `::chunk` (`width` + `margin` give Windows-style segments) | `::part(fill)` (P); segments through `background-image: repeating-linear-gradient` | ARCH; repeating gradient **MISSING → OW-CSS (0.1)** |
| `:horizontal`, `:vertical` | `[orientation="…"]` | MISSING with `orientation` |
| text | `::part(label)` (P) | **MISSING → OW1a (0.05)** |
| busy animation (QStyle, not stylable) | `progress-bar:indeterminate::part(fill)` with a CSS `animation` (keyframes) or the built-in sweep driven by `--sv-busy-period` | keyframes: **MISSING → OW-CSS (0.5)**; the built-in sweep: ARCH (OW1a) |
| spinner arc | `spinner` `color`, `width`/`height`, `--sv-spinner-period` | ARCH (OW1a) |

**Beyond Qt.**
- `prefers-reduced-motion` makes the busy sweep static.
- `:state(cad.pending)` drives a busy bar from Rust.
- egui's `ProgressBar` has `fill`, `animate`, `show_percentage`, `text` and `corner_radius`, horizontal only.

### 2.10 QLabel → `label`, `hyperlink`

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| `text` (plain) | `text` | have |
| `textFormat` RichText / MarkdownText / AutoText | Q4: inline children (`<b>`, `<i>`, `<code>`, `<a>`, `<span class>`) lowered to one layout job | **Q4 → OW1a (1.0)** (counted as MISSING) |
| `alignment` | CSS `text-align`, `align-content` / host `align-items` | ARCH (OW0b) |
| `wordWrap` | `wrap-mode` (`wrap`, `truncate`, `extend`) | have |
| elision (`QFontMetrics::elidedText`, ElideLeft/Middle/Right) | `elide="end" \| "middle" \| "start"` | ARCH (OW0b, WG6 QW13) |
| `margin`, `indent` | CSS `padding`, `text-indent` | `padding`: have; `text-indent`: P2 → OW-P2 (0.05) |
| `buddy` (Alt+mnemonic and a click focus the buddy) | `for="<anchor>"` (a click on the label focuses the target; the label becomes its accessible name, `labelled_by`) | **MISSING → OW1a (0.25)** |
| `openExternalLinks` | `hyperlink url` (opens through `svelters:shell`) | have |
| `linkActivated`, `linkHovered` | `hyperlink` `clicked`; inline links: Q4 (`link` signal with `href`) | have / Q4 |
| `textInteractionFlags` (TextSelectableByMouse, TextSelectableByKeyboard, LinksAccessibleByKeyboard) | `selectable` (mouse and keyboard, read-only TextInput, ARCH §6.1) | have / ARCH (OW1a) |
| `selectedText`, `setSelection`, `hasSelectedText` | as §2.4 selection props on a selectable label | **MISSING → OW1a (0.1)** |
| `pixmap`, `scaledContents` | `image` kind (`fit`) | have |
| `font-weight` | CSS `font-weight` (prop kept for compatibility) | have |
| `clicked` (not in Qt) | `clicked` | have |

**Styling**

| QSS | Ours | Status |
|---|---|---|
| box model, `color`, `font`, `qproperty-alignment` | CSS on `label`; `text-transform`, `letter-spacing`, `text-overflow` | ARCH (OW-CSS) |
| selection colours | `label::selection` | ARCH (OW-CSS) |
| link colour (palette `Link`, `LinkVisited`) | `hyperlink { color: var(--sv-link) }`, `:hover` underline (R18); `:visited` | `:visited`: P2 → OW-P2 (0.1) |
| inline spans (rich text) | `label span.cls`, `label a:hover` (Q4) | Q4 |

**Beyond Qt.**
- Middle elision as a prop, without a font-metrics call in app code.
- `text-overflow`.
- AccessKit names from `for`.
- egui's `Label` has `wrap_mode`, `selectable`, `sense` and `truncate` (end only, **verify**), and `RichText` for
  per-label colour and size in code.

### 2.11 QScrollArea / QScrollBar (QAbstractScrollArea) → `scroll-area`; standalone `scrollbar`

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| `horizontalScrollBarPolicy`, `verticalScrollBarPolicy` (AsNeeded, AlwaysOff, AlwaysOn) | CSS `overflow-x`/`overflow-y` (`auto`, `hidden`, `scroll`); `direction` prop kept as shorthand | **MISSING → OW2 (0.25)** |
| bar style (Windows classic vs overlay; not in Qt Widgets, macOS has it) | `Scrollable bar: overlay \| classic \| none` → CSS `scrollbar-gutter` + `--sv-scrollbar-mode` | ARCH (OW2) (unit param); CSS hook **MISSING → OW2 (0.1)** |
| `widgetResizable` | the content is a flex box: `min-width: 100%` etc. | CSS |
| `alignment` (of a smaller content widget) | `justify-content`/`align-items` on the area | have |
| `ensureVisible(x, y, xmargin, ymargin)`, `ensureWidgetVisible(w, xmargin, ymargin)` | `scrollIntoView(node, block, inline)`, scroll on focus | ARCH (OW2, WG6 QA13) |
| margins for `ensureVisible` | `scroll-margin` / `scroll-padding` (CSS) | **MISSING → OW2 (0.1)** |
| scroll offsets | `bind:scroll-x`, `bind:scroll-y` | have |
| `setCornerWidget` | `slot="corner"` child (the square where the bars meet) | **MISSING → OW2 (0.1)** |
| `setViewportMargins` (a ruler or header inside) | `slot="top"`/`"left"` children outside the scrolled content (sticky header row, line-number gutter) | **MISSING → OW2 (0.5)** |
| `addScrollBarWidget` (widgets beside a bar, as Qt Creator's editor) | N/A for now | P2 → OW-P2 (0.5) |
| kinetic scrolling (QScroller) | `kinetic` per Policy | ARCH (OW2) |
| overscroll / chaining | `overscroll-behavior` (CSS) | ARCH chaining; CSS property **MISSING → OW2 (0.1)** |
| smooth wheel steps | Policy + egui `smooth_scroll_delta` | ARCH (OW2) |
| QScrollBar standalone (`value`, `minimum`, `maximum`, `pageStep`, `singleStep`, `orientation`) for a custom view (a canvas, cadatomic's viewport, a plot) | `scrollbar` kind: `value`, `min`, `max`, `page` (thumb size), `step`, `orientation`, `bind:value` | **MISSING → OW2 (0.5)** |
| scroll arrows (`::add-line`/`::sub-line`; classic Windows, Qt Fusion) | `::part(inc)`, `::part(dec)` (N, Pressable repeat), hidden in the base sheet (VS Code has none) | **MISSING → OW2 (0.25)** |
| track click: page vs jump to the point (`SH_ScrollBar_LeftClickAbsolutePosition`) | `track-click` per Policy (Shift+click jumps on GTK) | **MISSING → OW2 (0.1)** |
| context menu on the bar (Scroll here, Top, Bottom, Page up/down) | N/A (Qt-only habit) | P2 → OW-P2 (0.25) |
| `sliderMoved`, `valueChanged` | `changed` / `input` | have |

**Styling**

| QSS | Ours | Status |
|---|---|---|
| `QScrollBar` box (the trough) | `::part(scrollbar-x)`, `::part(scrollbar-y)` (N) | ARCH (OW2) |
| `::handle` (+ `:hover`, `:pressed`) | `::part(thumb)` (N; one per bar, distinguished as `::part(thumb-x)`/`::part(thumb-y)`) | ARCH has `thumb` per bar; the axis names are **MISSING → OW2 (0.05)** |
| `::add-line`, `::sub-line`, `::up-arrow`, `::down-arrow`, `::left-arrow`, `::right-arrow` | `::part(inc)`, `::part(dec)`, glyphs from `content` (P2) | MISSING with the arrows |
| `::add-page`, `::sub-page` (trough on either side of the handle) | `::part(track-after)`, `::part(track-before)` (P) | **MISSING → OW2 (0.1)** |
| `:horizontal`, `:vertical` | the two part names; the standalone `scrollbar[orientation]` | ARCH / MISSING with `scrollbar` |
| `QAbstractScrollArea::corner` | `::part(corner)` (P) | **MISSING → OW2 (0.05)** |
| scroll edge shadows (no Qt equivalent) | `::part(shadow-top/bottom/left/right)` (R15) | ARCH (OW2) |
| fade in and out (overlay bars) | `transition: opacity` on the bar parts, `:hover` on the area | ARCH (OW2, DS2) |

**Beyond Qt.**
- Overlay bars with fade (VS Code).
- Edge shadows (R15).
- Per-axis CSS overflow.
- `scroll-margin`.
- One `scroll_into_view` chain for focus, the caret and `scrollIntoView`.
- egui's `ScrollArea` has `ScrollBarVisibility`, `auto_shrink`, `stick_to_bottom`, `scroll_bar_rect` and global
  `Spacing.scroll` (floating bars, widths, fade). It has no corner widget, arrows, edge shadows or per-instance
  bar style.

### 2.12 QTabWidget / QTabBar → `tab-view`, `tab`

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| `currentIndex`, `currentChanged` | `selected`, `changed` | have |
| tab text, `setTabToolTip`, `setTabWhatsThis` | `tab title`; per-tab tooltip `tab-tooltip` (`tooltip` on `tab` is the page's) | title: have; `tab-tooltip`: **MISSING → OW5c (0.05)** |
| `setTabIcon` | `tab icon` | **MISSING → OW5c (0.1)** |
| `setTabEnabled`, `setTabVisible` | `tab enabled`, `tab visible` (Widget props, given tab-header meaning) | **MISSING → OW5c (0.1)** (the props exist; the strip ignores them) |
| `tabPosition` (North, South, West, East) | `tab-position="top" \| "bottom" \| "left" \| "right"` (P7) | **MISSING → OW5c (0.75)** |
| `tabShape` (Rounded, Triangular) | N/A (CSS shapes the tab) | N/A |
| `tabsClosable` | `tab closable` (per tab; Qt's is per bar) | have |
| `tabCloseRequested` | `close-requested` (button or middle click) | have |
| `movable`, `tabMoved` | `movable` (drag to reorder within the strip; the app reorders its `tab`s on `move(from, to)`) | **MISSING → OW5c (0.75)** |
| `elideMode` | `elide` (default `end`, VS Code `tab.sizing`) | **MISSING → OW5c (0.05)** |
| `expanding` (tabs fill the bar) | CSS: `tab-view::part(header) { flex-grow: 1 }` (P3) | CSS (with P3) |
| `documentMode` | CSS (a class on `tab-view`) | CSS |
| `usesScrollButtons` / overflow | `overflow="scroll" \| "menu" \| "wrap"` (VS Code: scroll + a tab list dropdown; `::part(overflow)` exists in ARCH) | **MISSING → OW5c (0.25)** (ARCH has the part only) |
| `drawBase` | CSS on `::part(strip)` (`border-bottom`) | MISSING with the strip part |
| `autoHide` (hide the bar with one tab) | `auto-hide` | **MISSING → OW5c (0.05)** |
| `changeCurrentOnDrag` (hovering a tab during a drag selects it) | `switch-on-drag` (DnD hover delay from Policy) | **MISSING → OW5c (0.1)** |
| `selectionBehaviorOnRemove` (SelectLeftTab, SelectRightTab, SelectPreviousTab) | `on-remove="left" \| "right" \| "previous"` (keeps an MRU of selections) | **MISSING → OW5c (0.1)** |
| `setTabButton(index, LeftSide/RightSide, widget)` | `tab` children with `slot="start"` / `slot="end"` (pin, dirty dot, a menu), laid out in the header | **MISSING → OW5c (0.25)** |
| `setCornerWidget(TopLeft/TopRight)` | `tab-view` children with `slot="start"` / `slot="end"` (actions to the right of the strip, VS Code editor actions) | **MISSING → OW5c (0.25)** |
| `tabBarClicked`, `tabBarDoubleClicked` | `tab-clicked`, `tab-double-clicked` (index) | **MISSING → OW5c (0.05)** |
| tab context menu | a `context-menu` child of the `tab` (applies to its header) | **MISSING → OW5c (0.1)** |
| `setTabData` | N/A (the app keys its tabs) | N/A |
| keyboard: Ctrl+Tab / Ctrl+PgUp/PgDn, arrows with Roving | Roving + `Policy` chords | ARCH (OW5c) |
| dragging tabs out to other windows (document tabs) | berth (document tabs are berth's) | berth |

**Styling**

| QSS | Ours | Status |
|---|---|---|
| `QTabWidget::pane` | `tab-view::part(pane)` (N; the page area) | **MISSING → OW5c (0.05)** |
| `QTabWidget::tab-bar` (position via `left:`) | `tab-view::part(strip)` (N; P3 positions it) | **MISSING → OW5c (0.05)** |
| `QTabBar::tab` (+ `:selected`, `:hover`, `:!selected`, `:disabled`) | `tab-view::part(tab)` (ARCH) or, per tab, `tab::part(header)` (P6) | ARCH (OW5c) + P6 |
| `::tab:first`, `:last`, `:middle`, `:only-one` | `::part(tab):first-child` … (P1) | P1 |
| `::tab:previous-selected`, `:next-selected` | N/A (§1.2) | N/A |
| `::tab:top`, `:bottom`, `:left`, `:right` | `tab-view[tab-position="…"]::part(tab)` | MISSING with `tab-position` |
| `QTabBar::close-button` (+ `:hover`) | `::part(close)` (N) | ARCH (OW5c) |
| `QTabBar::tear` | N/A (Qt draws a tear for elided scroll; we use edge shadows) | N/A |
| `QTabBar::scroller`, `QTabBar QToolButton` (scroll arrows) | `::part(overflow)` (N), `::part(scroll-start)`, `::part(scroll-end)` | ARCH has `overflow`; scroll buttons **MISSING → OW5c (0.1)** |
| `QTabWidget::left-corner`, `::right-corner` | slotted children: plain CSS | MISSING with the corner slots |
| dirty marker, active-tab top border (VS Code) | `::part(dirty)`, `tab-view::part(tab):selected { border-top: 2px solid var(--sv-tab-active-marker) }` (R13) | ARCH (OW5c) |
| per-tab colour (`setTabTextColor`) | `tab.modified::part(header) { color: … }` (P6) | P6 |

**Beyond Qt.**
- R13 (active tab joins the content, `elev-2` hover).
- `:focus-visible` on tab headers.
- Per-tab `:state()` through P6, e.g. `:state(cad.failed)` on a tab whose document failed to rebuild.
- egui has no tab widget (berth and egui_dock bring their own). Today's `tab-view` is `ScrollArea` +
  `Button::selectable`.

### 2.13 QToolBar → `toolbar` (new kind; the seam with cadatomic CH1 is for the coordinators, §6)

Today there is no kind; a toolbar is a `box` of `icon[frame]` buttons. QT-SURVEY QS3 put the toolbar and the split
button in cadatomic (the command strip, CH1). The parity requirement needs the generic kind. Recommendation: OW3b
ships `toolbar`, and cadatomic's command strip is a `toolbar` fed from its command registry.

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| `addAction`, `addWidget`, `addSeparator` | children: buttons, `toggle-button`s, `menu-button`s, any element, `separator` | **MISSING → OW3b (`toolbar` kind 1.0)** |
| `orientation` | `orientation` (P7) | MISSING with the kind |
| `toolButtonStyle` (for all its buttons) | a class on the toolbar plus §2.1's CSS (`.text-under button { flex-direction: column }`) | CSS (with P3) |
| `iconSize` | `--sv-icon-size` on the toolbar, inherited by `::part(icon)` | CSS |
| keyboard: one tab stop, arrows move (Qt does not do this; ARIA `toolbar` does) | Roving | MISSING with the kind |
| overflow: the extension button (`qt_toolbar_ext_button`) with a popup of the hidden items | `overflow="menu" \| "wrap" \| "clip"`; hidden items are listed in `::part(overflow)`'s menu (items need a `label` or `tooltip` to be listed) | **MISSING → OW3b (0.75)** |
| `movable`, `floatable`, `allowedAreas`, `toggleViewAction`, the handle | berth (toolbars as dock items) / cadatomic (shell customisation, QS6) | berth / cad |
| `actionTriggered` | each child's own signal | have |
| AccessKit `Role::Toolbar`, orientation | yes | MISSING with the kind |

**Styling**

| QSS | Ours | Status |
|---|---|---|
| box model; `spacing` | `toolbar { gap }` | MISSING with the kind |
| `::separator` | `toolbar > separator` (a real child) | have (`separator` kind) |
| `::handle` | berth | berth |
| `:top`, `:bottom`, `:left`, `:right` (dock area) | `[orientation]`, or a class the host sets | MISSING with the kind |
| `:first`, `:last`, `:middle`, `:only-one` (Qt 6) | real children: `:first-child` etc. | have (structural on nodes) |
| buttons inside (`QToolBar QToolButton:hover`) | `toolbar button:hover` | have |
| the overflow chevron | `::part(overflow)` (N), `content: icon(chevron-down)` / `icon(ellipsis)` (R14) | MISSING with overflow |

**Beyond Qt.** Container queries (`@container`, later) let a toolbar change its button style by width, which Qt
has no stylesheet way to do. The `overflow` menu re-uses the OW3 menu, with no second implementation.

### 2.14 QMenu / QMenuBar (QAction) → `menu-bar`, `menu`, `action`, `menu-separator`, `context-menu`

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| QAction `text` | `action label` | have |
| QAction `icon`, `iconVisibleInMenu` | `action icon`; `show-icon` (Policy: hidden on macOS) | **MISSING → OW3 (0.25)** |
| QAction `shortcut`, `shortcuts`, `shortcutContext`, `shortcutVisibleInContextMenu` | `action shortcut` (chords, `std:` names, QA4); context is the owner's draw (have) | have; chords and `std:` names: **MISSING → OW3 (0.25)** |
| QAction `checkable`, `checked`, `toggled` | `checkable`, `checked`, `toggled` | have |
| QActionGroup `exclusionPolicy` Exclusive / ExclusiveOptional | `group`; `group-optional` (a pick of the checked item unchecks it) | Exclusive: have; ExclusiveOptional: **MISSING → OW3 (0.05)** |
| QAction `enabled`, `visible` | `enabled`, `visible` (MenuEntry) | have |
| QAction `statusTip` | `status-tip` (feeds the status bar, QA5) | **MISSING → OW3 (0.1)** |
| QAction `toolTip` with `toolTipsVisible` on the menu | `tooltip` on `action`; `menu tooltips` | `tooltip`: have; showing it in menus: **MISSING → OW3 (0.05)** |
| QAction `triggered`, `hovered` | `activate`, `hover` | `activate`: have; `hover`: **MISSING → OW3 (0.05)** |
| QAction `autoRepeat` (held shortcut repeats) | `repeat` | P2 → OW-P2 (0.1) |
| QAction `menuRole` (macOS app menu) | the native-menus pack (QS5) | pack |
| QAction `priority` (which toolbar items hide first) | `toolbar` overflow `priority` on children | MISSING with overflow (§2.13) |
| QMenu `title`, `icon` | `menu label`, `menu icon` | label: have; icon: MISSING with `action icon` |
| QMenu `addSection(text)` (a titled separator) | `menu-separator label` | **MISSING → OW3 (0.1)** |
| QMenu `separatorsCollapsible` | always on (leading, trailing and doubled separators hidden, as Qt's default) | **MISSING → OW3 (0.05)** |
| QMenu `tearOffEnabled` | N/A (berth has floating panels) | N/A |
| QMenu `defaultAction` (bold) | `action default` → `:default` | **MISSING → OW3 (0.05)** |
| QMenu `aboutToShow`, `aboutToHide` (lazy content) | `menu` `open`/`changed` signal; children rendered only while open | **MISSING → OW3 (0.1)** |
| QMenu `exec(pos)` / `popup(pos)` | `context-menu` `popup()` (at the pointer), `popup(x, y)`; keyboard Menu key and Shift+F10 | `popup()`: have; position and keys: ARCH (OW3, FM1 QA8) |
| a scrollable tall menu (`QMenu::scroller`) | Scrollable inside the menu popup with scroll arrows | **MISSING → OW3 (0.25)** |
| submenu open on hover after a delay (`SH_Menu_SubMenuPopupDelay`), the "safe triangle" (`SH_Menu_SubMenuSloppyCloseTimeout`) | Policy delays; the sloppy triangle in Anchored/Dismissable | ARCH delay (OW3); the triangle **MISSING → OW3 (0.25)** |
| menu-bar: Alt focuses the bar, mnemonics, hover switches while open | Alt → first menu (Policy, Windows/KDE), hover-to-switch | hover: ARCH (OW3); Alt: **MISSING → OW3 (0.1)**; mnemonics P2 (QA16) |
| QMenuBar `cornerWidget` | `menu-bar` children with `slot="end"` | **MISSING → OW3 (0.05)** |
| QMenuBar `nativeMenuBar` | the native-menus pack (QS5) | pack |
| widgets in menus (QWidgetAction) | any element child of `menu` (sliders, a search field) | **MISSING → OW3 (0.25)** (menus take only `action`/`menu`/`menu-separator` today) |

**Styling**

| QSS | Ours | Status |
|---|---|---|
| `QMenu` box (background, border, radius, padding; shadow is not QSS) | `context-menu`, `menu::part(popup)` (N) with `box-shadow` (R7) | ARCH (OW3) |
| `QMenu::item` (+ `:selected`, `:disabled`, `:checked`) | `action` (a real node: `:hover`, `:current`, `:disabled`, `:checked`) | ARCH (OW3) |
| `QMenu::indicator` (+ `:exclusive`, `:non-exclusive`, `:checked`) | `action::part(check)` (P); `action:not([group=""])::part(check)` for radio items | ARCH (OW3) |
| `QMenu::icon` (+ `:checked`) | `action::part(icon)` | MISSING with `action icon` |
| `QMenu::separator` | `menu-separator` (a node) | ARCH (OW3) |
| section title | `menu-separator::part(label)` | MISSING with sections |
| `QMenu::right-arrow` (submenu) | `menu::part(chevron)` (`content: icon(chevron-right)`, P2) | ARCH (OW3) |
| `QMenu::scroller` | `::part(scroll-up)`, `::part(scroll-down)` | MISSING with scrolling |
| `QMenu::tearoff` | N/A | N/A |
| shortcut text (not stylable in QSS) | `action::part(shortcut)` (P) | ARCH (OW3) |
| `QMenuBar::item` (+ `:selected`, `:pressed`) | `menu-bar > menu` (the title is the node): `:hover`, `:open` | ARCH (OW3) |
| `QMenuBar` `spacing` | `gap` | have |
| `:first`, `:last` items (rounded first and last rows) | real children: `:first-child`, `:last-child` | have (structural on nodes) |

**Beyond Qt.**
- R7 shadows, stroke and tone on menus.
- Transitions for the open fade, instant under reduced motion.
- `:has(action:checked)` for menus that show a check column only when one is needed.
- `:state()` on actions from cadatomic's command availability (R9 disabled with a reason, readable in menus).
- egui's `menu` has `MenuConfig` (close behaviour, style) and `Button::shortcut_text`. It has no check or radio
  items with indicators, sections, scrolling or per-item styling.

### 2.15 QToolTip → the common `tooltip` prop (Tooltipable)

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| `setToolTip(text)` on any widget | `tooltip` on every kind | have |
| rich text tooltips (HTML) | a `{#snippet tooltip()}` (any content); Q4 inline rich text for the string form | ARCH (OW3: "text or snippet"); Q4 |
| tooltips on disabled widgets | yes (Tooltipable runs when disabled; R9) | ARCH (OW3, WG6 QA5) |
| `QToolTip::showText(pos, text, w, rect, msecShowTime)`, `hideText`, `isVisible` | `svelters:tooltip` `show(text, { at, anchor, rect, duration })`, `hide()`; Rust `tooltip::show` | **MISSING → OW3 (0.25)** |
| `rect` (the tooltip hides when the pointer leaves a sub-rect) | `tooltip-rect` per hit region of a canvas / provider kind (a `Behavior` sets the region) | **MISSING → OW3 (0.1)** |
| wake-up delay (`SH_ToolTip_WakeUpDelay`, about 700 ms, **verify**), fall-asleep window (instant follow-on) | `Policy::tooltip_delay_ms` + follow-on window; `tooltip-delay` per node | Policy: ARCH (OW3); follow-on and the per-node prop **MISSING → OW3 (0.25)** |
| `msecShowTime` (auto-hide after a time proportional to the text length) | `tooltip-duration` (0 = while hovered, the default) | **MISSING → OW3 (0.05)** |
| `statusTip`, `whatsThis` | `status-tip` (QA5); whatsThis replaced by cadatomic's live tooltips | MISSING (counted in §2.14) / N/A |
| placement | Anchored (below the pointer, flipped at edges); `tooltip-placement` | **MISSING → OW3 (0.05)** |
| live tooltip content (cadatomic ide-shell §6) | the snippet is reactive while shown | ARCH (OW3) |

**Styling**

| QSS | Ours | Status |
|---|---|---|
| `QToolTip` box, `opacity` | `:root::part(tooltip)` (a viewport-level node part) or `.tooltip` on the snippet root; `box-shadow` (R7), `opacity`, `transition` | ARCH (OW3); the selector **MISSING → OW3 (0.05)** (where the tooltip's style host is: the hovered element, so `button::part(tooltip)` styles that button's tooltip) |
| palette `ToolTipBase`, `ToolTipText` | tokens `--sv-tooltip-bg`, `--sv-tooltip-fg` | ARCH (DS3) |

**Beyond Qt.**
- Tooltips per element type through the style host (`.danger::part(tooltip)`).
- Snippet tooltips with live content.
- AccessKit `described_by`.
- Fade transitions.
- egui has `on_hover_text`, `on_disabled_hover_text`, `on_hover_ui`, `show_tooltip_at` and global
  `Style::interaction.tooltip_delay` / `tooltip_grace_time` (**verify**). There is no per-tooltip styling.

### 2.16 QGroupBox (and QToolBox) → `frame`; `collapsing-header`

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| `title` | `frame label` | have |
| `alignment` (of the title) | CSS `text-align` / `justify-self` on `::part(caption)` | MISSING with the caption part (Styling) |
| `flat` (only the top border) | CSS (a class) | CSS |
| `checkable`, `checked`, `toggled`, `clicked` (the check enables or disables the children) | `checkable`, `bind:checked`, `changed`; unchecked disables the subtree (`:disabled` inherited) | **MISSING → OW1a (0.5)** |
| AccessKit `Role::Group` named by the title | yes | ARCH (OW1a) |
| QToolBox (`addItem`, `currentIndex`, one page open at a time) | `collapsing-header`s, plus an `accordion` group (one open at a time) | `collapsing-header`: have; accordion: **MISSING → OW1a (0.25)** |
| QToolBox `setItemIcon`, `setItemToolTip`, `setItemEnabled` | `collapsing-header icon`, `tooltip`, `enabled` | `icon`: **MISSING → OW1a (0.05)**; the rest: have |
| collapsing-header header actions (VS Code view titles; not in Qt) | `slot="end"` children in the header (icon buttons shown on hover) | **MISSING → OW1a (0.25)** |

**Styling**

| QSS | Ours | Status |
|---|---|---|
| `QGroupBox` box (`margin-top` for the title) | `frame { … }` | ARCH (OW1a) |
| `QGroupBox::title` (`subcontrol-origin: margin; subcontrol-position: top center`) | `frame::part(caption)` (P3 positions it) | ARCH (OW1a) + P3 |
| `QGroupBox::indicator` (+ `:checked`) | `frame::part(indicator)`, `::part(mark)` (as checkbox) | MISSING with `checkable` |
| `QToolBox::tab` (+ `:selected`, `:first`, `:last`, `:only-one`) | `collapsing-header::part(header)` (N), `:expanded`; positions on real nodes | ARCH (OW1a) |
| the twistie | `::part(twistie)` (`content: icon(chevron-right)`, rotated by `transform` on `:expanded`) | ARCH (OW1a) + P2 |

**Beyond Qt.** The expand and collapse animation comes from TR transitions and the `transform` on the twistie.
Header actions show on `:hover`/`:focus-within`. egui's `CollapsingHeader` has `default_open`, `icon` (a custom
painter), `show_background` and `open`.

### 2.17 QSplitter → `split-view`, `panel`

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| `orientation` | `orientation` | have |
| N children with N−1 handles | two panes only today | **MISSING → OW2 (0.75)** |
| `sizes`, `setSizes` | `bind:divider-position` (two panes); with N panes, `bind:sizes` (a list) | two: have; N: with the row above |
| `setStretchFactor(i, n)` (how extra space is shared on resize) | `flex-grow` on panes (taffy) | **MISSING → OW2 (0.1)** (the panes ignore their flex props today) |
| `handleWidth` | CSS `width`/`height` of `::part(sash)`, hit area from `--sv-sash-hit` (R11: 1 px line, 4 px hit) | ARCH (OW2) |
| `childrenCollapsible`, `setCollapsible(i)` (dragging past the minimum collapses the pane) | `collapsible` (per pane: `data-collapsible` or a pane prop) | **MISSING → OW2 (0.25)** |
| double-click the handle to restore (QW16; not in Qt, common in IDEs) | `restore-on-double-click` (the last uncollapsed size) | **MISSING → OW2 (0.1)** |
| `opaqueResize` (live vs on release) | `commit-mode="release"` (a ghost line while dragging) | **MISSING → OW2 (0.1)** |
| `splitterMoved` | `changed`, `drag-started`, `drag-stopped`; `commit` | have / ARCH |
| `saveState`, `restoreState` | `bind:` + `uiState()` (DX1) | DX1 |
| min and max sizes per pane | `min-size` (global); per pane `min-width`/`max-width` (Widget props) honoured | global: have; per pane: **MISSING → OW2 (0.1)** |
| keyboard: the handle takes focus, arrows move it (Qt does not; WAI-ARIA `separator`) | Resizable keyboard step; F6 cycles panes | ARCH (OW2) |
| `panel`: `side`, `size`, `resizable`, `separator` | have | have |

**Styling**

| QSS | Ours | Status |
|---|---|---|
| `QSplitter::handle` (+ `:hover`, `:pressed`, `:horizontal`, `:vertical`) | `split-view::part(sash)` (N) with `:hover`, `:active`, `:dragging`, `:focus-visible`; `[orientation]` on the host | ARCH (OW2) |
| grip dots (no QSS) | `::part(grip)` (P; R11 shows it after 300 ms hover via `transition-delay`) | **MISSING → OW2 (0.05)** |
| collapsed pane state | `[collapsed="true"]` on the pane (the widget writes the prop; no new bit) | MISSING with collapsible |

**Beyond Qt.** R11 sashes (hairline, delayed grip, focus colour), `cursor: col-resize`, and keyboard-resizable
sashes with AccessKit `Role::Splitter`. egui has no splitter (`SidePanel::resizable` only).

### 2.18 Item views: QAbstractItemView, QListView, QTreeView, QTableView, QHeaderView → `virtual-list`, `tree-view`, `table`, `table-column`

`tree-view` (WG5b, merged `433ec422`) is missing from ARCH-WIDGETS §6.1. It joins OW5a with the same units as
`virtual-list` (Selectable, Typeahead, Scrollable, Expandable, DragSource/DropTarget).

#### 2.18.1 Common (QAbstractItemView)

| Qt | Ours | Status |
|---|---|---|
| `selectionMode` NoSelection / SingleSelection / ExtendedSelection | `selection="none" \| "single" \| "multi"` (multi = Qt's Extended: Ctrl/Shift) | have (list, tree, table) |
| `selectionMode` MultiSelection (a click toggles, no modifier) | `selection="toggle"` (checklists) | **MISSING → OW5a (0.1)** |
| `selectionMode` ContiguousSelection | `selection="range"` (Selectable `Range`, ARCH) | **MISSING → OW5a (0.05)** (unit ARCH; prop value not) |
| `selectionBehavior` SelectItems / SelectRows / SelectColumns | `table selection-unit="row" \| "cell" \| "column"` (rows only today; cells keyed by `(rowId, column)`) | **MISSING → OW5b (0.75)** |
| `currentIndex` (cursor vs selection) | `current` (table), `:current` (bit 19); list/tree `bind:current` | table: have; list/tree: **MISSING → OW5a (0.1)** |
| `editTriggers` (NoEditTriggers, CurrentChanged, DoubleClicked, SelectedClicked, EditKeyPressed, AnyKeyPressed) | `edit-triggers="double-click f2 typing selected-click"` (a token list; today's fixed set is double-click + F2 + typing) | **MISSING → OW5b (0.25)** |
| editing in list and tree rows (rename in place) | rows are nodes: a `text-edit` in the row snippet, with `edit(id)`/`end-edit` methods and the same triggers | **MISSING → OW5a (0.5)** |
| `dragDropMode` (NoDragDrop, DragOnly, DropOnly, DragDrop, InternalMove), `dragEnabled`, `defaultDropAction`, `showDropIndicator`, `dragDropOverwriteMode` | tree: `row-drag-type`/`row-drop-types` (have); list and table: same props **MISSING → OW5a (0.5)**; `drop-actions` (copy, move, link by modifier, QA9) ARCH (OW5a); `internal-move` (reorder, `move(from, to)` signal) **MISSING → OW5a (0.25)** | mixed |
| `alternatingRowColors` | `table striped` (have); list and tree: CSS `::part(row):nth-child(even)` by logical index (P1) | table: have; list/tree: P1 |
| `iconSize` | CSS `--sv-icon-size` on the view | CSS |
| `textElideMode` | `elide` on `table-column` and on label rows | **MISSING → OW5b (0.05)** |
| `verticalScrollMode` / `horizontalScrollMode` (ScrollPerItem, ScrollPerPixel) | per pixel always; `scroll-snap="row"` | P2 → OW-P2 (0.25) |
| `autoScroll`, `autoScrollMargin` (during drags) | DropTarget auto-scroll edges | ARCH (OW5a) |
| `tabKeyNavigation` (Tab moves between cells) | table: Tab to the next cell while editing (WG5a QW3); `tab-navigation` to also move the cursor when not editing | editing: have; the prop: **MISSING → OW5b (0.05)** |
| `keyboardSearch` | Typeahead (labels from `splice`/`set-ids`) | have |
| `scrollTo(index, hint)` (EnsureVisible, PositionAtTop, PositionAtBottom, PositionAtCenter) | `scrollTo(pos)` / `scroll-to-row(row)`; the `hint` argument | method: have; hint: **MISSING → OW5a (0.1)** |
| `activated`, `clicked`, `doubleClicked`, `pressed`, `entered` | `activate`/`activated`; `row-clicked`, `row-hovered` | activate: have; the others: **MISSING → OW5a (0.1)** |
| `setItemDelegate`, `setItemDelegateForColumn` (custom paint and editors) | list and tree: rows are scene nodes (any content); table: `table-column editor` snippets (have); custom cell paint: Q3 | have / Q3 |
| `openPersistentEditor` | table: an always-on editor column (checkboxes in a column) | **MISSING → OW5b (0.5)** |
| `setIndexWidget` | list/tree rows: any content (have); table cells: Q3 | have / Q3 |
| empty state (no Qt API; VS Code views have "no items" text) | `slot="empty"` child shown when `count() == 0` | **MISSING → OW5a (0.1)** |
| context menu on rows (`customContextMenuRequested`) | `context-menu` child; `row-context` signal with the row under the pointer (the selection follows, per Policy) | **MISSING → OW5a (0.1)** |
| Ctrl+A, copy (`copy` signal on tree) | Selectable Ctrl+A; `copy` on all three | Ctrl+A: ARCH; `copy` on list and table: **MISSING → OW5a (0.05)** |
| AccessKit: list, tree and grid roles, set size, level, `active_descendant` | yes | ARCH (OW5a/b) |

#### 2.18.2 QListView

| Qt | Ours | Status |
|---|---|---|
| `viewMode` ListMode / IconMode, `flow`, `isWrapping`, `gridSize`, `spacing`, `uniformItemSizes` | `layout="list" \| "grid"` (grid is drawn as a list today); `item-width`, `gap`; uniform sizes in grid mode | **MISSING → OW5a (1.0)** (the grid layout) |
| `movement` (Static, Free, Snap: dragging icons in IconMode) | N/A (an icon canvas is cadatomic's) | N/A |
| `resizeMode`, `layoutMode` Batched, `batchSize` | N/A (virtualised) | N/A |
| `setRowHidden` | the app splices rows out | have (JS) |
| `wordWrap` | rows are nodes: `wrap-mode` on their labels; measured row heights | have |
| `selectionRectVisible` (rubber band) | `rubber-band` (a drag on empty space selects, Selectable `Range`) | **MISSING → OW5a (0.5)** |
| sticky section headers (QML `ListView.section`, QT-SURVEY QW17) | `section` per row (from `splice` labels) + `::part(section)` sticky header | **MISSING → OW5a (1.0)** (unless WG5a delivered it: **verify**) |

#### 2.18.3 QTreeView

| Qt | Ours | Status |
|---|---|---|
| `indentation` | `indent` | have |
| `rootIsDecorated` | `root-decorated` (twisties on level 0) | **MISSING → OW5a (0.05)** |
| `itemsExpandable`, `expandsOnDoubleClick` | `expandable`, `expand-on="double-click" \| "click" \| "twistie"` | **MISSING → OW5a (0.1)** |
| `animated` (expand animation) | CSS: rows enter with `transition`/TR (instant under reduced motion) | **MISSING → OW5a (0.25)** |
| `uniformRowHeights` | `row-height` as the assumed and minimum height; `uniform-rows` skips measuring | **MISSING → OW5a (0.1)** |
| `autoExpandDelay` (expand on DnD hover) | DropTarget auto-expand (QA9) | ARCH (OW5a) |
| `expandAll`, `collapseAll`, `expandToDepth`, `expandRecursively` | JS owns the tree (`setNodes`, `splice`): helpers in `svelters:views/tree` | **MISSING → OW5a (0.25)** |
| `isExpanded`, `setExpanded` | `setNodes` flags (2 = expanded) | have |
| `expanded`, `collapsed` signals | `expand`, `collapse` | have |
| multiple columns with a header (QTreeView + QHeaderView; `allColumnsShowFocus`, `firstColumnSpanned`, `treePosition`) | a tree with `table-column` children (the header of §2.18.5, rows laid out on the columns' widths; the tree column holds the twistie) | **MISSING → OW5a (1.5)** |
| `sortingEnabled` (header click sorts) | with the multi-column tree: `header-clicked`, the app sorts | MISSING with columns |
| `headerHidden` | `header={false}` | MISSING with columns |
| `wordWrap` | rows are nodes | have |
| `resizeColumnToContents` | with columns: double-click on the header edge | MISSING with columns |

#### 2.18.4 QTableView

| Qt | Ours | Status |
|---|---|---|
| `showGrid`, `gridStyle` | CSS on `::part(grid)` (`border-color`, `border-style`), `--sv-grid-lines: none \| rows \| cols \| both` | **MISSING → OW5b (0.25)** |
| `sortingEnabled`, sort indicator | `table-column sort`, `header-clicked` | have |
| `wordWrap` in cells | P2 (fixed row height) | P2 → OW-P2 (0.5) |
| `cornerButtonEnabled` (select all) | `::part(corner)` (N; Ctrl+A on click) | **MISSING → OW5b (0.1)** |
| `setSpan` (merged cells) | N/A for now | P2 → OW-P2 (1.5) |
| `hideRow`, `hideColumn`, `setColumnHidden` | rows: the app's ids; columns: `table-column visible` | rows: have; `visible` honoured: **MISSING → OW5b (0.05)** |
| `setColumnWidth`, `columnWidth` | `table-column width` | have |
| `resizeColumnsToContents` | double-click on the edge (have); a `fit-columns()` method | **MISSING → OW5b (0.1)** |
| frozen leading columns (not in QTableView) | `sticky-columns` | have |
| row height | `row-height` (fixed) | have |
| vertical header (row numbers) | `row-header` (`::part(row-header)`) | P2 → OW-P2 (0.5) |
| cell editors, commit, reject | `editable`, `editor` snippet, `commit` with `e.result` | have |
| cell tooltips (`ToolTipRole`) | `tooltip` per cell in the row data (`{ value, tooltip }` cells) | **MISSING → OW5b (0.1)** |
| per-cell styling (`ForegroundRole`, `BackgroundRole`, `FontRole`) | per column: `table-column.numeric::part(cell)` (P6); per cell: cells as `{ value, part: "negative" }` add a part name (`table::part(cell negative)`, P10) | **MISSING → OW5b (0.5)** |
| checkable cells (`CheckStateRole`) | a boolean column paints `::part(cell-check)` and toggles on Space or click (`commit`) | **MISSING → OW5b (0.25)** |
| icons in cells (`DecorationRole`) | `{ value, icon }` cells, `::part(cell-icon)` | **MISSING → OW5b (0.1)** |

#### 2.18.5 QHeaderView → the `table` header, `table-column`

| Qt | Ours | Status |
|---|---|---|
| `sectionResizeMode` Interactive / Fixed / Stretch / ResizeToContents (per section) | `table-column resize="interactive" \| "fixed" \| "stretch" \| "contents"` (`resizable` stays as shorthand) | **MISSING → OW5b (0.5)** |
| `stretchLastSection` | `stretch-last` on `table` (default true, as Qt's QTreeView) | **MISSING → OW5b (0.05)** |
| `cascadingSectionResizes` | N/A | N/A |
| `defaultSectionSize`, `minimumSectionSize`, `maximumSectionSize` | `table-column width` default 100; `min-width`/`max-width` on `table-column` honoured | **MISSING → OW5b (0.1)** |
| `sectionsMovable` (drag to reorder), `setFirstSectionMovable`, `moveSection`, `sectionMoved`, `visualIndex`/`logicalIndex` | `columns-movable`; `column-moved(from, to)`; the app reorders its `table-column` children (FM1 QW4 "reorder deferred") | **MISSING → OW5b (0.75)** |
| `sectionsClickable`, `sectionClicked`, `sectionDoubleClicked`, `sectionPressed`, `sectionEntered` | `header-clicked` (have); `header-double-clicked` | **MISSING → OW5b (0.05)** |
| `sectionHandleDoubleClicked` (fit to contents) | double-click on the edge fits | have |
| `sectionResized` | `column-resized(column, width)`; `bind:width` on `table-column` | **MISSING → OW5b (0.1)** |
| `setSortIndicator(section, order)`, `sortIndicatorShown`, `sortIndicatorClearable` (Qt 6.1), `sortIndicatorChanged` | `table-column sort` (none, ascending, descending; the app cycles); a secondary-sort index badge (`sort-priority`) | `sort`: have; `sort-priority`: **MISSING → OW5b (0.1)** |
| `highlightSections` (header of a column with selected cells is highlighted) | `:selected` on the header cell when its column has selection (cell selection mode) | MISSING with cell selection |
| `defaultAlignment` | `table-column align` applies to the header too; `header-align` to differ | **MISSING → OW5b (0.05)** |
| `hideSection` | `table-column visible` | MISSING (counted in §2.18.4) |
| header context menu (show and hide columns, QW4) | `header-context` signal + a default column-visibility menu (`column-menu`) | **MISSING → OW5b (0.25)** |
| `saveState`, `restoreState` | `bind:width`, order and visibility through `uiState()` (DX1) | DX1 |
| header tooltip | `table-column tooltip` on the header | **MISSING → OW5b (0.05)** |
| multi-line or icon headers | `table-column icon`; a `header` snippet (Q3's answer applies) | **MISSING → OW5b (0.1)** (icon) |

#### 2.18.6 Styling (all item views)

| QSS | Ours | Status |
|---|---|---|
| `::item` (+ `:selected`, `:hover`, `:selected:!active` for an unfocused view, `:alternate`, `:focus`) | list and tree: the row node `::part(row)` (or the row's root element); table: `::part(row)`, `::part(cell)` (virtual); `:selected`, `:hover`, `:current`, `:focus-visible` on the cursor row, `:not(:focus-within)` on the view for the inactive selection (R6's four states) | ARCH (OW5a/b); the inactive selection via `:focus-within`: have |
| `:alternate` | `:nth-child(even)` by logical index (P1) | P1 |
| `QTreeView::branch` (+ `:has-children`, `:has-siblings`, `:adjoins-item`, `:open`, `:closed`) | `::part(twistie)` (P; `content: icon(chevron-right)`; `:expanded` rotates), `:has-children` (P5) on the row; `::part(guide)` indent guides (VS Code `tree.indentGuidesStroke`) instead of branch lines | twistie: ARCH (OW5a); P5; `guide`: **MISSING → OW5a (0.25)** |
| `show-decoration-selected` (the selection covers the branch area) | CSS: the row's `::part(row)` spans the indent (default, VS Code); `--sv-select-indent` to exclude it | CSS |
| `QListView::item` icon (`::icon`) | row content: plain CSS | have |
| `::indicator` (checkable items) | rows: a `checkbox` in the row; table: `::part(cell-check)` | have / MISSING (§2.18.4) |
| `QHeaderView::section` (+ `:first`, `:last`, `:middle`, `:only-one`, `:next-selected`, `:previous-selected`, `:selected`, `:checked`, `:horizontal`, `:vertical`) | `table::part(header-cell)` or per column `table-column::part(header)` (P6); positions by P1; `:selected` with cell selection; `:hover`, `:active` | ARCH (OW5b) + P1 + P6 |
| `QHeaderView::up-arrow`, `::down-arrow` (sort) | `::part(sort)` (P; `content: icon(arrow-up)` per `[sort="ascending"]`) | ARCH (OW5b) + P2 |
| resize handle between sections (no QSS) | `::part(resize)` (N; R11 look) | ARCH (OW5b) |
| `QTableCornerButton::section` | `table::part(corner)` | MISSING (counted in §2.18.4) |
| `gridline-color` | `::part(grid) { border-color }` | MISSING with `showGrid` |
| `selection-background-color`, `selection-color`, `alternate-background-color` (properties) | `--sv-list-active-selection-bg` etc. (tokens) and the rules above | ARCH (DS3) |
| `paint-alternating-row-colors-for-empty-area` | `::part(filler)` rows below the last row (P2) | P2 → OW-P2 (0.1) |
| drop indicator (no QSS) | `::part(drop-line)`, `:drop-target` (bit 18) on rows (R12) | ARCH (OW5a) |
| drag ghost | `::part(ghost)` (R12 lift: `elev-3`, 0.9 opacity) | ARCH (OW5a) |
| empty-state text | `slot="empty"` | MISSING (counted in §2.18.1) |
| sticky section header | `::part(section)` | MISSING with sections |

**Beyond Qt.**
- R6's four row states as CSS.
- `:state(cad.stale)` rows set in bulk by a `StateProvider` with no JS per row.
- Rows that are real Svelte content with no JS per scrolled row in the tree (WG5b).
- Frozen columns.
- 100k-row keyboard use.
- Indent guides.
- egui has no list, tree or table view (`egui_extras::TableBuilder` has `striped`, resizable columns and `sense`,
  with no keyboard, selection model, header state or editing).

### 2.19 QDateEdit / QCalendarWidget → `date-picker`; new `calendar`

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| `date` | `value` (ISO 8601) | have |
| `minimumDate`, `maximumDate`, `setDateRange` | `min`, `max` (days outside are `:disabled`) | **MISSING → OW6 (0.25)** |
| `displayFormat` | `format` (strftime) | have |
| `calendarPopup` | `date-picker` is always a popup; inline is the new `calendar` kind | **MISSING → OW6 (`calendar` kind 0.75)** |
| QDateEdit field editing (type into day, month, year sections; arrows step the current section; `currentSection`, `sectionCount`) | `editable`: the button becomes a sectioned field (Steppable per section, digits jump to the next) | **MISSING → OW6 (1.0)** |
| `firstDayOfWeek` | from IN1's locale; `first-day` override | **MISSING → OW6 (0.1)** |
| `gridVisible` | CSS on `::part(day)` borders | CSS |
| `horizontalHeaderFormat` (NoHorizontalHeader, SingleLetterDayNames, ShortDayNames, LongDayNames) | `weekday-format="none" \| "narrow" \| "short" \| "long"` (IN1 `DateTimeFormat`) | **MISSING → OW6 (0.1)** |
| `verticalHeaderFormat` ISOWeekNumbers | `calendar-week` | have |
| `navigationBarVisible` | `navigation={false}` | **MISSING → OW6 (0.05)** |
| `setDateTextFormat` (per-date formats: holidays, marked days) | `day-parts`: `{ "2026-10-22": "milestone" }` adds that part name to the day: `::part(day milestone)` (P10) | **MISSING → OW6 (0.25)** |
| `setWeekdayTextFormat` (weekends) | the widget adds the `weekend` part name to those days: `::part(day weekend)` (P10) | **MISSING → OW6 (0.1)** |
| `selectionMode` NoSelection | `read-only` | **MISSING → OW6 (0.05)** |
| Save/Cancel (our popup) vs immediate pick | `commit-mode="immediate" \| "confirm"` (Qt picks immediately) | **MISSING → OW6 (0.1)** |
| `dateChanged`, `editingFinished`, `activated`, `clicked`, `currentPageChanged`, `selectionChanged` | `input`, `commit` (`changed` today); `page-changed(year, month)` | ARCH (§2.4); `page-changed`: **MISSING → OW6 (0.05)** |
| QTimeEdit, QDateTimeEdit (`time`, `timeSpec`, `timeZone`) | `time-edit` kind; `date-picker time` (QW24) | P2 → OW-P2 (1.0) |
| calendar systems (`QCalendar`) | IN1 locale | P2 → OW-P2 (0.5) |

**Styling** (Qt styles the calendar through object names: `#qt_calendar_navigationbar`, `#qt_calendar_prevmonth`,
`#qt_calendar_nextmonth`, `#qt_calendar_monthbutton`, `#qt_calendar_yearbutton`, `#qt_calendar_calendarview`,
plus `QCalendarWidget QAbstractItemView:enabled` and `:disabled` for the day grid)

| Qt | Ours | Status |
|---|---|---|
| the button (QDateEdit box), `::drop-down`, `::down-arrow` | `date-picker`, `::part(button)`, `::part(chevron)` (`content: icon(calendar)`) | ARCH (OW6) + P2 |
| `#qt_calendar_navigationbar` | `::part(header)` | ARCH (OW6) |
| `#qt_calendar_prevmonth`, `#qt_calendar_nextmonth` | `::part(prev)`, `::part(next)` (N) | **MISSING → OW6 (0.05)** |
| `#qt_calendar_monthbutton`, `#qt_calendar_yearbutton` | `::part(month)`, `::part(year)` (N; open a month or year picker) | **MISSING → OW6 (0.25)** |
| the day grid and days (`:enabled`, `:disabled`, the selected day, today) | `::part(grid)`, `::part(day)` (N) with `:selected`, `:hover`, `:current`, `:disabled`; extra part names `today`, `weekend`, `other-month` (`::part(day today)`) | `day`: ARCH (OW6); the extra names **MISSING → OW6 (0.1)** |
| weekday header cells | `::part(weekday)` | **MISSING → OW6 (0.05)** |
| week-number cells | `::part(week-number)` | **MISSING → OW6 (0.05)** |
| sectioned field (QDateEdit) | `::part(section)` (N; `:focus-visible` on the current section) | MISSING with `editable` |

**Beyond Qt.**
- Multi-name parts (`::part(day today)`), which QSS cannot express without object names.
- Locale-correct names through IN1.
- An AccessKit grid with roving.
- `egui_extras::DatePickerButton` has `calendar_week`, `show_icon`, `format`, `highlight_weekends`,
  `start_end_years` and `combo_boxes`/`arrows` (**verify**); its look is egui's global one.

### 2.20 QColorDialog → `color-picker`; new `color-editor`

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| `currentColor`, `currentColorChanged` | `value`, `input` (`changed` today) | have |
| `colorSelected` (OK) | `commit` | ARCH (OW6, §2.4) |
| `ShowAlphaChannel` | `alpha` (`opaque`, `blend`, `blend-or-additive`) | have |
| `NoButtons` (live editing without OK/Cancel) | `commit-mode="live" \| "confirm"` | **MISSING → OW6 (0.1)** |
| `DontUseNativeDialog` | always ours (no native colour dialog on egui) | N/A |
| the dialog body inline (Qt has no separate widget; GTK `ColorChooserWidget` has) | `color-editor` kind (the popup's content, inline: in a property panel) | **MISSING → OW6 (0.5)** |
| `customColor(i)`, `setCustomColor` (16 user slots, persisted) | `custom-colors` (bindable list; `uiState()` persists it) | **MISSING → OW6 (0.25)** |
| `standardColor`, the basic colours grid | `swatches` (a palette: list of colours, or a theme token set) | **MISSING → OW6 (0.25)** |
| HSV, RGB and HTML fields (spin boxes and a hex line edit) | `fields="hex rgb hsv"` (`drag-value`s and a hex `text-edit` in the popup) | hex: ARCH (OW6); rgb/hsv: **MISSING → OW6 (0.25)** |
| "Pick Screen Color" (eyedropper across the screen) | eyedropper inside the app's viewports only (egui cannot read other windows' pixels, **verify**) | P2 → OW-P2 (0.5) |
| colour models beyond HSV (OKLCH: not in Qt) | `model="hsv" \| "oklch"` | P2 → OW-P2 (0.5) |

**Styling** (QColorDialog has no sub-controls; it is styled through its child widgets)

| Ours | Status |
|---|---|
| `::part(swatch)` (the closed button's colour), `::part(popup)`, `::part(square)` (saturation/value), `::part(hue)`, `::part(alpha)`, their `::part(handle)`s (`:dragging`, `:focus-visible`), `::part(swatches)`/`::part(swatch-cell)` (P1 positions), `::part(fields)` | ARCH (`swatch`, `popup`, `square`, `hue`, `alpha`); handles, swatch cells and fields **MISSING → OW6 (0.1)** |

**Beyond Qt.** A checkerboard under alpha via `background-image` (repeating gradients, §2.9's OW-CSS item), the
editor inline in panels, and AccessKit sliders on each channel. egui has `color_edit_button_hsva` and the
`color_picker` module (`Alpha`), with a single look.

### 2.21 QDialog / QDialogButtonBox (and QMessageBox) → `modal`, `inner-window`; new `dialog-buttons`

**Configuration**

| Qt | Ours | Status |
|---|---|---|
| `modal`, `open`, `exec`, `show` | `modal` (blocking backdrop) vs `inner-window` (non-modal); `open`; async by nature (no nested loop) | have |
| `accept`, `reject`, `done(r)`, `result`, `accepted`, `rejected`, `finished` | `close(result)` method; `closed` carries `result` (`"accept"`, `"reject"`, a custom string); `accepted`/`rejected` signals | **MISSING → OW7 (0.25)** |
| Escape rejects, Enter accepts the default button | `dismissable`; `cancel`/`default` buttons in the dialog scope (EV3) | have |
| `sizeGripEnabled` | `modal resizable` (Resizable edges and a `::part(grip)`) | **MISSING → OW7 (0.25)** |
| initial focus (`setFocus` in the dialog) | `autofocus` within the `dialog` focus scope | have (EV3) |
| the dialog's title bar (QDialog is a window) | `title`; `inner-window` adds drag, close, collapse | have |
| `windowFlags` (no close button, stays on top) | `inner-window closable`, layer order | have |
| QDialogButtonBox `standardButtons` (Ok, Cancel, Apply, Reset, Help, Save, Discard, Close, Yes, No, …) | `dialog-buttons` kind: `buttons="ok cancel apply"` (labels localised by the host), or explicit `button` children with `role` | **MISSING → OW7 (0.75)** |
| button roles (AcceptRole, RejectRole, DestructiveRole, ActionRole, HelpRole, YesRole, NoRole, ApplyRole, ResetRole) | `button role="accept" \| …` inside `dialog-buttons` | MISSING with the kind |
| platform button order (WinLayout, MacLayout, KdeLayout, GnomeLayout) | `Policy::button_order` (FM1 QW11) applied by `dialog-buttons` | ARCH (OW7) |
| `centerButtons`, `orientation` | CSS `justify-content`; `orientation` | CSS / MISSING with the kind |
| `clicked(button)`, `helpRequested` | `clicked` carries the role; `help` signal | MISSING with the kind |
| destructive actions (not distinct in Qt beyond the role) | `role="destructive"` → `button[role="destructive"]` styling (red), never the default | MISSING with the kind |
| QMessageBox (icons, detailed text, "don't ask again") | native `messageBox` (rfd, WG4b) or a cadatomic Svelte component (QW12) | have / cad |
| QWizard | cadatomic (QW22) | cad |

**Styling**

| QSS | Ours | Status |
|---|---|---|
| `QDialog` box | `modal::part(dialog)` or the modal node; `box-shadow` (R7 `elev-4`) | ARCH (OW7) |
| the backdrop (not stylable in Qt) | `modal::part(backdrop)` (dim colour, `backdrop-filter` later) | ARCH (OW7) |
| title | `::part(title)` | ARCH (OW7) |
| `dialogbuttonbox-buttons-have-icons` | `dialog-buttons button::part(icon) { display: none }` | CSS (with OW0's icon part) |
| inner-window title bar, close and collapse buttons, edges | `::part(titlebar)`, `::part(close)`, `::part(collapse)`, `::part(edge-*)` | ARCH (OW7) |
| active vs inactive window | `inner-window:focus-within` / `:backdrop` | ARCH |
| open and close animation | `:exiting` (bit 12) + transitions | have |

**Beyond Qt.**
- A styled backdrop.
- Open and close transitions with `:exiting`.
- `role="destructive"` as a styling and default-button rule.
- egui's `Modal` (backdrop colour, `frame`) and `Window` (`title_bar`, `collapsible`, `resizable`, `anchor`,
  `default_pos`, `constrain`) use a global `Visuals.window_*` look. There is no default or cancel button and no
  button box.

### 2.22 QStatusBar → new `status-bar` (the seam with cadatomic CH3 is for the coordinators, §6)

| Qt | Ours | Status |
|---|---|---|
| `showMessage(text, timeout)`, `clearMessage`, `currentMessage`, `messageChanged` | `show-message(text, timeout)`, `clear-message`, `message` (read-only), `message-changed` | **MISSING → OW7 (`status-bar` kind 0.75)** |
| `addWidget(w, stretch)` (temporary area, hidden by messages) | children (normal) | MISSING with the kind |
| `addPermanentWidget(w, stretch)` (right side, never hidden) | children with `slot="end"` | MISSING with the kind |
| status tips from hovered actions and widgets (QA5) | `status-tip` routed to the nearest `status-bar` of the viewport | **MISSING → OW7 (0.25)** |
| `sizeGripEnabled` | N/A (OS windows resize from their edges) | N/A |
| QSS `QStatusBar::item` (border around permanent widgets) | `status-bar > *`, `::part(message)` | MISSING with the kind |
| AccessKit `Role::Status`, announcements of messages (QA11) | `announce` politeness polite | MISSING with the kind |

**Beyond Qt.** Toasts (`toast-host`: corner, duration, levels, `dismiss`) cover KDE-style passive notifications,
which Qt Widgets does not have.

### 2.23 QDockWidget → berth

Docking, floating, title bars, tab groups, auto-hide and dock areas are berth's (QT-SURVEY QS1–QS2). QSS
equivalents for berth's chrome (`QDockWidget::title`, `::close-button`, `::float-button`, `:closable`,
`:floatable`, `:movable`) are berth's styling API, not svelters'. Only one item applies to svelters: when OW-RUST
lands, berth's Rust chrome can use the same units and the same CSS (ARCH-WIDGETS Q5). The decision about whether
berth adopts it is berth's and cadatomic's.

---

## 3. Beyond Qt (and egui), across all widgets

| Capability | Qt (QSS, QStyle) | egui 0.37 | svelters | Status |
|---|---|---|---|---|
| Design tokens | none in QSS (palette roles only; no `var()`) | `Visuals`/`Style` structs set in code | `var(--sv-*)` tokens, `@layer sv.base`/`sv.affordance`, one theme per root | have (CS0, CS5e) |
| Light, dark, high contrast, reduced motion | QPalette + QStyle; QSS has no media queries | dark and light `Visuals`, theme preference | `@media prefers-color-scheme` / `prefers-contrast` / `prefers-reduced-motion` | have |
| Transitions and animations | none in QSS (QPropertyAnimation in C++) | `animate_bool` with one global `animation_time` | `transition` (DS2 store behind `StyleView`), keyframes (§2.9), Svelte transitions (TR1/TR2), `:exiting` | `transition`: ARCH (OW-CSS); keyframes: MISSING (OW-CSS) |
| Selectors | type (C++ class), `.Class`, `#objectName`, `[property="v"]`, descendant and child; no siblings, no general `:not`, no `:has` | none | the full `selectors` crate: all combinators, `:is/:where/:not/:has`, structural, attributes reading props | have |
| States from the application | dynamic properties, which need `unpolish`/`polish` to restyle | none | `:state(ns.name)` provider bits set in bulk from Rust (`StateProvider`) and restyled in the same frame | have (SG4, CS5e) |
| Keyboard-only focus ring | no `:focus-visible` | focus ring on any focus | `:focus-visible` (bit 14), outline with offset (R8) | ARCH (OW0, OW-CSS) |
| Shadows | none in QSS (`QGraphicsDropShadowEffect` per widget, slow) | window and popup shadow only | `box-shadow`, outer and inset, on every box and part (R1, R2, R7) | have (CS5g; moves into the box primitive) |
| Borders and radii | per side and per corner | per-corner radius, uniform stroke | per side, per corner, dashed for high contrast | ARCH (OW0b) |
| Gradients | `qlineargradient`, `qradialgradient`, `qconicalgradient`, any stops | none (meshes in code) | `linear-gradient` 2-stop (ARCH), n-stop and radial (P4), repeating (§2.9) | ARCH + MISSING (OW-CSS) |
| Images in sub-controls | `image`, `border-image` | custom painter closures on a few widgets | `content: icon()` / `url()` / `paint()` on glyph parts (P2, §4), SVG at any DPI (ICR1) | MISSING (P2) |
| Cursor per part | `setCursor` in C++ | `on_hover_cursor` in code | `cursor` in CSS on hosts and parts (R10) | ARCH (OW-CSS, OW0b) |
| Behaviour on any element | subclass + event filters | the `Response` API | `use:` units (`use:pressable`, `use:steppable`, `use:keys`) | ARCH (OW0b on) |
| Composition | subclassing, Designer promotion | functions over `Ui` | Svelte composites over primitives and units; Rust kinds through the same macro as cadatomic's | ARCH |
| Live editing | `setStyleSheet` at run time; widgets need a rebuild; QML Hot Reload is commercial | none | CSS and Svelte hot reload with `$state` kept; the GUI edits its own sources (module loader) | have / in flight (HMR) |
| Responsive by container | none | none | `@container` (CSS.md: "a later item", needs laid-out size) | planned: proposed lane **CS-CQ** after OW-CSS (M, 1–2 d), first users toolbar density and one- vs two-column property rows |
| Measured look | none | none | VIS1 gate on painted shapes per part (PaintTrace) | ARCH (VIS1c) |
| Tooling | Qt Designer, GammaRay | egui inspection UI | MCP selector queries and hit tests per part; AccessKit per part | have / ARCH |
| Accent | `QPalette::Accent` (6.6) | `selection` colour | `accent-color` (§2.2) | MISSING (OW-CSS) |

egui's set, for the user's "more than egui" bar: egui 0.37 styles every widget from one `Style`/`Visuals`, with
four interaction states (inactive, hovered, active, open) plus `noninteractive`. Its spacing constants are
global. Per-instance changes are code (`Button::fill`, `Frame`, `RichText`). It has no pseudo-classes, parts,
selectors, tokens, transitions (except `animate_bool`), per-side borders or gradients. Its widgets lack default and
cancel buttons, editable combos, spin buttons, ticks, tabs, trees, list and table views, tooltips styled per
widget, and validation. Every row of §2 that reads "have" or "ARCH" is already beyond egui. The MISSING rows close
the distance to Qt.

---

## 4. The QStyle equivalent: overriding drawing beyond CSS

### 4.1 What Qt offers

- **QStyle / QProxyStyle**: `drawPrimitive` (PE_IndicatorCheckBox, PE_PanelButtonCommand, …), `drawControl`,
  `drawComplexControl` (CC_Slider with SC_SliderGroove, SC_SliderHandle), `subControlRect`, `pixelMetric`,
  `styleHint`, `sizeFromContents`, `standardIcon`. A QProxyStyle overrides one function and forwards the rest,
  which is how apps change one piece of one widget's drawing everywhere.
- **QStyleSheetStyle**: QSS, layered over the platform style.
- **QStyledItemDelegate**: per-view, per-column paint and editors.
- **Subclassing** with `paintEvent`.

### 4.2 What ARCH-WIDGETS gives, and what it lacks

| Need | ARCH-WIDGETS | Gap |
|---|---|---|
| Change the look (colours, sizes, borders, glyphs) | CSS on parts and states (W3) | the glyph swap (P2) |
| Draw something extra over a widget (badge, overlay, pick highlight) | `Behavior::paint_over` on any node (SG4) | none |
| A different widget with the same behaviour and look language | own kinds in the app's namespace, composed from the same units, primitives and parts (`#[svelters::widget]`) | the theme does not style `cad:slider` like a `slider` unless every rule is duplicated |
| Redraw **one part** of a built-in everywhere (QProxyStyle::drawComplexControl for SC_SliderHandle) | **nothing**: `replaces=` is crate-private (§6.2, *decided (design)*), and paint primitives read only CSS | **missing** |
| Change a behaviour hint (tooltip delay, drag distance, double-click time) | `native::Policy` rows per platform (W6) | no host override on the builder |
| Rust shell chrome in the same look | OW-RUST (Q5) | none beyond the above |
| Per-cell custom paint in tables | rows as nodes in lists and trees; `editor` snippets for table cells | table cell paint (Q3) |

### 4.3 Recommendation

1. **Part painters, selected in CSS.** This is the QProxyStyle equivalent. *Recommended; Q1.*

   ```rust
   // svelters-egui::paint (OW0b)
   pub trait PartPainter: Send + Sync + 'static {
       /// Paint `part` into `rect`. Called where the box primitive would paint the background image layer
       /// (`background-image: paint(..)`: after background-color and inset shadows, before the border), or
       /// instead of a glyph (`content: paint(..)`). Pure paint: no input, no layout, no scene writes.
       fn paint(&self, cx: &mut PaintCx, style: &StyleView, rect: egui::Rect, part: &PartInfo, args: &PaintArgs<'_>);
   }
   #[non_exhaustive]
   pub struct PartInfo { pub tag: Atom, pub part: Atom, pub state: StateBits, pub ppp: f32 }
   /// Widget-specific, documented per part: slider thumb { fraction, orientation }, progress fill
   /// { fraction, phase }, twistie { expanded }, header cell { sort, index, count }, …
   pub enum PaintArgs<'a> { None, Value { fraction: f32, orientation: Orientation }, Progress { fraction: Option<f32>, phase: f32 }, Expanded(bool), Custom(&'a dyn Any) }

   Embed::builder().painter("cad.knurl", KnurlPainter::default())
   ```

   ```css
   slider::part(thumb)                { background-image: paint(cad.knurl); --knurl-pitch: 2px; }
   tree-view::part(twistie)            { content: paint(cad.twistie); }
   slider:focus-visible::part(thumb)   { --knurl-color: var(--sv-focus); }      /* states and tokens still apply */
   ```

   Why `paint()`: it is the CSS Painting API's syntax (Houdini, `background-image: paint(name, args)`), so the
   choice of painter cascades, follows states, themes and media queries, and can be hot-reloaded. Custom
   properties pass arguments, and their transitions animate the painter. The MCP and VIS1 tools see the part as
   before (PaintTrace attributes the painter's shapes to `(node, part)`). An unknown painter name is a
   diagnostic and falls back to the default paint.

   Cost: OW-CSS parses `paint(<ident> [, <args>]*)` into the computed `Image::Paint` and `Content::Paint` (0.25 d);
   OW0b adds the registry and the two call sites in `paint_box`/`paint_icon` (0.5 d); each widget lane documents
   its parts' `PaintArgs`, which is a docs-only change. Painters are Rust, so extensions get them only with native
   code; that stays behind the deferred extension policy (ARCH-SECURITY), and the API leaves room for a principal.
2. **`style_as` for own kinds**: `#[svelters::widget(namespace = "cad", tag = "slider", style_as = "slider")]`. The
   kind also matches the built-in's type selector and part names, so the theme styles `cad:slider` as a slider and
   a cadatomic variant needs only its differences in CSS. **MISSING → OW0b (0.1)**. `replaces` stays crate-private:
   a global swap would change every extension's widget without its knowledge and would void the "same
   vocabulary" check. An app swaps by using its own tag.
3. **Policy overrides on the builder** (the `styleHint` equivalent): `Embed::builder().policy(|p| { p.tooltip_delay_ms
   = 400; })`, with `Policy` `#[non_exhaustive]`. **MISSING → OW0b (0.1)**.
4. **Item delegates**: lists and trees already render rows as scene nodes. Table cells are Q3.
5. **OW-RUST** uses the same painters, `style_as` and Policy, so cadatomic's Rust chrome and Svelte panels cannot
   drift.

Rejected:

- **A QStyle-like trait with one method per control and primitive.** It is a large surface frozen early, it
  duplicates CSS, and every widget lane would have to route through it.
- **Opening `replaces=` to apps.** See point 2.
- **Svelte-defined painters (canvas ops from JS per frame).** That puts JS in the paint path; the existing
  `canvas` element covers the cases where JS should draw.

---

## 5. Per-lane acceptance

Each lane's acceptance is ARCH-WIDGETS §6.1/§7.2's, **plus** the items below. An item is done when its prop,
signal, method, part or pseudo-class:

- exists under the name in this document;
- is in `metadata/semantics/egui.json` (props, signals and methods) or in the widget's docs page (parts);
- has a kittest (behaviour) or a VIS1/PaintTrace check (styling);
- passes the sweeps (`spec.rs`, `observe.rs`, `widgets.rs`, `scene/tests.rs`).

The day figures are agent-days added to the lane by parity, not the lane's total. Section numbers refer to §2.

| Lane | ARCH size | Parity items (§) | + days | New size |
|---|---|---|---|---|
| **OW0** (OW0b) | L | `button icon` with `::part(icon)` and `::part(label)`; toolButtonStyle through CSS layout of the parts; `press`/`release`; `repeat` (§2.1). Rules: P2 (icon primitive reads `content`), P3 (part layout), P8. §4: the `PartPainter` registry and call sites, `style_as`, Policy builder override | 2.4 | L+ (OW0b ≈ 3.5 d) |
| **OW-CSS** | L | P1 logical indices and structural pseudo-classes after `::part()`; P2 `content: icon()/url()`; P4 n-stop and radial gradients; P5 bit 23; P6 style host; P10 multi-name `::part()`; `paint()` values (§4); `accent-color`, `caret-color`, `tab-size`, repeating gradients, keyframe `animation` (§2.2, §2.4, §2.5, §2.9) | 2.8 | XL (split keyframes into a follow-up if the lane runs long) |
| **OW1a** | L | toggle-button `group` (§2.1); checkbox `indeterminate`/`tristate`, `icon`, `::part(label)` (§2.2); radio `icon`, label part (§2.3); progress `value/min/max`, `indeterminate`, `orientation`, `inverted`, `format`, `::part(label)` (§2.9); label `for` (buddy), selection props, inline rich text if Q4 is accepted (1 d) (§2.10); frame `checkable`, accordion, header `icon`, header actions (§2.16) | 3.9 (2.9 without Q4) | XL |
| **OW1b** | M | drag-value `special-value-text`, `accelerated`, `step-type`, `correction`, `read-only`, `group-separator`, IN1 wiring, `formatter`/`parser` registry, `step()`/`select-all`, limit `:disabled`, glyph parts (§2.6); slider `page-step`, `orientation`, ticks and `tick-interval`, `marks`, `inverted`, `track-click`, `scale="log"`, `::part(rest)` (§2.7) | 3.75 | L |
| **OW2** | L | CSS `overflow-x/y`, bar-mode hook, `scroll-margin`, corner slot, viewport slots, `overscroll-behavior`, standalone `scrollbar` kind, arrows, `track-click`, `thumb-x/y`, track parts, corner part (§2.11); split-view N panes, stretch, `collapsible`, restore on double-click, commit mode, per-pane min/max, `::part(grip)` (§2.17) | 3.65 | XL |
| **OW3** | L | button `shortcut`, menu-button `split` and `popup-mode="delayed"` (§2.1); combo option objects, `value`, `max-visible`, `size-to`, `popup`/`popdown`, `wheel`, `activate`, `highlight`, `::part(button)`, `::part(label)`, option part names (§2.8); action `icon`, chords and `std:` names, `group-optional`, `status-tip`, menu tooltips, `hover`, sections, collapsing separators, `default`, lazy `open`, scrolling menus, the submenu triangle, Alt to the menu bar, menu-bar corner slot, any element in a menu (§2.14); tooltip `show`/`hide` API, `tooltip-rect`, follow-on and `tooltip-delay`, `tooltip-duration`, `tooltip-placement`, the tooltip style host (§2.15) | 5.2 | XL |
| **OW3b** (new; after OW3 and OW4's field) | — | `toolbar` kind: orientation, Roving, `overflow` menu with `priority` (§2.13); editable `combo-box`: the field, `insert-policy`, `allow-duplicates`, prefix completion (§2.8) | 3.05 | L |
| **OW4** | L | `text-edit`: `placeholder`/`maxlength` aliases, `read-only`, `echo="none"`, `--sv-password-char`, `rejected`, selection props and signals, edit methods and `can-*` props, `modified`, the default context menu, `inputmode`, `::part(preedit)` (§2.4); `wrap`, `tab-focus`, `overwrite`, `undo={false}`, `can-copy` (§2.5) | 2.1 | L+ |
| **OW4b** (new; after OW4) | — | `input-mask`, `validator` registry (shared with §2.6 and the editable combo), the completer seam (caret-anchored popup), `clearable`, `slot="start"/"end"` adornments (§2.4); `highlights` (§2.5) | 3.0 | L |
| **OW5a** (lists, tree, DnD) | part of OW5 XL | `tree-view` added to the own-widget inventory; `selection="toggle"/"range"`, list/tree `bind:current`, in-row editing, list/table drag props and `internal-move`, the `scrollTo` hint, row click/hover signals, `slot="empty"`, `row-context`, `copy` everywhere (§2.18.1); grid layout, rubber band, sticky sections (§2.18.2); `root-decorated`, `expandable`/`expand-on`, expand animation, `uniform-rows`, tree helpers, **multi-column tree** (§2.18.3); `::part(guide)`, P5 (§2.18.6) | 7.05 | XL |
| **OW5b** (table, header) | part of OW5 XL | `selection-unit` (cells, columns), `edit-triggers`, column `elide`, `tab-navigation`, persistent editor columns (§2.18.1); grid lines, corner button, column `visible`, `fit-columns()`, cell tooltips, cell part names, checkable cells, cell icons (§2.18.4); column `resize` modes, `stretch-last`, min/max widths, movable columns, `header-double-clicked`, `column-resized`, `sort-priority`, `header-align`, the header context and column menu, header tooltips and icons (§2.18.5) | 5.15 | XL |
| **OW5c** (tabs) | part of OW5 XL | `tab icon`, `tab-tooltip`, `enabled`/`visible`, `tab-position`, `movable`, `elide`, `overflow`, `auto-hide`, `switch-on-drag`, `on-remove`, tab and strip slots, click signals, tab context menus, `::part(pane)`, `::part(strip)`, scroll buttons, P6 first user (§2.12) | 3.4 | L |
| **OW6** | M | date `min`/`max`, the `calendar` kind, sectioned editing, `first-day`, `weekday-format`, `navigation`, `day-parts`, weekend names, `read-only`, `commit-mode`, `page-changed`, `prev`/`next`/`month`/`year`/`weekday`/`week-number` parts, day part names (§2.19); colour `commit-mode`, the `color-editor` kind, `custom-colors`, `swatches`, rgb/hsv fields, handle and swatch parts (§2.20) | 4.75 | XL (split `calendar` + sectioned date editing into OW6b if run in parallel) |
| **OW7** | M | `autoDefault` per Policy (§2.1); dialog results and signals, `resizable` modal, the `dialog-buttons` kind with roles and platform order (§2.21); the `status-bar` kind and the status-tip feed (§2.22) | 2.5 | L |
| **OW-RUST** | M | the façade exposes parts, `style_as`, painters and Policy; the Rust button, toggle, menu-button and tab strip pass the same VIS1 checks as the Svelte kinds | 0 (scope only) | M |
| **VIS1c** | S | rule checks for every new part named here as its lane lands (PaintTrace lookups), and a "no size constant in `widgets/`" lint (P9) | 0.25 | S |

**Total:** about **53 agent-days** of parity work on top of ARCH-WIDGETS' lanes.

**Splits recommended to the coordinator**, so no lane exceeds about a week:

- OW3 → OW3 + **OW3b**;
- OW4 → OW4 + **OW4b**;
- OW5 → **OW5a / OW5b / OW5c**. OW5c can start as soon as OW2's Scrollable lands, in parallel with OW5a;
- optionally OW6 → OW6 + OW6b.

**OW-P2** (not scheduled; about 12 agent-days; listed so that "as much as Qt" has a complete ledger):

- mnemonics;
- `echo="password-on-edit"`, the reveal toggle, the password mask delay;
- `text-drag`, spell check;
- `base` (hex spin box);
- `inverted-controls`, the slider `step` signal;
- `text-indent`, `:visited`;
- `addScrollBarWidget`, the scroll bar context menu;
- `scroll-snap` rows;
- table cell word wrap, spans, the vertical header;
- the empty-area alternation (`filler`);
- `time-edit` / date-time, calendar systems;
- the screen eyedropper, OKLCH;
- action `repeat`;
- `border-image`;
- inner-window maximise and minimise.

---

## 6. Open questions

### For the user (architecture; at most four)

| # | Question | Recommendation |
|---|---|---|
| **Q1** | The QStyle equivalent: how does cadatomic (and later an extension with native code) redraw one part of a built-in widget everywhere, beyond what CSS can say? (a) **CSS-selected Rust part painters**: `background-image: paint(cad.knurl)` / `content: paint(...)`, registered on the builder, called with the part's computed style, rect, state and widget-specific `PaintArgs` (§4.3). (b) Open `replaces=` to apps (swap whole kinds globally). (c) A QStyle-like trait with one method per control. | **(a)**, plus `style_as` for own kinds and Policy overrides on the builder. It keeps CSS as the one place that decides the look (the painter choice cascades, follows states and themes, hot-reloads), costs about 1 d, and leaves `replaces` crate-private as designed. (b) changes extension widgets behind their back; (c) duplicates CSS and freezes a large API early. |
| **Q2** | Parity scope: is "as much as Qt" every Qt feature, or Qt's **IDE-relevant set** with the rest kept as a ledger? | **The IDE set now (§5, about 53 agent-days across the OW lanes), with the OW-P2 ledger (about 12 d) kept and scheduled after OW-DONE or on demand.** Excluded on purpose: QDial, tab shapes, tear-off menus, MDI and dock widgets (berth), `cursorMoveStyle` (no RTL). |
| **Q3** | Table cell rendering (Qt's `QStyledItemDelegate`): cells are strings painted by the table today. For custom cells (a colour swatch, a progress bar, a unit badge, a sparkline) choose: (a) **per-column Rust cell renderers**: a column names a provider kind (`renderer="cad:quantity"`) that paints each visible cell from the row data with no scene node per cell; (b) **cell snippets**: a Svelte snippet realised per visible cell (as the `editor` is per edit); (c) both. | **(c), with (a) the default for cadatomic's large tables** (no node or JS per cell, the 2k-row grid of ARCH-SCENE §10.2) and (b) for small tables and extensions (a pool of realised snippets, as `virtual-list` rows). Both read P6/P10 for styling. (a) is about 1 d in OW5b; (b) about 1.5 d. |
| **Q4** | Inline rich text: Qt's QLabel and QToolTip take HTML and Markdown. Should `label`, the string form of `tooltip`, toast text and read-only text support inline formatting, and how? (a) **inline child elements** (`<b>`, `<i>`, `<code>`, `<a href>`, `<span class>`) lowered by the compiler into one text layout job, each span styled by CSS; (b) a Markdown subset in a `markdown` prop; (c) no rich text (compose `box` + `label`s). | **(a)**, about 1 d in OW1a. It is styled by CSS like everything else, links are focusable with a `link` signal, and there is no HTML parser or untrusted-markup surface. (b) can be a later convenience over (a); (c) breaks wrapping across spans. |

### For the coordinators (not the user)

- **`toolbar` and `status-bar` ownership.** QT-SURVEY QS3/QS4 assigned them to cadatomic (CH1 command strip, CH3
  status sections). Recommendation: svelters ships the generic kinds (OW3b, OW7), and cadatomic builds CH1/CH3
  on them. svelters-agent decides for the svelters-side API; main-cadatomic decides whether CH1/CH3 adopt it.
- **The completer seam** (§2.4): cadatomic owns the completer (QW9). svelters' half is the caret-anchored popup
  and the key forwarding (OW4b).
- **Lane splits** (§5): OW3b, OW4b, OW5a/b/c, optionally OW6b.
- **ARCH-WIDGETS updates** to fold in after this document is accepted:
  - add `tree-view` to §6.1 (OW5a);
  - add P1–P10 to §1.3 and §2.2 (bit 23);
  - add §4.3's painters, `style_as` and the Policy builder override to OW0b;
  - add the new kinds (`toolbar`, `status-bar`, `dialog-buttons`, `scrollbar`, `calendar`, `color-editor`) to §6.1.
- **verify** items before their lane relies on them:
  - egui `ImePurpose`;
  - egui's `Checkbox::indeterminate` and `Slider` options;
  - the `DatePickerButton` options;
  - Qt's DelayedPopup delay and `maxVisibleItems` default;
  - the tooltip wake-up delay;
  - whether WG5a delivered QW17 sticky sections.
