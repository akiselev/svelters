# NT0 handoff (N0 GTK pilot of plans/research/NON-GUI-TYPES.md)

Branch `worktree-agent-ab68746002833b005`, worktree `/projects/svelters/.claude/worktrees/agent-ab68746002833b005`. Not merged with master; not pushed; no CI run yet.

## Done

- **ABI v2.5** (additive): `SVL_LIFE_SHARED=5`, `SVL_T_TYPE=11`, `SVL_M_STATIC=16`, `SVL_M_CTOR=32`, nullable `construct` / `call_static` members, `describe(SVL_KIND,0,kind)` (qualified name + nearest ancestor kind). Header, Rust mirror, safe wrapper (`Backend::supports_construct/construct/call_static`), Qt template passes explicit `nullptr`. AppKit/WinUI use the trait defaults. Docs: `plans/architecture/backend-abi.md`, `docs/site/.../concepts/abi.md`, `ownership.md`.
- **Host**: ops `construct`, `callStatic`, `typeChain` (`crates/svelters-host/src/core/statics.rs`, `quickjs.rs`); VALUE handles remember their kind. TestHost (`packages/testhost`) and `svelters-backend-log` log `construct #h Kind {json}` and `static Kind.method [json] [#h]`; `tools/oplog-compare.mjs` and `oplog_replay.rs` understand them.
- **GTK backend**: every GObject is a handle (widgets OWNED/BORROWED, others SHARED, strong ref); reflected kinds/props/signals (`src/reflect.rs`, ids `KIND_BASE=0x10000`, `DYN_BASE=0x1000000`); `construct` via `g_object_new_with_properties`; `call_static`; VALUE handles with kinds and methods. `gi.rs` rewritten: all 8 typelibs, receiver-GType dispatch (incl. interfaces of unknown runtime classes), out/inout, caller-allocates structs, GList/GSList, C arrays (hidden length args), GType, unichar, statics/ctors.
- **Tooling** (`tools/metadata/gtk/`): `types_facts.py` (promoted classifier, scope closure, mutator hide rules, `--report/--write/--check`, facts in `metadata/gtk/types/*.json` + `report.txt`, overrides `metadata/overrides/gtk/types/gtk-types.yaml`), `gen_types.py` + `gen_types_extra.py` (modules, typings, manifest, docs pages, ledger facts, generated tests), `types_closure.py`, `test_types.py` (12 pytest).
- **Q9 (ES module named exports)**: `packages/types` (`@svelters/types`, `sideEffects:false`): one file per class under `gtk/<ns>/`, `index.js` re-exports, no registration, import cycles containing `extends` are avoided by the generator, `gtk/manifest.json` carries imports + signature, closure computed by `types_closure.py` (tested). Aliases `svelters:gtk/<ns>` via `packages/types/vite.js` in `packages/build` and `native-runtime/vite.js`. Rust compiler path works (`experiments/native-compiler/compiled` case `gtk-types`, state-equal).
- **Curated set** (10 types) has generated tests `crates/svelters-gtk/tests/types_verify.rs` (50 tests, 5 fact groups each) and docs pages `docs/site/src/content/docs/types/gtk/`. Scope: 301 bound classes (131-type closure plus Gtk non-widgets and roots); members bound 80-97% per namespace (see `metadata/gtk/types/report.txt`).

## Verified

- `cargo nextest -p svelters-gtk` full run before the last test edits: 595 tests, 6 failures that were all test expectations of the old BOXED/borrowed model (borrowed.rs x2, gi_calls.rs x2, serdes.rs x1) plus `lists_e2e` (fixture dist not built). The first four were then updated and re-run green together with host_types, types_core, oplog_replay (39/39). `types_verify` 50/50 green.
- `cargo nextest -p svelters-abi -p svelters-host -p svelters-backend-log` green (incl. new `construct.rs`, `types_ops.rs`, fixtures). `cargo check --workspace --all-targets` clean.
- `pnpm --filter @svelters/types test` (12 node tests incl. vite tree-shaking proof + tsc strict on the typings) green; pytest `test_types.py` green; compiled-experiment case `gtk-types` passes.

## NOT done / not run

- Full gates not run: `pnpm -r build`, full `cargo nextest --workspace` under xvfb, `pnpm test`, `pnpm docs:test`, ledger `refresh`.
- `python3 tools/metadata/ledger.py refresh --toolkit gtk` must run (new facts `svelters-api:v1:type:gtk:*` exist in `metadata/gtk/types/facts.json`; the ledger hook is `gtk_type_facts()` in `ledger.py`) and `metadata/verified/gtk.json` committed. `ledger.py check` fails until then.
- Merge master (AI1 edits `probe.py`, `infer.py`, ledger, generated verify tests; my ledger edit is one function) and push a `ci/**` branch for the 4.14 job.
- 4.14 hardening of generated tests: `types_verify.rs` calls zero-arg readers/props/signals; members with a `since` newer than the running lib will fail with "unknown method". Plan: in `gen_types_extra.py`, skip (eprintln SKIP) when the member has `since` and the error says unknown method/property/signal. Other new code uses only typelib lookups, no newer symbols.
- Add `test:meta`/docs gates: nothing wired into root `package.json` yet (pytest picks up `test_types.py` automatically). Docs build (`docs:build`, link validator) not run; sidebar entry added in `astro.config.mjs`.
- Flaky once: `types_core.rs:18` ("expected a handle") in the full run; passed on retry; not reproduced. Suspect `Gdk.Display.get_default`/xvfb start-up.
- Out of scope per task: async/callbacks (N1), Qt (N2), reactive getters, rich text. Fundamental classes (GskRenderNode, GdkEvent), GVariant, hash tables, GArray/GPtrArray are blocked with reasons in facts. Rich text deferred.
- No `ingest.py --namespace`: classification reads the GIR directly (deviation from plan section 9; facts are the checked-in product).
- TS typings: members whose signature conflicts with an ancestor's are typed `(...args: any[]): any` (2 today).

## Next steps

1. Merge master; `pnpm install`; `pnpm -r build`; run all gates; `ledger.py refresh --toolkit gtk`; commit ledger.
2. 4.14 guard for generated tests (above); push `ci/nt0` and check the 4.14 job.
3. Review `host.typeChain`/`describe(SVL_KIND)` naming and the `new/new_*/from_*` static-handle convention of TestHost/log backend.
4. Report coverage: curated 10/10; bound 301 types (`report.txt`).

## Key decisions / traps

- SHARED non-widgets: `create(kind)` (element path) still gives OWNED for non-widget kinds; `construct` gives SHARED.
- Wrapper class is the *declared* class of the signature (no registry, Q9); `wrapObject` upgrades the prototype when a more specific declared class is later seen; `Class.from(obj)` is an unchecked cast.
- Dropped wrappers are released by the engine's GC (renderer node finalizer -> `destroy`): op logs are only comparable across hosts while the app keeps what it made (see `fixtures/gtk-types/src/App.svelte`). `tests/gtk-types-gc` proves release on the real host (not an op-log fixture).
- Tooling traps: rolldown removes `void [a,b]`; the sandbox refuses some heredoc shell commands, so edit via scripts; `types_facts.py --write` must not delete `facts.json` (excluded).
- Facts depend on the installed GIR (4.22); do not run `types_facts.py --check` in CI (pytest does so only with `SVELTERS_CHECK_GIR=1`).

## NT0b update (wrap-up, quota)

Done since e0e1ac41: master merge was a no-op (already contained); 4.14 `since` guard in `gen_types_extra.py` (+ `Harness::has_prop/has_signal`), tests regenerated; docs sidebar entry fixed for Starlight 0.42 (`items: [{ autogenerate }]`) and generated frontmatter descriptions quoted; `pnpm -r build` passes (after `python3 tools/docs/build_ir.py`); `ingest.py --namespace` not done (reason in nt0-types-pilot.md).

Not run: ledger refresh (killed mid-run; `ledger.py check` still fails with "not in the ledger"), cargo nextest workspace, `pnpm test`, `pnpm docs:test`, CI push. A `cargo check -p svelters-gtk --tests` was queued but its result is unverified.

Next steps:
1. `tools/dev/cargo-slot cargo check -p svelters-gtk --tests`; then `python3 tools/metadata/ledger.py refresh --toolkit gtk` (slow under load; runs lib verify tests + test bins), commit `metadata/verified/gtk.json`.
2. `tools/dev/cargo-slot xvfb-run -a cargo nextest run --workspace --no-fail-fast`; `pnpm test`; `pnpm docs:test`.
3. Push as `ci/nt0-types`; check linux-gtk (4.14), linux-qt, macos, windows; known flakes listed in the task.
