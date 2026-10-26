# Lane COMPILER: svelters-compiler integrations (ids, passive, templates, change classes)

Branch `ci/presto-compiler`. Owns `crates/svelters-compiler/**`, a new `packages/native-runtime/src/template.js`,
and the spec generator's id/fingerprint output for egui (`crates/svelters-egui/src/spec.rs` and the kind
tables' stable ids — coordinate with lane RUNTIME on `packages/native-runtime/src/index.js`: you add calls to
new exports; RUNTIME owns the file's existing functions). Read: `plans/research/presto/compiler-and-js.md`
§2–§4; notes `round2/compiler-integration-local.md`, `round2/compiler-prior-art.md`.

Every change is a **backend capability** (spec `capabilities`): the DOM conformance backend and GTK/Qt/AppKit/
WinUI keep today's output. Conformance (628/774), `core-contract.test.mjs` and the 50 byte-exact
`expected.oplog` files must not change. CI runs them.

Deliver (push after each):
1. **Stable ids (W-C1):** the spec gains stable ids for kinds, props, signals, methods, slots (non-positional:
   assigned once, kept in a checked-in id table per backend, new rows appended; a `fingerprint` per id and a
   spec hash). egui's `spec.rs` emits them. With capability `ids`, codegen emits integer constants
   (`$n.element(K_LABEL)`, `$n.setp(n, P_LABEL, v)`); the host checks the spec hash at load; op logs keep
   names (the host maps back).
2. **Passive inference (W-C5):** a listener is passive when its handler has no parameter or its event
   parameter never escapes and is never the receiver of `preventDefault`/`stopPropagation`/
   `stopImmediatePropagation`; `|passive`/`|nonpassive` override (warn on contradiction); emit
   `LISTEN_PASSIVE` (= 4) in `listen_ex` flags; spec fields `passive`/`cancelable`; diagnostic
   `nonpassive_scroll_listener` (row in `codes.rs`, `UPDATE=1` for the docs page).
3. **Native templates (W-C2), compiler side:** an `Item::Template` IR for a fragment's static prefix (kinds,
   parent index and slot, static props as typed constants, static classes, listener slots with handler
   indices, holes for blocks/components/render tags); codegen with capability `templates` emits a module-scope
   template table (content-hashed) and `$n.instantiate(T, [handlers…])` → base handle, `base + k` addressing,
   `$n.destroy_range` in teardown; dev builds keep per-node source offsets for DZ0. Runtime side
   (`template.js`): `instantiate` calls `host.instantiate` when the host has it, else expands to today's op
   sequence (so every backend works and op logs stay identical).
4. **Change classes (W-C4):** tag dynamic write sites, `class:` toggles (from the scoped rules a class can set)
   and `style:` directives with the spec's change class; dev warning when a layout-class prop is driven from a
   per-frame source (pointer move, scroll handlers).
5. **A11y diagnostics (W-C9):** from the spec's `a11y` fields (an input without an accessible name or
   `labelled-by`; an icon-only button without a label).

Tests (write, don't run locally): compiler unit tests per feature; template op-log expansion equals today's;
codes.rs rows.
