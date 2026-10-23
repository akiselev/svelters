# FIX-ALLOC: SG2's zero-allocation mutation test, red on master

**Symptom.** `crates/svelters-egui/tests/scene.rs::mutations_allocate_nothing_without_observers`
counted 50 allocations over 50 rounds (expected 0) on master ba5f7239 and later.

**Cause.** The allocations came from combining OW1a2 and RA3. Layout and `MeasureCx` play no part.
A temporary backtrace-on-allocation hook in the test's counting allocator found the site:

`NodeMut::set` → `Scene::set_value` → `scene/props.rs` → `provider::set_replaced_prop` →
`svelters_host::exports::__private::with_cx` → `objects::current().unwrap_or_default()`.

OW1a2 made `checkbox` and `slider` own widgets, so a Rust scene write of their props now goes
through `set_replaced_prop`, which converts the value under `with_cx`. RA3's `with_cx` builds a
fresh `Rc<AppObjects>` when no host op is running. That is one heap allocation per call, and one
per round.

**Fix.** `crates/svelters-host/src/exports/mod.rs`: when there is no current object table,
`with_cx` passes an empty `AppObjects` on the stack instead of a new `Rc`. An empty `HashMap`
doesn't allocate. Behaviour is unchanged: the default table was dropped after each call before
too, so any object adopted into it outside an op was lost either way. The test is unchanged.

**Tests (local).** All 57 svelters-egui tests in `scene`, `scene_memory` (SG6 budget), `ow1a`,
`ow1a2`, `own_widgets`, `widgets` and `spec` pass. `cargo check -p svelters-host --all-targets` is
clean (the one `CatchUnwind` dead-code warning was already there). Not run locally: the svelters-host
tests, the full workspace nextest, the gates and conformance (CI runs them).
