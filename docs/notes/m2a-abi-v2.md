# M2a: ABI v2 notes

Vtable layout decisions (delta commit): `handle_info` follows `listen`; `schedule_wake` follows `wake`. `on_destroyed` gains `uint32_t reason`. Backend trait gains required `handle_info` and `schedule_wake`.

Delta commit ready: 316effd (`abi: v2 interface delta`).
Gate note: `memory_limit_is_enforced` (svelters-host) fails here with "Load: null" instead of OutOfMemory; no ABI code path touched. Other 56 nextest tests pass, pnpm test green.
