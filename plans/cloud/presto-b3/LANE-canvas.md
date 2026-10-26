# Lane CANVAS: viewports on OS layers and native canvas layers (W-V1, W-V2, presto side)

Repository: svelters. Branch `ci/presto-canvas`, made from `origin/ci/presto-base`; push only there.
`plans/cloud/presto-b1/BRIEF.md` applies: minimal building and testing, rules, attribution.

**Read first:**
- `plans/research/presto/plan.md` §2.5;
- `architecture.md`: the layer, surface and present sections, and the canvas section;
- `notes/round2/` on OS compositing (DComp, CAMetalLayer, Wayland subsurfaces).

**Deliver, in presto-compositor and presto-render plus a new crate `presto-canvas`:**
1. **Viewports (V1).** A `Surface` layer can be promoted to an OS layer through the backend's
   `OsLayerCommitter`, which COMPOSITOR added: `set_os_layer_committer`, `os_compositing()`. Promotion gives it a
   DComp child visual, a CAMetalLayer sublayer, or a Wayland subsurface (through the backend lanes' extension
   traits, or a trait you define in presto-platform for them to implement). Specifically:
   - the viewport gets its own wgpu surface;
   - the per-OS resize handshake: never present a stretched frame;
   - when OS compositing is unavailable, the fallback is the existing `Prim::Surface` path;
   - a counter test: orbiting the viewport never re-records or redraws the UI layers.
2. **Canvas layers (V2).** `presto-canvas` provides typed instance chunks (rects, lines, circles, arcs, polygons,
   text), with:
   - culling and LOD per chunk;
   - camera-relative coordinates (f64 world, f32 relative to a camera origin);
   - a static cache with a dynamic overlay layer;
   - Slug-style GPU canvas text.

   It's drawn by presto-render as a layer kind, and the compositor pans and zooms it with a property update, no
   UI commit.

   Test: 10⁶ primitives, with a synthetic CPU-side timing test for culling cost. The display-rate gate is
   **[local]**: record it in REVISIT, don't fake it.

The wgpu fork patches (P1–P4) are out of scope; note where they would help.

**Building:** `cargo check` on the crates you touch. Write tests, but don't run them. Push, and let CI run them.

**Record:** `plans/milestones/PRESTO-canvas.md`.

**Report** to the coordinator `session_01CqZ417NuvkRujZaYSm7Pwo` with `send_message`. Don't ask the user anything.
