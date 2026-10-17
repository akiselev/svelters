# N1-P: packages, compiler tests and docs after the N1 rename

Sub-lane of N1 (see `n1-rename.md`). Scope: package tests/sources (not testhost), compiler tests, docs site.

## bind: / on<kebab> with hyphens

- Vendored Svelte fork, custom-renderer mode: accepted. `bind:visible-child-name={t}` compiles to
  `$.bind_renderer(node, 'visible-child-name', ...)`, `ontext-changed` / `onclose-request` to
  `$.event('text-changed' | 'close-request', ...)`.
- Stock DOM mode rejects `bind:visible-child-name` (`bind_invalid_name`), as it rejected any non-DOM
  binding before (`bind:currentIndex` too); not a regression.
- svelters-compiler: accepted. Qt `<tab-widget bind:current-index={i}>` lowers to
  `$n.bind($$n3, "current-index", "current-changed", ...)`, `<line-edit ontext-changed>` to
  `$n.on(.., "text-changed", ..)`, `<main-window onclose>` to `"close"`. GTK
  `<stack bind:visible-child-name>` parses but is `bind_no_plan` (no binding annotation for it; a
  metadata question, not syntax). `onclose-request` on a GTK window works.
- Open: `src/semantics/mod.rs:268` doc comment still lists `onclick`, `click` as examples of event
  attributes (compiler src is the lead's; cosmetic).

## Changes

- renderer tests: tags (`header-bar`, `check-button`, `message-box`, `about-dialog`, `spin-button`,
  `toggle-button`), `click` -> `clicked`, `close` -> `close-request` (the listener type is now the signal
  name). `style-setters-qt.test.mjs` now runs on the Qt TestHost schema with Qt tags (`main-window`,
  `box-layout`, `push-button`) and kebab Qt props in the log (`minimum-width`, `contents-margins`).
- renderer `check/App.svelte`: `placeholder` -> `placeholder-text`, `onclick` -> `onclicked`.
- renderer `types/index.d.ts` (hand-written wiring): GTK tags that are also HTML tags (`button`,
  `label`, `image`, `menu`, `picture`, `switch`, `text`, `video`) were typed by svelte-check's own HTML
  entry (an `extends` cannot override it); `onclick` passed only because it is an HTML attribute, so
  `onclicked` failed. Their native props now go into `svelteHTML.HTMLAttributes` (the hook svelte-check
  merges into every HTML entry). Prop types are checked; unknown attribute names on those tags are
  still not reported (as before).
- style test cases: tags renamed; the cross-toolkit cases (`card`, `setters`, `unsupported`) got Qt
  twins (`push-button`, `line-edit`, `box-layout`, `main-window`, `direction`) in selector lists, since
  `button`/`entry`/`box`/`window` are no longer Qt elements. Goldens regenerated.
- native-runtime tests: `virtual-list`, `listen('clicked')`. (`ListView`'s `rowHeight` prop became
  `row-height` in packages/lists and native-runtime `lists.js` during this lane, by another sub-lane;
  tests pass with it.)
- compiler tests: `native_x3.rs` and `semantics_specs.rs` use each backend's vocabulary (GTK `window`,
  `grid`, `entry`; Qt `main-window`, `grid-layout`, `box-layout`, `line-edit`, `push-button`,
  `key-sequence-edit`), kebab props/attrs (`column-span`, `row-span`, `tab-label`, `alignment`,
  `css-classes`, `echo-mode`, `window-title`), `direction` instead of Qt `orientation`, GTK `window` =
  Gtk.Window (`application-window` = Gtk.ApplicationWindow), no event aliases (`onclick` is now an
  unknown event; `events_aliases_and_signals` -> `events_and_signals`). Rune case `rune-errors`:
  `onclick` -> `onclicked` (column of the expected diagnostic +2).
- docs site: guides (events, getting-started, bindings, svelte-support, lists-and-snippets,
  annotation-workflow, platforms, styling) use native names and no aliases; smoke test checks the Qt
  counter pages under the Qt names and the shared `label` for cross-links; `render.mjs` caution text
  says `scrolled-window`; the mini-IR golden updated for that.
- styling guide (docs/site): setter tables regenerated from STYLE_TABLES (kebab setter names, Qt
  `alignment`, `direction`, `box-layout`), prose updated (toolkit vocabularies, `toggle-button`).

## Notes for the lead

- style translator: a selector list `box#main, box-layout#main { spacing }` on GTK reports both
  `<box-layout> is not an element on gtk; rule dropped` and `'spacing' does not apply to <box-layout> on
  gtk` (the setter path checks the dropped compound again). Cosmetic, left as is.
