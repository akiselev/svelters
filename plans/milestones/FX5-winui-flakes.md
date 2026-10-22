# FX5: two windows-winui flakes in "svelters-winui generated"

Lane FX5, started 2026-10-22 on master 797835a5. A flake-fixing lane: two tests of
`crates/svelters-winui/tests/generated.rs` failed intermittently on lanes that never touched WinUI,
then passed on rerun or on the next sha.

## The failures

| Test | Message | Seen on |
|---|---|---|
| `event_content_dialog_closed` (row `t::content_dialog_show()`) | `show-async did not open the dialog` | run 39853736281, sha b4094d13 |
| `event_text_box_text_changed` | `event delivered once`: left 0, right 1 | run 39859313324, sha 95fcfe79 (ci/ow1a2) |

## Cause

Both tests pumped the message loop for a fixed time, then asserted on an event that XAML raises
from a later dispatcher turn:

- `content_dialog_show` (`crates/svelters-winui/src/testing.rs`) called `show-async`, pumped
  600 ms, and required `opened`. ContentDialog.Opened comes after ShowAsync has laid out the popup
  and started its open transition; then `hide` and another fixed 600 ms for `closed`.
- The generated event tests (`tools/metadata/winui/gen_backend.py`, trigger `set:`/`invoke`)
  wrote the trigger, pumped 200 ms, and counted. TextBox.TextChanged is raised asynchronously
  (unlike TextChanging and BeforeTextChanging, which the same tests see synchronously), so on a
  loaded runner it can arrive after the 200 ms window: count 0. The `mount` triggers (Loaded,
  SizeChanged, LayoutUpdated) had the same shape with a fixed 300 ms.

Neither failure was a wrong count: the event came late, not never or twice.

## Fix

Wait on the condition, not on the clock:

- `Harness::pump_until(ms, cond)`: pumps in 10 ms turns until `cond` holds or `ms` pass.
- `Harness::await_events(h, event, n)`: pumps until `event` reached the host `n` times (up to
  `EVENT_WAIT_MS` = 10 s), then `EVENT_SETTLE_MS` = 200 ms more so a second delivery still fails the
  "delivered once" assertion (the old window was 200 ms, so duplicates are caught as before).
- `content_dialog_show` waits up to 10 s for `opened`, then up to 10 s for `closed`.
- The generator emits `h.await_events(w, "<event>", 1)` for every trigger and mount event test
  instead of a fixed pump plus `h.events`. `generated.rs` and `facts.json` are regenerated
  (`python3 tools/metadata/winui/gen_backend.py`).

A passing test is no slower than before except for the settle window; a failing one now takes 10 s
instead of 200 ms.

Ledger: the changed test bodies and `testing.rs` (part of every check and observe fact's
fingerprint) change the fingerprints of those winui facts, so the ledger was re-imported from this
lane's windows-winui artifact (`python3 tools/metadata/ledger.py import --toolkit winui <artifact>`).

## Verification
