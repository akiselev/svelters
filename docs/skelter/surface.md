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
