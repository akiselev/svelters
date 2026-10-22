# ST2: AppKit styling (the decl dialect) and a styled todo app

State: DONE (1b0540b5, 2026-10-21, local Mac: Intel, macOS 15.7.5; CI runs the new todo suite on arm64 / macOS 26).
User decision (2026-10-21): a runtime engine (states, dark scheme, tokens), not compile-time setters only.

## Why

AppKit had seven setter-only style properties (`gap`, `padding`, `text-align`, ...): no colours, fonts,
backgrounds, borders or sizes, and no `:hover`, dark scheme or `var(--sv-*)`. GTK and Qt get those from
their stylesheet languages; AppKit has none.

## As built

```text
<style> --Svelte scoping--> translateSheet(css, 'appkit')            packages/style/src/decl.js
          decl rules { sel: [compound + combinator], state, dark, spec, order, decls (var() kept) }
          setter declarations stay with svelters-compile (compile-time setters, unchanged)
registerStyles(id, { decl })                                            vite.js -> the bundle
runtime cascade engine (native-runtime/src/style.js, target 'decl'):
  match rules on the native ancestor chain, cascade by specificity/order, dark rules while dark,
  resolve var() (tokens + custom properties of ancestors), inherit color/font-* from the parent,
  inline style= on top (and removed from state blocks), expand shorthands
  -> host.style(SVL_STYLE_DECLS = 7, h, "base\x01p:v...\x02hover\x01p:v...")   only when it changed
  re-run on class/id/style changes, inserts (subtree), sheet registration, scheme/token changes
AppKit backend (crates/svelters-appkit/src/style.rs):
  merge base + active state blocks (hover, focus, checked, active, disabled), apply by group,
  restore what was replaced; states from an NSTrackingArea (hover, cursor), KVO on the button
  cell's `highlighted` (active), and re-reads after ops / actions / NSWindowDidUpdate (focus,
  disabled, checked); SVL_STYLE_SCHEME sets NSApp.appearance, and with no text answers the system's
```

- Metadata: `metadata/style/appkit.yaml` has `dialect: decl` and typed `via: css` properties (colour,
  length, border, shadow, font-weight, enums): `color`, `background(-color)`, `border(-width|-color|-style|-radius)`,
  `box-shadow`, `opacity`, `accent-color`, `font-size|-weight|-family|-style`, `text-decoration`,
  `width|height|min-*|max-*`, `flex-grow`, `align-self`, `cursor`. `style_table.py` accepts the dialect;
  the type goes to the semantics JSON as `decl_type` (the compiler reads `type` as a setter value kind).
- ABI: `SVL_STYLE_DECLS` 7, `SVL_STYLE_TARGET_APPKIT` 3 (additive; `backend-abi.md`).
- Scroll views: a stack or grid view as `document-view` is wrapped in a flipped `SveltersDocumentView`
  pinned to the clip view (top, leading, trailing), so it scrolls from the top and sizes itself.
- `flex-grow: 0` hugs along the parent stack's axis (750), `> 0` stretches (1), `align-self: stretch`
  fills the cross axis.
- Found on the way: `NSTextAlignment` center/right (and `NSImageResizingMode`) differ between arm64 and
  x86-64 (`TARGET_ABI_USES_IOS_VALUES`). The metadata came from an arm64 runner, so `text-align:
  center` right-aligned on Intel. `ingest.py` now parses for x86-64 too and records `x86_64` values;
  `gen_backend.py` emits `if cfg!(target_arch = "x86_64")` member lists.
- Fixture `fixtures/todo-mac` (add, toggle, delete, filters, clear completed, empty state, scheme
  toggle, scrolling list), styled entirely from `<style>`.

## Tests

- `packages/style/test/decl.test.mjs` (8): rules, states, combinators, dark, diagnostics, values,
  matching, cascade order, shorthands. Golden `unsupported` refreshed (appkit now has `width`, `box-shadow`).
- `examples/counter-mac/tests/todo.rs` (5, in macos-appkit CI): fonts, layer background/border/radius/
  shadow, flat button, `:disabled` alpha, descendant `.row.done .label` strikethrough toggled by
  `class:`, the danger title colour, `flex-grow` width, add/toggle/delete through native actions,
  `text-align: center` per architecture, `:hover` through the tracking area's owner, scheme switch
  (NSApp appearance + token surface colour), 20 rows: equal heights, document taller than the clip
  view, flipped document view. Screenshots in `target/todo/`.
- Local: all counter-mac suites green (backend 22, generated 1270, types 126, types_core 8, counter 5,
  gallery 6, todo 5); `pnpm -r test`, `pnpm test:meta`, `cargo test -p svelters-compiler` green.
- By hand with real mouse/keyboard input: typing + Return adds, hover on rows and the Add button, focus
  background on the field, light/dark switch, wheel scrolling of a long list.

## Open

- `:active` is wired (cell `highlighted` KVO) but not verified: synthetic presses did not register as clicks.
- A state applies to the element it is written on; `.row:hover .label` is an error and inherited
  properties do not follow a state into children.
- Live system appearance changes are not followed after startup (the scheme is read once).
- No `padding` on buttons or text fields (setter on stack views only); no transitions.
- `<progress-indicator style="spinning">`: the native `style` property collides with the `style`
  attribute (inline CSS), so a spinner cannot be declared. Found in WP-A's gallery; needs a rename.
- Tri-state checkbox: `state="mixed"` is applied before `allows-mixed-state` and lost.
- The non-GUI types enums leave out arch-dependent members (objc2 has them behind cfg).
- The WP-A gallery pages still collapse boxes, tab and split views; they can now use `width`/`height`.
- WinUI could take the same dialect (a `DECL_TOOLKITS` code and a backend applier).
