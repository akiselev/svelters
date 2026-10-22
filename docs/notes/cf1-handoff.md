# CF1 handoff (Svelte conformance)

Branch `ci/cf1`. Lane record (result, expected rejections, fixes, failing buckets with effort,
the template-effect decision, CI, conflict risks): `plans/milestones/CF1-conformance.md`.
origin/master (E2, E3, FU2) is merged in (`1e3af8a8`).

## Score

**565 / 774 pass**, 161 expected rejections, 45 failing, 3 skipped (before: 500). `baseline.json`
holds the 565. No sample that passed before fails now.

## How to run conformance

The upstream samples come from the GitHub tarball of the tag, extracted into a dir of its own:

```
mkdir -p $UP && curl -sSL https://codeload.github.com/sveltejs/svelte/tar.gz/refs/tags/svelte@5.57.2 -o $UP/svelte.tgz
mkdir $UP/svelte && tar -xzf $UP/svelte.tgz -C $UP/svelte --strip-components=1
```

1. Build the compiler: `tools/dev/cargo-slot cargo build -p svelters-compiler`.
2. In `experiments/native-compiler/conformance`:
   `UPSTREAM=$UP TIMEOUT_MS=60000 JOBS=8 node run.mjs --check`.
   - It takes about 10 minutes, longer under load.
   - Run it on its own, not alongside nextest.
3. Filtered runs (`node run.mjs name-substr...`) print per-sample results, with `{expected: <category>}`
   on expected rejections. They don't touch `results.json`, `REPORT.md` or the baseline.
4. `--update-baseline` grows `baseline.json`. It refuses to drop a passing sample.

## Expected rejections

`expected.json` lists them: categories with `why`, `ref` and `match`, and per-sample entries with
their own `match` where the assertion message is generic. A listed sample that passes or fails
differently is reported as stale and counts as failing. Only list a sample that needs a feature
outside the native subset or design, never one that fails for a fixable reason.

## Next steps

1. **F-dev-warn (8): ownership warnings.**
   - Emit Svelte's dev component identity in the compiler: `Component[$.FILENAME] = 'file'` and
     `$.push($$props, true, Component)`.
   - Then emit `$$ownership_validator = $.create_ownership_validator($$props)` with `.mutation(...)`
     around prop member mutations, and `.binding(...)` for `bind:`. Compile a sample with stock
     `svelte/compiler` (`dev: true`) to see the exact shape.
   - Then `$.validate_binding` for `binding_property_non_reactive`.
2. **R-spec / R-style-dir (12)**: add the missing props/events to the dom test spec
   (`metadata/semantics/dom`), one by one.
3. **F-order (7)**: waits for the op-log owners' decision (milestone section 6).
4. **F-inspect-stack (13)**: hide the native dispatch frames from Svelte's `get_stack`. Low value.

## Gotchas

- Name scratch files uniquely. This session's scratchpad directory is shared with other lanes:
  `nextest.log` collided with another lane's log.
- `pnpm` only works through corepack, via `/tmp/claude-1000/pnpmshim`.
- `experiments/native-compiler/compiled/test.mjs` needs `pnpm` on PATH.
