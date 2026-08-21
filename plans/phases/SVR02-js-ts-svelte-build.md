# SVR02 - JS/TS/Svelte build pipeline

References: `plans/architecture/runtime-stack.md`, `plans/architecture/native-binding-linker.md`.

## Goal

Replace every old application-compilation path with the modern Svelte/Vite toolchain before embedding it in QuickJS.

## Work

- Pin current Svelte 5 and official `@sveltejs/vite-plugin-svelte`.
- Use Vite 8/Rolldown/Oxc. Delete/retire `src/esbuild-svelte.ts`, `build.js`, `build-esbuild-svelte.js`, `esbuild-svelte`, and equivalent manual rewrites from the active build.
- Configure `generate: "client"` and `fragments: "tree"`; enable Svelte HMR in development.
- Configure `vitePreprocess({ script: true })` for TS constructs requiring emitted JS.
- Set modern `tsconfig`: `verbatimModuleSyntax`, `isolatedModules`, strict mode, explicit QuickJS-compatible output target selected by a runtime syntax fixture.
- Use TypeScript 7 for `.ts` checking. Add the TS6 compatibility alias required by the current `svelte-check` TS7 integration; run `svelte-check --tsgo` in CI and track its upstream removal condition.
- Build a small custom Vite environment/plugin skeleton for `svelters` but initially emit runnable Node/TestHost artifacts to validate compilation.
- Make source maps mandatory.
- Establish virtual module namespaces for `svelters:host` and `svelters-bind:*` even before native binding resolution exists.

## Delete rather than port

The old `lib/compiler` SWC TypeScript stripper and embedded Svelte compiler are not upgraded. Preserve any uniquely useful tests, then remove them from the active dependency graph.

## Acceptance

- A Svelte 5 rune-based counter and components using `if`, `each`, snippets, props and TypeScript compile through Vite.
- No active dependency on esbuild 0.13, Svelte 3, TypeScript 4, old SWC crates, `node-resolve`, or the local Svelte fork.
- `pnpm check` runs TypeScript/Svelte diagnostics separately from bundling.
- Build output contains structured `from_tree` templates and complete source maps.
