# CSS: a Rust style engine and what each platform can support

Status: research, 2026-10-21. Not a plan of record. Branch `research/css`.

The question (user): how do we make component `<style>` look more like browser CSS? The MCP/e2e test framework needs
Rust-side selector logic anyway, so the same code could implement the CSS. Which CSS features can each platform
reasonably support (`:hover`, for example)?

Inputs: the X6 and ST1 milestones and notes, `docs/notes/styling-gtk-css-qss.md` (GTK 4.22 / Qt 6.11 probes),
`plans/research/EGUI.md` (D3, D5), the code on master `d110cb3d`, four survey passes (AppKit + UIKit, WinUI 3 +
Android, egui 0.37 + taffy 0.15, Rust CSS crates), plus new probes: GTK/Qt specificity (§3.7) and crate builds
(§2.1). Claims about a toolkit cite its docs or
a probe. Where a claim comes from memory and not a doc, it says so.

## 0. Summary

**Recommendation: option (c), a hybrid with one Rust engine as the selector authority.** It is a new crate,
`svelters-style`, linked into the host. The engine's jobs:

- parse every component sheet;
- mirror the retained widget tree the host already half-keeps;
- match all selectors there, including `:has()`, `:nth-child()`, attribute selectors, sibling combinators and
  `@media (min-width)`, plus the interaction states it learns from the backend;
- run the cascade, `var()` and inheritance;
- invalidate per checkpoint.

What it outputs depends on the backend:

| Backend | What the engine sends |
|---|---|
| GTK, Qt | **Rule classes.** Each rule becomes `.sv-r<N>` (padded to equal specificity, in cascade order) in the native sheet, and the engine puts the classes of matched rules on each widget through the existing `CLASSES` op. The native engine still does painting, the theme, native `:hover`/`:active` on the subject, transitions and (GTK) `var()` and inheritance. |
| AppKit, WinUI, iOS, Android | **Computed values through property setters.** Hover, focus and the other states are tracked by the backend and reported to the host. |
| egui | **Computed styles read in-process.** taffy `Style` for layout, our paint code for visuals, same-frame hover. |

The engine's matcher also serves the e2e/MCP driver: query by selector, `:has()`, text and role.

Why not the alternatives:

- **(a) Keep delegating to native engines, emulate elsewhere.** Selector semantics stay different on every backend.
  QSS can't do `+`, `~`, `:first-child` or `:not()`; GTK can't do attribute selectors or `:has()`. Tests would still
  need a Rust matcher.
- **(b) The Rust engine pushes computed values everywhere.** That means per-widget inline values on GTK, where any
  new value is a global provider reload (about 250 ms per 1000 widgets, X6 bench), or a per-widget QSS sheet on Qt
  (re-polish plus parent relayout). It loses native transitions and has every hover go through our event loop.

**Per-platform headline.**

| | Cheap | Hard | Not reasonable |
|---|---|---|---|
| **GTK 4** | Nearly all visual CSS is native (cascade, `var()` ≥ 4.16, `@media prefers-*` ≥ 4.20, transitions, keyframes, filter, transform). All states are readable as `GtkStateFlags`. Rule classes are local (0.1 ms). | Layout (no CSS layout at all: props or taffy in a custom container), text-align/overflow (widget props), `cursor` (a setter) | pseudo-elements other than real CSS nodes; `@container` beyond emulation |
| **Qt 6 Widgets** | color/font/border/padding/background incl. gradients via QSS, subject states (`:hover`, `:pressed`, `:checked`, `:focus`) | the box only paints once "customised" (native look lost); no inheritance; `var()` only for global tokens; transitions (none); per-element values are expensive | QSS animations, filter, transform, `box-shadow` |
| **AppKit** | states via tracking areas / KVO / target-action; alpha, hidden, enabled, fonts, alignment, truncation, dark mode and accessibility media; `NSAnimationContext` transitions | background/border/radius/padding/gradient need layer access or a backend `NSView` subclass; backgrounds on bezeled controls drop the native look; `:active` on buttons (modal tracking loop); attributed strings for line-height/letter-spacing | arbitrary transforms, `outline`, `::before/::after` |
| **WinUI 3** | nearly all box/visual/typography props are DPs; `FocusState.Keyboard` is a real `:focus-visible`; implicit `BrushTransition`/`ScalarTransition` | visual states override local brushes on hover/press (needs lightweight-styling resource keys); `box-shadow` (ThemeShadow is rectangular); `cursor` (protected member) | `filter`, general `outline`, `text-transform`, `::before/::after` |
| **egui** | everything we paint ourselves; hover/active/focus known at the start of the frame; all of flexbox and grid (taffy); cursor; text props | font weights (need variable or multiple fonts); per-side borders; radial gradients; transforms beyond translate+scale | `filter`, rotating subtrees |

**Open decisions for the user** are in §8. The biggest: ST1's rule that style property names follow the native API,
with no CSS aliases. "Look more like browser CSS" contradicts it, and the AppKit and egui tables already bend it
(`gap`, `flex-direction`, `opacity`).

## 1. Current state

### 1.1 The pipeline today

```text
App.svelte <style>
  │
  ├─ svelters-compile (Rust)  crates/svelters-compiler/src/style.rs   subset parser (styling-syntax.md grammar)
  │     setter / child-attr declarations: matched at compile time   lower/mod.rs match_selector()
  │     (compound type/.class/#id only; class: → conditions)        → $n.set / $n.child_attr / rule_set
  │     --css-scope svelte-<sha10> adds the scope class to every native element of the component
  │
  └─ Vite plugin (JS)  packages/native-runtime/vite.js nativeSheets()
        @native blocks lifted out (extractNative)
        stock svelte@5.57.2 compile(css:'external', cssHash) on a bindings-blanked copy  → scoped + pruned CSS
        @svelters/style translateSheet(css, toolkit, {setters:false})   packages/style/src/translate.js
           GTK CSS  or  QSS + dark QSS (tokens as ${--sv-x} templates)  + diagnostics
        → registerStyles(file, {gtk} | {qt, qtDark})  in the bundle

runtime (QuickJS)  packages/native-runtime/src/style.js
   host.style(TARGET)  → 'css' | 'qss' | none
   SHEET per component, @tokens provider (GTK) / rendered templates (Qt), SCHEME + palette
   per element: CLASSES (diffed, plus sv-k-<tag> markers), NAME (#id), INLINE (parsed, translated), OPACITY
   dynamic style={...} setter declarations → setProp with getProp capture / restore

backend (ABI v2.3 style member)   GTK: crates/svelters-gtk/src/backend/style.rs   Qt: templates/backend.cpp.in
   GTK: one CssProvider per sheet (APPLICATION), tokens at APPLICATION-10, inline deduped into sv-s<n> classes
   Qt:  sheets concatenated into qApp->setStyleSheet once per loop turn; class property "<QClass> a b" + repolish
   AppKit, WinUI: style member NULL ("styling is not supported by this backend")
   egui: style answers SVL_STYLE_TARGET_NONE, every other op is a no-op
```

### 1.2 Inventory: what works where, and who decides

"Decided" columns: **C** = compile time in the Rust compiler, **B** = build time in the JS translator (Vite), **R** =
runtime in the JS runtime, **N** = the native style engine at run time.

| Feature | GTK 4 | Qt 6 | AppKit | WinUI 3 | egui |
|---|---|---|---|---|---|
| Dialect (`TARGET`) | `css` | `qss` | none (no `style` member) | none (no `style` member) | `none` |
| CSS properties (`via: css`) | 63: color, background*, border*, radius*, padding*, margin/-left/-right, font-family/size/weight/style, opacity, min-width/height, text-decoration, letter-spacing, box-shadow, text-shadow, transition*, animation* (B → N) | 46: the same minus shadows, transitions and animations; `background` colour only; opacity inline only (B → N) | – | – | – |
| Setter properties (`via: setter` / `child-attr`) | 23 + 1: width/height-request, h/vexpand, h/valign, margin-start/end/top/bottom, sensitive, orientation, spacing, homogeneous, grid spacings, xalign, yalign, justify, wrap (C) | 9 + 3: minimum/maximum-width/height, enabled, direction, spacing, contents-margins; stretch, alignment (C) | 7: opacity→alpha-value, visibility-hidden, enabled, gap, flex-direction, padding (stack view), text-align (C) | 14 + 4: width, height, min-*, margin, padding, opacity, visibility, alignments, font-size, spacings; Grid row/column/spans (C) | the layout set (D3): width/height, min/max, flex grow/shrink/basis, align-self, margin, flex-direction/wrap, gap, padding, align-items, justify-content; one number per side, points only (C) |
| Type selector | our tag → CSS name; `sv-k-<tag>` marker for 21 ambiguous tags (B, N) | `.QClass` or `.sv-k-<tag>` (B, N) | setter rules only (C) | setter rules only (C) | setter rules only (C) |
| `.class`, `#id` | native classes (diffed `add/remove_css_class`), widget name (R, N) | `class` dynamic property `"<QClass> a b"` + repolish; objectName (R, N) | setter rules; static or `class:` conditions (C) | same (C) | same (C) |
| `*`, descendant, `>` | yes (B, N) | yes (B, N) | no (setter rules take one compound) | no | no |
| `+`, `~` | yes (B, N) | error: QSS parses but never matches (B) | no | no | no |
| `:where()`/`:is()` | one compound flattened (Svelte's own `:where(.svelte-h)`), lists are errors (B) | same (B) | no | no | no |
| `:not()` | simple compound of classes/states (B, N) | `:not(:state)` → `:!state` only (B, N) | no | no | no |
| `:has()`, attribute selectors, `:nth-child()`, pseudo-elements | errors (B); GTK supports `:nth-child` natively but the translator's table lacks it | errors (B); QSS supports `[prop="v"]` natively but the translator refuses it | no | no | no |
| State pseudo-classes | hover, active, focus, focus-visible, focus-within, disabled, checked, selected, backdrop, first/last/only-child (B → N) | hover, active→pressed, focus, focus-visible→focus (warning), disabled, checked, selected (B → N) | none | none | none |
| `@media` | `prefers-color-scheme` / `-contrast` / `-reduced-motion` passed through; the host binds the provider property (B → N, GTK ≥ 4.20) | `(prefers-color-scheme: dark)` split into a dark sheet appended by JS (B → R) | – | – | – |
| `@keyframes`, `transition` | passed through (N) | dropped with a warning (B) | – | – | – |
| `var()` / tokens | native `var()` (GTK ≥ 4.16, FG1); `:root` token provider (R → N) | `${--sv-x}` templates rendered with the active token set; global only (R) | – | – | – |
| `!important`, `@layer`, `@supports`, `@container`, nesting | error / error / error / error / flattened by the translator (B) | same (B) | – | – | – |
| `style=` / `style:` | parsed by JS; CSS part deduped into `sv-s<n>` classes; setters → setProp (R) | per-widget `.QClass { … }` sheet; setters → setProp (R) | setters (C static, R dynamic) | same | same |
| Dark scheme | `setColorScheme` → token provider + per-provider `prefers-color-scheme` (R → N) | dark token set + `QPalette` + dark sheet, full re-polish (R → N) | – | – | – |
| `@native gtk|qt {}` | verbatim, unscoped (B) | verbatim, unscoped (B) | – | – | – |

Precedence today (FX1): explicit attribute > `style=` / `style:` > rules (specificity, then source order) > the
native value, which is captured with `getProp` before a rule first overrides it and written back when no rule sets
it any more.

### 1.3 Findings from reading the code

1. **There are three selector implementations, each with its own subset.**
   - **The compiler** (`crates/svelters-compiler/src/lower/mod.rs:1488`): it matches setter rules against the static
     template, with descendant and child combinators within one component. Classes from `class:` directives become
     runtime conditions. It does no states, no siblings, and nothing across component boundaries.
   - **The JS translator** (`packages/style/src/translate.js`): it rewrites selectors per dialect.
   - **The native engines.**

   A rule can therefore mean different things depending on whether its declarations are setters or CSS. With
   `.card > .title { spacing: 4 }`, the setter half is matched at compile time inside one component, while
   `.card > .title { color: red }` is matched by GTK at run time across components.
2. **The translator refuses things the native engine can do.**
   - GTK supports `:nth-child`, `:nth-last-child`, `:not(<compound>)`, `:dir()` and `:root`. `vocab.js` doesn't
     list them, so they are errors.
   - QSS supports attribute selectors on dynamic and Q_PROPERTY values. The translator errors on them for
     portability.

   A Rust engine makes "portable" mean "works everywhere" instead of "the intersection".
3. **AppKit and WinUI never see classes, ids, sheets or tokens.** Their `style` vtable member is NULL
   (`svelters-abi/src/safe.rs:1139` returns "styling is not supported by this backend"). Only compile-time setter
   rules reach them. MATRIX.md's styling row (🟡 "style-table setters only, no stylesheet") is accurate.

   There is also a probable bug, not run. The runtime's `targetOf()` (`native-runtime/src/style.js`) treats only
   a *missing* `host.style` function as "no styling". But `host.style` always exists, and on these backends the
   `TARGET` op throws. An AppKit or WinUI component with `class="…"` would then throw from `applyStyle` instead of
   printing the one-time warning. No AppKit or WinUI example uses `class` today. egui is unaffected because it
   answers `TARGET_NONE`.
4. **The host already keeps half a tree.**
   - `CoreState` (`crates/svelters-host/src/core.rs:270`) has `handles: handle → kind id`,
     `tree: child → parent` (filled by `op_insert`, cleared by `op_remove`, used by reload teardown) and the
     `toplevels` set.
   - It has no child order, classes, id, attributes or state.
   - The JS runtime's `Native` (`packages/native-runtime/src/index.js:43`) has `np` (the native parent), `attrs`
     (class/id/style), and `last` (props written).
   - Neither is a complete DOM-like tree. The inspector research (`docs/notes/inspector-demo.md` §1) found the same.
5. **Svelte's CSS parser is not ported.**
   - `crates/svelters-compiler/src/svelte/parse/read/style.rs` captures `<style>` raw and leaves parsing to the
     subset parser.
   - `plans/research/QUICKJS-RUNTIME.md` §5 says the compiler "already has the parser port"; that is not true on
     master.
   - Q3 (CSS in Rust: Svelte's analyze/prune/scope and the translator) is therefore bigger than the 4–6 days it is
     sized at. A Rust CSS parser that this work needs anyway shrinks it.
6. **The style tables already drift toward CSS names.**
   - The ST1 rule (2026-10-17) is "property names follow the native API … no CSS-standard aliases".
   - `metadata/style/appkit.yaml` maps `opacity` → `alpha-value`, `gap` → `spacing` and `flex-direction` →
     `orientation`.
   - The egui table is taffy's CSS names.
   - WinUI uses `margin`, `padding`, `width` and `font-size`, which happen to be both.

   Browser-like CSS needs this settled (§8 D1).
7. **Conformance `dom-css`** (on `ci/cf1`, `experiments/native-compiler/conformance/expected.json`): three samples
   are expected rejections.
   - The samples are `component-css-props-falsy`, `dynamic-component-css-props` and `svelte-meta-css-wrapper`.
   - The reason recorded is "DOM CSS output: the scoping hash class on elements and the `<svelte-css-wrapper>` element
     for `--css-props`".
   - The `--css-props` feature, `<Comp --color="red">`, sets custom properties on an implicit wrapper. A Rust engine
     with custom properties can support it **without** a wrapper element: the engine attaches the declarations to
     the component's root nodes in its tree. The DOM-shape assertions in those samples stay rejected.

## 2. The engine

### 2.1 Building blocks

There are two probe builds for this note: `cssparser` + `selectors` with a hand-written element, and `lightningcss`
alone. Both used `cargo-slot`, 4 jobs and a clean target, on a loaded machine (load average ~55), so the
wall-clock times are inflated. The dependency counts and sizes are exact. `stylo` was not built; its numbers are
from crates.io and its sources.

| Crate | Version (date) | License | Gives | We'd still write | Cost | Verdict |
|---|---|---|---|---|---|---|
| `cssparser` | 0.38.0 (2026-09-07) | MPL-2.0 | the spec tokenizer; rule/declaration/`!important`/nested-block parsers; `ToCss` | rules, our property values, `@layer` | ~10 deps | **use** |
| `selectors` | 0.41.0 (2026-09-30) | MPL-2.0 | selector parse + match through an `Element` trait; specificity; `:is/:where/:not/:has`, `:nth-child(… of S)`; nth and `:has` caches; bloom filter; `ElementSelectorFlags`; `SelectorVisitor` | `SelectorImpl`, the `Element` impl (~30 lines in the probe), rule index, cascade, invalidation | with cssparser: 27 deps; clean release build 2 min 7 s under load; **+142 KB** stripped over hello-world | **use, pinned exactly** |
| `stylo` | 0.22.0 (14 releases in 2026) | MPL-2.0 | the whole browser cascade, computed values, `var()`/`calc()`, restyle hints, snapshot invalidation | ~1.5k lines of glue (Blitz's `stylo.rs`: `TDocument`/`TNode`/an 84-method `TElement`); per-node `ElementData` | 118 deps, ~137k lines + Mako templates, **python3 at build time** | no: the browser's property set, not our backends' |
| `lightningcss` | 1.0.0-alpha.72 | MPL-2.0 | typed parse/minify/transform; typed colours, lengths, `calc()`, custom properties | matching and cascade (none; its `parcel_selectors` fork has `:has` as `unreachable!()`) | 47 deps; **15 min 48 s** clean build under load (730 s in the one crate); **+3.97 MB** stripped; pulls a second cssparser (0.37) | no; read its `values/` code for reference |
| `cssparser-color` | 0.6.0 | MPL-2.0 | CSS Color 4 on cssparser tokens (relative colours, `color-mix`) | – | tiny | use |
| `simplecss` | 0.2.2 | MIT/Apache | CSS2 parse + a 5-method matcher | everything from `:has` to `var()` | 1 dep | too limited |
| `swc_css_parser`, `raffia` | 26.0 / 0.13 | Apache / MIT | full-fidelity ASTs | matching, cascade, values | swc_common stack | not needed |
| taffy 0.15 style types | (in tree) | MIT | the layout target type (`stylo_taffy` shows a mapping) | the mapping | 0 | target |

The probe matched three selectors against a three-node arena tree, with hover from our own state bits in
`match_non_ts_pseudo_class`:

- `.a.svelte-xyz > button:where(.svelte-xyz):hover` matched the button at specificity (0,3,1), which is Svelte's
  scoped shape;
- `box:has(> button:hover)` matched the box at (0,1,2);
- `label:nth-child(2):is(label):not(.q)` matched the label at (0,2,2).

The `Element` trait asks for tree navigation (parent, siblings, first child), `has_local_name`, `has_id`,
`has_class`, `attr_matches`, `match_non_ts_pseudo_class`, `is_empty` and `is_root`. It also asks for a few
shadow-DOM, slot and part hooks, which we answer with "no", plus `add_element_unique_hashes` for the bloom filter and
`apply_selector_flags` for invalidation bookkeeping. Every one of them maps onto the `StyleTree` of §2.2.

**Churn.** `selectors` had about 14 breaking 0.x releases in 16 months. The 0.38/0.41 bump alone removed
`ParserInput` and the `location` in `ParseError`, so diagnostics must capture `current_source_location()`
themselves. Pin the version, keep our own AST in front of it, and plan a small migration per bump. The compiler
already does this with its acorn port.

**Licence.** The workspace is MIT, and MPL-2.0 is file-level copyleft. Depending on these crates is fine, and they
go into `THIRD_PARTY_NOTICES.md`. Copying lightningcss or Stylo source into our files would make those files MPL,
so read them, don't copy them. bevy_flair's `calc/` is MIT.

**Prior art.**

- **bevy_flair 0.9** (2026-10-14, MIT/Apache) is the closest design: `cssparser` + `selectors` over a non-DOM tree
  (the Bevy ECS).
  - It supports `:has/:is/:where/:not`, nesting, `@layer`, `@media`, `@keyframes`, `var()` and `calc()`.
  - Hover is a `Hovered` component copied into a per-entity state field.
  - Invalidation is recorded while matching: visiting an element as parent, sibling or first child ORs a
    "recalculate descendants / siblings / ancestors" bit into it. A change then re-matches the element plus the
    relatives its bits name.
  - It is coarse, simple, and has no bloom filter.
- **Blitz** (Dioxus Native) uses all of Stylo + taffy 0.15 + parley. It records a hover change as a snapshot and
  lets Stylo diff it against its invalidation map.
- **GTK's own engine** is the same shape as our plan: a parallel `GtkCssNode` tree with name, classes and state
  flags, invalidated through `GtkCssChange` bitmasks.
- **Freya, Kas, Xilem, Iced and Slint** have no CSS.

**Recommendation (D5).**

- Use `cssparser` 0.38 + `selectors` 0.41 + `cssparser-color`, pinned.
- Write our own sheet/rule parser with spans, rule index, cascade, inheritance, `var()` (cycles become "guaranteed
  invalid") and computed values for the style-table property set. That is a few thousand lines.
- Invalidation:
  - v1 is bevy_flair's recorded relation bits via `apply_selector_flags`;
  - v2 is a dependency map built with `SelectorVisitor` (class/id/attr/state → selectors), modelled on Stylo's
    invalidation map without depending on it.
- The compiler links the same crate for build-time parsing and diagnostics, and later for Q3's scoping port.

### 2.2 Where the tree lives

The engine lives in the host (`crates/svelters-host`), next to `CoreState`. It sits there, not in JS and not in each
backend, for four reasons:

- **Every structural op already passes through the host core.** That covers `op_create`, `op_insert` (with
  `before`), `op_remove`, `op_destroy`, `op_style` and `op_set_prop`.
- **Hover needs no JS.** A pointer moving over 1000 rows must not enter QuickJS, run a job drain or allocate JS
  objects. The host can restyle between checkpoints using only Rust.
- **One implementation for five backends,** and for the log backend. Op-log fixtures then cover styling on every
  backend.
- **The test driver and MCP live in Rust** (the CLI is Rust per CL0; egui_mcp is Rust).

The mirror, `StyleTree`, owned by the core:

```text
StyleNode {
  kind: svl_id, tag: Atom                 // from op_create
  parent, prev_sibling, next_sibling,     // from op_insert(parent, child, before, slot) / op_remove
  first_child, last_child                 //   (the slot name is kept; a node in a named slot is still a child)
  classes: SmallVec<Atom>                 // user classes, from style(CLASSES) (now intercepted, see 2.7)
  id: Option<Atom>                        // from style(NAME)
  attrs: SmallMap<Atom, Value>            // only the props some attribute selector names (sheet-driven set)
  state: StateFlags                       // hover active focus focus-visible focus-within checked disabled ...
  inline: Option<Arc<DeclBlock>>          // style= / style:, parsed in Rust
  scope_vars: Option<Arc<DeclBlock>>      // --css-props of a component instance
  computed: Option<Arc<ComputedStyle>>    // setter backends and egui; GTK/Qt keep only matched rule ids
  matched: SmallVec<RuleId>, dirty bits
}
```

Ordering comes from `before` in `op_insert`. The runtime sends `before` today (`nextNative`), so no ABI change is
needed. Nodes the toolkit creates on its own, such as GTK's internal `text` node, a QComboBox popup or WinUI template
parts, are not in the mirror. That matches what the user can name in Svelte.

**Attributes.** A selector like `[placeholder-text]` or `[value="3"]` names a prop. The engine registers that name.
`op_set_prop` records the value for that name only, so other props pay nothing. Props the toolkit changes on its own
come from R1 observation, so the engine asks the core to `observe` the attribute props it uses where the fact is
`exact`. For props that are not observable, an attribute selector is a compile-time diagnostic (the R2 machinery
already classifies them).

### 2.3 State: how `:hover` reaches the host

Add a backend member **`state(h, mask)` (ABI v2.8, additive)**. It turns tracking on for the given state bits and
returns the current flags. The backend reports changes through a new host callback, `state_changed(h)`. That
callback inserts into a dirty set, the same shape as R1's `prop_changed`, wakes on the first insert, and is read at
the next checkpoint. Nothing is delivered to JS unless a `bind:`/`observe` asks for it.

| Bit | GTK | Qt | AppKit | WinUI | egui |
|---|---|---|---|---|---|
| hover | `GTK_STATE_FLAG_PRELIGHT`, `state-flags-changed` | `QEvent::Enter/Leave` (X8 event filter already exists) or `underMouse()` | `NSTrackingArea` with an NSObject owner, `.inVisibleRect` | `ButtonBase.IsPointerOver` DP; else `PointerEntered/Exited` | `ctx.read_response(id).hovered()` before the pass |
| active | `ACTIVE` | `QAbstractButton::isDown()` / mouse press | `isHighlighted` (polled; modal tracking loop) | `IsPressed`; else `AddHandler(PointerPressed, handledEventsToo)` | `is_pointer_button_down_on` |
| focus | `FOCUSED` | `QApplication::focusChanged` | KVO `NSWindow.firstResponder` (map field editor → field) | `GotFocus/LostFocus`, `FocusState` | `has_focus` |
| focus-visible | `FOCUS_VISIBLE` | `QFocusEvent::reason()` Tab/Backtab/Shortcut | heuristic: `NSApp.currentEvent.type` at the focus change | `FocusState.Keyboard` | focus on a non-text widget (only keyboard focuses them) |
| focus-within | `FOCUS_WITHIN` | ancestor walk in the engine | ancestor walk | ancestor walk | ancestor walk |
| checked / indeterminate | `CHECKED` / `INCONSISTENT` | `isChecked()` + `toggled` | `NSButton.state` (target-action, KVO) | `IsChecked` + Checked/Unchecked/Indeterminate | node prop |
| disabled | `INSENSITIVE` | `isEnabled()` + `QEvent::EnabledChange` | `isEnabled` | `IsEnabled` + `IsEnabledChanged` | node prop |
| selected, backdrop | `SELECTED`, `BACKDROP` | `QStyle::State_Selected`; `isActiveWindow()` | – ; `NSWindow.isKeyWindow` | – ; window activated | – |

`:focus-within`, `:placeholder-shown` (placeholder prop and empty text prop), `:empty`, `:first-child` and their
relatives are computed by the engine from the mirror and need nothing from the backend. `:invalid` has no native
model on any desktop toolkit (WinUI's input validation was withdrawn from 1.5 stable,
microsoft-ui-xaml discussion #9572), so it needs our own constraint props (`required`, `pattern`, `min`, `max`) in
the runtime. That is a later item.

The backend turns tracking on only for the bits the registered sheets use on that node's possible subjects. The
engine computes, per tag, the state bits any selector could test, so a sheet with no `:hover` costs nothing.

### 2.4 Matching, cascade, values

1. **Rule index.** Bucket rules by their rightmost compound's id, then class, then tag (Stylo's `SelectorMap`), so
   a node only tries rules that could match. Svelte-scoped sheets put the scope class on every compound, so the class
   bucket is selective.
2. **Matching**, right to left with a bloom filter of ancestor tags, classes and ids. `:has()` runs a subtree walk,
   cached per checkpoint. `:nth-child` uses a per-parent index cache.
3. **Cascade.** The order is origin (theme tokens < component sheets < `@native` < inline), then `!important`,
   `@layer`, specificity and source order.
   - Each property in the backend's table has one `via` per backend, so the cascade for `via: css` properties on
     GTK/Qt is the native engine's (§2.7) and the engine never decides a value there.
   - For every other property the engine computes the value.
4. **Inheritance and `var()`.**
   - Custom properties are computed per node in the engine, inheriting like CSS (`--x` declared on `.card` reaches
     its descendants).
   - `var()` in a setter value resolves against the node's computed custom properties, which is new: today tokens
     are global on Qt.
   - Inherited standard properties (color, font-*, letter-spacing, line-height, text-align, visibility, cursor) are
     inherited by the engine on the setter backends. On WinUI only the leaves that render text receive them;
     panels have no Foreground.
5. **Computed values.**
   - Lengths (`px`, `pt`, `em`, `rem`, `%` where the target accepts it) and `calc()` with known bases resolve to the
     table's `type`.
   - Colours resolve to RGBA, using the `cssparser` colour parser with `color-mix()` and relative colours.
   - Keywords resolve to the target enum through a value map in the style table. That map exists today only as a
     note (`flex-direction: row -> horizontal`, "value map not yet written").
6. **Media.** The engine evaluates every `@media` itself:
   - `prefers-color-scheme` from `setColorScheme` or the platform signal;
   - `prefers-reduced-motion` and `prefers-contrast` from platform settings (§3);
   - `width`/`height` from the toplevel's size.

   A media change re-matches the rules inside that block. `@container` is the same thing with a node's laid-out size,
   which only egui and the taffy-laid-out containers know before paint. It's a later item.

### 2.5 Invalidation and the checkpoint

Invalidation follows Stylo's model, cut down. When the sheets change, the engine builds dependency maps:
`class → selectors`, `id → selectors`, `attr name → selectors`, `state bit → selectors`, and for each selector
whether the dependency sits in the subject, an ancestor compound, a sibling compound, or inside `:has()`.

When something changes, the engine marks:

- the node itself when the dependency is in the subject;
- its descendants when it is in an ancestor compound;
- its following siblings and their subtrees for `+`/`~`;
- its ancestors for `:has()`.

`!important` and media changes mark the nodes the affected rules matched last time.

The restyle runs once per checkpoint, as a new **step 5, after the job drain**. That way all of Svelte's
`class`/`style`/insert ops from the turn are applied before matching, as a browser recalcs style before paint:

```text
checkpoint: 0 changed batch → 1 events → 2 timers → 3 drain → 4 wake/arm → 5 restyle dirty nodes, push diffs
state_changed(h) between checkpoints → dirty set → wake → checkpoint with no JS work: step 5 only
```

A hover change then costs:

- one `state_changed`;
- one checkpoint that runs no JS;
- matching a handful of rules on one node, which is microseconds;
- one diffed push: a GTK class add/remove at 0.1 ms local (X6 bench), a few DP writes on WinUI, a few setters on
  AppKit, or nothing extra on egui, where the restyle happens inside the frame.

`state_changed` must be callable from inside the toolkit's event dispatch. Like `prop_changed` today, it only
inserts into the dirty set and wakes; it never calls into JS or back into the backend.

On GTK and Qt, subject-compound states with a native pseudo-class never reach this path at all (§2.7), so the common
`button:hover` is still handled by GTK/Qt in the same frame.

### 2.6 Scoping

Nothing changes in the scoping model:

- Svelte, or the Q3 port later, scopes and prunes at build time.
- The scope class rides in `class`, added to every native element by `--css-scope`.
- `:global()` is plain CSS after scoping.

The engine sees an ordinary sheet and ordinary classes. Two consequences:

- **Svelte prunes selectors it thinks can't match the template.** It knows only HTML semantics, so it may keep or
  drop selectors differently from what our matcher would do. That is already true today and is reported as
  `css_unused_selector`.
- **Cross-component selectors work naturally.** Examples are `:global(.dark) .card` and a parent's
  `.list > :global(*)`, because the mirror spans the whole app. The compile-time matcher could never do this.

The compile-time setter matching in `lower/mod.rs` retires. The compiler keeps parsing and validating `<style>` for
diagnostics, using the same `svelters-style` parser (§2.1), so build and run time agree on what a selector means. It
emits sheets the host registers (`registerStyles`), and setter declarations stop being lowered to `$n.set`. An
optimisation is possible later: precompute the matches of fully static elements at build time as an initial style,
the "static template op" idea in EGUI.md §7. It is only worth doing if measurements show first-paint cost.

### 2.7 Outputs per backend

**GTK and Qt: rule classes, native cascade.** For each rule (selector × block), the engine splits the selector:

1. **The native residue** is the subject compound's state pseudo-classes that the toolkit supports natively:
   - GTK: `:hover`, `:active`, `:focus`, `:focus-visible`, `:focus-within`, `:checked`, `:disabled`, `:selected`,
     `:backdrop`;
   - Qt: `:hover`, `:pressed`, `:focus`, `:checked`, `:disabled`, `:selected`.

   Keeping them native keeps hover same-frame, keeps GTK's transitions triggered by GTK's own state change, and
   avoids a Qt re-polish per hover.
2. **Everything else** is matched by the engine, which puts `sv-r<N>` on every node it matches.

The generated native sheet holds one rule per original rule, with the original declarations:
`.sv-r12.sv-r12.sv-r12:hover { background-color: var(--sv-accent-hover); }`. The class is repeated to pad every
rule to the same native specificity. The rules are emitted in the engine's cascade order: specificity, then source
order, with `!important` and `@layer` folded into the order. Native ties are then resolved by source order, which is
our order.

**Probe (§3.7): GTK 4.24.1 and Qt 6.12.0 both count repeated classes in specificity**, so the padding works and the
toolkit's cascade reproduces ours exactly. Declarations stay native, so `var()`, inheritance (GTK), `transition` and
`@keyframes` (GTK) keep working. Theme rules sit below, at THEME priority, and `@native` blocks keep their own
provider.

What this removes:

- the translator's selector rewriting, `sv-k-<tag>` markers, `:where` flattening and the per-dialect pseudo-class
  maps;
- the reasons `+`, `~`, `:not()`, `:has()`, `:nth-child()` and attribute selectors fail on one toolkit or the other.

What stays per toolkit:

- **Declaration translation** (the `decls.js` value checks, the QSS border trap, `background-image: none` for
  Adwaita).
- **Qt's dark sheet becomes unnecessary.** The engine evaluates `prefers-color-scheme`, and a scheme switch flips
  rule classes on the affected nodes instead of re-setting the app sheet. Re-setting the app sheet is a ~1 s
  re-polish per 1000 widgets.

Qt keeps its `var()` limit: QSS has no variables, so a `var()` in a `via: css` declaration on Qt resolves only
against the global token set. That is today's behaviour, and it is diagnosed if the variable is declared anywhere
but `:root`/tokens. Without that limit, we'd need per-node rule variants in the app sheet, each one a full
re-polish.

The `CLASSES` op carries `user classes ∪ rule classes`. The host now composes it instead of forwarding the runtime's
string, which removes JS `applyStyle`'s class diffing. GTK's backend already diffs against what it set, and Qt's
already re-polishes only on a change.

**AppKit, WinUI, iOS, Android: computed values through setters.** These backends get a `style` member that answers
`SVL_STYLE_TARGET_NONE` plus the new `state` member. The engine pushes each changed computed property through
`set_prop` or `set_child_attr` with the style table's target, exactly as setter rules do today. It captures the
native default before the first override and restores it when no rule sets it. The visual properties these
platforms lack as plain props (layer border, radius, gradient, shadow, padding on controls) need backend-side
"virtual props" implemented natively (§3). The style table then names those virtual props, as the existing `text`
and `parent:` virtual members do.

**egui: in-process.** The egui backend reads `ComputedStyle` straight from the engine: same process, no ABI marshalling:

- layout properties map into the node's `taffy::Style`, replacing `layout::restyle`'s `Common` scalars;
- visuals go to the node's paint, through `Frame`/`RectShape` for boxes and the child `Ui`'s
  `visuals.widgets.{inactive,hovered,active}` for stock widgets, which egui switches by state in the same frame.

The `state` member is implemented by querying `ctx.read_response(id)` at `before_pass`. That needs every leaf to
carry an explicit id; today only `TextEdit` does (`render.rs`).

### 2.8 Precedence and ownership of props

The order stays explicit attribute > inline > rules > native default, now enforced in one place:

- An `op_set_prop` from JS marks `(h, prop)` as **explicit**. The engine never writes an explicit prop.
- When JS stops setting it (the attribute is removed, which the runtime sends as a `RESTORE` today), the mark goes
  and the engine re-applies the rule value.
- `style=` / `style:` reach the host as `INLINE` text. The engine parses it in Rust as the node's inline block,
  which replaces `inlineToNative` in JS and the compiler's `rule_set`/`explicit` bookkeeping.
- JS's `Native.last` cache must not assume it owns a prop the engine wrote. The engine writes only non-explicit
  props, and JS only dedupes against values it wrote itself, so the two sets don't overlap. Mixing them inside one
  prop, for example a `bind:` on a prop that a rule also sets, is the case to test.

### 2.9 The same engine for e2e tests and MCP

Today there is no selector or query code for tests:

- GTK/Qt integration tests reach widgets by handle;
- egui_kittest queries AccessKit by label, role and value;
- EGUI.md D5 (decided: yes, MCP) relies on `egui_mcp` for the egui half;
- CLI decision 7 ("no MCP server for now") is waiting to be revisited.

The mirror plus matcher gives one query layer for every backend:

| Query | Mechanism |
|---|---|
| CSS selector incl. `:has()`, `:nth-child()`, `[prop="v"]`, states | the engine's matcher over the mirror (attribute values read live with `get_prop` in query mode, not only the sheet-registered ones) |
| text | `:text("Save")`, `:text-matches(/re/)`, `:has-text()` (Playwright's names). The text prop per kind already exists (FX1 `text_prop` for all four toolkits); read with `get_prop` |
| role, accessible name | `:role(button)`, `:name("OK")`. Needs a role per kind: a generated table from metadata (GTK `GtkAccessibleRole`, Qt `QAccessible::Role`, AppKit `NSAccessibilityRole`, WinUI `AutomationControlType`, egui AccessKit `Role`) mapped to ARIA names. A live read where the toolkit exposes it |
| component | `:component(TodoItem)` and `:source("App.svelte:12")`. Needs the D4 `identity` from the compiler; nodes without one fall back to handles |
| geometry, visibility | `:visible` and bounding boxes from a new backend query (the inspector proposal's picker/bounds adapter) |
| computed style | `computed(h)` returns the engine's values plus, for GTK/Qt, matched rules. This is "why is this red" for both tests and agents |

These test-only pseudo-classes are parsed only by the query entry point (`SelectorImpl` with an extra
`NonTSPseudoClass` set), never in component sheets. API layers:

- a Rust `Locator` in `svelters-host::testing`, `host.query(sel) -> Vec<svl_handle>`, usable from `cargo test` on
  every backend and from the log backend in op-log tests;
- `svelters mcp` tools `query`, `inspect`, `click`, `type`, `computed_style`, `screenshot` over a debug socket of the
  running host;
- on egui, merged into `egui_mcp::UiServer` as D5 planned.

Actions (click, type) go through the backend's own synthetic input where it exists (kittest, `QTest`, GTK
`gtk_widget_activate`, UIA on Windows, AX on macOS) and otherwise through the element's method calls. That is a
separate design, out of scope here.

## 3. Per-platform survey

Targets: GTK 4.14–4.24 (CI floor 4.14, FG1; features marked with their minimum), Qt 6.5+ Widgets with Fusion,
AppKit macOS 11+, WinUI 3 (Windows App SDK 1.5+, winmd 2.3.9 in our pipeline), egui 0.37.0 with taffy 0.15.0,
UIKit iOS 15+ and Android Views (one line each).

Legend:

- **N**: native (the toolkit does it, or exposes the exact signal).
- **E**: emulatable, with the mechanism and cost given.
- **✗**: not reasonable.

Costs:

- **S**: a table entry or a few lines.
- **M**: a backend feature: a virtual prop, a tracking object, a converter.
- **L**: a new subsystem (a subclass with custom drawing, a layout container).

A selector, cascade or media feature marked E on every column is the engine's job, at cost S per backend once the
engine exists.

### 3.1 Selectors

| Feature | GTK 4 | Qt 6 | AppKit | WinUI 3 | egui |
|---|---|---|---|---|---|
| type | N, through the CSS name (lossy: 12 CSS names are shared by several kinds, X6 research §1.3); engine: S | N, `.QClass`; engine: S | E, engine (S) | E, engine (S) | E, engine (S) |
| `.class` | N, `add_css_class` | N, `class` property + repolish (0.25 ms) | E, engine | E, engine | E, engine |
| `#id` | N, widget name | N, objectName | E, engine | E, engine (mirror `Name` and `AutomationId` for UIA) | E, engine |
| `[attr]`, `[attr="v"]` | ✗ native; E engine (S, M for non-observable props) | N, on Q_PROPERTY and dynamic properties (needs a repolish on change); engine | E, engine | E, engine | E, engine |
| descendant, `>` | N | N | E, engine | E, engine | E, engine |
| `+`, `~` | N | ✗ native (parse, never match); E engine | E | E | E |
| `:not()`, `:is()`, `:where()` | N `:not(compound)` only; lists ✗; E engine | `:!state` only; E engine | E | E | E |
| `:has()` | ✗ native; E engine (M: invalidation upwards) | ✗; E engine | E | E | E |
| `:first-child`, `:last-child`, `:only-child`, `:nth-child()`, `:nth-last-child()`, `:empty` | N (except `:empty`) | ✗ (`:first`/`:last` only on tab-like items); E engine | E | E | E |
| `*` | N | N | E | E | E |

Sources: GTK [CSS overview, selectors](https://docs.gtk.org/gtk4/css-overview.html); the X6 probes in
`docs/notes/styling-gtk-css-qss.md` §1.2 and §2.1; [Qt stylesheet syntax](https://doc.qt.io/qt-6/stylesheet-syntax.html).

### 3.2 Interaction and state pseudo-classes

How the state reaches us is in §2.3. This table says what can be styled natively with it.

| Feature | GTK 4 | Qt 6 | AppKit | WinUI 3 | egui | iOS / Android |
|---|---|---|---|---|---|---|
| `:hover` | N `:hover` (PRELIGHT) | N `:hover` (needs a real pointer move; `WA_UnderMouse` faking doesn't trigger it) | E: `NSTrackingArea` with an NSObject owner; no subclass (S–M) | E: `IsPointerOver` DP on ButtonBase, `PointerEntered/Exited` elsewhere (S). **But** the Button template's PointerOver state animates the presenter's Background, which beats a local value, so hover colours on stock controls must go through lightweight-styling resource keys (`ButtonBackgroundPointerOver` …) (M) | N `Response::hovered`, readable before the pass (S) | iPadOS/Catalyst only (`UIHoverGestureRecognizer`) / `isHovered`, `state_hovered` |
| `:active` | N | N as `:pressed` | E/✗: `isHighlighted`, polled, because `NSButton` runs a modal tracking loop in `mouseDown:`; leave the native pressed look (M) | E: `IsPressed` DP; `AddHandler(..., handledEventsToo)` elsewhere (S–M) | N `is_pointer_button_down_on` | N `.highlighted` / `isPressed` |
| `:focus` | N | N | E: KVO on `firstResponder`, mapping the field editor back to its `NSTextField` (M) | N `GotFocus`, `FocusState` | N `has_focus` | N |
| `:focus-visible` | N | E: `QFocusEvent::reason()`; QSS has no such state, so it's a rule class (S) | E, heuristic on the event that moved focus (M) | N `FocusState.Keyboard` | E: only TextEdit takes focus on click (S) | iPad focus system / ✗ |
| `:focus-within` | N (GTK sets it on all ancestors) | E engine | E engine | E engine (`FocusManager.GotFocus` + walk) | E engine | E engine |
| `:disabled` / `:enabled` | N | N | N `isEnabled` | N `IsEnabled` | N | N |
| `:checked`, `:indeterminate` | N | N `:checked`, `:indeterminate` | N `NSButton.state` | N `IsChecked` (three-state) | N | N |
| `:placeholder-shown` | E engine (placeholder prop + empty text) | E engine | E engine (`controlTextDidChange`) | E engine (`TextChanged`) | E engine | E |
| `:invalid` | ✗ native (`.error` class is a convention); E with our own constraint props (M) | ✗; E | ✗; E | ✗ (input validation withdrawn); E | ✗; E | ✗; E |
| `:selected`, `:backdrop` | N | N `:selected`; backdrop E (`isActiveWindow`) | E | E | E | E |

### 3.3 Pseudo-elements

| Feature | GTK 4 | Qt 6 | AppKit | WinUI 3 | egui |
|---|---|---|---|---|---|
| `::placeholder` | N as the internal `placeholder` node (`entry > text > placeholder`), via a parts map in metadata | N `placeholder-text-color` property | N `placeholderAttributedString` | N `PlaceholderForeground` | N `hint_text` styling |
| `::selection` | N `selection` node | N `selection-color`, `selection-background-color` | E: `selectedTextAttributes` on the shared field editor, set on focus (M) | N `SelectionHighlightColor` (background only) | N `visuals.selection` |
| named parts (`::indicator`, `::handle`, `::groove`, `::chunk`, `::tab` …) | N internal CSS nodes (X6 research §6.3 `parts` map, not built) | N sub-controls (`::indicator`, `::drop-down`, `::handle`, `::groove`, `::chunk`, `::tab` …) | ✗ mostly (cell drawing) | E: template parts are fragile; lightweight-styling keys cover common ones (M) | N (we paint the parts) |
| `::before`, `::after` | ✗ | ✗ | ✗ | ✗ | E (we paint; content text only) |

Pseudo-elements are matched by the engine on their host node and translated to the toolkit's part through the
metadata parts map. The map is the existing X6 proposal, `style.gtk.parts` / `style.qt.subcontrols` in the kind
schema.

### 3.4 Box model and layout

| Feature | GTK 4 | Qt 6 | AppKit | WinUI 3 | egui (taffy) |
|---|---|---|---|---|---|
| margin | N CSS `margin` (the CSS box) + widget margins `margin-start/end/top/bottom` (setters) | N QSS margin; layout `contents-margins` for containers | E: no NSView margins; stack-view custom spacing or taffy (M) | N `Margin` | N per side (today: one scalar) |
| padding | N CSS | N QSS (box widgets) | E: `NSStackView.edgeInsets`; controls have none, so a wrapper view (M–L) | N `Padding` on Control/Border/panels | N |
| border, border-radius | N CSS (Adwaita gradients need `background-image: none`) | N QSS, but a background then needs a border: **the native look is lost** (Fusion's generic drawing) | E: layer `borderWidth`/`cornerRadius` on layer-backed views (works but unsupported by the `wantsUpdateLayer` docs), `NSBox .custom`, or a backend `NSView` subclass (M–L) | N `BorderThickness`, `BorderBrush`, `CornerRadius` | N uniform `Stroke`, per-corner u8 radius; per-side widths painted by us (M) |
| width/height | ✗ CSS; setter `width-request`/`height-request` (minimums, not exact) | N QSS `width`/`height` on some widgets; setters `minimum-/maximum-*`, `setFixedSize` | E: Auto Layout constraints (M) | N `Width/Height` | N |
| min/max | N `min-width/height` CSS; ✗ max (setters only on Qt-like props) | N QSS + setters | E constraints | N `Min/Max*` | N |
| flex (direction, wrap, grow/shrink/basis, justify, align) | E: `GtkBox` covers direction + expand + align, no wrap (`GtkFlowBox` wraps); full flex needs a taffy-driven `GtkLayoutManager` (L) | E: `QBoxLayout` stretch/alignment; full flex = a taffy `QLayout` subclass (L) | E: `NSStackView` (~70%); full flex = taffy in a container subclass's `layout()` (L) | E: `StackPanel` (no wrap); full flex = taffy + Canvas, or a composed Panel subclass (L) | N (D3, done) |
| grid | E: `GtkGrid` (row/col/span); template areas need taffy (L) | E: `QGridLayout` | E: `NSGridView` | N-ish `Grid` rows/columns/spans; template areas need taffy | N, behind taffy's `grid` feature (off today) |
| gap | N as setters (`spacing`, row/column-spacing) | N setter (`spacing`) | N `NSStackView.spacing` | N `Spacing`, `RowSpacing`/`ColumnSpacing` | N |
| position absolute/fixed + inset | E: `GtkFixed` or overlay | E: no layout, `move()` | E: frames in a container | E: `Canvas.Left/Top` | N but hoisting is stubbed (`layout.rs`) (M) |
| `display: none` | N `visible=false` | N `hide()` | N `isHidden` (stack views detach hidden views by default) | N `Visibility.Collapsed` | N `Display::None` |
| `visibility: hidden` | E: `opacity 0` + `can-target=false` | E: `QSizePolicy::setRetainSizeWhenHidden` + hide | E: `isHidden` with `detachesHiddenViews=false` | E: `Opacity 0` + `IsHitTestVisible=false` | N (paint skip) |
| `overflow` | ✗ CSS; container kinds (`scrolledwindow`) | ✗; `QScrollArea` | ✗; `NSScrollView` | ✗; `ScrollViewer` | N for layout; clip and scroll are ours (M) |
| `calc()`, `%` | N in GTK CSS values; E engine for setters | ✗ QSS; E engine | E engine | E engine | N (taffy `calc` feature, off; `%` native) |

**Layout is the largest decision in this survey** (§8 D4). Flexbox on GTK, Qt, AppKit and WinUI means a
taffy-driven layout container per backend: a `GtkLayoutManager`, a `QLayout` subclass, an `NSView` with
`layout()`, and a composed WinUI `Panel` or a Canvas. Each container measures its children through the toolkit's
own size request (`gtk_widget_measure`, `sizeHint`, `fittingSize`, `Measure`/`DesiredSize`) and places them. egui
proves the taffy-on-our-tree approach (D2). Without it, flex/grid properties map onto native containers only as far
as their models overlap, which is today's setter tables.

### 3.5 Visual

| Feature | GTK 4 | Qt 6 | AppKit | WinUI 3 | egui |
|---|---|---|---|---|---|
| color | N | N | N `textColor`; `contentTintColor` on borderless buttons only | N `Foreground` (inherits through ContentPresenter) | N |
| background-color | N | N (border trap on buttons) | E: layer `backgroundColor`, `NSBox fillColor`; on bezeled `NSButton` it needs `isBordered=false` (native look lost) (M) | N `Background`; hover/press states override it (lightweight-styling keys, M) | N |
| gradients | N `linear-/radial-/conic-gradient` | N `qlineargradient`/`qradialgradient`/`qconicalgradient` (translator: M) | E: `CAGradientLayer` in a backend view (M–L) | N Linear/RadialGradientBrush | E: `gradient_rect` (2 stops, axis-aligned); others need a mesh (M–L) |
| background-image | N | N | E: `layer.contents` (M) | N `ImageBrush` | N `Brush` texture |
| opacity | N (CSS and `gtk_widget_set_opacity`) | E: `QGraphicsOpacityEffect` (today inline only; QSS `opacity` is tooltip-only) | N `alphaValue` | N `Opacity` | N `set_opacity` |
| box-shadow | N (incl. inset) | ✗ | E: `NSView.shadow`, conflicts with clipping radius; two layers (M) | E: `ThemeShadow` (rectangular, no colour/blur control) or Composition `DropShadow` (L) | N one outer `Shadow` (integer limits) |
| outline | N `outline*` | N `outline*` | ✗ (focus ring only) | ✗ (focus visuals only) | N `StrokeKind::Outside` |
| filter | N `filter` | ✗ | N `contentFilters` (CIFilter), costly | ✗ (Win2D) | ✗ (only `blur_width` on rects) |
| transform | N `transform` (2D/3D, paint-only) | ✗ | ✗ mostly: `frameCenterRotation`; AppKit owns layer geometry | N `Rotation/Scale/Translation/TransformMatrix` (no relayout) | E: translate + uniform scale (`TSTransform`); rotation per shape only |
| `transition` | N | ✗ (E: engine-driven tweens through setters, M) | N-ish: `NSAnimationContext` + `animator()` for frame/alpha; layer props via `CABasicAnimation` (M) | N subset: `BrushTransition` (Background on Panel/Border/ContentPresenter), `ScalarTransition` (Opacity, Rotation), `Vector3Transition`; others Storyboard (M) | E: per-node tweens + `request_repaint` (M) |
| `@keyframes`, `animation` | N | ✗ | E: `CAKeyframeAnimation` (M–L) | E: Composition keyframes / Storyboards (M–L) | E (M) |

### 3.6 Typography

| Feature | GTK 4 | Qt 6 | AppKit | WinUI 3 | egui |
|---|---|---|---|---|---|
| font-family, size, style | N | N | N `NSFont`/descriptor | N | N family (registered fonts), size; italic is a skew without an italic face |
| font-weight | N | N | N (descriptor traits) | N | E: variable fonts (`wght`, new in 0.37) or one family per weight (M) |
| line-height | N | ✗ | E: attributed string paragraph style (M; every text update rebuilds it) | N `LineHeight` + `LineStackingStrategy` | N |
| letter-spacing | N | N | E: attributed `.kern` (M) | N `CharacterSpacing` (1/1000 em, converted) | N `extra_letter_spacing` |
| text-align | ✗ CSS; setter `xalign`/`justify` | N `text-align` (some widgets) / `alignment` setter | N `alignment` | N `TextAlignment` | N |
| text-decoration | N | N | E: attributed underline/strikethrough (M) | N `TextDecorations` | N |
| text-overflow: ellipsis, line clamp | ✗ CSS; setters `ellipsize`, `lines` (the Pango enum needs values in metadata) | E: `QLabel` elision is manual (`QFontMetrics::elidedText`) (M) | N `lineBreakMode`, `maximumNumberOfLines` | N `TextTrimming`, `MaxLines` | N `TextWrapMode::Truncate`, max rows |
| text-transform | N | ✗ (E in the engine: transform the text prop, M) | E (engine) | E (engine; `CharacterCasing` for TextBox input) | E (engine) |
| white-space / wrap | ✗ CSS; setter `wrap` | setter `wordWrap` | N `usesSingleLineMode` | N `TextWrapping` | N |

### 3.7 Cascade, values and at-rules

| Feature | GTK 4 | Qt 6 | AppKit | WinUI 3 | egui |
|---|---|---|---|---|---|
| custom properties + `var()` | N (≥ 4.16) | ✗ native: global tokens rendered into the sheet; engine for setter values | E engine | E engine | E engine |
| `calc()`, `min()`/`max()`/`clamp()` | N | ✗ native; E engine for setters | E | E | E (taffy `calc` hook) |
| `inherit`/`initial`/`unset`/`revert` | N (`revert` untested) | ✗ (E: engine resolves before emitting, but a rule-class sheet can't express `inherit` per node: diagnose) | E engine (`initial` = the captured native default) | E engine | E engine |
| `!important` | ✗ native (rejected): E by cascade order in the rule-class sheet | ✗ (same) | E | E | E |
| `@media (prefers-color-scheme)` | N per provider (≥ 4.20) / E engine | ✗ native; E engine (no more dark sheet) | E engine; KVO `NSApp.effectiveAppearance` | E engine; `ActualThemeChanged`, `RequestedTheme` | E engine; `ctx.system_theme()` |
| `@media (prefers-reduced-motion / contrast)` | N (≥ 4.20) | E: `QStyleHints` (6.5+ colour scheme; contrast via the platform theme, from memory) | E: `NSWorkspace.accessibilityDisplayShouldReduceMotion` / `…IncreaseContrast` + options-changed notification | E: `UISettings.AnimationsEnabled`, `AccessibilitySettings.HighContrast` | ✗ (no OS probe in egui; our own) |
| `@media (width …)` | ✗ native; E engine (toplevel size) | ✗; E | E (`NSWindowDidResizeNotification`) | E (`SizeChanged`) | E |
| `@container` | E engine (needs laid-out size; after taffy containers) | E | E | E | E (taffy layout known before paint) |
| `@supports` | E engine against the backend's style table (`@supports (gap: 1px)` is true only where the table has `gap`) | E | E | E | E |
| `@layer` | E engine (order in the rule-class sheet) | E | E | E | E |
| `@keyframes` | N | ✗ | E (M–L) | E (M–L) | E (M) |
| nesting `&` | E: engine flattens (Svelte already handles scoping) | E | E | E | E |
| `cursor` | E setter: `gtk_widget_set_cursor_from_name` (CSS names) (S) | E setter: `setCursor(Qt::CursorShape)` (S) | E: tracking area `.cursorUpdate` + `NSCursor.set` (M) | E: `ProtectedCursor` via `IUIElementProtected` QI (untested) or a composed subclass (M) | N `CursorIcon` (35, all CSS names) |
| `pointer-events: none` | E `can-target=false` | E `WA_TransparentForMouseEvents` | E: `hitTest:` override needs a subclass (L) | N `IsHitTestVisible` (whole subtree) | N |

**Probe, run for this note** (`docs/notes/scripts/css_spec_probe_gtk.c` and `css_spec_probe_qt.cpp`, built with
`pkg-config gtk4` / `Qt6Widgets` and run under `tools/dev/headless`):
`.a.a.a { color: red } .a.b { color: blue }` on a label with classes `a b`.

- **GTK 4.24.1 and Qt 6.12.0 both paint red.** Both count a repeated class three times, so `.a.a.a` (0,3,0) beats
  `.a.b` (0,2,0).
- Controls behaved as CSS:
  - `.a.b` then `.a` gives blue: higher specificity wins.
  - `.a` then `.a` gives the later rule: ties go to source order.

This is what §2.7's specificity padding relies on.

**Sources.**

- GTK:
  - [CSS overview](https://docs.gtk.org/gtk4/css-overview.html) (selectors, pseudo-classes; `:focus-within` and
    `:focus-visible` are set on ancestors "unlike CSS");
  - [CSS properties](https://docs.gtk.org/gtk4/css-properties.html) (custom properties "starting with 4.16",
    media queries "since GTK 4.20", no `cursor`);
  - [GtkStateFlags](https://docs.gtk.org/gtk4/flags.StateFlags.html).
- Qt:
  - [stylesheet reference](https://doc.qt.io/qt-6/stylesheet-reference.html) (pseudo-states, sub-controls,
    properties; `opacity` "only supported for tooltips"; gradients; `outline*`, `placeholder-text-color`,
    `selection-*`).
- AppKit (developer.apple.com/documentation/appkit/…):
  - `nstrackingarea/init(rect:options:owner:userinfo:)` (the owner may be any object);
  - `nswindow/firstresponder` (KVO compliant);
  - `nswindow/fieldeditor(_:for:)`;
  - `nsview/wantsupdatelayer`, `nsview/contentfilters`, `nsview/shadow`, `nsview/framecenterrotation`;
  - `nsbutton/isbordered`, `nsbutton/contenttintcolor`;
  - `nstextfield/drawsbackground`, `nstextfield/placeholderattributedstring`, `nstextfield/maximumnumberoflines`;
  - `nsbox/fillcolor`;
  - `nsapplication/effectiveappearance`;
  - `nsworkspace/accessibilitydisplayshouldreducemotion`;
  - `nsanimationcontext/runanimationgroup(_:completionhandler:)`;
  - `nsstackview/detacheshiddenviews`;
  - `nsview/hittest(_:)`, `nsview/resetcursorrects()`.
- WinUI (learn.microsoft.com):
  - [XAML styles / lightweight styling](https://learn.microsoft.com/en-us/windows/apps/design/style/xaml-styles);
  - [dependency property precedence](https://learn.microsoft.com/en-us/windows/apps/develop/platform/xaml/dependency-properties-overview);
  - [property animations / implicit transitions](https://learn.microsoft.com/en-us/windows/apps/design/motion/xaml-property-animations);
  - [FocusState](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.focusstate);
  - [FocusManager](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.input.focusmanager);
  - [UIElement.AddHandler](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.uielement.addhandler);
  - [ThemeShadow](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.media.themeshadow);
  - [CharacterSpacing](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.controls.control.characterspacing);
  - [ProtectedCursor](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.uielement.protectedcursor);
  - [input validation withdrawn](https://github.com/microsoft/microsoft-ui-xaml/discussions/9572).
- WinUI, local evidence:
  - the Button template in `Microsoft.UI/Themes/generic.xaml` (WinUI 2.3.9 package), whose PointerOver/Pressed
    states animate the presenter's Background to `{ThemeResource ButtonBackgroundPointerOver}`.
- egui (docs.rs, egui 0.37.0 and taffy 0.15.0):
  - [`Context`](https://docs.rs/egui/0.37.0/egui/struct.Context.html) (`read_response`, `interaction_snapshot`,
    `set_theme`, `animate_value_with_time`);
  - [`Response`](https://docs.rs/egui/0.37.0/egui/struct.Response.html);
  - [`WidgetVisuals`](https://docs.rs/egui/0.37.0/egui/style/struct.WidgetVisuals.html);
  - [`RichText`](https://docs.rs/egui/0.37.0/egui/struct.RichText.html) (`variation`);
  - [`Shadow`](https://docs.rs/epaint/0.37.0/epaint/struct.Shadow.html);
  - [`TSTransform`](https://docs.rs/emath/0.37.0/emath/struct.TSTransform.html);
  - [`taffy::Style`](https://docs.rs/taffy/0.15.0/taffy/style/struct.Style.html);
  - [`CompactLength`](https://docs.rs/taffy/0.15.0/taffy/style/struct.CompactLength.html) (the `calc` hook);
  - [kittest `Queryable`](https://docs.rs/kittest/0.5.0/kittest/trait.Queryable.html).
- From memory, not checked:
  - layer-backing of all views since the macOS 10.14 SDK;
  - which AppKit properties animate implicitly;
  - WinUI resource changes after template application not refreshing;
  - Qt contrast hints;
  - Android rows.

## 4. Architecture options

| | (a) Native engines + emulation | (b) Rust engine, native as sink | (c) Hybrid: Rust selector authority, native cascade where it exists |
|---|---|---|---|
| Selector semantics | differ per toolkit (QSS no siblings/structural/`:not`; GTK no attributes/`:has`); setter rules a third subset | identical everywhere | identical everywhere (the engine matches; native sees only `.sv-rN[:state]`) |
| Visual property values | native cascade | engine computes; GTK sink = per-widget values via deduped provider classes (a new value = global provider reload, ~250 ms/1000 widgets); Qt = per-widget sheets (re-polish + parent relayout, 107–343 ms per change in a 1000-item grid) | native cascade over rule classes (GTK/Qt); engine on setter backends |
| Hover / state cost | native, same frame | every state change through host + sink; GTK transitions only between deduped classes | native for subject states on GTK/Qt; engine + setter on others; egui same frame |
| Theme, dark mode | native theme kept; Qt dark sheet re-set (full re-polish) | theme defaults must be captured per prop; dark switch = recompute + push | native theme kept (GTK's own widgets still follow the provider's `prefers-color-scheme`); every `@media` in component sheets evaluated by the engine: rule-class flips on GTK/Qt, setter pushes elsewhere, both local |
| Transitions | GTK native; others none | engine tweens everywhere (consistent, costs frames) | GTK native; WinUI implicit transitions where they exist; engine tweens on others (later phase) |
| Tests / MCP | needs a separate matcher anyway | same engine | same engine |
| What breaks | nothing | the X6 translation, GTK inline perf, Qt perf; native transitions | the translator's selector rewriting (replaced), `sv-k-` markers (gone), compile-time setter matching (retired), Qt dark sheet (gone); declaration translation and `@native` stay |
| Metadata | as today | every visual prop needs a setter target on GTK/Qt (they don't exist: GTK has no `background-color` property) | as today for `via: css`; new virtual props for AppKit/WinUI visuals; value maps; parts map; role table |

(b) fails on GTK specifically. GTK has no widget property for most visual CSS, so the only sink for a computed
`background-color` is CSS, and per-node CSS values are the expensive path X6 measured. (a) can't give consistent
selectors and still needs the matcher for tests. (c) keeps what X6 measured as cheap: classes are local on GTK, and
on Qt the sheet is set once before show. It also moves everything else into one place.

### 4.1 What changes in the existing pipeline under (c)

- **Removed, or reduced to declaration-only work:**
  - `packages/style/src/translate.js` selector code;
  - `vocab.js` `PSEUDO` maps and `QT_NEEDS_BORDER`, which stay in the declaration pass;
  - `kinds.generated.js` ambiguity markers;
  - the JS setter-rule matcher in `native-runtime/src/style.js` (`applySetters`, `ruleMatches`);
  - the compiler's compile-time matcher (`lower/mod.rs` `match_selector`, `rule_set`/`explicit`).
- **Kept:**
  - Svelte scoping and pruning;
  - the `@native` lift-out;
  - GTK/Qt declaration validation (the QSS fail-stop gate stays essential, because the generated sheet must still
    parse);
  - the token defaults (`tokens.js`, moved to Rust or to data);
  - the `style` ABI op and its `SHEET`/`CLASSES`/`SCHEME` payloads.
- **New:**
  - `svelters-style` (parser, matcher, cascade, invalidation, computed values, queries);
  - the host `StyleTree`;
  - ABI `state` member and `state_changed` callback;
  - the style member on AppKit/WinUI/egui;
  - virtual visual props on AppKit/WinUI;
  - the test `Locator` and MCP tools.
- **Op logs.**
  - The log backend gains `state` lines in scripts (`hover #4 on`).
  - Expected logs show rule classes (`style #4 classes "primary svelte-h sv-r3"`) and engine setter writes.
  - Every styling fixture's expected log changes once.
- **Metadata** (`tools/metadata/style`):
  - Each table gains `values:` maps for enums (CSS keyword → native enum).
  - It also gains `inherited:`, `initial:` (or "captured"), `animatable:`, and `transition:` (native implicit
    transition available, e.g. WinUI `BackgroundTransition`) per property.
  - It gains per-backend `states:` (bits the backend reports natively) and `parts:` (pseudo-element → GTK node / Qt
    sub-control / WinUI resource key).
  - `gen_props.py` emits a Rust table (`style_tables.rs`) next to `props.generated.js`; the JS one may shrink to
    what the build plugin still needs. `style_table.py`'s writable-target check stays the gate.
  - The docs reference pages gain a "selectors / states / at-rules" section per backend from the same data, and
    the docs test ties the guide to it.

## 5. Recommendation and phases

**(c), built in phases sized like the repo's lanes.** Every phase ends with op-log fixtures on the log backend
plus native read-back tests, as X6 and ST1 did.

| Phase | Content | Size | Needs | Acceptance |
|---|---|---|---|---|
| **CS0** engine crate | `crates/svelters-style`: parser (per §2.1), selector matching through an `Element` trait, specificity, cascade (origins, `!important`, `@layer`, source order), custom properties + `var()` + `calc()` for setter types, media evaluation, invalidation maps, `ComputedStyle` for the setter property set; the Rust style tables from `gen_props.py` | 1 lane (~1 week) | – | unit tests over a fixture corpus (selectors incl. `:has`/`:nth-child`/siblings, cascade order, `var()` inheritance); a WPT-style subset for selectors; fuzz the parser; no toolkit linked |
| **CS1** host mirror + setter rules everywhere | `StyleTree` in the core from create/insert/remove/destroy/`CLASSES`/`NAME`/`INLINE`; restyle as checkpoint step 5; explicit-prop tracking; engine pushes setter/child-attr properties on **all** backends; compiler stops lowering rules (keeps diagnostics via `svelters-style`); JS setter matcher removed | 1 lane | CS0 | every ST1/FX1 setter fixture passes unchanged in effect (op logs differ only in ordering); cross-component and dynamic-class setter rules now work; GTK/Qt/AppKit/WinUI/egui CI green |
| **CS2** queries, tests, MCP | `host.query`, `Locator` in `svelters-host::testing` (selectors + `:text`, `:role`, `:name`), kind→role table generated from metadata, `computed(h)`, `svelters mcp` (query, inspect, computed_style) over a debug socket; egui merged into `egui_mcp` | 1 lane | CS1 (CS0 alone for static queries) | the gallery tests rewritten with locators on GTK and Qt; an MCP session drives the counter on two backends |
| **CS3** GTK/Qt rule classes | sheet generation `.sv-rN…` with specificity padding; native residue for subject states; rule classes through `CLASSES`; engine media evaluation (Qt dark sheet retired); `state` ABI on GTK (state flags) and Qt (event filter) for non-subject states; translator reduced to declarations | 1 lane | CS1 | the X6 styling galleries pixel-identical; new fixtures for `:has()`, `+`/`~` on Qt, `:nth-child`, attribute selectors, `@media (min-width)`; hover over 1000 rows: no JS entry, frame time within 10% of today's native `:hover` |
| **CS4** AppKit + WinUI states and visuals | `style` + `state` members; tracking areas, firstResponder KVO; WinUI pointer/focus DPs and **lightweight-styling resource keys** for state colours on stock controls; virtual visual props (AppKit: layer border/radius/background with a backend `NSView` subclass for padding/gradient/shadow; WinUI: Brush/Thickness/CornerRadius converters, Font*); tokens + `prefers-color-scheme` from `effectiveAppearance` / `ActualTheme` | 1–1.5 lanes (one per platform, parallel) | CS1 | the styling gallery page renders on macOS and Windows CI with screenshot tests; hover/focus fixtures on both |
| **CS5** egui cascade (G2's styling item) | engine computed style → taffy `Style` and paint (Frame/RectShape, `WidgetVisuals` per state), same-frame state from `read_response`, explicit ids on every leaf, tokens on `Visuals`, per-side insets, `%`, taffy `grid`/`calc` features on | 1 lane | CS1 | gallery parity page under kittest snapshots; a hover restyle in the same frame (kittest) |
| **CS6** motion and layout | engine-driven transitions/animations on Qt/AppKit/egui (tweens through setters, `request_frame`), WinUI implicit transitions mapped; **taffy layout containers** for GTK/Qt/AppKit/WinUI (if D4 = yes) | 2 lanes, later | CS3–CS5 | a `transition: background-color 150ms` fixture animates on every backend; one flex-wrap gallery page identical in structure on all |
| **Q3 (re-sized)** | Svelte CSS analyze/prune/scope ported to Rust on top of the `svelters-style` parser, so `<style>` builds without Node | ~1 lane | CS0 | byte-identical scoped output vs svelte@5.57.2 on the fixture corpus |

The order is CS0 → CS1 → (CS2 ∥ CS3 ∥ CS4 ∥ CS5) → CS6. CS2 can start on CS0's matcher with the existing
`tree`/`handles` maps if testing is the more urgent need.

## 6. Risks

- **Two cascades on GTK/Qt.** The engine orders rules and the toolkit applies the declarations. Native-only
  constructs could break the equivalence:
  - `@native` blocks, which sit in a separate provider, intentionally;
  - theme rules, which sit below, intentionally;
  - a native pseudo-class with different semantics, such as GTK's `:focus-within` on all ancestors "unlike CSS".

  The engine knows which states it left native and can diagnose that.
- **Specificity padding length.** Padding grows with the number of distinct native residues, not with sheet size,
  because the padding is the same for every rule. Selector length is bounded by the number of native states on a
  subject plus one, which is small. The probe covers three repeats. A probe at 10 and 30 repeats should join CS3's
  tests.
- **Explicit-prop ownership.** A prop written by JS (`bind:`, `$n.set`) and also by the engine needs a clear rule.
  The proposal is that explicit always wins and the engine releases the prop. Two-way bindings of style-set props
  (`bind:value` on a slider whose `min` a rule sets) must be tested.
- **State on AppKit `:active` and WinUI template visuals.** Neither platform exposes these fully. Pressed styling
  of stock buttons on AppKit stays native. Pressed/hover colours on WinUI must go through resource keys that are
  read when the template is applied. Changing them after load may not refresh (from memory, to be probed in CS4).
- **Native look.** Backgrounds on bezeled AppKit buttons and on QSS push buttons switch off native drawing. The
  engine should warn (as the Qt border injection does today), not silently degrade.
- **Hover traffic.** Tracking areas, event filters and pointer handlers are added only for nodes whose possible
  subjects use a state (§2.3). Without that gate, a large list tracks every row.
- **Selector cost of `:has()`.** Upward invalidation on large trees. Stylo's relative-selector invalidation is the
  reference. Start with "re-match ancestors that have a `:has` rule" and measure on the 10k-row list fixture.
- **Crate churn.** Whichever parser we pick (§2.1) must not tie us to Servo's release cadence. We pin and isolate
  it behind `svelters-style`'s own AST, as the compiler does with its acorn port.

## 7. Notes on what this does not change

- Layout stays props wherever a backend has no CSS layout, until D4 is decided. D3 already allows layout properties
  in `<style>` where the table has them.
- `@native gtk|qt {}` stays the escape hatch for toolkit internals.
- Svelte's own `transition:`/`animate:` directives are a separate topic (they need a `getComputedStyle`-like and
  element-animation API, which the engine's `computed(h)` could later feed).

## 8. Decisions for the user

1. **D1 Property names.** ST1 says style properties use native names, with no CSS aliases. Browser-like CSS needs
   `background-color`, `gap`, `flex-direction`, `width` and `opacity` to mean the same thing everywhere.
   - (a) Keep native names only.
   - (b) **Standard CSS names** wherever a backend has a faithful mapping (a table entry with a `values:` map),
     native names kept alongside for things CSS can't say (`hexpand`, `homogeneous`, `contents-margins`).
   - (c) CSS names only.

   **Recommended: (b).** This reverses ST1's "no portability layer" for the property names only. A CSS property
   with no faithful mapping on a backend stays a diagnostic naming that backend.
2. **D2 Where the engine runs.**
   - (a) **The Rust host.**
   - (b) The JS runtime.
   - (c) Each backend.

   **Recommended: (a).** Hover never enters JS, one implementation serves five backends and the log backend, and the
   tests/MCP are Rust.
3. **D3 GTK/Qt output.**
   - (a) **Rule classes with native declarations and native subject states.**
   - (b) Computed values through per-node CSS.

   **Recommended: (a).** It keeps transitions, themes and the cheap path X6 measured.
4. **D4 Layout.**
   - (a) Native containers only, mapped as far as they go (today).
   - (b) **taffy-driven layout containers on every backend**, so flexbox/grid/gap/position mean the same everywhere,
     egui's model extended.
   - (c) Only on AppKit/WinUI, where native layout is weakest.

   **Recommended: (b) as CS6, after the engine.** It is the biggest single item and the main source of "looks
   different on each platform" (X6 §5.1).
5. **D5 Parser and matcher crates** (§2.1).
   - (a) **`cssparser` 0.38 + `selectors` 0.41**, pinned, with our own cascade and values: +142 KB, 27 deps.
   - (b) `lightningcss`: +4 MB, a ~16 min clean build, and no matching anyway.
   - (c) Stylo: 118 deps, python3 at build time, browser property set.
   - (d) Extend our own subset parser.

   **Recommended: (a).** (d) would re-implement `:has`, `:nth-child(of S)` and the bloom filter for no gain.
6. **D6 Native-look policy.** Backgrounds and borders on controls that lose native drawing (QSS push buttons,
   bezeled AppKit buttons, WinUI state visuals):
   - (a) **Allow it with a warning.**
   - (b) Refuse it.
   - (c) Allow it silently.

   **Recommended: (a).** This extends today's Qt `border: none` injection.
7. **D7 Transitions where the toolkit has none** (Qt, AppKit layer props, egui).
   - (a) **Engine tweens through setters.**
   - (b) Drop with a warning (today on Qt).

   **Recommended: (b) until CS6, then (a)** for colour and opacity only.
8. **D8 MCP and the CLI.** CLI decision 7 says no MCP server; EGUI D5 says yes for egui.
   - (a) **A generic `svelters mcp` over the host query API**, with egui_mcp merged.
   - (b) egui only.

   **Recommended: (a).** The query layer is backend-independent once the mirror exists.
9. **D9 Compile-time matching.**
   - (a) **Retire it in CS1** (one semantics).
   - (b) Keep it as a static fast path.

   **Recommended: (a)**, and re-add a static-template optimisation only if first-paint measurements ask for it.
10. **D10 `:invalid` and form validation.** Add constraint props (`required`, `pattern`, `min`, `max`) to the runtime
    so `:invalid`/`:valid` work on every backend?
    - (a) Yes, later.
    - (b) No.

    **Recommended: (a), later**, because no desktop toolkit has a native model.
