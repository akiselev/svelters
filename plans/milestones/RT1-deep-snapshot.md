# RT1: deep plain copy of `$state` prop values

## Bug

From cadatomic's K1 `<code-editor>`: the native runtime's `set` copied only a top-level `$state`
array (WG5a). A nested array in a record prop (`selection = { ranges: [{ anchor, head }] }`) reached
the host as a Svelte Proxy, which the host's list conversion does not read as an array:
`<code-editor>.selection.ranges: expected array, got object` (`crates/svelters-host/src/convert.rs`,
`expected`). cadatomic worked around it with `$state.raw`. A second, latent bug: a nested mutation
(`selection.ranges[0].head.line = 3`) re-ran nothing, and had it re-run, the identity check
(`Object.is(last, value)` on the same proxy) would have dropped the write.

## Fix

`packages/native-runtime/src/index.js`:

- `plainData(v)`: copies every `$state` proxy in `v`, deeply (arrays and plain objects, as
  `$state.snapshot` does). Other objects (handles, assets, Dates, typed arrays) go through as they are,
  where `$state.snapshot` would `structuredClone` them. A value with no proxy in it is returned
  uncopied, and scalars return at once. Reading the proxies inside the template effect tracks their
  nested signals, so a nested mutation writes the prop again.
- `set` sends `plainData(value)`. When a copy was made it compares with the last value by content
  (`sameParam`), so an equal copy is not written twice. Otherwise it compares by identity, as before.
  The WG5a top-level `Array.from` is gone, since `plainData` covers that case.
- EV2b behaviour params (`hs.setProp` in the behaviour action) go through `plainData` too.

Proxy detection uses Svelte's `STATE_SYMBOL` (`is_state_proxy` in `src/core/svelte.js`, re-exported by
`src/core/internal.js`). For that, `svelte/internal/client/constants` becomes a hidden module: an alias in
`package.json`, `CONSTANTS_SPECIFIER` in `vite.js` `HIDDEN_MODULES`, and a check in
`tools/check-svelte-internals.mjs`. Decided by the lane: detecting proxies keeps plain values uncopied,
which copy-everything would not do.

## Test

`packages/native-runtime/test/runtime.test.mjs`, "RT1: a record prop from $state reaches the host as
plain data, nested mutations included". It covers a `$state` record holding an array of objects with
objects inside, and an array inside an object inside an array. It checks that no Proxy reaches the
TestHost at any depth, that a nested field mutation and a nested `push` write the prop again as plain
data, that a plain value with a handle object inside goes through uncopied, and that an equal new
proxy writes nothing.

## Run locally

- `node tools/check-svelte-internals.mjs`
- `node --import ./test/register.mjs --test test/runtime.test.mjs test/behaviors.test.mjs test/spread.test.mjs test/lists.test.mjs` (35/35)
- `pnpm run check` (svelte-check, 0 errors)

## Not run

The rest of the native-runtime test files, the other packages, conformance, and all Rust builds and
tests. CI runs them.
