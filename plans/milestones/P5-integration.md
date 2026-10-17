# P5 - integration

## Work
- `examples/counter`: one binary with `--backend gtk|qt`, loading the P1 bundle in the P2 host on the P3/P4 backends. `keyed-list` and `if-toggle` run the same way.
- Xvfb tests: launch, inject clicks through backend test hooks (later desktop-level automation), and take screenshots as artifacts.
- Measure startup time, bundle size and binary size per backend, and record them as the first baseline.

## Acceptance
The same JS bundle shows the counter and the keyed list in both GTK4 and Qt. Clicks update the UI, and list add/remove/reorder works. The screenshots and baseline numbers are in the completion record.
