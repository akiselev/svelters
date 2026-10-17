# Weekly Svelte fork refresh (agent procedure)

Goal: keep `vendor/svelte/` current with the upstream custom-renderer stack and `main`, and tell the owner exactly what changed. One merge, one test run, one report. Stop and ask on anything not listed as mechanical.

Background: `docs/notes/custom-renderer-trial.md` (§2 stack, §7 maintenance). Current pin: `vendor/svelte/PIN`.

Budget rules (global CLAUDE.md): no subagent fan-out for this job, builds and test runners capped at 4 cores (`--network-concurrency=4`, `--workspace-concurrency=4`, no more than 4 vitest workers).

## 0. Setup (first time only)

Work outside the repo checkout, in the scratchpad or `~/src`:

```sh
git clone --filter=blob:none https://github.com/sveltejs/svelte.git svelte-fork
cd svelte-fork
git remote rename origin upstream
git checkout -b svelters/renderer <SHA in vendor/svelte/PIN>   # first time: the pinned commit
```

Later runs reuse the clone and the `svelters/renderer` branch (merge-based history, any local fix commits stay on top, each linked to an upstream issue or PR).

## 1. Fetch and record

```sh
git fetch upstream main svelte-custom-renderer fix-interleaving add-custom-renderer-flag custom-condition
git rev-parse upstream/main upstream/svelte-custom-renderer upstream/fix-interleaving \
              upstream/add-custom-renderer-flag upstream/custom-condition
```

Write the five SHAs into the report. Also read the PR threads for #18042, #18405, #18461, #18511 (`gh pr view <n> --comments`, `gh pr view <n> --json state,mergedAt,headRefOid`) for new comments, force-pushes, renames of renderer methods, or a merge.

## 2. Did upstream land it?

- If #18042 (or the whole stack) is merged into `upstream/main`, stop and report: the next step is to switch to an npm release and retire the fork, which is a decision for the owner.
- If `upstream/custom-condition` moved, merge it first: `git merge upstream/custom-condition` (it is usually already refreshed against `main`).

## 3. Merge main, no rebase

```sh
git merge-tree --write-tree --name-only HEAD upstream/main     # conflict preview; exit 0 means clean
git merge upstream/main
```

Resolve only mechanical conflicts: import lists, renamed helpers, and the generated files (`packages/svelte/src/internal/{client,shared,server}/errors.js`, `messages/**/*.md`, `documentation/docs/98-reference/*`), which you regenerate with `node scripts/process-messages` instead of hand-merging. Expected hot spots: `dom/elements/attributes.js`, `dom/blocks/boundary.js`, `svelte-element.js`, `events.js`, `operations.js`.

Stop and report (do not resolve) when a conflict touches renderer semantics: `custom-renderer/`, `push_renderer`/`current_renderer` use, `insert`/`remove` ordering in `branches.js` or `each.js`, or anything where both sides changed logic rather than text. Report the file list and the conflicting hunks; leave the merge aborted (`git merge --abort`).

## 4. Build

```sh
pnpm install --frozen-lockfile --network-concurrency=4
cd packages/svelte
npx rollup -c && node scripts/process-messages && node scripts/generate-types.js
```

(The full `pnpm build` also runs the slow `generate:browser-support` scan; not needed for the tarball.)

## 5. Upstream renderer suite

```sh
cd packages/svelte && pnpm vitest run --maxWorkers=4 tests/custom-renderers
```

Must be green (at `ece83dd`: 4 files, 87 tests; the count only goes up). Also run `pnpm vitest run --maxWorkers=4 tests/snapshot` once, since render-path changes show up there. Failures are reported, never skipped.

## 6. Pack the candidate tarball

```sh
cd packages/svelte
pnpm pack --pack-destination "$SCRATCH/candidate"
mv "$SCRATCH/candidate/svelte-"*.tgz "$SCRATCH/candidate/svelte-<version>-<shortsha>.tgz"
```

`<version>` is `packages/svelte/package.json`; `<shortsha>` is `git rev-parse --short=7 HEAD` of the fork branch.

## 7. Run our conformance and fixtures against it, in the svelters worktree

Use a throwaway branch in a svelters worktree, never master.

1. Copy the candidate into `vendor/svelte/`, update the `overrides.svelte` path in `pnpm-workspace.yaml`, run `pnpm install --network-concurrency=4`.
2. Renderer conformance, Node + qjs (obtains quickjs-ng v0.17.0 into `tests/renderer-conformance/.qjs/` on first use, sha256-checked, via `get-qjs.sh`):

   ```sh
   cd tests/renderer-conformance && node run.mjs
   ```

   It builds prod, dev, values2 and hmr bundles, runs each under Node and qjs, compares Node vs qjs, and compares with `expected/*.txt` after stripping node ids and read counters. A failure prints the first differing line.
3. Host-level fixtures on both paths (op logs must stay byte-identical on both):

   ```sh
   pnpm test                              # renderer path (default)
   SVELTERS_BACKEND=shim pnpm test        # DOM-shim fallback
   ```

4. Op-stream diffs: for each failing run, show `diff expected/<run>.txt out/<run>.txt` (after `node run.mjs --update` writes `expected/` from the new build, `git diff tests/renderer-conformance/expected` is the diff to report). **Any diff is reported, not auto-accepted.** The only changes you may adopt without asking are node-id renumbering with identical structure (the runner already tolerates those and prints a note).

## 8. Publish (only if steps 3 and 5 passed and the op streams are unchanged or the owner accepted the diff)

- `git rm` the old tarball, add the new one, update `vendor/svelte/PIN` (all five SHAs, tarball name, sha256 via `sha256sum`, date), and the `overrides.svelte` path.
- Commit on a branch `svelte-refresh-YYYY-MM-DD`. Do not merge to master.

## 9. Report (always, even when nothing changed)

- Upstream SHAs before and after (five branches), and whether any stack PR merged or closed.
- Merge: clean or the conflict count and file list; which were mechanical.
- Upstream renderer suite result (files/tests), snapshot result.
- Conformance result per run (prod, dev, values2, hmr; Node and qjs), fixture results on both backends.
- Op-stream diffs, verbatim, with a one-line cause if obvious.
- New tarball name and sha256, bundle size change of `fixtures/counter/dist/app.js`.
- Thread summary for #18042, #18405, #18461, #18511.
- Whether the two upstream bug reports in `docs/notes/svelte-fork-upstream-bugs.md` are still reproducible (re-run their repros) or fixed.
