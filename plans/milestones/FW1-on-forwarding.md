# FW1: `on:` forwarding in the compiler

The user's decision (svelters `CLAUDE.md`, "Decisions already made"; TRACKS decision 2): a bare
`on:event` on a component compiles to forwarding an `on<event>` callback prop; there are no legacy
component events. cadatomic's Svelte code relies on it (requests SV9, coordinator correction C-2).
Spec: `plans/research/ARCH-EMBED.md` §8.7 and the FW1 row of §10; `plans/research/CADATOMIC.md` §6
D1 option (b).

Before FW1, a bare `on:clicked`, any `on:` modifier and any `on:` on a component were
`unsupported_feature` ("on: directives with modifiers or without a handler").

Branch `ci/fw1`: `origin/master` (`06cdb4db`) with `origin/ci/merge-sp1-tr1` (SP1 + TR1, not yet on
master) merged in.

## 1. What the lowering emits

Inside a component `C` (`$$props` is the compiler's own props parameter of `C`'s function, which
runes-mode user code cannot name):

| Source | IR | Generated JS |
| --- | --- | --- |
| `<button on:clicked />` | `Op::Listen { handler: Handler::Forward { key: "onclicked" } }` | `$n.on($$n3, "clicked", function (...$$args) { $$props.onclicked?.apply(this, $$args); });` |
| `<window on:close-request />` | same | `... { $$props["onclose-request"]?.apply(this, $$args); }` |
| `<button {...rest} on:clicked />` (runtime spread) | `SpreadEntry::Forward` | `$n.spread($$n2, $$kinds, () => ({ ...(rest), onclicked: $$props.onclicked }));` |
| `<svelte:element this={tag} on:clicked />` | `SpreadEntry::Forward` | `$n.spread($$n3, $$kinds, () => ({ onclicked: $$props.onclicked }));` |
| `<Child on:accepted />` (also `<svelte:component>`) | `ComponentProp::Forward { name: "onaccepted" }` | `Child(..., { get onaccepted() { return $$props.onaccepted; } })` |
| `on:x={h}` on an element | unchanged: a listener | `$n.on(node, "x", h)` |
| `on:x={h}` on a component | `ComponentProp::Dynamic { name: "onx" }` (as the attribute `onx={h}`) | `{ onx: h }` or `{ get onx() { ... } }` |
| `on:x\|modifier` (any form) | none | `unsupported_feature` with a fix-it (below) |

- The element form is Svelte's own handler for `onclicked={$$props.onclicked}`: the prop is read
  when the event fires, so a parent that passes a new handler, or none, needs no reconnection, and
  a missing handler is a no-op (`?.`). The native listener is always connected.
- The event of the bare form is resolved against the spec as for `onclicked={…}`: an unknown event
  is `unknown_event`, a capture suffix and phases are checked the same way.
- A snippet containing a bare `on:` reads `$$props`, so it is never hoisted to module level
  (`transform::forwards_events`, used by `hoist_snippet` and its recursion over referenced
  snippets).

## 2. Diagnostics

Text as emitted (golden `crates/svelters-compiler/tests/fw1/*/expected.txt`):

- **`legacy_forwarding_lowered`**: a new severity, **`note`** (`Severity::Note`, serialized
  `"note"`, rendered `note[...]`). Never fails a build, is not a warning (the Vite plugin shows only
  warnings and errors, so it stays quiet there), once per component, at the first bare `on:`.
  > `` `on:clicked` forwards this component's `onclicked` callback prop: the one handler its parent passes gets the event (legacy forwarding re-dispatched it to every listener; svelters has no component events) ``
  > help: `` for svelte-check, declare the prop in this component's `$props()` type, e.g. `Props & Forward<'button', 'clicked'>` (types from `@svelters/native-runtime/types`) `` (component form: `` ForwardFrom<typeof Child, 'accepted'> ``)
- **`duplicate_event`** (error), at the bare directive, when a bare `on:x` is not the only handler
  of `x` on its element or component (`onx={h}`, `on:x={h}`, a second bare `on:x`):
  > `` `on:clicked` forwards this component's `onclicked` callback prop, and <button> also handles `clicked`: a bare `on:clicked` must be its only handler `` (component: `` <Child> also gets the prop `onaccepted` ``)
  > help: `` to handle `clicked` here and forward it, destructure the prop (`let { onclicked } = $props()`) and call it from one handler: `onclicked={(e) => { handle(e); onclicked?.(e); }}` ``
- **`duplicate_event`** (error) on a component, at the later one, when two `onx` props come from
  `on:x={h}` plus `onx={…}` or another `on:x={…}` (the later would silently replace the earlier):
  > `` <Child> gets the `onaccepted` prop twice (`on:accepted={…}` is the prop `onaccepted`): only the last one would be passed ``
  > help: `` pass one `onaccepted={…}` that does both ``
- **`unsupported_feature`** for modifiers, with the rewrite as help (the handler's name when it is
  an identifier, else `handler`):
  > `` `on:clicked|preventDefault|once`: event modifiers are not supported by the native compiler (runes mode has no modifiers) ``
  > help: `` write `onclicked={(event) => { event.preventDefault(); h(event); }}` and, in the handler, ignore the calls after the first (`|once`) ``
  - `on:clicked|capture={h}` → help `` write `onclickedcapture={h}` `` (exact).
  - bare `on:clicked|stopPropagation` → `` destructure the prop (`let { onclicked } = $props()`), then write `onclicked={(event) => { event.stopPropagation(); onclicked?.(event); }}` ``
  - component `on:accepted|once={h}` → `` `on:accepted|once`: event modifiers on a component are not supported ... `` / help `` pass `onaccepted={h}` and, in the handler, ignore the calls after the first (`|once`) ``

`on:x={h}` and `onx={h}` on one element stay two listeners (unchanged; no diagnostic).

## 3. Typings

svelte-check (svelte2tsx) builds a component's props type from its `$props()` annotation only, so
the compiler cannot add the forwarded prop to it. `packages/native-runtime/types/index.d.ts` adds:

- `Forward<Tag, Event>`: `{ on<Event>?: <Tag>'s own on<Event> handler type }` from
  `svelteHTML.IntrinsicElements` (`(event: any) => void` for a tag the typings do not know).
- `ForwardFrom<typeof Child, Event>`: `{ on<Event>?: Child's on<Event> prop type }`.
- `on:<event>` keys on every native element, typed as its `on<event>` handler (`OnDirectives` in
  `WithBindings`). Before, svelte-check rejected any `on:` on a native element
  (`'"on:clicked"' does not exist in type ...`).

`packages/native-runtime/check/{Toolbar,Panel}.svelte` + `App.svelte` exercise both helpers through
`pnpm check`. A probe file (not committed) showed svelte-check rejecting a wrongly typed forwarded
handler on both levels (`(n: number) => …` is not assignable to `SvlHandler`), an unknown
`onclickd` prop, and `on:clickd` on `<button>`.

## 4. Tests

- `crates/svelters-compiler/tests/fw1_forwarding.rs`: goldens in `tests/fw1/<case>/` (`input.svelte`,
  optional `backend`, `expected.txt` = diagnostics with severity and help, then the module):
  `element-bare`, `component-bare`, `duplicate-element`, `duplicate-component`, `modifiers`,
  `spread-and-dynamic`, `snippet-not-hoisted`, `egui-element` (egui spec). Plus: the note is
  reported once and nothing else; a bare `on:` with an unknown event is `unknown_event`.
  `UPDATE=1` rewrites the goldens.
- Runtime: fixture `fixtures/forward-events` (TestHost op log). `App` → `Panel` (bare `on:clicked`
  on `<Toolbar>`) → `Toolbar` (bare `on:clicked` on `<button>`): the grandparent's handler gets
  the click (`Count: 1`), swaps itself and the next click reaches the new one (`Count: -9`); a
  second `Panel` without a handler ignores its click.
- svelte-check: `packages/native-runtime/check` (above).
- Conformance: the samples with bare `on:`/component `on:` (`event-attribute-delegation-4`,
  `-5`, `event-attribute-spread-collision`, `event-listener-spread`, `event-on-2`,
  `dynamic-event-handler-3`) stay rejected as `legacy`. Three are now rejected by
  `duplicate_event` (Svelte 4 fan-out: `on:click` and `onclick` both on one component), so their
  `expected.json` entries carry `match: ^duplicate_event$` and a note; the legacy category also
  matches the new modifier message. The other three still hit `<slot>`. No score change, so no
  baseline update.

## 5. Decisions made inside the spec

1. `duplicate_event` is an **error**, not a warning: the result would otherwise depend on attribute
   order in a way legacy code did not expect.
2. A second **bare** `on:x` on the same element is also `duplicate_event` (it would connect twice).
3. On a **component**, `on:x={h}` together with `onx={…}` (or two `on:x={…}`) is
   `duplicate_event` too, though §8.7 only names the bare case: as props, the later silently
   replaces the earlier, while Svelte 4 ran both.
4. The note is a new **`note` severity** rather than a warning: it never fails `--strict-native`
   or a warnings-clean build, and the Vite plugin does not print it on every build.
5. The bare listener is **always connected** and reads the prop when the event fires (Svelte's
   member-expression handler shape), rather than connecting only when the parent passes a handler.
6. `<svelte:element>` and runtime spreads forward through the spread object
   (`onclicked: $$props.onclicked`); the runtime's spread already treats a nullish handler as
   "disconnect".
7. Typing is opt-in through `Forward`/`ForwardFrom` (section 3), since svelte2tsx owns props types.

## 6. Open questions (recommendation taken)

1. **Automatic typings.** The callback prop could be added to the component's props type
   without the author writing `Forward<…>`, but only by changing what svelte-check sees (a
   svelters check wrapper that patches svelte2tsx output, or the future native checker,
   `docs/notes/egui-all-rust.md` §2.2). Recommendation: keep the opt-in helpers now; revisit with
   the LOADER/COMPILER tracks' own checker. A cheap middle step: a warning when a component with a
   typed `$props()` destructuring (no rest element) forwards a prop it does not declare.
2. **`mixed_event_handler_syntaxes`.** Svelte 5 rejects mixing `on:` and `onx` syntax in one
   component; svelters never ported that check and FW1 relies on mixing being allowed (cadatomic
   writes `on:clicked` next to `onclick={…}`). Recommendation: do not port it.
3. **Egui typings.** There is no `egui.d.ts` yet (RA1 dumps `IntrinsicElements`), so `Forward<'button',
   'clicked'>` resolves against the GTK superset today. Recommendation: nothing to do in FW1; RA1's
   dump makes it backend-correct.
4. **The payload type in the note's help.** §8.7's typing column says "payload from the kind's
   signal"; the help names the helper type instead of spelling the signature out, since the
   handler types come from the generated typings. Recommendation: keep.

## 7. Not run locally

- The full compiler test suite and the workspace nextest (CI). Locally: only
  `cargo nextest run -p svelters-compiler --test fw1_forwarding`.
- The full conformance run: only the six affected samples (`node run.mjs …`).
- The other fixtures and package tests: only `fixtures/forward-events` (`pnpm test`) and
  `packages/native-runtime` `pnpm check`.
- No backend (GTK/Qt/egui) tests: the change is compiler-side and the runtime path is the existing
  `$n.on` / `$n.spread`.
