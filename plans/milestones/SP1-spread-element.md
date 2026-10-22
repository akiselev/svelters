# SP1: spread attributes and `<svelte:element>` on native elements

Before SP1 the native compiler rejected `{...props}` on a native element (`native_spread_unsupported`,
unless the keys were static) and `<svelte:element this={tag}>` (`unsupported_feature: this svelte:
element`). Both are only known at run time, so they now go through a runtime-checked path that reads
the same spec the compiler uses. Static attributes stay checked at compile time.

## 1. Design

### Spec tables at run time: `@svelters/native-runtime/kinds`

The compiler is the source of truth for what an attribute means (property by JS or native name,
writable, event by signal name / `js_event` / alias, capture suffix and phases, static value
coercion). So the compiler also writes the runtime table: `svelters-compile --kinds` (same
`--backend` / `--spec` as a compile) prints an ES module, generated from the loaded spec by
`semantics::kinds_module`:

- `tags` (tag -> kind id), `kinds` (per kind id reachable from a tag: `chain`, `props`, `events`,
  `text`, `top`), `aliases` (event aliases), `native_style` (class/id/style drive the X6 styling
  rather than a `class` prop), `backend`.
- A prop: `[js, native, flags, type, default, values, binding]` (flags: writable, readable,
  nullable, string_form), only the fields the runtime needs.

Generated code imports it only when a component needs it:
`import * as $$kinds from '@svelters/native-runtime/kinds'`. The build resolves the specifier to a
virtual module the plugin produces with the compiler (`svelteNative` in `vite.js`; the conformance
runner writes it once and its hooks resolve it). So the table is in a bundle only when some
component has a runtime spread or a `<svelte:element>`, once per bundle, and only for the backend
built. At run time a tag's merged lookup (props and events of its whole chain) is built on first use
and cached (`kindInfo`), so the cost is per tag used, not per table. Sizes of the full module:
gtk ~50 KB, qt ~80 KB, appkit ~40 KB, winui ~20 KB, egui ~2 KB, dom ~10 KB (unminified JSON).

Alternatives not taken: a per-kind table inlined in each component (duplicated per component, and
`<svelte:element>` needs every kind); a checked-in per-backend module (breaks `--spec` paths and
needs a freshness test).

### Spread on a native element: `$n.spread(node, kinds, fn, deps, css)`

As Svelte's `attribute_effect`: when an element has a spread whose keys are not static, **all** of
its attributes go into one object in source order (`{ label: "x", ...p, onclicked: f }`), and a
render effect applies it. Later keys win, as in Svelte. Bindings, `use:`, `{@attach}`, `bind:this`
and `parent:` attributes stay separate and run after it (bindings after attributes, as before).
Explicit attributes are still resolved and type-checked at compile time (unknown names are errors,
static values are coerced); only the spread part is resolved at run time. `{...{ a: 1 }}` (an object
literal with plain keys, none of them `slot`/`class`/`style`) keeps its compile-time lowering (one op
per key). `{...constObject}` used to be lowered the same way; it now takes the runtime path, so keys
added to the object later are applied as in Svelte (that deviation is gone). Call expressions in the
object are memoized as deriveds (`deps`, Svelte's Memoizer) and inline handlers are hoisted, so a
re-run passes the same function.

For each key of the object, against the tag's spec (`kindInfo`):

| key | what happens |
| --- | --- |
| a writable prop (JS or native name) | `set(node, prop, value)`; a string for a non-string prop is coerced as a static attribute would be (bool, ints, numbers, enums, flags, string lists); a prop with a binding plan gets the `track` mirror (as an explicit one-way prop does) |
| `on<event>`, `on<event>capture` | the event resolved as the compiler does (alias, signal or `js_event`, then the `capture` suffix with its phase check); one native connection per key, whose handler calls the key's current value (a new function does not reconnect, as Svelte); null/undefined disconnects |
| `class`, `style`, `id` on GTK/Qt/AppKit/WinUI/egui | the X6 styling (`attr`): `class` through `clsx`, merged with the `class:` toggles; `style` with the `style:` directives; plus the component's CSS scope class |
| `class`/`style` on a backend where they are props (the DOM conformance backend) | the prop, `class` through `clsx` plus the `class:` toggles |
| `parent:<name>` | `child_attr(node, name, value)` (unchecked, as for an element whose parent is not known) |
| an `@attach` symbol key (`createAttachmentKey()`) | an attachment, re-run when the function changes |
| `slot`, `children` (Svelte's own), `$$...` | ignored (`slot` must be static) |
| unknown | DEV: `console.warn` with the compiler's code and message (`unknown_property`, `read_only_property`, `unknown_event`, `listen_phase_unsupported`, `bad_attribute_value`); never a crash; the key is skipped |

A key that disappears (or becomes null/undefined) is reset: to the spec `default` when the spec
gives one; else (no default known) to the value read from the native object before the spread first
wrote it (`capture`/`RESTORE`, as the style fallbacks do); a prop that is not readable and has no
default is written `null`. A removed event key disconnects; a removed `parent:` attribute sends
`null`.

Bindings and spreads, as in Svelte: the binding's write comes after the spread at creation, so
`<input bind:value {...p}>` with `p.value` shows the bound value; a later change of the spread
writes the spread's value (the binding writes again only when its state changes).

### `<svelte:element this={tag}>`: `$n.dynamic_element(container, getTag, render, kinds)`

As Svelte's `element()`: a block tracks `getTag() || null`; a nullish or empty tag renders nothing;
a different tag builds a new element (`create`, then `render(node)` sets its attributes and builds
its children, then it is appended), then the old one is disposed; the same tag keeps the element.
A toplevel kind (per the table) is created as a toplevel. An unknown tag renders nothing and warns
in DEV. All attributes go through `$n.spread` (the kind is not known at compile time), `bind:this`
binds the runtime node per instance, `use:`/`{@attach}` run per instance; other `bind:` directives
are a compile error (`dynamic_element_bind`). Text children go to the kind's text-content prop at
run time (`$n.text`), element children to its default slot (or `slot="..."`), checked by the host at
insertion.

### Diagnostics

`native_spread_unsupported` and `unsupported_feature: this svelte: element` are gone. Explicit
attributes keep their compile-time errors (`unknown_property`, `unknown_event`,
`bad_attribute_value`, ... next to a spread too). One new compile error: `dynamic_element_bind`
(`bind:<prop>` on `<svelte:element>`, which needs the kind's binding plan; `bind:this` works). At
run time the compiler's codes come back as DEV warnings (`[svelters] <code>: <message>`, once per
message) for keys the spec does not allow, plus `unknown_element` / `text_not_allowed` for
`<svelte:element>` and Svelte's own `svelte_element_invalid_this_value`.

### AppKit (ST2) and the other native-styled backends

On GTK, Qt, AppKit, WinUI and egui no kind has a writable `class` prop, so a spread's `class`,
`style` and `id` keys go through `attr(node, ...)`, the call the compiled `class=`/`style=`/`id=`
attributes use. On AppKit that marks the element dirty for ST2's decl cascade, which reads
`el.attrs.class` / `id` / `style` (`style.js` `declClasses`, `applyDecl`), so a class that only a
spread provides is matched by the stylesheet rules like a static one, and inline `style` keys go
through the same decl parsing (setter properties included). Not tested on a Mac host locally (the
TestHost has no decl styling); the macos-appkit CI runs ST2's own tests on the merged tree.


