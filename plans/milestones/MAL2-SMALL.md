# MAL2-SMALL: svelters small items (SV72, R1, R2, SV-B2-1..4, SV75, FQ-108)

Lane `S-C small` of cadatomic's MAL2 batch, branch `ci/mal2-small`, base master `af787afe`.
Coordinator: cadatomic's batch coordinator session. Every seam is generic.

**Status:** done; CI: see the end.

## What landed

### SV72: a lazy icon source (`skelter-widgets/src/paint/icon/{registry,vector}.rs`)

- `IconSource` (a trait, and any `Fn(&str) -> Option<Icon> + Send + Sync`), installed with
  `IconRegistry::set_source`. A lookup that misses (`contains`, `get`, `resolve`; so measure and
  paint) asks the source once and caches the answer, a `None` too. The source runs without the
  registry's locks. A registered icon always wins; `ext.` and malformed keys are never asked.
- `invalidate(keys) -> usize`, `invalidate_all() -> usize`: drop cached answers, their parsed vector
  art (`VectorArt::forget`) and, through the raster-key generation, their rasters.
  `revision()` grows on each change. `host::icons_changed(doc)` repaints a document (the registry is
  the process's and marks no nodes).
- Returns an `Icon`, not SVG text, so a source can give multicolour icons and several masters.
- Tests: `paint::icon::registry::tests::source_*` (4).
- Docs: `docs/skelter/icons.md` "A lazy icon source".

### R1: reduced motion for scroll areas and the compositor

- skelter-widgets: the hosted kinds' `EventCx` carries `reduced_motion`, read from the document's
  media preferences on each event. `ScrollArea` jumps smooth steps when it or the document asks,
  and its overlay bars hide at the end of their hold without fading (`bar_policy`). Flings, which
  follow the user's hand, still run. In a document, wheel scrolling of a scroll container is core's
  and already jumps; the kind's animation path is the one this covers.
- skelter-compositor: `Msg::SetReducedMotion`, `CompositorHandle::set_reduced_motion(window, on)`.
  Finite animations sample at their end (keyframes: their end time; springs: settled), smooth
  scrolls land on their target in one frame; flings and infinite animations run as before.
- skelter: `DocumentApi::reduced_motion()` (default `None`; `CoreDocument`: the document's media
  preference) and `CompositorApi::set_reduced_motion`. The runner sends it when a window opens and
  after each commit when it changed, like the caret blink.
- Tests: skelter-widgets `views::scroll::tests::the_documents_reduced_motion_jumps`;
  skelter-compositor `engine::reduced_motion_jumps_animations_and_smooth_scrolls`.

### R2: the `title` tip after an event-time hover (`skelter-core/src/runner.rs`)

- `Document::hover_at(window, pos, now)`: moves the input clock to `now` (never back), then hovers;
  a tip the hover starts waits its whole delay from `now`. `Document::hover` is `hover_at` at the
  last input's time, as before.
- `DocumentApi::hover_at` (default: `hover`); `CoreDocument` forwards it; the runner's event-time
  hover of a coalesced move passes the move's OS time. An embedder that overrides
  `DocumentApi::hover` should override `hover_at` too.
- Test: skelter-core `tests::overlay::a_timed_hover_starts_the_tip_at_its_own_time`.

### SV-B2-1..4: the skelter surface (`svelters-skelter/src/surface/tier0.rs`)

- Every widget: `pointerenter`, `pointerleave` (core's common signals; non-bubbling, passive, the
  pointer record).
- `text-edit`: the `read-only` prop and the `cancel` event (Escape in the field).
- `slider`'s doc names `enabled` (the native `disabled` is not a surface name).
- `docs/skelter/surface.md`: surface names vs native names, hover, `text-edit`.
  `empty-state` is not a svelters kind (an app kind takes the names its own spec declares; the page
  says so generically).
- `metadata/semantics/skelter.json` regenerated (`SVELTERS_UPDATE=1 cargo test -p svelters-skelter --test spec`).
- Test: svelters-skelter `host_document::hover_events_and_text_edit_read_only_and_cancel_reach_svelte`.

### SV75: the system clipboard in svelters-host (`svelters-host/src/platform/system_clipboard.rs`)

- `SystemClipboard` (arboard, the in-process MIME store beside the text) and the Linux primary
  selection helper (`system_primary`, `PRIMARY_SUPPORTED`) move from svelters-egui;
  `svelters_host::platform::system()` and `Platform::system()` build the desktop platform.
- Feature `system-clipboard` (off by default in svelters-host; on by default in svelters-skelter,
  whose graph already links arboard through skelter-platform). svelters-egui enables it and
  re-exports `svelters_egui::platform::{system, SystemClipboard}`; its primary-selection provider
  API is unchanged.
- Test: svelters-host `tests/system_clipboard.rs` (feature-gated; skips without a display).
  `svelters-egui/tests/os_clipboard.rs` stays: it tests egui-winit's clipboard.

### FQ-108: a skelter-side dump (`svelters-skelter/src/dump.rs`)

- `svelters_skelter::dump(dir, &Kinds)`, `dump_with(dir, &Kinds, Option<&SkelterBackend>, &[BindSource])`,
  `SkelterBackend::dump(dir, bind_sources)`: runs `svelters_host::exports::dump`, then writes
  `kinds.json` (the app kinds' spec fragment, with `behaviors` and draft `bind_sources` when given),
  `kinds.d.ts` (props, `bind:` forms, events, methods, `HTMLElementTagNameMap`), and with behaviours
  `behaviors.d.ts` and `behaviors/<ns>.js`. Only changed files are rewritten (`DumpReport`).
- `spec::BindSource`, `spec::fragment_with`; `SkelterBackend::behavior_tags`.
- Gaps: skelter behaviours carry no param or event spec (typed as `Record<string, unknown>`);
  `MethodSpec` has no optional parameters; draft bind sources are passed in (no registry).
- Test: svelters-skelter `tests/dump.rs`.

## Tests run (this container, `cargo test`; nextest is not installed)

- skelter-core `timed_hover` 1/1; skelter-compositor `--test engine reduced_motion` 1/1;
  skelter-widgets `reduced_motion` 2/2 and `paint::icon::` 27/27; svelters-skelter
  `--test spec` 5/5 (regenerated), `--test dump` 1/1, `--test host_document hover_events` 1/1;
  svelters-host `--features system-clipboard --test system_clipboard` 1/1 (under `tools/dev/headless`).
- `cargo check -p <crate> --all-targets` for skelter, skelter-core, skelter-widgets, skelter-compositor, skelter-test, svelters-skelter, svelters-host, svelters-egui, svelters-automation: clean. The full `--workspace` check needs GTK system libraries this container lacks.
- Not run: the full workspace tests, conformance, pnpm tests (CI runs them).

## For cadatomic (one line per request)

- SV72: `IconRegistry::set_source` + `invalidate`/`invalidate_all` + `host::icons_changed`; a theme switch invalidates and repaints instead of re-registering.
- R1: nothing to call for scroll areas (they read the document's media preferences); the compositor follows `DocumentApi::reduced_motion`, which `CoreDocument` answers: a `DocumentApi` wrapper should forward it.
- R2: forward `DocumentApi::hover_at` to `Document::hover_at` in a `DocumentApi` wrapper; the runner passes the move's time.
- SV-B2-1..3: `onpointerenter`/`onpointerleave` on any widget, `read-only` and `oncancel` on `text-edit`.
- SV-B2-4: `docs/skelter/surface.md`; an app kind such as `empty-state` documents its own props.
- SV75: `svelters_host::platform::system()` (svelters-skelter turns the feature on); the local copy can go.
- FQ-108: `svelters_skelter::dump_with(dir, &kinds, Some(&backend), &bind_sources)` from the app's dump.

## CI
