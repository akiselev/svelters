# Upstream bug reports for the Svelte custom renderer stack (not filed)

Found against `custom-condition` @ `f3de3f236bd586c764fd48bab04f41c54c5ff80d` (top of the #18042 stack, `svelte` 5.57.2; see `vendor/svelte/PIN`). Both reproduce on #18042 @ `ece83dd` as well (trial: `custom-renderer-trial.md` §5, §7.5). Re-verify against the current stack head before filing; `tests/renderer-conformance/WEEKLY.md` step 9 does that each week.

Suggested repo/target: `sveltejs/svelte`, label with the custom renderers PR (#18042) in the text; the bugs are in code that only that PR adds or changes.

---

## (a) Compiler crash: inline `failed`/`pending` snippet in `<svelte:boundary>` with a custom renderer and `experimental.async` off

**Title:** `TypeError: Cannot read properties of undefined (reading 'body')` compiling `<svelte:boundary>` with an inline `{#snippet failed()}` when `experimental.customRenderer` is set

**Repro** (Node, in any project that has this branch's `svelte`; run with `--conditions=custom-renderer`, or just import `svelte/compiler`, which has no conditional exports):

```js
import { compile } from 'svelte/compiler';

const src = `<svelte:boundary>
  {#snippet failed(error)}<p>failed</p>{/snippet}
  <p>ok</p>
</svelte:boundary>`;

for (const experimental of [undefined, { customRenderer: true }]) {
  try {
    compile(src, { generate: 'client', experimental });
    console.log(JSON.stringify(experimental), '-> compiles');
  } catch (e) {
    console.log(JSON.stringify(experimental), '->', e.constructor.name + ':', e.message);
  }
}
```

**Actual:**

```text
undefined -> compiles
{"customRenderer":true} -> TypeError: Cannot read properties of undefined (reading 'body')
```

`{#snippet pending()}` fails the same way. Passing the snippet as an attribute (`<svelte:boundary {failed}>`) compiles.

**Cause:** `packages/svelte/src/compiler/phases/3-transform/client/visitors/SvelteBoundary.js` (~line 93):

```js
const snippet_fn = dev
  ? snippet.declarations[0].init.arguments[1]
  : snippet.declarations[0].init;

if (!context.state.options.experimental.async) {
  snippet_fn.body.body.unshift(...const_tags...);   // <- snippet_fn has no .body
}
```

With a custom renderer the snippet declaration is wrapped as `renderer_snippet(<fn>)` (a `CallExpression`), so in prod `snippet.declarations[0].init` is the call, not the function, and `.body` is `undefined` (in dev the `arguments[1]` index likely points at the wrapper's argument list too). The `const_tags` unshift needs the unwrapped function: take the function from the wrapper's arguments when `init` is a `renderer_snippet` call. A regression test next to `tests/custom-renderers/samples` using the repro above (compiles, renders `ok`, and renders `failed` after a throw) would cover it.

**Expected:** compiles and behaves like the DOM output.

---

## (b) `set_attributes` (spread) silently drops function-valued props under a custom renderer

**Title:** `{...spread}` on an element drops non-`on*` function props instead of passing them to `renderer.setAttribute`

**Repro** (uses the harness in `tests/renderer-conformance`; `renderer.mjs` there is a ~190-line renderer that logs every call, any renderer that logs `setAttribute` shows it):

`SpreadFn.svelte`:

```svelte
<script>
  const rest = { label: 'x', model: { rows: 1 }, cb: function cb() {} };
</script>
<box {...rest} />
```

Mount it with a custom renderer (`mount(App, { target, renderer })`) and log the `setAttribute` calls.

**Actual:**

```text
set #4<box>.label = "x" [string]
set #4<box>.model = object {"rows":1} [object]
```

There is no `set ... cb` line: the function never reaches the renderer. The same value passed without a spread (`<box cb={fn} />`) arrives intact as `fn cb [function]` (non-spread attributes go through `set_attribute` and pass any value to the renderer).

**Cause:** `packages/svelte/src/internal/client/dom/elements/attributes.js`, end of the `set_attributes` loop (~line 523):

```js
} else if (
  is_default ||
  ((is_custom_element || typeof value !== 'string') && setters.has(name))
) {
  element[name] = value;
  ...
} else if (typeof value !== 'function') {   // <- guard exists for the DOM (a function attribute would stringify)
  set_attribute(element, name, value, skip_warning);
}
```

The `typeof value !== 'function'` guard protects the DOM from `setAttribute('x', 'function...')`, but with `current_renderer != null` the value is passed through intact elsewhere, so spread and non-spread differ. Suggested fix: skip the guard when a custom renderer is active (`current_renderer != null || typeof value !== 'function'`). Pairs with a sample in `tests/custom-renderers/samples` asserting the spread path calls `setAttribute` with a function.

**Impact:** wrapper components that forward `...rest` containing callbacks (our `Entry.svelte` pattern) lose them silently. Workaround: pass callbacks as `on*` props or explicit attributes.

**Expected:** spread and explicit attributes behave the same for a custom renderer.
