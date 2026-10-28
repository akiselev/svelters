# skelter: names on the Svelte surface

The Svelte surface (`crates/svelters-skelter/src/surface/`) gives each skelter kind the JS-facing
names of egui's spec, so a component written for egui runs unchanged. The surface name is what an
author writes; the native kind's own name can differ, and the compiler refuses a name the surface
lacks (`svelters::event::unknown`, an unknown attribute), even when the native kind has it.
`metadata/semantics/skelter.json` lists every surface name.

## Names that differ from the native kind

- **`enabled`, not `disabled`.** Every widget takes the common `enabled` (default true); the surface
  writes the native `disabled` from it, inverted. `slider` is a widget like the others: write
  `enabled={false}`, not `disabled`.
- **App kinds** (`Kinds::with_app`) take the props, events and methods their own core spec declares,
  under those names: read the app's spec (its dump, `svelters_skelter::dump`, writes `kinds.d.ts`),
  not a guess from a similar built-in kind.

## Hover: `pointerenter` and `pointerleave`

Every widget has them, as in the DOM: they don't bubble, `pointerenter` comes once for each node the
hovered path gains (outermost first) and `pointerleave` once for each node it loses (innermost
first), including when the pointer leaves the window. They come at event time, before the frame's
`pointermove`. A hover preview starts on `pointerenter` and ends on `pointerleave`.

## `text-edit`

- `read-only`: the text can be focused, selected and copied but not typed into. `enabled={false}`
  is different: no focus and no selection.
- `oncancel`: Escape in the field. The field took the key, so a form can revert that field's edit
  before it treats Escape as its own (closing a preview, a dialog).

## The views, tabs and pickers (polish-b2)

Every skelter-widgets kind is a Svelte element. The ones added in polish-b2 take egui's tag and kind
names, base kind and default slot name, and **their own kind's members** (props, events, methods,
types and defaults), so a few names differ from svelters-egui's for the same element:

| Element | Differs from svelters-egui |
|---|---|
| `popup` | `light-dismiss` (egui's `autohide`), `escape-dismiss`, `gap`, `modal`, `flip`, `shift`, `fit`, `match-width`, `anchor-to` (the `anchor` of the element it positions against instead of its owner: `<button anchor="t"/><popup anchor-to="t">`; it closes with `anchor-hidden` when that element goes away); events `open`, `close(reason)`, `toggle(open)`; no `popup`/`popdown` methods (write `open`). Open, it is in the window's top layer, placed by `placement` next to its anchor and flipped when it does not fit. |
| `modal` | `heading` (egui's `title`); events `cancel(reason)` (cancelable) and `close(reason)`; no `close()` method (write `open`). It covers its parent and centres its children in the dialog box. |
| `inner-window` | `heading`, `x`, `y`, `width`, `height`, `min-width`, `min-height`, `collapsed` (egui's `default-*`, `open`); events `raise`, `close`, `toggle(expanded)`, `move`/`resize(x, y, width, height)`. It is laid out at `x`/`y` in its parent. |
| `split-view` | `changed(position)`; `commit-mode` is `live` or `release`. |
| `toast-host` | No `toast` children: show toasts with `show(text, level?, duration?, actions?)` (returns the id), `dismiss(id)`, `clear()`; `max-visible`; events `closed(id, reason)`, `action(id, index)`. |
| `status-bar` | `show-message(text, timeout?)`, `clear-message()` as egui's. Write the sections in order (start, temporary, end): the bar does not reorder them. |
| `tooltip` | skelter's own element (egui has only the `tooltip` prop): the tooltip surface, drawn where it is placed. |
| `menu-bar` | `native-bar`; events `activate(item, checked)`, `toggled(item, checked)`, `hover(item)`, `open(menu)`, `close(menu)`. |
| `tab-view`, `tab` | As egui's (`selected`, `changed`; `title`, `closable`, `close-requested`), plus `close-requested(index)` on the view. |
| `calendar`, `date-picker`, `color-editor`, `color-picker` | As egui's, with skelter's enum values (`commit-mode`: `confirm`/`immediate` on the date picker, `live`/`confirm` on the colour picker; `alpha`: `opaque`, `blend`, `blend-or-additive`). The drops have no `open` prop. |
| `scrollbar`, `dialog-buttons`, `panel`, `menu`, `menu-separator` | As egui's. `dialog-buttons` places its buttons by `dialog-role` in the build OS's order. |

`context-menu` also takes `open` (bindable): true opens it where it last opened (its own top left the first
time), as `popup()` does without a point.
