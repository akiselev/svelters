# M2b dev loop notes

Written incrementally by lane M2b.

## Host reload design (crates/svelters-host)

- New modules: `reload.rs` (watcher thread + reload), `sourcemap.rs` (Rust-side mapping with the `sourcemap` crate). Plumbing in `core.rs`: a child->parent `tree` map (so teardown can find toplevels and descendants), `maps`, and `roots/destroy_subtree/swap_timers/clear_events`; `lib.rs`: `Host::watch/on_reload/request_reload/toplevels`.
- The watcher is a thread over `notify` (inotify) on the bundle's directory. It debounces 120 ms of quiet and then only sets a flag and calls the thread-safe `wake`. The reload itself runs at the start of the next checkpoint on the toolkit thread.
- Reload order is "load new, then destroy old": the new runtime builds and presents its windows while the old ones are still up, then the old toplevels are destroyed (descendants first, through the ordinary destroy op). Reasons: the toolkit loop never sees zero windows (GtkApplication would quit, Qt would quit on last window closed until M2a makes quit host-decided), and a failed load can keep the previous version running.
- Failed load (syntax error, throw during mount): whatever the failed load created is destroyed, the previous runtime and its timers are put back, the mapped error is printed and recorded in `take_errors`, and the process waits for the next change.
- A bundle file that does not exist (build clearing `dist/`) is skipped; its reappearance triggers the reload.
- Source maps: `<module>.map` next to the bundle, loaded eagerly for the entry at `load_bundle` (so a rebuild landing later cannot skew an already-running bundle). `module.js:L:C` in error message/stack and in console error/warn/trace text is rewritten to `src/App.svelte:L:C` (leading `..` of the map's source paths dropped).
