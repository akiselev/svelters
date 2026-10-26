# Lane CORE: presto-core

Branch `ci/presto-core`. Owns `crates/presto-core/**`. Read: architecture §4 (tree), §5 (style, layout),
§6 (recording, property trees, layers, commit), §11 (input), §13 (a11y); notes `round2/svelters-inventory.md`
(the port list: what moves "M" from svelters-egui), `ARCH-SCENE.md`, `ARCH-EMBED.md` §5.4 (settle).

Deliver, in this order (push after each):
1. **API skeleton (first push, within 45 min):** `Document`, `Kind`, `KindRegistry`, `KindSpec` (props with
   `ChangeClass`, signals with passive/cancelable/sync, slots, a11y role, text-content prop), `KindId`,
   `PropId`, `SignalId`, `SlotId` (stable u32 ids with a fingerprint fn: hash of kind name + member name +
   type), `Value`, `TreeError`, `PropError`, the contexts (`StyleCx`, `MeasureCx`, `PrepareCx`, `RecordCx`,
   `HitCx`, `EventCx`, `A11yCx`) as in architecture §4.2 and the brief. Compiles.
2. **The tree:** copy (don't move: svelters-egui stays as it is) svelters-egui's `scene/` arena (ids, columns,
   dirty bits, journal, queries, compaction, virtualisation `RowPool`) into presto-core with presto-base
   value types; fix the `SCROLL_CENTER`/`INK_OVERFLOW` bit collision; payload = boxed `dyn Kind` per node.
3. **Change classes and transactions:** `set_prop` returns the class from the kind's spec and marks exactly
   that work; transactions bump `Revision`; native-owned state with the revision check (architecture §4.5).
4. **Style:** implement svelters-style's `Element` on the tree (CS5e), the cascade per node, computed values
   with change classes; transitions trigger → `AnimationCmd`s (sampling is presto-anim's).
5. **Layout:** taffy 0.15 low-level traits on the tree (as svelters-egui `layout.rs` does), `TextMeasure`
   from presto-base for text leaves, layout boundaries, the measure guard.
6. **Recording and commit:** property trees (transform/clip/effect/scroll) built from layout + style;
   layerization rules (architecture §6.3); call `Kind::record` for content-dirty nodes only; build the
   `Commit` delta (lists, layers, props, runs, resources, animations, a11y).
7. **Input:** hit testing on the last frame's hit list; capture/target/bubble dispatch (port EV1 from
   svelters-egui `events.rs`); pointer capture; hover; focus model (tree order, `contain`/`dialog`,
   `:focus-visible`); IME-first key routing hook; passive listeners; the `Dispatched` result listing the
   JS-facing events (signal id, node, args) so svelters-presto can deliver them at event time.
8. **Accessibility:** AccessKit projection of dirty nodes into `Commit.a11y`, with a validating builder
   (never panics: repairs or drops with a debug assertion).
9. **Settle:** `busy()` bits (ARCH-EMBED §5.4) plus `COMMIT_IN_FLIGHT`.

Tests (write, don't run): the ported scene tests; change-class marking; commit deltas; dispatch order
(capture/bubble, passive); focus traversal; a11y projection validity. No kinds beyond a minimal `Box` and
`Text` test kind (widgets are wave 2).
