# R1: host batching, compare, read-back; reactive getters (plans/research/REACTIVITY.md §10 R1, decisions §13)

Lane: `crates/svelters-host`, `crates/svelters-backend-log`, `packages/testhost`, the GTK backend's observe
path and tables (`crates/svelters-gtk`, `tools/metadata/gtk/gen_backend.py`), `packages/native-runtime/src/observe.js`,
`@svelters/types` core, the GTK and Qt types generators. Branch `ci/r1`. Out of scope: compiler `bind:` lowering and
`$n.bind_*` (R2), the Qt backend's QBindable/event/signal mechanisms (R3), AppKit/WinUI `observe` (R4).

## What changed

```text
native change ─prop_changed(h,p)─▶ dirty set (ordered, dedup; wake on first insert)
                                        │ checkpoint
                                        ▼
                     get_prop (skipped: SVL_M_PROPORTIONAL) ─▶ compare with last ─▶ drop equal
                                        │
                     __svelters_changed([h, p, v, ...])  ◀── one batch, before queued events, no drain in between
                                        │
         native-runtime tracker (observe.js): update() of each subscriber, observeProp callbacks
                                        │
                              Svelte flushes once (job drain after the first event or at the end)
```

### Host (`crates/svelters-host/src/core.rs`, `core/ops.rs`)

Every rule of REACTIVITY.md §5.2; the contract is written down in `plans/architecture/backend-abi.md`
"ABI v2.6 additive / Host contract":

- `Observed { count, name, proportional, last, force }` per (h, prop id). The 0→1 transition calls the backend and reads a
  baseline `last` (unless proportional), so a GTK re-notify of an identical value is dropped from the start.
  (FU3 removed the baseline read: `last` starts unknown; see REACTIVITY.md §5.2 "No observe-time baseline".)
- Dirty set: `Vec` + `HashSet`, first-announcement order; `prop_changed` returns true on the first insert into an empty
  set, and `Callbacks::prop_changed` wakes the backend then (one wake per turn).
- `take_changes` reads each dirty pair still observed (`get_prop`, never JS), compares (`same_value`: handles by id,
  strings by content across UTF-8/UTF-16, lists/records/maps deeply, f64 by bits with NaN = NaN), keeps the differing
  ones and updates `last`. A value with a `BOXED` handle is always delivered and never kept; reads nobody sees are
  released.
- `deliver_changes` runs at the start of every checkpoint (live and scripted) and before `call_sync` runs JS.
- Set path (`after_set`): after a successful `set_prop` on an observed, non-proportional pair, read back once into
  `last`; if the read-back differs from the written value, the pair is forced dirty (delivered once, normalized).
- `destroy`/`on_destroyed` drop observations and dirty entries; reload clears the dirty set with the event queue.
- `JsEngine::dispatch_changed(&[ChangedProp])` replaces `dispatch_prop_changed`; R1a's `__svelters_prop_changed` is gone.

### ABI: one additive member flag

`SVL_M_PROPORTIONAL = 128` on prop descriptors (Rust `svelters_abi::SVL_M_PROPORTIONAL`, C header). The host must
know a prop is proportional to skip the read, and the API R2 codes against (`observeProp(h, prop, fn)`) carries no
cost hint, so the backend says it in the descriptor, from its facts. Default 0 (cheap): backends that do not set it
keep working. `observe`/`prop_changed` are unchanged.

### Log backend, TestHost, op log

- Scripts: `{"changed": [h, prop, value?]}`. The value becomes the native value; the log line is
  `changed #h prop <json>`.
- The TestHost implements the same compare (baseline, read-back after `setProp`, deep equality) and calls
  `__svelters_changed([h, prop, value])`.
- `tools/oplog-compare.mjs` parses the value.

### JS

- `packages/native-runtime/src/observe.js` (exported from `index.js`: `observeProp`, `trackProp`, `trackLocal`,
  `invalidateProp`). It is built on a port of Svelte's `createSubscriber` in the core adapter (`core/svelte.js`
  `subscriber`), because Node resolves `svelte/reactivity` to the server no-op without the `browser` condition.
- The table is keyed by the host object, then the handle number, then the canonical prop name (dashes and
  underscores removed, lowercased). It never holds a wrapper.
- `@svelters/types` core gains:
  - `installTracker`, `observed`, `observedLocal`, `observedSignal`, `invalidate`, `untracked`;
  - `NativeObject.peek(prop)`;
  - `setProp` now invalidates locally.
- Generated getters, by class:

  | Class | GTK (`gen_types.py`) | Qt (`gen_types_js.py`) |
  |---|---|---|
  | exact, partial | `observed` | `observed` |
  | set_only | `observedLocal` | `observedLocal` |
  | constant, none, no fact | `getProp` | `getProp` |

  Counts: GTK: 413 `observed`, 0 `observedLocal`. Qt: 60 `observed`, 28 `observedLocal`.

  Typings get `@remarks` for none and partial props, and for proportional ones. The docs pages of the curated
  types mark each property as reactive, reactive on writes or not reactive. There is a new concepts page,
  `docs/site/src/content/docs/concepts/reactivity.md`.
- Qt `QTextDocument.plainText` has no Q_PROPERTY: `toPlainText()` re-read after `contentsChanged` (`observedSignal`,
  table `SIGNAL_ACCESSORS` in `gen_types_js.py`). This is what makes the §7 example mirror-free on Qt before R3.

### GTK backend

`crates/svelters-gtk/src/backend/observe.rs` (header comment has the details), tables from
`tools/metadata/gtk/gen_backend.py`:

- The observe facts of `metadata/observe/gtk.json` are in the generated `OBSERVE` table, keyed by (declaring GType,
  property). `describe` reports `SVL_M_OBSERVABLE` only for `exact`/`partial` facts (R0's open item: the 40
  rule-made constants no longer report observable) and `SVL_M_PROPORTIONAL` for `value_cost: proportional`
  (TextBuffer/TextView `text`). A prop without a fact keeps the mechanical rule (readable, not construct-only).
- `via` (O9, GtkRange `value` on its adjustment): watched on the target object; the element's own
  `notify::<via>` re-targets the handler and reports the prop changed.
- Echo: proportional props only. `set_prop` holds an `EchoGuard` for the (h, prop) it writes, so that `notify`
  is dropped; cross-notified props and other handles on the same object still report (§5.4, §13 #7).
- Teardown: one connection and a count per (h, prop), disconnected by the last `observe(off)` and by
  `release_entry`. Handlers capture ids, the host cell and (for `via`) a weak target ref.
- `tests/oplog_replay.rs` replays `observe`/`changed` lines; `tests/observe.rs` (14 tests) covers via
  re-targeting, the proportional echo, descriptors and teardown.

## Acceptance

- NON-GUI-TYPES §7 without a `$state` mirror: `fixtures/gtk-types` (`buffer.text.length`, script step
  `{"changed": [1, "text", "Hello, world"]}`) and `fixtures/qt-types` (`doc.plainText.length`).

- `host_bench` "coalesced" numbers hold with real batches (see the completion record).
- Generated observe tests in the GTK ledger: `generated::verify::observe_<owner>` rows, refreshed and checked.

## Completion record

### Commits (branch `worktree-agent-acc4edf519da1e53a`, pushed as `ci/r1`)

| Commit | What |
|---|---|
| 537699f9 | host dirty set, read + compare, batch before events, set-path read-back, sync flush; `SVL_M_PROPORTIONAL`; scripted `changed` (log backend, TestHost) |
| dcbfc63a | native-runtime tracker (`observe.js`); `@svelters/types` `observed`/`observedLocal`/`installTracker`/`peek`; setters invalidate locally |
| 9a3fe3f7 | reactive getters by class (GTK `gen_types.py`, Qt `gen_types_js.py`), `@remarks`, docs; `QTextDocument.plainText`; ABI v2.6 host contract in `backend-abi.md` |
| 5ccff2a1 | GTK observe facts in tables, `via` re-targeting, proportional echo suppression, op-log replay, `host_bench` tracker scenario |
| a96eb7d5 | `host_bench`: report timer fires before the second checkpoint |
| 4b827809 | this milestone draft |
| 0e14fa9f | merge `master` (26e97040, `cargo-slot` sccache fix); no conflicts |
| 57fa79bd | test pinning the own-write contract for R2 (`observe.test.mjs`) |

The work was taken over mid-milestone; the earlier `svelters-host` test failures in the predecessor's log (`reload.rs`, 4
failures) and the `lists_e2e` failure were build-order artifacts (no `node_modules`, fixture not built), not regressions.

### Gates

nextest, `pnpm test`, the bench and CI ran on 57fa79bd (the last code change; this record is the only later edit). The
ledger refresh/check, `pnpm -r build` and the docs gates ran on 0e14fa9f, which differs from 57fa79bd only by the one added
test in `packages/native-runtime/test/observe.test.mjs`.

| Gate | Result |
|---|---|
| `tools/dev/headless tools/dev/cargo-slot cargo nextest run --workspace --no-fail-fast` | 2309 run, **2309 passed**, 7 skipped, 0 retries |
| `cargo nextest run -p svelters-host` | 60/60 |
| `ledger.py refresh --toolkit gtk` | 2678 facts, 2703 tests, 0 manual / 0 inferred not passing, 5 parked (no ledger diff) |
| `ledger.py refresh --toolkit qt` | 845 facts, 1922 tests, 0 not passing, 0 parked (no ledger diff) |
| `ledger.py check` | ok: gtk 550 manual + 2128 inferred (5 parked); qt 634 + 211; appkit 79 + 147; winui 60 + 335 (8 parked) |
| `cargo build -p svelters-compiler`, `pnpm -r build` | ok |
| `pnpm test` | ok: types 19/19, style 17/17, native-runtime 79/79 (svelte-check 0 errors), canvas 3/3, lists 2/2, 40 fixtures PASS, native-compiler compiled 79 PASS + 12 expected rejections, handwritten 6 PASS; `test:meta` 140 passed, 1 skipped, all `--check`s ok |
| `python3 tools/docs/build_ir.py`, `pnpm docs:test` | 11 OK (1 skipped); 17/17 node |

`pnpm test` was re-run on 57fa79bd after the own-write test was added (78 → 79). CI below ran the same tree.

### CI (`ci/r1` @ 57fa79bd)

| Workflow | Run | Result |
|---|---|---|
| js | 39546244107 | success |
| linux-gtk (jobs `gtk`, `gtk-4-14`) | 39546244150 | success, both jobs |
| linux-qt | 39546244212 | success |
| macos-appkit | 39546244093 | success |
| windows-winui | 39546244074 | success |

### Bench (`host_bench`, N = 10,000, release, GTK 4.24 under Xvfb on WSL2, 3 runs)

| Scenario | Result | REACTIVITY §6 |
|---|---|---|
| `getProp` from JS | 2.65-4.32 µs | 3.7-6.9 µs |
| delivery per event, empty handler (`deliver-noop`) | 14.6-17.4 ms per 10k | 27-35 ms |
| delivery + `getProp` read-back | 46.9-57.7 ms per 10k | 75-91 ms |
| `createSubscriber` per event | 836-1006 ms per 10k, 10,001 effect runs | 1.72-1.98 s |
| `createSubscriber`, one update for the burst | 30.4-34.4 ms, 2 effect runs | 55-70 ms |
| **R1 tracker, real host batch** (`r1-tracker-batched`) | **0.2 ms checkpoint**: 1 batch, 1 entry, 2 effect runs, last value 10000 | (new) |
| same 10k native changes, unobserved | checkpoint 0.0 ms; GI loop 60-77 ms vs 70-73 ms observed | |

The coalesced bound holds and is beaten: with real batching the 10k `notify`s collapse into one dirty entry, so the
checkpoint does one read, one `__svelters_changed` and one effect re-run. The cost of observing on the native side
(`notify` → `prop_changed` → dirty-set insert) is within the noise of the GI loop (< 1 µs per change).

### API guarantees for R2 (`packages/native-runtime/src/observe.js`, replaces R2's stand-in wholesale)

- `observeProp(h, prop, fn) -> unsubscribe`: `fn(value)` runs once per **delivered** change; `value` is the host's
  native read, `undefined` for proportional props (read live). Each call holds one host-side observation
  (`host.observe` refcount), released by `unsubscribe()` (idempotent). `unsubscribe.observed` is false when the
  host refused (no notification for that prop on this backend, or a dead handle); `fn` then never runs. A throwing
  `fn` is logged and does not stop the others.
- `trackProp(h, prop)`: subscribes the running effect (no-op outside effects and inside `untrack`); the caller reads
  live. Teardown one microtask after the last effect stops (`createSubscriber`).
- Prop spellings are canonicalised (`cursor-position` = `cursorPosition`).
- `globalThis.__svelters_prop_changed` **no longer exists**; the host calls only `__svelters_changed(batch)`.
  R2's stand-in hook is dead code after the merge.
- **Own writes (the `bind-observe` risk):** after a successful JS `setProp(h, p, v)` on an observed,
  non-proportional pair the host reads `p` back into `last`.
  - Its announcement (the toolkit's echo, or a scripted `{"changed": [h, p]}` without a value, which leaves the
    native value at `v`) compares equal and is **dropped**: `observeProp` callbacks do **not** run. Writing the
    current value announces nothing either.
  - If the toolkit normalised the write (read-back ≠ `v`), the pair is forced dirty and the normalised value is
    delivered **once** to `fn` and the trackers.
  - `trackProp` readers re-run for own writes through local invalidation (`invalidateProp` in the generated
    setters / `@svelters/types setProp`), not through the host. `invalidateProp` does not call `observeProp`
    callbacks.
  - Proportional props are never read: the backend suppresses the notify of the prop it is writing (GTK
    `EchoGuard`); a cross-notified prop still arrives as `undefined`.
  - Not observed at write time: no read-back; the 0→1 `observe` reads the baseline then.
  - So `bind-observe` must drive native→state with value-carrying steps (`{"changed": [h, "text", "<new>"]}`,
    a value different from what JS last wrote). Pinned by the TestHost test "observeProp is not called for the
    echo of our own setProp, only for a different native value" and by the host test
    `our_own_write_is_read_back_its_echo_dropped_and_a_normalized_value_delivered_once`.

### Open items

1. R2 merge: take this `observe.js`; switch `fixtures/bind-observe(-qt)/script.json` to value-carrying `changed`
   steps and regenerate their `expected.oplog`; `bind-observe.js`' own `host.observe` coexists (host refcounts).
2. R3/R4 merges change `metadata/observe/<tk>.json`: rerun `gen_types_js.py`, `gen_semantics.py`, the docs IR and the
   ledgers; do not hand-merge generated files. AppKit/WinUI backends must set `SVL_M_PROPORTIONAL` themselves
   (default 0 = read every time, correct but O(n) for large text).
3. 309 GTK `exact` facts are still only `connect`/`refused` verified (R0); delivery evidence comes from the
   `observe.rs`/`host_observe.rs` tests, not from those rows.
4. R5: the JS value cache from the batch (saves one `getProp` per re-run) stays optional.
5. `docs/notes/scripts/reactivity/host_bench/Cargo.lock` is untracked (standalone crate); left out on purpose.

### Conflict risks at merge

- `packages/native-runtime/src/observe.js` and `src/index.js` export line (R2).
- `crates/svelters-gtk/tests/oplog_replay.rs` and `examples/counter-qt/tests/oplog_replay.rs` (R2 edits both for
  `observe`/`changed` lines; R1 also changed the GTK one, 5ccff2a1).
- Generated files: `crates/svelters-gtk/src/generated/tables.rs`, `packages/types` generated `.d.ts`/`.js`, Qt
  types output, `metadata/semantics/*`, docs IR: regenerate after merging.
- `plans/architecture/backend-abi.md` (ABI v2.6 host contract section) if R3/R4 document their mechanisms there.
