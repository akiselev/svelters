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
workflow+ref with cancel-in-progress; Swatinem/rust-cache. macOS and Windows have path filters.

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
