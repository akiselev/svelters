# Lane waits: one wait engine for tests, tours and agents (AU7)

Branch `ci/b10-waits`; record `plans/milestones/AU7-waits.md`. The design is cadatomic's
`docs/research/2026-10-24-waiting-and-synchronization.md` (read it whole; §3 is the spec, §2 the precedent). The
user's answers (2026-10-24): built in svelters now; tests replay recorded external sources (network, workers) and
Watch it / Try it run live; the core first. What exists: `Session::wait_settled`/`wait_for`, the embed's `Busy`
bits and `BusyProbe`, `Session::set_idle` (cadatomic's IdleBarrier as one opaque predicate), the SG2 journal
(`journal_subscribe`/`journal_take`), the virtual clock (`clock.rs`), actionability (§1.5 of E2E-INSPECTOR).

1. **Event journal** (§3.2): an app-wide append-only journal `{ seq, at_virtual_ms, at_wall_ms, kind, subject,
   fields }` in a bounded ring (overflow is an error a wait reports, never a silent drop); typed, registered event
   kinds (`EventKind` id, field schema, clock domain, display text key); `emit` from any thread. Fold the SG2 scene
   journal in as kinds, or bridge it, whichever keeps its API.
2. **Cursor waits:** `cursor()`; `wait_event(matcher, since: Cursor, timeout)` succeeding on the first unconsumed
   match after the cursor; `consume` semantics so two identical waits match two events; `never(matcher)` checked
   until a step's completion; `seen(predicate)` latched across settled frames since a cursor (§3.1). JSON methods
   `journal.cursor`, `wait.event`, `wait.never`, `wait.seen` in `Session::call`, so the IPC and MCP surfaces carry
   them.
3. **Named busy sources** (§3.3): `register_busy_source(id, Foreground | Background, Fn() -> BusyReport { count,
   reason, since })`; the registry refuses a source without a kind; `wait_settled` blocks on foreground sources and
   the embed's `Busy` bits only; a timeout reports every foreground source with count, reason and busy-for, sorted
   by duration (Detox's `--debug-synchronization`). Keep `set_idle` working (deprecated) until cadatomic moves.
4. **Actionability** (§3.4): attached, visible, stable (same rect over two consecutive frames: Playwright's
   definition), hit-tested at the action point (no overlay on top, naming the covering node), enabled, editable for
   `fill`; each failing check named at the timeout ("still moving", "covered by <node>"). If it already exists,
   close the gaps against this list and test each check.
5. **Clocks** (§3.5): every event kind and busy source declares a clock domain (`Virtual` or `Wall`); waits budget
   each domain separately; the virtual clock advances only while no foreground `Wall` source is busy (Compose's
   `autoAdvance`), so a virtual timeout never cuts off real work.
6. **Record and replay hooks** (§3.6), generic only: a `Replayer` seam that a host source (an HTTP client, a worker
   pool) can be backed by in test mode, emitting the same journal events from a recorded transcript; a transcript
   format (NDJSON, redaction of URLs and payloads to ids); recording through the same seam. cadatomic supplies the
   sources.
7. **Failure reports:** a timeout of any wait carries the unmet conditions with their last values, the events since
   the cursor, the busy foreground sources and the failed actionability check, as one structured value and its text.
8. **Docs:** a `docs/egui/rust/waiting.md` page (when to use a condition, an event, settle; the anti-patterns:
   sleeps, "network idle", "no events for N ms", settle as success), linked from `automation.md`.

Tests: the arm-before-act race cannot happen (an event emitted inside the act is seen by a wait taken after it);
two identical waits consume two events; a background source never blocks settle; a stuck foreground source names
itself in the timeout; a virtual 30 s timeout does not fire while a `Wall` source is busy; a moving node fails "still
moving" and a covered one names the cover; a replayed transcript emits the same events as the live run.
