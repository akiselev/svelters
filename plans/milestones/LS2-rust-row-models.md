# LS2: Rust row models for `table` and tree rows (SV41, SV44, SV47, SV52)

Lane `ci/b8-tables` (cloud batch 8, for cadatomic), from `master` c3034451. Brief:
`plans/cloud/batch8/LANE-tables.md`. Status: in progress (this record is kept current from the first push).

## Scope
1. SV41: an engine-neutral Rust row-model trait (`svelters_host::rows`), read by the egui `table` with no JS call
   per page; the JS window (`range` → `set-rows`) stays as the second source.
2. SV44/SV52: typed columns, Rust-side sort and filter, tree rows with expand/collapse by key; key selection across
   sort and filter.
3. SV47: T2 benches (bind per row, page rebind, fling; 20k and 100k rows; `virtual-list` and `table`; release in CI).
4. Docs: `docs/egui` row-model page, `table.md` front matter, a nets-list example.
