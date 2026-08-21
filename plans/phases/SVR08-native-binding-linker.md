# SVR08 - native binding linker

References: `plans/architecture/native-binding-linker.md`, `plans/architecture/native-api-ir.md`.

## Goal

Implement Q1 end to end: statically identify native calls, preserve them as linker-visible virtual imports through JS optimization, and emit a minimal release binding manifest after Rolldown DCE.

## Work

Build `svelters-bind-resolve` plus Vite/Rolldown plugin integration.

1. Resolve native member access/calls against generated declarations and API IR.
2. Model inheritance, overload resolution, nullable values, enums/flags, optional/default args, callbacks, and explicit disambiguation.
3. Lower exact calls to `svelters-bind:<backend>:<stable-id>` virtual external imports.
4. Run stage-A optimization/tree shaking.
5. Collect only surviving binding imports.
6. Merge explicit roots from `svelters.toml`.
7. Expand type/converter/adapter dependencies.
8. deterministically assign dense IDs and emit the release manifest.
9. Run stage B to bind virtual calls to the generated/native dispatch table.

Add `svelters bindings {trace,explain,unused,diff}`. Define development trace records that connect runtime binding use back to static/manual reasons.

## Manual/dynamic policy

Dynamic property access does not silently force the entire toolkit API into release. The resolver reports it and requires an explicit root/pattern or a deliberately dynamic native API wrapper. `--strict-bindings` fails unresolved runtime observations in CI.

## Acceptance

Golden fixtures prove:

- unreachable native calls vanish from both JS output and native manifest;
- inherited/overloaded calls select the expected canonical IDs;
- manual roots work without source callsites;
- wildcard roots report expansion size;
- adding dead code does not change the native manifest hash;
- manifest ordering/IDs are reproducible across machines given the same API DB/build inputs.
