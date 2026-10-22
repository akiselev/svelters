# CX1: the compiler as a diagnostics library (cadatomic SV27)

TRACKS §2.1 COMPILER, milestone CX1: check one file in process for diagnostics only (no codegen),
incremental state per file, fix-its, and namespaced, documented, stable diagnostic codes. It is
called by the module loader (Q1, `crates/svelters-modules`, through its overlay of unsaved buffers)
and by cadatomic's editor (quire / ED1), and is what SV33's language server will sit on (CX4).

Branch `ci/cx1`.

## 1. API

`crates/svelters-compiler/src/check.rs`:

```rust
pub fn check(path: &str, source: &str, backend: &dyn PlatformSemantics, state: &mut FileState) -> CheckResult;
pub fn check_with(path: &str, source: &str, backend: &dyn PlatformSemantics, opts: &CheckOptions, state: &mut FileState) -> CheckResult;

pub struct CheckOptions<'b> { pub strict_native: bool, pub dev: bool, pub portability: Vec<&'b dyn PlatformSemantics> }
pub struct FileState { .. }                 // one per file; new(), stats(), reset()
pub struct CheckResult { pub path: String, pub kind: FileKind, pub diagnostics: Vec<CheckDiagnostic>, pub reuse: Reuse, pub timings: CheckTimings }
pub enum FileKind { Component, Module, Other }          // by path: .svelte, .svelte.js, anything else
pub enum Reuse { None, Unchanged, CarriedOver }
pub struct CheckDiagnostic {
    pub id: &'static str,        // svelters::bind::partially_observed (stable)
    pub code: &'static str,      // bind_partially_observed (as before)
    pub severity: Severity,      // error | warning | note
    pub message: String,
    pub help: Option<String>,
    pub backend: Option<String>,
    pub range: Range,            // UTF-8 byte start..end + start_pos/end_pos
    pub fixes: Vec<Fix>,         // { message, edits: [{ range, text }] }
    pub stale: bool,             // carried over a syntax error
}
pub struct Position { pub line: u32, pub column: u32, pub utf16_column: u32 }   // all 1-based; column in UTF-8 bytes
```

- `backend` is any `PlatformSemantics`; `SpecBackend` (built-in spec plus RA1 fragments) is the
  usual one. New trait method `fingerprint() -> Option<u64>` (default `None`); `SpecBackend` hashes
  the JSON texts it was built from (spec, type table, every merged fragment).
- What runs: a `.svelte` file goes through the whole pipeline (parse, scopes, `<style>`, lowering
  against the spec, the rune transform) and then `codegen::component::traverse`, which walks the
  template like `generate` but into `Writer::sink()`: nothing is written and no source map is
  built. The walk has to run because the rune transform reports while it rewrites template
  expressions (`constant_assignment`, `bind_invalid_value`, `each_item_invalid_assignment`, ...).
  `compile::check_component` is that pipeline; its diagnostics equal `compile`'s (test below). A
  `.svelte.js` module runs `compile_module`. Other files: no diagnostics.
- `compile()`'s output is unchanged: `generate` is now `module()` + `finish()` with the same body,
  and the `cx1_check` corpus test plus the existing goldens compare it.

CLI (`svelters-compile`):

- `--check [--json]`: diagnostics only, printed as `file:line:col: error[svelters::element::unknown] (gtk): ...`
  with `help:` and `fix:` lines; `--json` prints the `CheckResult` above. Exit 1 on errors.
- `--explain <code | id>`: the registry row (id, code, severity, meaning, fix-it, svelte.dev link).
- `--list-codes`: `id code severity` per registered code.
- `compile --json` diagnostics gain `id` and, when there are any, `fixes` (byte edits); nothing
  else changes.

## 2. Incremental state

`FileState` keeps the last result and the last *complete* result (one that ran the whole pipeline):

1. **Unchanged input** (same path, source, options, and backend fingerprint): the previous result
   is returned and nothing runs (`Reuse::Unchanged`). This is the common case for an editor (cursor
   moves, focus, save, a re-request after a no-op edit) and for the loader checking a buffer the
   editor has already checked. A backend without a fingerprint is never reused.
2. **Syntax errors while typing.** The parser stops at the first error, so the semantic diagnostics
   are missing rather than fixed. The last complete check's diagnostics are carried over: mapped
   through the edit since then (common prefix and suffix, on character boundaries), dropped when the
   edit touches them, marked `stale`, and without fix-its (`Reuse::CarriedOver`). The editor keeps
   showing them, dimmed, instead of flickering. `CompileResult.timings.stopped` (not serialized)
   says the pipeline stopped early.
   The edit is found by diffing (longest common prefix and suffix), so an insertion that repeats
   the text around it can look like an edit of a neighbouring diagnostic, which is then dropped
   rather than shifted. An editor that knows its edit ranges could pass them; not needed so far.

Finer reuse (re-parsing only the edited region, keeping the scope analysis of an unchanged script,
caching spec lookups) was not built: the timings in §4 show that a full check of the largest
components in the repo fits the 20 ms per keystroke budget (cadatomic AP6) about three times over.

## 3. Codes

`crates/svelters-compiler/src/codes.rs` registers every code: short `code`, namespaced `id`,
origin, usual severity, a one-line summary and the fix-it offered. 172 codes: 60 of svelters'
own (`svelters::<area>::<name>`, areas element, property, method, event, bind, reactivity, slot,
child_attr, style, list, transition, feature) and 112 ported from Svelte (`svelte::<code>`, Svelte's
message template, link to `https://svelte.dev/e/<code>`). Fifteen of the Svelte ones are Q3's
`css_*` codes (`svelte/css`, merged while CX1 ran), registered after the merge. They reach `compile`
and `check` only as parse errors in HTML syntax (the DOM conformance spec); a native `<style>` is
parsed by `crate::style` (`svelters::style::*`), and Q3's scoping pass reports them to callers of
`compile_component_css`.

- Stability: an id is never renamed or reused. A code that stops being reported keeps its row
  (`style_unknown_declaration` is registered but not reported yet).
- Enforcement: the `cx1_codes` test scans `src/` for codes in reporting positions and fails on an
  unregistered one, or on a registered one it no longer finds. `Diagnostics::push` resolves the id,
  and in a debug build an unregistered code panics with a message naming `codes.rs`. **Other lanes
  adding a code must add a row** (and run `UPDATE=1` for the page).
- Docs: `docs/site/src/content/docs/guides/diagnostics.md`, generated by `codes::docs_markdown()`
  (`UPDATE=1 cargo nextest run -p svelters-compiler --test cx1_codes`; the test fails when the page
  is stale), in the site's Guides sidebar as "Compiler diagnostics".

## 4. Fix-its

A fix-it is a titled list of byte edits. It is attached where the diagnostic is reported, or
derived from the source text in `Diagnostics::finish` (`src/fixits.rs`) from a `FixHint` the
reporting site sets. A rule that cannot place its edit exactly offers no fix-it rather than a guess.

| Codes | Fix-it |
| --- | --- |
| `unknown_element`, `unknown_property`, `unknown_event`, `unknown_child_attr`, `unknown_method`, `bind_no_plan` | "did you mean": one fix-it per suggestion (at most 3) that replaces the name; for an element, the start and end tags; for `bind:x` shorthand, `bind:y={x}` |
| `read_only_property`, `bind_set_only`, `bind_not_observable` (constant), `nonreactive_redundant` | set it one way: `bind:x={v}` becomes the `` `prop={...}` `` the help names |
| `bind_modifier_invalid`, `transition_modifier_invalid` | remove the modifier |
| `unsupported_feature` (FW1 event modifiers) | the help's rewrite, when it is exact: a handler is given, no `\|once` or unknown modifier is left to the author, and no `\|capture` (the backend may have no capture phase: on GTK `onclickedcapture` is `listen_phase_unsupported`) (`on:clicked\|preventDefault={h}` becomes `onclicked={(event) => { event.preventDefault(); h(event); }}`; for a non-identifier handler, its text stays in place) |
| `style_unsupported_important` | remove `!important` |
| `svelte::block_invalid_elseif` | `elseif` becomes `else if` |

The `cx1_check` tests apply every fix-it in their inputs and in the corpus and check again: the
diagnostic's code is reported fewer times, and the first fix-it of each diagnostic adds no other
code. Later "did you mean" alternatives may name an element that does not fit there.

## 5. Timings

`cargo run --release -p svelters-compiler --example check_timing -- --backend <b> <files>`
(`crates/svelters-compiler/examples/check_timing.rs`): medians of 50 warm in-process runs, WSL PC
(16 cores), one build slot, under the machine's normal lane load (load average 8–10), 2026-10-22.
`keystroke` types one character per check into the file's last attribute value with one
`FileState`; `syntax error` is a buffer with `{1 + }` in it, with the carry-over.

| file | backend | bytes | compile | check | keystroke | unchanged | syntax error |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| tests/gtk-types-models/src/App.svelte | gtk | 6085 | 5.14 ms | 4.99 ms | 6.49 ms | < 1 µs | 1.12 ms |
| fixtures/lists-stress/src/App.svelte | gtk | 3095 | 3.87 ms | 3.12 ms | 3.08 ms | < 1 µs | 0.39 ms |
| fixtures/gallery-gtk/src/pages/06-layout.svelte | gtk | 2644 | 2.96 ms | 2.69 ms | 2.90 ms | < 1 µs | 0.26 ms |
| fixtures/counter/src/App.svelte | gtk | 223 | 0.38 ms | 0.35 ms | 0.35 ms | < 1 µs | 0.01 ms |
| tests/r2/diagnostics-gtk.svelte (12 diagnostics) | gtk | 936 | 1.64 ms | 1.66 ms | 2.42 ms | 4 µs | 0.15 ms |
| tests/qt-types-models/src/App.svelte | qt | 4348 | 2.96 ms | 2.64 ms | 2.63 ms | < 1 µs | 0.50 ms |
| fixtures/gallery-qt/src/pages/05-containers.svelte | qt | 3018 | 1.97 ms | 2.41 ms | 1.88 ms | < 1 µs | 0.27 ms |
| fixtures/counter-egui/src/App.svelte | egui | 1037 | 0.65 ms | 0.62 ms | 0.63 ms | < 1 µs | 0.07 ms |

- A check per keystroke of the largest component in the repo (6 KB) takes about 6.5 ms, a third
  of the 20 ms budget (AP6). A typical panel (1–3 KB) takes 0.6–3 ms.
- Skipping the output saves only 5–15%: lowering against the spec and the rune transform's
  template walk dominate, and both report diagnostics. The noise between runs (other lanes'
  builds) is about as large as that difference.
- The state answers an unchanged buffer in under a microsecond, and a buffer with a syntax error
  in 0.01–1.1 ms (the parser stops early; the carried-over diagnostics are mapped).

## 6. Notes for Q1 (the module loader's overlay)

- Keep one `FileState` per module path, next to the overlay entry. Check the overlay text when
  there is one, else the disk text, with the same path: the path decides the kind and appears in
  the diagnostics. A `FileState` is `Clone + Send` and holds at most two copies of the source.
- Pass the same `SpecBackend` value to every check. When the spec changes (an RA1 fragment merged
  at run time, a provider registering kinds), build a new `SpecBackend`: its fingerprint changes and
  no stale result is reused. A custom `PlatformSemantics` without a fingerprint is always re-run.
- Before swapping a module (HMR), call `check` and swap only when `!result.has_errors()`
  (`has_errors` ignores stale diagnostics). Then run `compile` for the code. The check result for
  the same text is reused, but `compile` itself does not share work with `check` today.
- `.svelte.js` modules go through `compile_module`; other `.js` files are `FileKind::Other` with no
  diagnostics (the loader's own JS parse errors are its own).

## 7. Notes for quire / ED1 (the editor)

- On each buffer change, `check(path, &buffer, &backend, &mut state)` on the UI thread is within
  budget (§5). Draw `stale` diagnostics dimmed; they have no fix-its.
- Ranges: `range.start..range.end` are UTF-8 byte offsets, which a ropey `TextBuffer` converts
  directly (`byte_to_char`); `start_pos`/`end_pos` are 1-based lines with columns in UTF-8 bytes
  and UTF-16 units (LSP's default `character` is `utf16_column - 1`).
- Quick fixes: apply a `Fix`'s edits from the last to the first (they are sorted and do not
  overlap), as one undo group, then check again.
- Key any per-code UI (icons, suppression lists, documentation links) on `id`, not `code` or the
  message; `svelters_compiler::codes::lookup(id)` gives the summary and, for `svelte::` ids, the
  svelte.dev link.
- `--check --json` gives the same data to tools out of process.

## 8. Tests

- `crates/svelters-compiler/tests/cx1_codes.rs`: the source scan against the registry, the
  generated page is current, `--explain` text.
- `crates/svelters-compiler/tests/cx1_check.rs`: "did you mean" fixes (start and end tag), the
  bind/modifier fixes over `tests/r2/diagnostics-gtk.svelte`, the event modifier fixes (exact or
  none), `!important` and `elseif`, byte and UTF-16 positions, the unchanged-input reuse, the carry
  over a syntax error (shifted, dropped where edited, cleared when fixed), modules and other files,
  the JSON shape; over the corpus (`tests/{fw1,r2,runes}`, `fixtures/`, `examples/`):
  `check_component` reports exactly `compile`'s diagnostics and every fix-it resolves.
- Unit tests: `codes::tests` (unique, well-formed ids), `check::tests` (`changed_region`).

## 9. Not run locally

- The full compiler test suite and the workspace nextest (CI). Locally, after the last code edit
  (the `css_*` rows, on the merge with Q3): `cargo check -p svelters-compiler --all-targets` and
  `cargo nextest run -p svelters-compiler --test cx1_check --test cx1_codes --test fw1_forwarding`
  (all pass). Earlier in the lane, before the capture fix-it change and the merges, also
  `--lib --test r2_observe --test runes --test fx1_style` (all pass). Q3's `css_scope` test was not
  run (no change to its code). `cx1_check`'s corpus test
  compiles every `.svelte` file under `tests/{fw1,r2,runes}`, `fixtures/` and `examples/` both ways.
- Conformance (CI): diagnostics' codes and messages are unchanged, so no score change is expected.
- The docs site build and link check (CI): the new page is Markdown with no internal
  links.
- No backend (GTK/Qt/egui) or JS package tests: nothing outside `svelters-compiler` and the docs
  changed. The first CI run (`js` 39796209252) caught `style_state_unsupported` unregistered (the
  debug panic, in `experiments/native-compiler/compiled`): the scan missed codes written as
  `Some(("code", why))`. Registered, and the scan now covers that form; after that edit only
  `cx1_codes` ran locally (the `compiled` experiment needs `pnpm install`, left to CI).

## 10. Open points (recommendation taken)

1. **Severity overrides (`svelte-ignore`, per-project levels).** Not in CX1. The ids are what such a
   setting should key on. Recommendation: CX4's language server, with the ids from this registry.
2. **Svelte's warnings in the parser** (`element_implicitly_closed`, `script_unknown_attribute`,
   `svelte_element_invalid_this`) are ported but not surfaced by `compile`. They are registered.
   Recommendation: surface them in a later lane together with `svelte-ignore`.
3. **`compile` after `check`** re-runs the front half. If the loader's swap path shows up in the
   edit-to-pixels time (dock-W7's 300 ms), `compile` can take the `check` state and keep the parsed
   tree; today a full compile of the largest component is a few ms, so not needed.
