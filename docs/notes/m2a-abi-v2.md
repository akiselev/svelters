# M2a: ABI v2 notes

Vtable layout decisions (delta commit): `handle_info` follows `listen`; `schedule_wake` follows `wake`. `on_destroyed` gains `uint32_t reason`. Backend trait gains required `handle_info` and `schedule_wake`.

Delta commit ready: 316effd (`abi: v2 interface delta`).
Gate note: `memory_limit_is_enforced` (svelters-host) fails here with "Load: null" instead of OutOfMemory; no ABI code path touched. Other 56 nextest tests pass, pnpm test green.

## Decisions and surprises (M2a)

- By the time GTK emits `destroy` for a window, GTK has already unparented its child, so `is_ancestor` found nothing and the P3 cascade never reported descendants. The cascade now walks the slot records.
- A parentless `QWidget` is a Qt window, so "toplevel" cannot be read off the object. The generator lists the window/dialog kinds.
- QuickJS throws `null` when it cannot allocate the OOM error (fixed on master in af931e9).
- The log backend cannot hold the virtual clock for unscripted runs because the host reads `Instant::now()`. Scripted mode already has one.
- Measured: nothing new; timers fixture on GTK/Qt completes in the same time as before without the helper thread.
