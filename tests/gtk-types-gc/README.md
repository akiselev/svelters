Garbage collection of SHARED handles on the real host (`crates/svelters-gtk/tests/host_types.rs`); lives in tests/ because fixtures/ are op-log fixtures. Not an
op-log fixture: dropping a wrapper releases its handle when the JS engine collects it, which is not
deterministic on the Node TestHost, so there is no `expected.oplog`.
