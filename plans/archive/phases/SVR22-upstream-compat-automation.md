# SVR22 - upstream Svelte/toolkit compatibility automation

## Goal

Make ongoing maintenance against fast-moving Svelte, Vite, QuickJS-NG, GTK and Qt an explicit automated product capability rather than periodic archaeology, without introducing a PR-based update queue.

## Direct-to-master candidate workflow

Automation evaluates each upstream candidate in an ephemeral worktree/branch or isolated CI checkout. It must not mutate `master` until all required gates pass.

For each candidate Svelte update:

1. update the exact package/commit pin in the candidate workspace;
2. diff the tracked `svelte/internal/client` overlay inputs;
3. compile/run the native runtime fixture corpus;
4. classify new upstream exports/DOM assumptions;
5. run TestHost, GTK, Qt and egui smoke suites;
6. run HMR/source-map tests;
7. produce a human-readable compatibility report/dashboard delta as a CI/Artifactum artifact;
8. if green and all semantic changes are acknowledged, commit/fast-forward the validated candidate directly to `master` and record the report identity in the commit/phase completion record;
9. if any gate fails, leave `master` unchanged and preserve diagnostics for the next agent/run.

Agents may update overlay code, but every semantic adaptation needs a fixture. Never accept a bulk copied runtime change without explaining the native impact.

## Toolkits

Qt update workflow regenerates the Shiboken API DB, runs `api diff`, identifies new/changed ownership/typesystem semantics, rebuilds the fat bridge, and tests thin manifests.

GTK update workflow regenerates the GI API DB, diffs introspection/invocation classification and reruns dynamic/thin bridge tests.

QuickJS/rquickjs and Vite/Rolldown/Oxc updates run syntax/runtime/build-output/source-map compatibility suites.

Toolkit/API DB changes that alter ownership, availability or stable API semantics require an explicit acknowledged compatibility report even when source signatures still compile.

## Concurrency and master safety

Only fast-forward a candidate built from the current master. If master moved while the candidate ran, rebase/recreate the candidate on the new head and rerun the affected gates. Never force-push master for automated compatibility maintenance.

## Acceptance

Run the automation against at least one intentionally different prior/current version of Svelte, Qt or GTK. Demonstrate both paths: a green candidate lands directly on master with a human-readable report artifact, and an intentionally incompatible candidate fails without changing master. Version bumps that change stable API semantics cannot pass without an acknowledged diff.
