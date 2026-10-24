---
title: Waiting on anything
description: "One wait engine for tests, tours and agents: conditions, the event journal with cursors, named busy sources, actionability, clock domains, record and replay, and the failure report."
status: available
verified_at: AU7
sources:
  - crates/svelters-automation/src/{journal,busy,replay}.rs
  - crates/svelters-automation/src/session/waits.rs
  - crates/svelters-automation/tests/waits.rs
  - plans/milestones/AU7-waits.md
---

# Waiting on anything

A test, a tour step or an agent waits for one of three things, and each has its own tool:

| You wait for | Use | Example |
|---|---|---|
| a **state** (the marker is on the board) | a condition: `wait_for`, `expect(..)` | `s.wait_for("role=label", Visible, Some(&text("Count: 1")), 5_000.0)` |
| an **occurrence** (the DRC job finished) | an event after a cursor: `wait_event` | `s.wait_event(&Matcher::kind("job.finished").subject("drc"), at, 30_000.0)` |
| **quiet** between two acts | settle: `wait_settled` (acts do it for you) | named busy sources decide what counts |

**Default to a condition.** Use an event when the thing has no lasting state, or when the state cannot
tell two runs apart (the second DRC run leaves the board as the first did). Settle is never the success
condition of a step: it only says nothing foreground is busy, not that what you wanted is ready.

## Conditions

`wait_for(selector, state, expect, timeout)` re-resolves the selector after every settled frame and
succeeds when the matches are in `state` and meet `expect`. A timeout names the condition and its last
value (`count 0 (want 1)`).

**Latched conditions** catch state that comes and goes (a toast, a progress value): `arm_seen(cond)`
returns a `SeenId`; the latch holds once `cond` held on any settled frame since it was armed, and
`wait_seen(id, timeout)` waits for that.

```rust
// not run
let toast = s.arm_seen(Condition::new(r#"text="Saved""#, WaitState::Visible, None))?;
s.click("role=button[name=Save]", ActOptions::default())?;
s.wait_seen(toast, 5_000.0)?;           // passes even if the toast is gone by now
```

## Events: the journal and cursors

The app has **one journal per run** (`Journal`, a cheap handle, `emit` from any thread). Share it with
the session: `session.set_journal(app_journal.clone())`.

1. **Register the kind** where the work lives. A kind has an id, a field schema, a clock domain and a
   display text key. Emitting or waiting on an unregistered kind fails (`event.unregistered`): not
   implemented means not registered.
2. **Emit** from the code that owns the work: `journal.emit("job.finished", "drc", json!({"status": "ok"}))`.
3. **Take a cursor before the act, wait after it.** An event emitted inside the act has a sequence
   number after the cursor, so it is seen: the arm-before-act race (Playwright's `waitForResponse` after
   the click) cannot happen.

```rust
// not run
journal.register(EventKind::new("job.finished", ClockDomain::Wall).field("status", FieldType::String))?;
let at = s.cursor();
s.click("role=button[name=Run DRC]", ActOptions::default())?;
let e = s.wait_event(&Matcher::kind("job.finished").subject("drc"), at, 30_000.0)?;
```

- **Consumed.** A wait consumes the event it returns; two identical waits need two events.
- **Never.** `never(matcher, at)` fails every later wait of the step (`wait.never`) when a matching event
  appears after `at`; `end_step()` checks once more and drops the guards and latches.
- **Matchers**: `kind` exact or a prefix (`job.*`), `subject`, `field(name, value)`, and `when(name, fn)`
  for a Rust predicate.
- **Bounded.** The journal is a ring (4096 events by default, `Journal::new(n)`). A wait whose window lost
  events fails with `journal.overflow`; nothing is dropped silently.
- **Scene changes** become `scene.changed` events with `bridge_scene_changes(filter)`; the
  `journal.subscribe`/`journal.take` pair is unchanged.

## Busy sources: what settle waits for

The code that owns background work registers a **named** source:

```rust
// not run
s.register_busy_source(
    BusySource::new("jobs", move || BusyReport::busy(jobs.running(), jobs.names())).foreground().wall(),
)?;
```

- **Foreground** sources hold settle; **background** sources never do. A source must choose, and must name
  its clock domain; the registry refuses it otherwise. A source is foreground only if a user action waits
  on it: polling, file watchers, a long-lived connection and an indeterminate spinner are background.
- A settle timeout lists every busy foreground source with its count, reason and how long it has been
  busy, **longest first**. Fix the first one that repeats; don't raise the timeout.
- `set_idle(fn)` (one opaque predicate) still works and is deprecated: its timeout can only say `HOST`.

## Actionability

Before a pointer act the target must be **attached**, **visible**, **stable** (the same rect over two
consecutive frames), **enabled**, the **hit** target at its centre (nothing on top), and for `fill`
**editable**. At the timeout the failing check is named: `still moving: from [...] to [...]`,
`covered by button 42 "Cover" at (100, 60)` (with the cover's node id in the report), `disabled`,
`not editable`. `force` skips the checks.

## Clocks

Every event kind and busy source names its domain: **virtual** (animations, timers, debounces: the
session's virtual clock) or **wall** (jobs, workers, network, disk). A wait has a `Budget` per domain
(`From<f64>` gives both the same number; JSON `timeout` and `wall_timeout`):

- Virtual ms count frames while the virtual clock moves.
- While a **foreground wall** source is busy the virtual clock **holds**: frames still run, their time
  counts as wall time, so a 30 s virtual timeout never cuts off a real solver.
- A wait on a wall-clock event kind counts all wall time against the wall budget.

Live (Watch it, Try it) everything is wall time; the same waits apply.

## Record and replay of external sources

A source that talks to the outside (an HTTP client, a worker pool) goes through a `SourceSeam`:

- **live**: does the work and emits through `Exchange::emit`;
- **record**: live, and writes every event and result to a `Recorder`'s `Transcript` (NDJSON);
- **replay**: does no work; `Exchange::replay()` emits the recorded events after their recorded delays
  on the virtual clock and returns the recorded result.

A `Redactor` turns URLs into route ids (`route:/api/parts/:id`) and payload fields into stable ids
(`body#1`) in every mode, so a live run and its replay put the same events in the journal and the
transcript holds no secrets. `seam.busy_source(kind)` is the seam as a busy source (wall clock live,
virtual clock in replay). Tests replay; Watch it and Try it run live.

## The failure report

Every wait timeout is an `Error` whose `data` is a `WaitReport` (`code: "timeout"` or
`"node.not_actionable"`): the wait, the budget that ran out and what was spent, the unmet conditions with
their last values, the events since the wait's cursor, the busy foreground sources, the embed's busy bits,
and the failed actionability check. The message is its text:

```
no `job.finished drc` event after cursor 12
wait.event job.finished drc ran out of wall time (180 ms virtual, 30000 ms wall spent)
  unmet: event job.finished drc — last: no unconsumed match among 3 events since cursor 12
  busy: jobs ×1 (drc on board 1) for 30000 ms wall
  events since cursor 12:
    #13 job.started drc @2000ms
    #14 job.progress drc {"pct":40} @2200ms
```

Over JSON-RPC the report is `error.data.report`.

## JSON methods

| Method | Params | Result |
|---|---|---|
| `journal.cursor` | | `{cursor}` |
| `journal.events` | `since`, `limit` | `{events, cursor}` |
| `journal.kinds` | | the registered kinds |
| `wait.event` | `matcher: {kind, subject, fields}` (or those at the top level), `since` (required), `timeout`, `wall_timeout` | the event |
| `wait.never` | a matcher, `since` (default: now) | `{id}` |
| `wait.seen` | `selector`, `state`, `expect` (arm) / `id`, `timeout` (wait) | `{id, seen}` |
| `wait.step_end` | | `{}` or `wait.never` |
| `busy.sources` | | `{sources, busy}` |

`wait.settled` and `wait.for` take `wall_timeout` too.

## Anti-patterns

- **Sleeps** (`thread::sleep`, a fixed `pump(ms)`): they fail when the app is slower and waste time when it
  is faster. Wait for a condition or an event.
- **"Network idle"** or **"no events for N ms"**: apps poll, stream and hold connections; quiet is not
  readiness. Wait for the response event you need.
- **Settle as success**: `wait_settled` then assert. Settle says nothing is busy, not that the thing you
  wanted happened; assert with a condition that retries.
- **Arming after the act** with your own listener: take the cursor before the act.
- **A foreground source that never goes quiet** (a poller): settle times out forever. Make it background.
- **Raising a timeout** without a reason: read the report's busy list first.
