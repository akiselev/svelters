# MAL1-FEEL: live feel for skelter (SV61–SV66)

Lane `S-A feel` of cadatomic's MAL1 batch (`plans/cloud/mal1/LANE-SA-feel.md`), branch `ci/mal1-feel`,
base master `6c1ea458`. Coordinator: cadatomic's batch coordinator session. Every seam is generic:
no app names, keys or concepts.

**Status:** done on the branch, waiting for the coordinator's merge (see "CI").

## What landed

### SV65: a custom-property change restyles descendants (`skelter-core/src/style/cascade.rs`)

- `svelters_style::ComputedStyle::custom_eq` (pointer-equal first, then by value).
- `restyle_node` sets `inherited` when the node's custom map changed, so descendants that read a
  `var()` restyle even when no inherited longhand changed.
- Test: `tests::style::a_custom_property_change_restyles_descendants` (a non-inherited
  `background-color: var(--accent)` two levels down; it fails without the fix).

### SV64: the key hook gets a context (`skelter-core/src/input/dispatch.rs`)

- `Document::set_key_hook(KeyHook)`, `KeyHook = Box<dyn FnMut(&KeyHookCx) -> bool>`.
- `KeyHookCx` is `#[non_exhaustive]`: `doc: &Document`, `window`, `target: NodeId` (the focused node,
  else the window root), `key: &KeyEvent`. A later pre-dispatch pointer filter can share it.
- **Breaking** for callers: `|window, key| …` becomes `|cx| …` with `cx.window`, `cx.key`.

### SV62: a live `Policy` (`skelter-widgets/src/host.rs`, `skelter/src/{backends,app}.rs`)

- `skelter_widgets::host::set_policy(doc, policy)`: sets the service and the document flags copied
  from it (`auto_default`, single selection), then re-records every node (`Document::rerecord_all`,
  new; no restyle, so a scrubbed value stays cheap). `host::install` uses the same path.
  `host::policy_of(doc)` reads the document's policy, else the OS's.
- Readers read on use: the table reads `Policy` at each event and prepare (its drag threshold and
  double-click time come from it, not `TableMetrics`' 4.0 px / 500 ms); a focused `text-edit` picks a
  new policy up at its next event, without refocusing.
- Caret blink: `CoreDocument::caret_blink` reads the document's policy each time
  (`skelter::caret_blink_of`); `App` sends a changed blink to every open window after a commit.
- `Document::service_ref` (read-only service access), new.
- Tests: `text::kind::tests::a_focused_field_follows_a_new_policy_without_refocusing`,
  skelter-test `harness::a_caret_blink_change_reaches_open_windows`, and the SV61 test below.

### SV61: the `title` prop shows the `tooltip` surface (`skelter-core/src/tips.rs`, `views/tooltip.rs`)

- skelter-core, generic: `tips::TitleTips` (a provider trait), `tips::TitleInput`
  (`#[non_exhaustive]`: the hovered host and its text, `pressed`, `scrolled`, `escape`),
  `Document::{set_title_tips, title_host, set_window_tip, window_tip_layer}`. The document steps the
  provider after each dispatch of a pointer, wheel or Escape event and each hover change; the
  provider's wake-up joins `next_timer`/`run_timers`. The tip draws in a per-window click-through
  layer above the overlay stack and below the drag image (`TIP_Z`).
- skelter-widgets: `views::tooltip::TitleTooltips` runs `Tooltip::step` with the document's `Policy`
  read on each step (delay, follow-on), records the `tooltip` surface with the theme and text system,
  and places it with `Tooltip::place`. `host::install` installs it.
- `interact::Tooltipable` is now a unit over the one `TooltipTimer` (its old copy of the timing
  logic is gone).
- Tests: skelter-core `tests::overlay::the_title_host_reaches_the_tip_provider_and_its_layer_tops_the_overlays`;
  skelter-widgets `views::tooltip::tests::a_title_shows_its_tooltip_after_the_live_policy_delay`
  (hover, GTK's 500 ms, the layer; `set_policy` with 1200 ms changes the next hover; a press hides).
- Not done: the tooltip's AccessKit node (the host's description) and a tooltip on a disabled host
  that the hit test skips. Neither was asked for.

### SV63: settings changes reach JS (`svelters-host`)

- `Host::set_dx` subscribes to `SettingsBackend::subscribe`. The callback (any thread) queues the keys
  and wakes the host; the next checkpoint calls `JsEngine::settings_changed(keys)` (new, default no-op),
  which calls `globalThis.__svelters_settings_changed`, installed by `svelters:settings` as
  `refresh(keys)`: `s:<key>` is invalidated and the effects that read it re-run.
- Test: `tests/dx.rs::a_backend_change_reaches_the_effects_that_read_it` (a write from another thread,
  one checkpoint, the effect has re-run).

### SV66: host-set media preferences (document half only)

- `skelter_core::MediaPreferences` (`#[non_exhaustive]`: `reduced_motion`, `contrast`) and
  `Document::{set_media_preferences, media_preferences}`: `prefers-reduced-motion` and
  `prefers-contrast` match them; a change restyles. With reduced motion CSS transitions don't start
  (values jump; a running UI transition stops). `skelter_core::Contrast` re-exported.
- Test: `tests::style::media_preferences_match_queries_and_reduced_motion_jumps`.
- **Left for MAL2: the OS probe** (macOS `accessibilityDisplayShouldReduceMotion`/`…IncreaseContrast`,
  Windows `SPI_GETCLIENTAREAANIMATION`/`SPI_GETHIGHCONTRAST`, Linux the portal's `contrast` and the
  animations setting), with change events. It touches three platform crates and can't be compiled
  for macOS or Windows in a Linux container, so it is over the lane's 0.5 d bar. The host can drive
  the document from its own setting today.

## Lines for cadatomic's `04-kernel-requests.md`

| Id | State |
|---|---|
| SV61 | done (MAL1 lane S-A, `ci/mal1-feel`): `title` → `TitleTooltips` (installed by `host::install`), timing from the live `Policy`; core seam `tips::TitleTips` |
| SV62 | done: `skelter_widgets::host::set_policy`; table and text field read `Policy` on use; caret blink follows into open windows |
| SV63 | done: `Host::set_dx` subscribes; changed keys invalidate `s:<key>` at the next checkpoint |
| SV64 | done: `set_key_hook(Box<dyn FnMut(&KeyHookCx) -> bool>)`, `KeyHookCx { doc, window, target, key }` (`#[non_exhaustive]`); breaking for existing hooks |
| SV65 | done: custom-map change sets `inherited` (pointer-equal first), with a test |
| SV66 | document half done (`Document::set_media_preferences`; reduced motion stops CSS transitions); OS probe → MAL2 |

## CI

See the last section, filled in after the run.

## Not run locally

The full workspace nextest, the conformance run, and egui/GTK/Qt/AppKit/WinUI tests. Only the
filtered tests named above ran, plus `cargo check` of skelter-core, skelter-widgets, skelter,
svelters-skelter (all targets) and svelters-host. `svelters-host`'s `tests/dx.rs`
`settings_ui_state_paths_and_logs` fails now and then under plain `cargo test` (threads in one
process: tracing's callsite interest cache hides its capture). It does this on master too, without
this lane's test; nextest runs each test in its own process.
