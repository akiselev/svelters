# CI1: GitHub Actions per platform

Branch `ci/platforms`. One workflow per target in `.github/workflows/`, each with its own badge:

| Workflow | Runner | What it runs |
|---|---|---|
| `js.yml` | ubuntu-24.04 | builds `svelters-compile`, `pnpm build`, package tests, conformance `test:compiler`, `test:meta`, `docs:test` |
| `linux-gtk.yml` | ubuntu-24.04 (GTK 4.14) | nextest: abi, backend-log, host, compiler, svelters-gtk, counter (`--features counter/gtk`, non-qt tests) under Xvfb; portable backend crates |
| `linux-qt.yml` | ubuntu-24.04 + Qt 6.11 (install-qt-action) | nextest: svelters-qt-build, counter-qt, counter (`--features counter/qt`, qt tests) under Xvfb |
| `macos-appkit.yml` | macos-latest (arm64) | `cargo test` apple/appkit units, counter-mac backend/generated/counter, a screenshot run |
| `windows-winui.yml` | windows-latest | Windows App Runtime from NuGet MSIX, `fetch.py --bootstrap`, winui lib + generated, counter-win selftest (report.json artifact) |

Shared: `.github/actions/setup-pnpm` (pnpm from `packageManager`, Node 22, store cache, frozen install).
Every workflow: push to master and `ci/**`, pull_request, workflow_dispatch; concurrency per
workflow+ref with cancel-in-progress; Swatinem/rust-cache; `permissions: contents: read`. The repo is
public, so there are no path filters: every push and PR runs all five. No workflow uses
`pull_request_target` or secrets (fork PRs are safe).

Decisions:
- `counter` defaults to both gtk and qt features, so each Linux job builds it with one feature and
  filters the other toolkit's tests out by name (`test(/qt/)`).
- `pnpm -r build` needs `svelters-compile` (the compiled experiment's vite plugin), so Linux jobs
  build it first. Mac and Windows only build `./fixtures/*`.
- Build scripts run Python with `-I`, so system Python needs `python3-yaml` (apt);
  `SVELTERS_PYTHON=/usr/bin/python3` in the Qt job because install-qt-action puts its own Python on PATH.
- `test:meta` does not need live GTK/Qt (`ledger.py check` only reads the ledger; `gen_kinds.py`
  is not in it), so it runs unsplit in `js.yml`.

## Run log

### Run 1 (8f598c9), 2026-10-17

- **linux-qt**: pass first time. Qt 6.11.3 via install-qt-action (aqt's `.pc` files work with
  `PKG_CONFIG_PATH=$QT_ROOT_DIR/lib/pkgconfig`). 78 tests passed, 28 skipped (counter's gtk tests).
- **macos-appkit** (macOS 26.6.2 arm64, Xcode 26.6, SDK 26.5): everything builds. Unit tests pass,
  `generated` 76/76, `counter` passes, and `cargo run -p counter-mac -- --screenshot` produces a
  correct window ("Count: 0" / "Increment"). `--test backend` has 2 real failures and 1 cascade:
  - `read_only_prop_is_refused`: setting `subviews` to `[]` errors with "cannot convert list to an
    object" instead of a read-only error. The value conversion runs before the read-only check.
  - `request_frame_delivers_one_frame`: 0 frames after `request_frame` and a 150 ms pump (expected 1).
    Either the CADisplayLink never fires (headless runner VM?) or the frame path is broken.
  - `no_leaked_handles`: 2 live handles. This is the cascade: the two tests above panicked before
    `destroy`.
- **windows-winui** (Server 2025, build 26100): the runtime install from the NuGet MSIX works
  (`Microsoft.WindowsAppRuntime.2` 2.5.1.0), `fetch.py --bootstrap` works, everything builds, and
  `--lib` passes (12). WinUI init fails at the `XamlControlsResources` step:
  `The application called an interface that was marshalled for a different thread. (HRESULT 0x8001010E)`.
  The `generated` test binary then panics in windows-core `weak_ref_count.rs:50` (subtract with
  overflow) while dropping the composed `App` in `Application::compose` (bindings.rs:481 <- app.rs:148).
  That is a refcount underflow in the outer/inner composition. Both the selftest and the counter
  test stop at the same init error. This is the "Application without Application.Start" risk from
  win1-winui.md.
- **js**: three CI-environment bugs, all fixed in 0ce25e6:
  - compiled diagnostics were coloured (vite colours its errors under `CI`);
  - the Qt docs determinism test needs `/usr/share/doc/qt6` qdoc indexes;
  - conformance `test:compiler` needs an upstream sveltejs/svelte checkout, so that step was dropped.
- **linux-gtk**: missing `libgirepository-2.0-dev`.

### Run 2 (0ce25e6)

js pass, qt pass. On GTK 4.14 (Ubuntu 24.04), svelters-gtk **does not link**:
`undefined symbol: gtk_accessible_hyperlink_get_type, gtk_popover_bin_get_type, gtk_svg_get_type,
gtk_try_expression_get_type`. These come from `crates/svelters-gtk/src/generated/types.rs` (static
`extern fn *_get_type`). This can't be gated per test. The fix is in the code: resolve post-4.14
types at run time (`g_type_from_name` after `ensure`, or dlsym) or gate them by GTK version.

### Run 3 (47df17c)

`linux-gtk` now runs the suite in an `archlinux:latest` container (GTK **4.24.1**): 820 passed,
36 skipped, plus 31 portable-crate tests. The Ubuntu 24.04 / GTK 4.14 build stays as an
informational `continue-on-error` job. All three Linux workflows are green.

### Run 4 (d9b6a1a, master with N1 merged)

Same results as before N1: js, linux-gtk (Arch: 820 passed) and linux-qt (78 passed) are green.
macOS has the same 3 `backend` failures, and Windows has the same XamlControlsResources init failure.
Then the macOS/Windows path filters were removed and `permissions: contents: read` was added.
