# Lane APP-TEST: the `presto` facade (app runner, latency instrumentation) and presto-test

Owns `crates/presto/**` and `crates/presto-test/**`. `presto` ties the pieces together on the UI thread:
`presto::App` implements `presto_platform::AppHandler`, owns the `Document`s, the `TextSystem`, the
`CompositorHandle`, a `presto_frame::UiFrameState` per window; event-time dispatch (discrete input → dispatch →
urgent frame), continuous input coalesced per frame, `ToUi` handling (BeginFrame → `Document::frame` → commit;
Presented; Animation; ScrollMoved; ResizeDone), window creation and the resize handshake, surfaces created on
the main thread and handed to the compositor. Latency instrumentation (architecture §16.3): `InputSeq` stages
`os → recv → dispatch_end → js_end → ui_frame_start → commit → commit_applied → draw → submit → gpu_done →
present → presented`, every seq terminates once; HDR-style histograms; Chrome-trace JSON export with flows.
`presto-test`: `Harness` with kittest's verbs (`run`, `run_until_settled`, `step`, `get_by_label`,
`get_by_role_and_label`, `query_by_*`, `click`, `type_text`, `key_press`, `event`, `render`, `state`),
on `HeadlessPlatform` and a virtual clock, rendering through the compositor on lavapipe; an `egui::Event`-shaped
injection adapter; AccessKit snapshot queries by author id. Tests (written, not run): the harness on a small
tree; instrumentation terminates every seq.
