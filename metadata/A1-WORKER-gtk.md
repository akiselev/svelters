# A1 worker brief: GTK annotation batch

You annotate one batch of GTK4 widget kinds for svelters (Svelte 5 apps in embedded QuickJS that
render native GTK4/Qt6 widgets through a fixed C ABI). Your batch is appended at the end of this
brief. Work only on it. Every fact you annotate must be backed by a passing generated test; a
fact you cannot verify is left out and listed in your batch's status line.

## 0. Rules

- Builds: always `export CARGO_BUILD_JOBS=2 CARGO_BUILD_BUILD_DIR=target/build`, nextest
  `--test-threads 2`. Never omit `CARGO_BUILD_BUILD_DIR` (the global cargo config shares a build
  dir across worktrees and corrupts artifacts). Run only your batch's tests until the final gate run.
- Edit only: `metadata/overrides/gtk/<Kind>.yaml` for your batch's kinds, your gallery page
  `fixtures/gallery-gtk/src/pages/NN-<family>.svelte`, your `gallery_page!` line(s) in
  `examples/counter/tests/gallery.rs`, your batch's `Status:` line in `metadata/A1-TODO.md`, and the
  generated outputs the commands below rewrite (commit them). Do not edit generators, crates,
  the renderer, templates or other batches' files. If the framework is missing something, do not
  work around it: leave the fact out and say which gap in `metadata/A1-GAPS.md` blocks it (or
  describe a new gap in your final report).
- Never expose blocking APIs (`gtk_dialog_run`, nested main loops).
- Don't push. Don't touch master. Commit on your own branch; messages end with:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01Lc9bA3PFUNUTFSuo1BzjWS
  ```
- Notes: append findings (surprises, toolkit traps, failed facts with the failure) to
  `docs/notes/a1-<batch-id>.md` as you go. Keep it short.

## 1. Setup (once)

```sh
git merge --ff-only worktree-agent-af26bb86d3e5158d3 || git reset --hard worktree-agent-af26bb86d3e5158d3   # the A1 branch
pnpm install && pnpm -r build          # fixture bundles, needed by the gallery tests
```

When the lead asks you to sync later: `git merge worktree-agent-af26bb86d3e5158d3`; on conflicts
in generated files take either side and rerun the generator commands of section 3.

## 2. Where things are

- `metadata/gtk/<Kind>.json`: ingested kinds (GIR + PyGObject), with your overrides merged in. Read
  them to find property names, types, defaults, ranges, signals (flags, params, `handler_result`)
  and method names. Never edit them by hand.
- `metadata/overrides/gtk/<Kind>.yaml`: your annotations. Schema:
  `tools/metadata/gtk/override.schema.json` (unknown keys are errors). Existing examples:
  `Box.yaml`, `Button.yaml`, `Editable.yaml`, `Entry.yaml`, `HeaderBar.yaml`, `Window.yaml`, `ScrolledWindow.yaml`.
- Generated: `crates/svelters-gtk/src/generated/{tables,slots,verify,types}.rs`, `facts.json`,
  `packages/renderer/types/gtk.d.ts`, `packages/renderer/src/bindings.generated.js`.
- Ledger: `metadata/verified/gtk.json`. Coverage: `metadata/coverage/gtk.md`.
- Semantics references: `docs/notes/signals-and-slots-semantics.md` 2.5 (slot op descriptors),
  `docs/notes/fork-bindings.md` 6 (bindings), `docs/notes/ownership-and-lifetimes.md`,
  `docs/notes/x2-sync-frame.md` ("What A1 should annotate": sync signals), `metadata/A1-GAPS.md`.

How GTK exposure works: every property and signal of every widget is already reachable by
reflection (dynamic GObject). Annotations add (a) JS names, (b) child slots, (c) signal semantics
(results, sync), (d) two-way bindings, (e) methods (zero-arg only, see G2), and (f) a generated test
per fact. Annotations on a base class or interface apply to subclasses (`inherited` in the merged
JSON); test them once on the base through `verify_subject`.

## 3. Annotation format and the loop

Every record needs `provenance` (a string, or a list of `{origin, ref}` with origin
`gir|runtime|manual|probe|docs|source`). Records under `properties`, `signals`, `methods` must
match an ingested record by `name`, unless marked `new: true`, `virtual: {...}` or
`derived_from: {...}` (strict merge: a typo is an error).

```yaml
# metadata/overrides/gtk/ToggleButton.yaml
properties:
  - name: active                     # GObject property name (kebab-case)
    js_name: active                  # optional; default is camelCase of the name
    echo_suppress: [toggled, "notify::active"]   # a programmatic set from JS does not reach these listeners
    sample: true                     # optional: value the generated test writes (default: derived from type/range/default)
    setup: 'h.set_str(o, "label", "x").unwrap();'  # optional Rust run before the test (h: Harness, o: handle)
    provenance: "GtkToggleButton:active (Gtk 4 docs)"
signals:
  - name: toggled
    handler_result: none             # none | bool_stop | value (X2: bool_stop handlers decide via e.preventDefault())
    sync_required: false             # true where an async fallback is wrong (close-request, key-pressed, state-set, ...)
    trigger: {property: active, values: [true, false, true]}   # how the test makes GTK emit it
    provenance: "GtkToggleButton::toggled, run-first, void (Gtk 4 docs)"
  - name: "notify::active"           # detailed notify signals are not ingested: declare them
    derived_from: {property: active}
    js_event: "notify::active"
    trigger: {property: active, values: [true, false, true]}
    params: []
    returns: {type: void}
    handler_result: none
    sync_required: false
    phase: first
    flags: [detailed]
    connection: {disconnect_on: [handle_drop, native_destroy]}
    provenance: "GObject::notify for GtkToggleButton:active"
bindings:
  - name: active                     # bind:active
    direction: two_way
    property: active
    type: {ref: bool, nullable: false}
    change: {signal: toggled, role: any_change, value: prop}   # value: arg0 (first signal arg) | prop (read the property)
    write: {natively_normalizes: false}
    echo: {strategy: host_suppressed, compare_before_set: true, read_back: false, sync_signals_on_set: [toggled, "notify::active"]}
    lifetime: {on_native_death: end_binding, on_stale_update: warn_noop, on_destroy: idempotent_disconnect}
    provenance: "set_active emits toggled and notify::active only on change (Gtk 4 docs)"
methods:
  - name: popup                      # zero-arg, void only (G2 blocks the rest)
    js_name: popup
    dispatch: zero_arg
    expect: 'h.get(o, "active") == V::Bool(true)'   # Rust bool expression checked after the call (h, o, w: gtk::Widget)
    provenance: "gtk_menu_button_popup (Gtk 4 docs)"
```

Signal triggers: `{property: <name>, values: [...]}` (sets the GObject property natively, typed
from the metadata), `{native: set_text, values: [...]}` (GtkEditable), or
`{rust: "<statement>", values: [...]}` with `o: &glib::Object`, `v: &str`, `i: usize` in scope, e.g.
`rust: 'o.downcast_ref::<gtk::Entry>().unwrap().emit_activate();'`. Parameterless action signals
need no trigger (the test emits them). The test fires three times: delivered, not delivered after
listen(off), not delivered after destroy.

Child slots (`children.slots`, see `Window.yaml`/`HeaderBar.yaml`/`Box.yaml`): `cardinality: one`
needs `get`/`set`/`remove` ops; `many` needs `get: {contains: parent|ancestor}`, `insert`, `remove`.
Op `native` is `"GtkClass::method(Arg types)"` and must exist in the metadata; `args` use `child`,
`null`, `prev_sibling`, `before`, `index` (position of `before`, -1 = append), `index_opt`
(`Option<u32>`, gtk4-rs style), `child_index`, `child_index_opt`. An `index_of` op
(e.g. `"GtkNotebook::page_num(GtkWidget*) const"`) defines positions; otherwise the position of the
direct child that contains the widget is used. `effects` record ownership (`child: parented |
unparented | wrapped:<Kind>`, `old: unparented`); the generated test checks insert, order,
replacement release and removal. `order: before_sibling|native_index` enables insert-before.

Other keys: `element: {tag: x}` renames the element tag (default: lower-case GTK name);
`unexposed: {reason: "..."}` marks a kind deliberately not exposed (shown in coverage);
`verify_subject: {tag: x}` for abstract kinds/interfaces; `lifetime: {class, provenance}` only when
the derived lifetime block in the JSON is wrong.

The loop:

```sh
python3 tools/metadata/gtk/ingest.py            # ~60 s: merges overrides, strict-merge errors appear here
python3 tools/metadata/gtk/gen_backend.py       # lists facts without a working test: fix or drop them
python3 tools/metadata/common/gen_bindings.py   # binding table; checks bindings against the facts
xvfb-run -a cargo nextest run -p svelters-gtk --test-threads 2 -E 'test(/<re>/)'   # <re> from your batch
```

A failing test means the annotation is wrong (or GTK behaves differently than documented): read the
failure, fix the fact, or drop it and record it in your status line (`dropped: Paned.position
(clamped to 0 without allocation)`). Never weaken a generated test.

## 4. Gallery page

- File `fixtures/gallery-gtk/src/pages/NN-<family>.svelte` (NN from your batch). First line:
  `<!-- kinds: Kind1 Kind2 ... -->` (GTK names without the Gtk prefix; read by the coverage report).
- Show each kind of the batch with its annotated props and signals; keep it compact (the screenshot
  is about 640x480; use a `box` with `spacing`).
- Use explicit props: `<button label="x" />`, numbers and booleans as expressions (`digits={2}`,
  `active={true}`), enum props as their nick strings (`halign="center"`). Text children only work on
  `label` and `button`. Events: `on<signal>` (e.g. `ontoggled`, `onclicked`).
- Each two-way binding gets a counter of change events after the app's own writes, shown as a
  label `echo <name>: {count}`, and the app writes the bound state once after mount
  (`$effect(() => { value = ... })`). The screenshot test fails if any `echo ...` label is not 0.
  See `00-overview.svelte`.
- Add `gallery_page!(gtk_NN_family, "gtk", "NN-family", ["GtkFoo", "GtkBar"]);` to
  `examples/counter/tests/gallery.rs` (native classes that must appear in the widget tree).
- Check: `pnpm --filter @svelters-fixtures/gallery-gtk test` (builds and opens every page in the
  TestHost) and `xvfb-run -a cargo nextest run -p counter --test gallery --test-threads 2 -E 'test(/gtk_NN/)'`.
  Look at the screenshot `target/gallery/gtk-NN-family.png` (Read tool) and fix layout problems.

## 5. Finish

```sh
SVELTERS_TEST_THREADS=2 python3 tools/metadata/ledger.py refresh --toolkit gtk   # runs the generated tests, writes the ledger
python3 tools/metadata/coverage.py
pnpm test && pnpm docs:test
xvfb-run -a cargo nextest run --workspace --test-threads 2                       # full gate, once at the end
```

Update your batch's `Status:` line in `metadata/A1-TODO.md` to `done (<n> facts verified;
dropped: ...; blocked: gap:Gx for ...)`. Commit everything (overrides, generated files, ledger,
coverage, page, test line, notes) in one or a few commits.

Final report (your last message, short): branch name and HEAD, facts verified (count by kind),
facts dropped with the reason, facts blocked by which gap, new gaps you found, gate results,
screenshot path.
