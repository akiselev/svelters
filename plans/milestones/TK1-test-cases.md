# TK1: one test case, every backend (cadatomic SV34)

Track: TESTKIT (`plans/research/TRACKS.md` §2.1). Request: SV34 (`plans/research/CADATOMIC.md` §2.6), "a single
`.svelte` case with inline expectations dispatched to all runners". Lane TK1, branch `ci/tk1`.

## 1. What landed

| Part | Where |
|---|---|
| Case format, parser, step interpreter, view model, op-log tree mirror, ratchet | `crates/svelters-testkit` (new crate) |
| Log backend runner (`log-gtk`, `log-qt`) and the handle-based plan for the real toolkits | `svelters_testkit::log` |
| egui runner (kittest, headless; snapshots through wgpu with feature `snapshots`) | `svelters_testkit::egui` |
| GTK and Qt runner (the real toolkits, through the `counter` driver) | `examples/counter/tests/testkit.rs` |
| `checkpoint_dumps` in the driver report (the settled tree at each `checkpoint` step) | `examples/counter/src/{driver,main}.rs` |
| Case builder `svelters-cases` and the portable kit (`svelters:testkit`) | `packages/testkit` (new package `@svelters/testkit`) |
| svelters' cases, ratchet, snapshots | `tests/cases/{counter,bind-text,keyed-each,transition}.svelte`, `baseline.json`, `snapshots/*.png` |
| CI | `linux-gtk.yml` (`-p svelters-testkit` in the gtk job; new job `egui-snapshots` on lavapipe), `linux-qt.yml` (builds the cases) |

Result matrix (all local runs, see §6): every case passes on `log-gtk`, `log-qt`, `egui`, `gtk` and `qt`; the egui
snapshots match on lavapipe.

## 2. The case format

One file, `tests/cases/<name>.svelte` (any directory works; `_`-prefixed files are helpers, not cases):

```svelte
<!--
@case A click reaches JS and JS updates the label.
@toolkits gtk qt egui
@xfail qt "why it fails there, with a ref"
-->
<script>
  import { Window, Column, Button } from 'svelters:testkit';
  let count = $state(0);
</script>

<Window title="Counter">
  <Column>
    <label>Count: {count}</label>
    <Button text="Increment" onclick={() => count++} />
  </Column>
</Window>

<!--
@test
expect "Count: 0"
click "Increment"
expect "Count: 1"
snapshot "one"
-->
```

Header keys (in any comment before `@test`): `@case <description>`, `@toolkits <names>` (default `gtk qt egui`),
`@xfail <backend> "<reason>"` (repeatable; backends are `gtk`, `qt`, `egui`, `log-gtk`, `log-qt`).

Steps, one per line (`#` comments; strings are JSON strings):

| Step | Meaning |
|---|---|
| `expect "<text>"` | some node shows exactly this text |
| `expect no "<text>"` | no node shows it |
| `expect order "<a>" "<b>" ...` | each is shown, in this order (tree order) |
| `click "<text>"` | click the button showing the text |
| `type "<field>" "<text>"` | replace the content of the text input whose placeholder (hint text, accessible name) is `<field>` |
| `wait <ms>` | let time pass (timers, transitions) |
| `snapshot "<name>"` | egui + `snapshots` feature: compare a wgpu rendering with `snapshots/<case>-<name>.png`; skipped (noted) elsewhere |

Expectations look at the **settled** state: before the expectations that follow an action every backend runs until
the host is idle. "Shows" means the node's text: label text, button text, text-input content, window title (GTK/Qt;
egui's AccessKit tree has no window title). Expectation failures are all reported; a failed action ends the case.
JS errors fail the case.

**The kit.** Toolkit vocabularies differ (`button`/`push-button`, `box orientation`/`box-layout direction`/taffy
`box`, `entry`/`line-edit`/`text-edit`), so a portable case imports `svelters:testkit`, which `svelters-cases` resolves
per toolkit to `packages/testkit/kit/<toolkit>/index.js`: `Window {title width height}`, `Column {gap}`, `Row {gap}`,
`Button {text onclick}`, `TextInput {placeholder bind:value}`, `Check {text bind:checked}`. `<label>` is the same
element on all three toolkits (text children), so cases use it directly, which also keeps native-element features
(keyed each of elements, `transition:`) on real elements. A case may also use one toolkit's own elements and say so
with `@toolkits egui` (that is how cadatomic writes egui-only cases).

## 3. How each backend runs a case

| Backend | Runner | View | Actions |
|---|---|---|---|
| `log-gtk`, `log-qt` | `svelters_testkit::log`: the bundle on the real QuickJS host and the log backend with that toolkit's prop schema (`fixture_config`, scripted mode, as the op-log fixtures) | the op log replayed into a tree (`oplog::Mirror`, same rules as `tools/oplog-compare.mjs`) | `event #h clicked`; `type` is a `changed #h text` step when JS observes `text` (R2 `bind:`), else the toolkit's `changed`/`text-changed` event |
| `egui` | `svelters_testkit::egui`: `TestApp` (kittest, headless) | AccessKit tree | kittest click; click + select all + type |
| `gtk`, `qt` | `examples/counter/tests/testkit.rs`: the `counter` binary on the real toolkit | the native widget dump at each `checkpoint` step | native click, native typing |

The log backend plays a script decided up front, so its runner **replays**: each settle runs the bundle again from
the start with the steps resolved so far (scripted mode is deterministic; handles are the same each run). The
resolved steps are a **plan**: a fixture-style `script.json` (handles in creation order, which is also how GTK and Qt
number them) plus the expectations of each `checkpoint`. The GTK and Qt runs perform that plan with the existing
`counter` driver (`--fixture <dir> --script`), which now records `checkpoint_dumps`; the expectations are checked on
those dumps. A case that fails on the log backend is not run on the real toolkit (its plan is incomplete). `wait <ms>`
becomes `ceil(ms/30)` extra checkpoint steps for the driver (each waits for 3 idle polls of 10 ms), without
expectations. Artefacts: `target/p5/testkit-<toolkit>-<case>.{script.json,json,png}`.

## 4. The ratchet

`ratchet::report` per backend, `tests/cases/baseline.json` (the conformance runner's `baseline.json` idea):

- a case that targets the backend passes, or is `@xfail` there with a reason;
- an `@xfail` case that passes fails the run ("remove its @xfail");
- every (backend, case) in `baseline.json` must still pass, so a regression cannot hide behind a new `@xfail` or a
  dropped toolkit. New passes are printed; `SVELTERS_UPDATE=1` records them (`FORCE=1` to drop one). A cases directory
  without `baseline.json` (cadatomic's, at first) gets the first two rules only.
- results per backend go to `target/testkit/<backend>.json` (uploaded by CI).

`TK_CASE=<substring>` runs matching cases only and skips the baseline check.

## 5. Running

```sh
tools/dev/cargo-slot cargo build -p svelters-compiler --bin svelters-compile
pnpm --filter ./tests/cases build                      # svelters-cases . -> tests/cases/dist/<toolkit>/<case>/dist/app.js
tools/dev/cargo-slot cargo nextest run -p svelters-testkit                             # log-gtk, log-qt, egui
WGPU_BACKEND=vulkan tools/dev/cargo-slot cargo nextest run -p svelters-testkit --features snapshots cases_on_egui
tools/dev/cargo-slot cargo nextest run -p counter --no-default-features --features counter/gtk testkit_gtk
tools/dev/cargo-slot cargo nextest run -p counter --no-default-features --features counter/qt testkit_qt
```

A missing snapshot is recorded on the first run (commit the PNG; CI uploads the ones it records in the
`egui-snapshots` artifact). Snapshots are lavapipe renderings; `WGPU_BACKEND=vulkan` keeps wgpu off its GL fallback,
which segfaulted at process exit under WSL. kittest writes `*.new.png`/`*.diff.png` next to a failing snapshot
(ignored by `tests/cases/.gitignore`).

**For cadatomic.** Depend on `svelters-testkit` (default feature `egui`, no eframe), keep cases in a directory of
your own, build them with `svelters-cases <dir>` (or any build that gives `<out>/egui/<case>/dist/app.js`), then:

```rust
for c in svelters_testkit::case::load_dir(&dir)?.iter().filter(|c| c.targets("egui")) {
    let (outcome, _) = svelters_testkit::egui::run(c, &c.bundle(&out, "egui"), Some(&dir.join("snapshots")));
    results.push(svelters_testkit::ratchet::CaseResult::new(c, "egui", outcome));
}
svelters_testkit::ratchet::report(&dir, &results_dir, "egui", &results)?;
```

Embed bundles (EM1 roots in slots) are not a case target yet; see §7.

## 6. Verification (this lane, local)

- `cargo check -p svelters-testkit -p counter --all-targets`: clean (no new warnings).
- `nextest -p svelters-testkit`: 6 unit tests (parser, view, mirror, ratchet rules) and `cases_on_log_gtk`,
  `cases_on_log_qt`, `cases_on_egui`: 4/4 cases each.
- `--features snapshots cases_on_egui` (lavapipe, `WGPU_BACKEND=vulkan`): recorded `counter-three.png` and
  `transition-back.png`, then matched them on the next run.
- `nextest -p counter testkit_gtk` (GTK feature) and `testkit_qt` (Qt 6.12 feature), headless: 4/4 each.
- Not run locally: the rest of `counter`'s tests after the driver change (the change only adds a dump at checkpoint
  steps), the workspace suites, conformance (TK1 does not touch the conformance runner). CI runs them.

## 7. Not in TK1 / follow-ups

- **Isolate target** (ISO2) and **embed bundles** as targets: a case with `@embed` would mount its default export
  in an `EmbedHarness` slot; the runner interface (`Target`) already fits.
- **Richer expectations**: props (`expect value "Name" "Ann"`, `checked`), node identity across keyed moves (the
  op-log mirror has handles; the dumps do not), counts. TK2's selectors (`:role`, `:name`) should replace text
  matching once they exist, so cases, kittest and MCP resolve the same node (SL5).
- **`Check` on GTK**: the `counter` driver clicks only `gtk::Button`s; a `click` on a `check-button` needs a
  `toggled` action there (no case uses `Check` yet).
- **AppKit and WinUI**: the kit and the runner shape fit; they need their `counter`-style drivers to dump at checkpoints.
- **The op-log ratchet**: the log runs could also commit their final op log per case and compare with
  `tools/oplog-compare.mjs` (state-equal) like the fixtures do; left out to keep cases single-file.
