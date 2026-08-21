# SVR22 - upstream Svelte/toolkit compatibility automation

## Goal

Make ongoing maintenance against fast-moving Svelte, Vite, QuickJS-NG, GTK and Qt an explicit automated product capability rather than periodic archaeology.

## Svelte update bot/agent workflow

For each candidate Svelte update:

1. update exact package/commit pin;
2. diff the tracked `svelte/internal/client` overlay inputs;
3. compile/run the native runtime fixture corpus;
4. classify new upstream exports/DOM assumptions;
5. run TestHost, GTK, Qt and egui smoke suites;
6. run HMR/source-map tests;
7. produce compatibility dashboard delta;
8. open an update PR with generated evidence.

Agents may update overlay code, but every semantic adaptation needs a fixture. Never accept a bulk copied runtime change without explaining the native impact.

## Toolkits

Qt update workflow regenerates the Shiboken API DB, runs `api diff`, identifies new/changed ownership/typesystem semantics, rebuilds the fat bridge, and tests thin manifests.

GTK update workflow regenerates GI API DB, diffs introspection/invocation classification and reruns dynamic/thin bridge tests.

QuickJS/rquickjs and Vite/Rolldown/Oxc updates run syntax/runtime/build-output/source-map compatibility suites.

## Acceptance

Run the automation against at least one intentionally different prior/current version of Svelte, Qt or GTK and produce a human-readable compatibility PR artifact. Version bumps that change stable API semantics cannot pass without an acknowledged diff.
