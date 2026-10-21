# FU1: GTK panic on kinds outside the tables, non-blocking `start` on AppKit and WinUI

Follow-ups from OV1 (open item "GTK backend panics on kinds outside the tables") and G0
(`plans/milestones/E0-rust-exports.md`, "AppKit and WinUI keep NULL").

## 1. GTK: no panic formatting a "does not belong to" error

`crates/svelters-gtk/src/backend.rs` formatted the error for a table member used on the wrong handle with
`KINDS[kind - 1].gtype`. A handle whose GType the tables do not name has kind 0 or a reflected kind
(`reflect::KIND_BASE + n`), so that index panicked (the C entry points turn the panic into a failure, but
the message was lost and a panic is not the contract). OV1 fixed the same thing in `backend/observe.rs`
only. Every such message now names the runtime GType (`obj.type_().name()`, as OV1 did):

| Path | Before | After |
| --- | --- | --- |
| `set_prop` (table prop) | `KINDS[kind - 1]` | `obj.type_().name()` |
| `get_prop` (table prop) | `KINDS[kind - 1]` | `obj.type_().name()` |
| `call` (table method) | `KINDS[kind - 1]` | `obj.type_().name()` |
| `listen`/`listen_ex` (table signal) | `KINDS[kind - 1]` | `obj.type_().name()` |
| `insert` (slot of the parent) | `KINDS[pk - 1]` | parent widget's GType |
| child attributes (slot of the parent) | `KINDS[pk - 1]` | parent widget's GType |
| `observe` | fixed by OV1 | unchanged |

The rest of the `KINDS[...]` uses index with a kind that comes from the tables themselves (`p.kind`
of a `PROPS` entry), so they cannot be out of range. Today `insert`/child attributes cannot reach the
bad index (a widget always has a table ancestor, `GtkWidget`); they were changed so the pattern is gone.

Other backends checked for the same pattern:
- **Qt** (`tools/metadata/qt/templates/backend.cpp.in`, `gen_cpp.py`): not affected. The generated
  `gen_set_prop`/`gen_get_prop`/`gen_call`/`gen_listen` `switch (e.kind)` and fall through to
  "unknown property id for this kind"; no table is indexed by the handle's kind.
- **AppKit**: not affected (`prop_spec`/`call`/`listen` say "does not apply to this kind"; no indexing).
- **WinUI**: not affected (`table::kind(kind).map_or("?", ..)`).

Tests: `crates/svelters-gtk/tests/foreign_kinds.rs` (2): a plain GObject subclass made through the
typelib (`Gio.Menu` → `GMenu`, reflected kind) and a type the probe skipped (`Gtk.MediaFile.new()`
returns `GtkNoMediaFile`, or the media backend's private subclass such as `GtkGstMediaFileBuiltin`
when GStreamer is there; the test asserts the type is not in `KINDS`). Each one gets a table prop
(`label:label`) through `set_prop` and `get_prop`, a table method (`progress-bar:pulse`) through
`call`, a table signal (`button:clicked`) through `listen`, and `observe`. Each must return `-1` with
"does not belong to <runtime GType>". With the old `set_prop` line restored, both tests failed with the
panic at `backend.rs:840`. They pass with the fix (locally and on CI for GTK 4.14 and the current GTK).

## 2. ABI v2.7 `start` on AppKit and WinUI

Same semantics as GTK/Qt (`backend-abi.md` "ABI v2.7 additive (G0)"):

- **AppKit** (`crates/svelters-appkit/src/backend.rs`): `supports_start` → true; `start(Some(host))`
  stores the host and wakes the run-loop source, so the first loop turn checkpoints (as `run` does).
  `AppKitBackend::new` already sets up `NSApplication`, its menu and the run-loop sources, so nothing
  else needs to come up; the caller pumps the run loop. `start(None)` clears the host. Wakes, timers and
  frames read the host when they fire, so nothing reaches a detached host.
- **WinUI** (`crates/svelters-winui/src/backend.rs`): `supports_start` → true; `start(Some(host))` stores
  the host, posts the type events raised before it attached (`flush_early`, as `attach_host`/`run` do),
  and queues a wake on the dispatcher queue (`wake_shared`, now shared with `waker()`). `start(None)`
  clears the host. `WinUiBackend::new` already initialised XAML and the queue; the caller pumps (`pump`).
- `run()` is unchanged on both. `attach_host` still works (the host's pre-attach passes the same pointer).
- **Attach before mount** now holds on all four desktop backends: `Host::load_bundle` attaches through
  `start` before mount, and `Drop` detaches (before, AppKit test hosts stayed attached after drop).

Tests (mirroring `crates/svelters-gtk/tests/host_start.rs`):
- `examples/counter-mac/tests/counter.rs` (macOS CI): `start_with_mounts_in_the_hook_while_the_test_pumps`
  (hook ran when `start` returned, a checkpoint on the first pump, the counter mounted, a real click reaches
  the host; after `stop` neither a wake nor a click reaches it), `run_with_mounts_in_the_hook_of_the_loop`
  (hook fires inside `[NSApp run]` and quits it), `desktop_order_mount_has_the_host_attached` (no
  `attach_host`: a wake and a click reach the host before `run`; a wake after `Drop` is harmless).
- `examples/counter-win`: new `--start-test <start-with|run-with|preattach>` mode with the same three
  checks, written as a JSON report; `examples/counter-win/tests/counter.rs` runs the three modes as
  `start_with_mounts_in_the_hook_while_pumping`, `run_with_mounts_in_the_hook_of_the_loop`,
  `desktop_order_mount_has_the_host_attached` (Windows CI; each in its own process, since WinUI allows
  one backend per thread).

Local cross-checks: `cargo check -p svelters-apple -p svelters-appkit --all-targets --target
aarch64-apple-darwin` and `cargo check -p svelters-winui -p counter-win --tests --bins --target
x86_64-pc-windows-gnu` both pass with no new warnings. The counter-mac test targets need the macOS SDK
(QuickJS in svelters-host) and were checked only on CI.

Docs: `plans/architecture/backend-abi.md` (v2.7 backends paragraph: all four desktop backends implement
`start`) and the G0 section of `plans/milestones/E0-rust-exports.md`.

## MATRIX notes (for the coordinator; MATRIX.md not edited here)

- Row "Non-blocking host start + started hook (G0, ABI v2.7)": AppKit ✅, WinUI ✅ (FU1). The
  "AppKit/WinUI keep `attach_host`" note is out of date: both implement `start`, and `attach_host` still works.
- STATUS E0+G0 open item "`start` on AppKit/WinUI" is done; iOS/Android remain.

## Completion record

Branch `ci/fu1`.

CI on af52bcaa (code complete; later commits are docs only): js 39625384126, linux-gtk 39625384095 (gtk
2188/2188, gtk-4-14 1930/1930, portable 48/48; `foreign_kinds` 2/2 on both), linux-qt 39625384163,
macos-appkit 39625384143 (counter: `counter_e2e`, `appkit_types_e2e` + the 3 start cases ok),
windows-winui 39625384098 (counter: both selftests + the 3 start modes ok): all success.

Local gates on c1cfc2b2 (the code is the same as af52bcaa), all rc 0:

| Gate | Result |
| --- | --- |
| `tools/dev/cargo-slot cargo check --workspace --all-targets` | rc 0 (only warnings that were already there) |
| `tools/dev/headless tools/dev/cargo-slot cargo nextest run --workspace --test-threads 4` | 2392/2392 passed, 8 skipped |
| `pnpm -r build` (after `cargo build -p svelters-compiler`, `python3 tools/docs/build_ir.py`) | rc 0 |
| `pnpm test` | rc 0 |
| `pnpm docs:test` | rc 0 |
| `python3 tools/metadata/ledger.py check` | rc 0 (gtk 580, qt 634, appkit 205, winui 105 manual facts verified) |
| `cargo check -p svelters-apple -p svelters-appkit --all-targets --target aarch64-apple-darwin` | rc 0, no warnings |
| `cargo check -p svelters-winui -p counter-win --tests --bins --target x86_64-pc-windows-gnu` | rc 0, no new warnings |

## Open items

- Qt's error for a table member on the wrong kind ("unknown property id for this kind") does not name
  the runtime class; it does not panic. It could name `o->metaObject()->className()` in `gen_cpp.py`
  (that means regenerating the Qt backend and refreshing the ledger).
- iOS/Android `start` (docs/notes only so far).
- Conflict risks: `crates/svelters-winui/src/backend.rs` (`wake_shared` replaced the waker body),
  `crates/svelters-appkit/src/backend.rs` (`Inner::start`, `Backend::start`), `examples/counter-win`
  (`main.rs` arguments, `tests/counter.rs` table shape), `plans/architecture/backend-abi.md` (v2.7 backends
  paragraph; G1 may touch the same section), `plans/milestones/E0-rust-exports.md` (G0 backends bullet).
  No shared host or ABI code changed.
