---
title: text-edit
description: A single- or multi-line text field (egui::TextEdit). Props, events, bind:text, CSS hooks, states and accessibility.
tag: text-edit
kind: TextEdit
category: inputs
status: available
verified_at: 424ebcf4
generated: partial   # regions between BEGIN/END GENERATED markers come from metadata/semantics/egui.json
sources:
  - metadata/semantics/egui.json#kinds.TextEdit
  - crates/svelters-egui/src/kinds/builtin.rs
  - crates/svelters-egui/src/leaf.rs
  - crates/svelters-egui/src/theme/base.css
cases:
  - docs/egui/cases/text-edit-basic.svelte
  - docs/egui/cases/text-edit-draft.svelte
---

<!-- SAMPLE (plans/research/DOCS-PLAN.md §8). Final path: docs/egui/reference/widgets/text-edit.md.
     GENERATED regions are what DOC1's generator writes from egui.json. Everything else is the hand-written
     part, kept by the generator between runs. Claims marked `verify` must be checked by a test before the
     page ships (DOCS-PLAN §5.4). -->

# `<text-edit>`

<!-- BEGIN GENERATED: tools/docs/egui/gen_reference.py, kinds.TextEdit. Do not edit. -->
A single- or multi-line text field.

**Native:** `egui::TextEdit` · **Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

Use it for free text and for values the user types (lengths, names, expressions). For numbers that
are dragged or stepped, use [`drag-value`](drag-value.md) or [`slider`](slider.md). For a unit-aware
CAD quantity, use cadatomic's own `<quantity-field>` kind.

## Example

<!-- snippet: docs/egui/cases/text-edit-basic.svelte -->
```svelte
<script lang="ts">
  let height = $state('');
  const valid = $derived(/^\d+(\.\d+)?( ?mm)?$/.test(height.trim()));
</script>

<grid column-gap={8} row-gap={4} align-items="center">
  <label>Height</label>
  <text-edit
    hint-text="Height"
    bind:text={height}
    invalid={!valid}
    tooltip="A length, e.g. 20 mm"
    anchor="height"
  />
</grid>
```

`docs/egui/cases/text-edit-basic.svelte` is a testkit case. CI compiles it and runs its `@test` steps on
egui (type "abc", expect `invalid`; type "20 mm", expect valid), so the example cannot rot.

## Props

<!-- BEGIN GENERATED: props of kinds.TextEdit. Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `text` | `string` | `""` | **yes** (exact) | The edited text; typing changes it. |
| `hint-text` | `string` | `""` | no | Shown while the text is empty; also the default accessible name. |
| `multiline` | `boolean` | `false` | no | Several lines (`TextEdit::multiline`); Enter inserts a newline. |
| `password` | `boolean` | `false` | no | Hides the characters. |
| `char-limit` | `number` (i64) | `0` | no | Maximum number of characters; 0 for none. |

Plus the 26 props every element has, from [`Widget`](widget.md#props): `enabled`, `visible`, `tooltip`,
`accessible-name`, `anchor`, `invalid`, `autofocus`, the size and flex props, `position` and its insets,
and `drag-type` / `drag-data` / `drop-types`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: signals of kinds.TextEdit. Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(text: string) => void` | The user edited the text; carries the new text (once per frame). |
| `ongained-focus` | `() => void` | Took keyboard focus. |
| `onlost-focus` | `() => void` | Lost keyboard focus. |

Plus the drag-and-drop events of [`Widget`](widget.md#events).
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.TextEdit. Do not edit. -->
| Method | Returns | Description |
|---|---|---|
| `request-focus` | `void` | Moves keyboard focus here on the next frame. |
<!-- END GENERATED -->

Call a method through `bind:this`:

```svelte
<script lang="ts">
  let field: any = $state();
</script>
<text-edit bind:this={field} hint-text="Name" />
<button onclicked={() => field.call('request-focus')}>Edit name</button>
```

Prefer `autofocus` when a field should take focus as soon as it appears, for example in a dialog.

## Behaviour you need to know

- **`changed` fires at most once per frame,** and it carries the whole text, not a keystroke.
  A paste is one `changed`.
- **Writes from JS don't fire `changed`.** Setting `text` (directly or through `bind:text`) updates the
  field silently; only user edits announce. So a handler that reformats the text won't loop.
- **Enter.** In a single-line field, Enter activates the `default` button of the field's focus scope
  (see [focus and keys](../../svelte/focus-and-keys.md)), and Escape activates the `cancel` button.
  In a `multiline` field, Enter inserts a newline and doesn't reach the default button. While an input
  method is composing, Enter and Escape go to the IME.
- **The focused field claims its keys:** text input, editing keys, Ctrl+Z, Delete and the arrows. Your
  shell's shortcuts don't see those keys. A Rust keymap that runs before the pass must ask
  `Embed::shortcut_override(ctx, &chord)`, and menu commands like Edit > Copy go through
  `Embed::text_command(ctx, TextCommand::Copy)`. See [rust/focus-and-keys](../../rust/focus-and-keys.md).
- **Native text behaviour.** Word boundaries, double-click selection and Escape follow the platform's
  native rules (GTK's on Linux, `svelters_egui::native`). On Linux, selecting text sets the primary
  selection and a middle-click pastes it.
- **No read-only mode yet.** `enabled={false}` greys the field out and blocks selection. A selectable,
  read-only field is FM1 (planned). Until then, use a `label` for text the user only copies.
- **Commit on Enter or blur** (`bind:text|commit`, a draft value) is CX3 (in progress). Until it lands,
  keep a draft and commit it on `onlost-focus` and through a default button. See
  `docs/egui/cases/text-edit-draft.svelte`.
- `multiline` fields ask for 4 rows of height by default; set `height` or `min-height` to change that.

## CSS

<!-- BEGIN GENERATED: style hooks of tag text-edit (egui table + theme/base.css). Do not edit. -->
**Selector:** `text-edit` (type), plus `.class` and `#id`. **Painted by:** egui (stock widget). CSS
reaches it through its per-state `WidgetVisuals`.

| Property | Applies | Default (theme) |
|---|---|---|
| `border-color` | yes | `var(--sv-input-border-color)` |
| `border-width` | yes | `var(--sv-stroke)` |
| `border-radius` | yes | `var(--sv-input-radius)` |
| `color` | yes | `var(--sv-input-fg)` |
| `background-color` | no: the fill comes from `--sv-input-bg` | `var(--sv-input-bg)` through the egui `Style` |
| `box-shadow`, `opacity` | yes | none |
| layout (`width`, `flex-*`, `margin`, `align-self`, insets) | yes | auto |

Full matrix: [css/properties](../../css/properties.md).
<!-- END GENERATED -->

<!-- verify: `background-color` on a text-edit. widget_visuals() sets bg_fill, while egui fills the field
     from Visuals::text_edit_bg_color (base.css comment; egui text_edit/builder.rs). DOC7 adds a kittest
     pixel check and corrects the row. -->

Tokens to change instead of writing rules: `--sv-input-bg`, `--sv-input-fg`, `--sv-input-border-color`,
`--sv-input-border-color-focus`, `--sv-input-radius`. See [css/tokens](../../css/tokens.md).

## States

<!-- BEGIN GENERATED: states set on kinds.TextEdit (needs the `states` field DOC1 adds to egui.json). -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over the field | egui response |
| `:active` | the primary button is down on it | egui response |
| `:focus` | it has keyboard focus | egui response |
| `:disabled` / `:enabled` | `enabled` is false / true | prop |
| `:invalid` / `:valid` | `invalid` is true / false | prop |
| `:read-only` | never yet (no read-only prop; FM1) | — |
<!-- END GENERATED -->

```css
text-edit:invalid { border-color: var(--sv-invalid-border); }
.compact text-edit { border-radius: 0; }
```

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.TextEdit (needs the `a11y` field DOC1 adds to egui.json). -->
| | |
|---|---|
| Role | `TextInput`; `MultilineTextInput` when `multiline`; `PasswordInput` when `password` (set by egui) |
| Name | `accessible-name`, else `hint-text` |
| Value | the text (`SetValue` action supported) |
| `invalid` | `invalid` prop → AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |
<!-- END GENERATED -->

A field with no `accessible-name` and no `hint-text` has no name. egui_kittest rejects it, and
screen readers announce "edit text". Give a visible `label` and a `hint-text`, or an `accessible-name`.

## Testing

<!-- snippet: examples/egui-docs/tests/text_edit.rs#type -->
```rust
use svelters_egui::harness::{EmbedHarness, Queryable};
use svelters_egui::embed::{EmbedConfig, MountSpec, SlotOptions};

let mut h = EmbedHarness::new(egui::vec2(400.0, 300.0), EmbedConfig::default()).unwrap();
let entry = h.embed.load("ui/dist/embed.js").unwrap();
let slot = h.embed.create_slot("panel.task", SlotOptions::fill());
h.embed.mount(MountSpec::new(entry, "TaskPanel", slot, "panel.task")).unwrap();
h.set_ui(move |e, ui| { e.show_slot(ui, slot); });
h.run_until_settled(10).expect("mounted");

h.harness.get_by_label("Height").type_text("20 mm");
h.run_until_settled(10).expect("settled");
```

Find fields by their accessible name (here the hint text) or by `anchor`, never by the `Debug` text of a
role. See [rust/testing](../../rust/testing.md).

## See also

[`drag-value`](drag-value.md) · [`combo-box`](combo-box.md) · [svelte/bindings](../../svelte/bindings.md) ·
[svelte/focus-and-keys](../../svelte/focus-and-keys.md) · rustdoc `svelters_egui::focus::TextCommand`
