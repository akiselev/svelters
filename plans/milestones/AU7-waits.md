# AU7: one wait engine for tests, tours and agents (cloud batch 10, lane `waits`)

Branch `ci/b10-waits`, base `master` `5d86f1d0`. Brief: `plans/cloud/batch10/BRIEF.md`, scope
`plans/cloud/batch10/LANE-waits.md`. Design: cadatomic `docs/research/2026-10-24-waiting-and-synchronization.md`
(WAITS; §3 the spec). The user's answers (2026-10-24): built in svelters now; tests replay recorded external sources,
Watch it / Try it run live; the core first.

## Status

Code, tests and docs written; local filtered tests: see "Tests". CI: see "CI".

## What landed

| Item (lane) | Where | What |
|---|---|---|
| 1 Event journal | `svelters_automation::journal` | `Journal` (a `Clone + Send + Sync` handle; one per app run, `Session::set_journal` shares it), `Event { seq, at_virtual_ms, at_wall_ms, kind, subject, fields }`, a bounded ring (`Journal::new(cap)`, default 4096). Typed kinds: `EventKind { id, fields: Vec<Field>, domain, text_key }` with `field`/`optional`/`text_key` builders; `register` (same definition again is a no-op, another is `event.kind_conflict`); `emit` from any thread validates against the schema (`event.unregistered`, `event.invalid`: missing, wrong type, unknown field). Overflow: the ring records the highest dropped `seq`; `since`/`find` over a window that lost events fail with `journal.overflow`. `emit_after(delay_ms, ..)` + `set_virtual_ms` (replay's timing on the virtual clock). SG2's scene journal: **bridged**, not folded: `Session::bridge_scene_changes(filter)` emits each frame's changes as `scene.changed` events; `journal.subscribe`/`journal.take` are unchanged. |
| 2 Cursor waits | `Session::{cursor, wait_event, wait_event_within, never, release_never, check_nevers, end_step, arm_seen, seen, wait_seen, release_seen}`, `Matcher`, `Condition`, `NeverId`, `SeenId` | `wait_event` succeeds on the first event after the cursor that matches and that this session has not consumed, and consumes it (two identical waits match two events). `never(m, since)` fails every later wait (and `end_step`) with `wait.never` (data: the matcher and the event). `arm_seen(cond)` latches a `wait.for` condition on any settled frame after arming (`after_frame`; hosts running their own frames call `observe_frame`). `Matcher`: kind exact or prefix (`job.*`), subject, field equality, optional Rust predicate (`when`). JSON: `journal.cursor`, `journal.events {since, limit}`, `journal.kinds`, `wait.event {matcher|kind…, since (required), timeout, wall_timeout}`, `wait.never`, `wait.seen` (arm with a condition, wait with `id`), `wait.step_end`, `busy.sources`. All are in `session.hello`'s capabilities and go through IPC/MCP unchanged. |
| 3 Named busy sources | `svelters_automation::busy`, `Session::{register_busy_source, busy_registry, set_busy_registry, busy_now}` | `BusySource::new(id, probe).foreground()/.background().wall()/.virtual_clock()`; `BusyRegistry::register` refuses a source without a kind or a domain, or a duplicate id (`busy.invalid`). Probes return `BusyReport { count, reason, since }` and run outside the registry lock. `wait_settled` blocks on the embed's `Busy` bits, pending input, foreground sources and the deprecated `set_idle`; background sources never. A timeout lists the busy foreground sources with count, reason and busy-for, longest first. `set_idle` still works (doc-deprecated, no `#[deprecated]` so cadatomic's `-D warnings` builds stay green). |
| 4 Actionability | `Session::actionable` (private), `Actionability { check, detail, cover }` | Checks: `attached`, `visible`, `enabled`, `editable` (`fill` only: not `read-only` and not a role that never takes text), `hit` (names the covering node: `covered by button 42 "Cover" at (x, y)`, `cover` = its id), `stable` (same rect over two consecutive frames; `still moving: from [..] to [..]`). The failing check is in the message and in the report. |
| 5 Clocks | `ClockDomain`, `Budget`, `Driver::hold_clock`, `Clocked` | Every kind and source declares `Virtual` or `Wall`. While a foreground `Wall` source is busy, `Session::step` calls `Driver::hold_clock(true)` (`Clocked` stops advancing its `VirtualClock`), the frame's time counts as wall time, not session virtual time, and the frame sleeps 1 ms to let the real work run. `Budget { virtual_ms, wall_ms }` (`From<f64>`: both): virtual counts unheld frames; wall counts held frames, and all wall time for a wait on a wall-domain event kind. `wait_settled_within`, `wait_for_within`, `wait_event_within`; JSON `wall_timeout`. |
| 6 Record/replay | `svelters_automation::replay` | `SourceSeam::{live, record, replay}` + `Exchange` (`emit`, `finish`, `replay`); `Recorder`; `Transcript` NDJSON (header `{"type":"transcript","version":1}`, then `event`/`result` lines keyed by source, exchange key and occurrence); `Redactor` (`url` → `redact_url`'s `route:/path/:id`, `intern` → `field#n`, `drop_field`) applied in every mode so live and replay journals are equal; replay emits recorded events after their recorded delays on the virtual clock and returns the recorded result (`replay.missing`, `replay.live_emit`, `replay.invalid`); `SourceSeam::busy_source(kind)` (wall live, virtual in replay). |
| 7 Failure reports | `WaitReport`, `Error::data` | Every wait timeout (`wait.settled`, `wait.for`, `wait.event`, `wait.seen`, actionability) is an `Error` with `data` = `WaitReport::to_json()`: the wait, the budget that ran out and what was spent, unmet conditions with last values, the cursor and the last 20 events since it, busy foreground sources, the embed's busy bits, pending input, `set_idle`'s verdict, the failed actionability check. The message keeps its old first line, then `WaitReport::text()`. JSON-RPC carries it as `error.data.report` (`rpc::outcome` restores it). |
| 8 Docs | `docs/egui/rust/waiting.md`, linked from `automation.md` and `START-HERE.md` | When to use a condition, an event, settle; the APIs; the JSON table; anti-patterns (sleeps, network idle, no events for N ms, settle as success, arming after the act, foreground pollers, raising timeouts). |

## Public API (all additive)

- `svelters_automation::journal::{Journal, Event, EventKind, Field, FieldType, ClockDomain, Cursor, Matcher, DEFAULT_CAPACITY}`
  (re-exported at the root: `ClockDomain, Cursor, Event, EventKind, FieldType, Journal, Matcher`).
- `svelters_automation::busy::{BusyKind, BusyReport, BusySource, BusyStatus, BusyRegistry}` (all re-exported).
- `svelters_automation::replay::{Transcript, TranscriptLine, TRANSCRIPT_VERSION, Recorder, Redactor, Redact, redact_url, SeamMode, SourceSeam, Exchange}`.
- Root: `Budget, Condition, Unmet, Actionability, WaitReport, NeverId, SeenId`.
- `Session`: `journal, set_journal, busy_registry, set_busy_registry, register_busy_source, busy_now, observe_frame,
  bridge_scene_changes, cursor, wait_event, wait_event_within, never, release_never, check_nevers, end_step, arm_seen,
  seen, wait_seen, release_seen, wait_settled_within, wait_for_within`.
- `Driver::hold_clock(&mut self, held: bool)` (default no-op; `Box<D>`, `Clocked`, `RealTime` forward/implement).
- `Error::data: Option<Value>` and `Error::with_data` (the one non-additive change: a new public field on a struct
  without `#[non_exhaustive]`; nothing in svelters or cadatomic builds `Error` with a struct literal).
- New error codes (also in `rpc::CODES`): `journal.overflow`, `event.unregistered`, `event.invalid`,
  `event.kind_conflict`, `wait.never`, `busy.invalid`, `replay.invalid`, `replay.missing`, `replay.live_emit`.
- Security room: `EventKind`, `Field`, `BusyReport`, `BusyStatus`, `Actionability`, `WaitReport` are
  `#[non_exhaustive]`. No principal id: journals and registries are per app run and sessions already carry `Grants`.

## Decisions (lane, within the agreed design)

- **The journal is a handle, not a global.** Each `Session` starts with its own; the app shares one with
  `set_journal`/`set_busy_registry` (cadatomic: made once at start, cloned into every emitter and session). A global
  would leak across tests in one process.
- **Consumption is per session** (each client consumes its own matches), not per journal.
- **`wait.event` requires `since`** over JSON: defaulting to "now" would re-create the arm-after-act race.
- **Settle is not a success condition in the API**: there is no event-wait on "idle"; settle stays a barrier.
- **Replay timing runs on the virtual clock** (deterministic); recorded `after_ms` are wall ms of the live run.
- **Results are recorded as given** (not redacted): the host puts in a result only what replay needs.

## Tests

`crates/svelters-automation/tests/waits.rs` (counter fixture; `fixtures/counter-egui/src/Waits.svelte` for
actionability) and unit tests in `journal.rs`, `busy.rs`, `replay.rs`:
- the arm-before-act race cannot happen (`scene.changed` from inside a click; an emit on another thread before the wait);
- two identical waits consume two events; the third times out with the events in its report; unregistered kinds fail;
- `never` trips waits and `end_step`, then is gone; `seen` latches a label that is gone by the time of the wait;
- background sources never block settle; stuck foreground sources name themselves, longest first; no-kind refused;
- a 30 s virtual wait is not cut off by 1.5 s of wall work behind a foreground wall source (the `VirtualClock` holds);
  a stuck wall source runs out of wall budget and is named;
- a moving node fails `stable` ("still moving"), a covered one `hit` naming the cover's id, `fill` on a button
  `editable`, a missing node `attached`;
- a replayed transcript emits the same events as the live run (unit), and a replayed delayed response holds settle and
  arrives on the virtual clock (integration); redaction removes the URL query and the payload;
- the JSON surface (`journal.cursor`, `wait.event` with and without `since`, `wait.seen`, `wait.never`, `wait.step_end`,
  `journal.events`, `journal.kinds`) and the report in `Error::data`.

Run locally: see the CI section's note.

## CI

(run ids added after the push)

## Handoff to cadatomic's `waits-adopt`

- Make one `Journal` and one `BusyRegistry` at app start; pass clones to the job registry, tool sessions, the HTTP
  client and the worker pool; call `session.set_journal(..)`/`set_busy_registry(..)` on every automation session.
- Register `command.receipt`, `job.*`, `tool.*`, `worker.*`, `net.*`, `fs.*`, `reload.*`, `diagnostic.raised` (WAITS
  §3.2) where the work lives; replace `IdleBarrier`'s unnamed closures with `BusySource`s, then drop `set_idle`.
- The tour runtime: a step takes `cursor()` before its first act, `until` → `wait_event`, `never` → `never` +
  `end_step`, `seen(..)` → `arm_seen`; the validator rules of WAITS §4 stay cadatomic's.
- HTTP client and worker pool: go through a `SourceSeam` (record once, replay in tests, live in Watch it/Try it).

## Not done / follow-ups

- A windowed host must call `Session::observe_frame` after its own frames for `seen` latches and replay timing.
- Progress-event coalescing per job per frame (WAITS §6) is the emitter's job; the journal does not coalesce.
- The TS client (`@svelters/test`) does not wrap the new methods yet (they work through `app.call`).
