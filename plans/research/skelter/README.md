# A GUI toolkit of our own: architecture and implementation plan

## Decided (user, 2026-10-25; coordinator for the placement)

- **Build it, all of it, now.** "No M1 gate … just keep going and implement the full framework. Use
  GitHub CI for Windows/mac. Everything else follow your recommendation. Go start. Minimize building
  and testing till consolidation." So the plan's milestones run back to back; M1's measurements still
  get taken, but nothing waits on them.
- **Every open question below is taken as recommended** (Q2–Q12). Q1, the name: **skelter**.
- **Placement (coordinator, 2026-10-25):** the skelter crates live in the svelters workspace for now
  (`crates/skelter-*`), not a new repository: svelters already has GitHub CI on Linux, Windows and macOS,
  `svelters-skelter` lives here, and svelters-style is a path dependency. They can move to their own
  repository later with `git subtree split`.
- **Batches:** `plans/cloud/presto-b1/` (wave 1: foundations), then waves 2 and 3 (`plan.md` §2).

Date: 2026-10-25. Status: **proposal for the user**; nothing is built. Supersedes the architecture
sections of `../2026-10-25-own-gui-toolkit.md` (round 1).

**The question** (user, 2026-10-25): cadatomic and svelters are hitting egui's limits, especially in
latency, and feel sluggish even with the low-latency setting. Since we already have the scene graph,
what would it take to write our own egui-style framework and renderer that makes svelters,
rquickjs/QuickJS and performance first-class? The user added three more requirements:
- research how GTK, Qt, WinUI, Win32, AppKit and UIKit do it;
- integrate closely with the svelters compiler;
- consider forking rquickjs/QuickJS-NG (no serialization between layers) and winit.

**"skelter" is a working name.** Q1 below picks the real one.

## Files

| File | Says |
|---|---|
| `architecture.md` | what the toolkit is: threads, the tree, change classes, the compositor, the renderer, text, scheduling, input, animation, accessibility, the egui island, testing |
| `compiler-and-js.md` | how the svelters compiler and the QuickJS engine integrate, and the engine fork question |
| `platform.md` | per-OS platform layer (winit 0.30.13 plus shims, then own backends), why not a winit fork, the present path on each OS |
| `plan.md` | work packages W-…, milestones M1–M6 with measured gates, the M1 go/no-go, risks |

**The evidence** is 21 notes in `notes/`, every claim graded and cited.
- **Round 1 (8 notes):**
  - Apple and Android;
  - Windows;
  - GTK, Qt and Wayland;
  - browsers, Flutter and latency research;
  - Rust GUI frameworks;
  - JS-driven runtimes;
  - GPU 2D and text;
  - today's pipeline.
- **`round2/` (13 notes):**
  - the GPUI port list;
  - wgpu integration;
  - IME, accessibility and OS integration;
  - animation and scroll physics;
  - the frame scheduler and latency instrumentation;
  - the text stack;
  - compiler prior art and the local compiler map;
  - svelters and cadatomic porting inventories;
  - JS host costs;
  - zero-copy JS bindings;
  - winit.

## Summary

1. **Why egui feels slow, and why tuning egui won't fix it all.** Most of today's latency is frames
   added by the pipeline:
   - 2 queued frames (the "low-latency" setting was never actually on);
   - JS for widget signals running a frame late;
   - wheel easing of 117 ms;
   - popup sizing passes.

   Those can be fixed on egui, and the deferred input-latency work measured each one. But it found
   that once the frames are gone, **pass cost limits latency**: egui repaints the whole window and
   rebuilds every visible widget each frame, 6–10 ms per shell pass in release, and more on the CPU-
   rendered WSL PC. Every mature toolkit solves this the same way, and egui's design can't. The
   pieces are:
   - a retained render tree;
   - properties separate from content;
   - a compositor thread;
   - a frame clock that starts work just in time;
   - damage-limited redraw.
2. **We already own half of a toolkit.** svelters' scene arena, taffy layout on it, the Rust CSS
   engine, the scheduler, the backend ABI, the JS host, the compiler, and the berth and quire cores
   are egui-free or nearly so. That is 35% of svelters-egui's 66.5k lines, which move as they are
   (`round2/svelters-inventory.md`).
3. **The architecture** (`architecture.md`):
   - **The tree:** one retained tree with four change classes (structure, layout, paint, property).
   - **JS:** runs on the UI thread at event time, so a click's JS result is in that click's frame.
   - **Direct manipulation is native** and never waits on JS: hover, press, focus, caret, scroll,
     drags, transitions.
   - **A compositor thread** owns wgpu, property animations, scroll physics and present pacing, and
     draws even when the UI thread misses its deadline.
   - **The renderer** is a small instanced-primitive one, adapted from GPUI's Apache-2.0 shaders,
     with damage-limited redraw and non-sRGB blending.
   - **Text:** parley and skrifa, with DirectWrite-style gamma.
   - **Heavy content:** 3D viewports and canvases on their own OS compositor layers.
   - **Testing:** deterministic scheduler tests, and latency instrumentation from the first commit.
4. **The compiler becomes part of the toolkit** (`compiler-and-js.md`). A generated `skelter.json`
   schema carries stable ids, change classes and passive flags. In priority order, the compiler
   emits:
   - integer ids;
   - native templates (one call per static subtree; the counter goes from 36 crossings to 1);
   - lazy JS node objects;
   - change-classed writes;
   - inferred passive listeners;
   - native transitions, FLIP and springs;
   - precompiled CSS.

   Each is a backend capability, so the DOM conformance backend and the op-log goldens are
   unchanged.
5. **Don't fork QuickJS or rquickjs now.** JS execution dominates, not the boundary: 1.5–1.9 s of
   QuickJS time against 16–18 ms of host time for 11k ops. Raw `rquickjs-sys` C functions with
   integer `magic` ids get a crossing to an estimated 40–80 ns without a fork. A light fork gains
   5–20 ns more for a monthly rebase; a deep fork is permanent and conflicts with isolates and engine
   choice. Measure first (W-J0), and reopen only on a stated threshold.
6. **winit: don't fork it as the strategy** (`platform.md`).
   - **Start on stable winit 0.30.13 behind skelter's platform trait,** with ~3k lines of side-channel
     shims through handles winit already exposes:
     - a Windows window subclass (snap layouts, modal-loop repaint, pointer history, IME
       reconversion);
     - a macOS app delegate and layers;
     - a Wayland side queue (presentation feedback, subsurfaces, occlusion).
   - **Then replace winit one OS at a time with our own backends, Windows first** (9–12k lines:
     TSF, `WM_POINTER`, DirectManipulation, our own modal-loop policy).
   - **Why not a fork:**
     - winit's Windows backend has had no maintainer since 2025-08;
     - it has no frame clock or presentation feedback;
     - its API churns: 0.30→0.31 touched 385 files, and 0.32 breaks again;
     - a fork carrying our needs would be 6–9k lines, mostly in the unowned win32 code.
   - **At most a one-patch bridge fork** for macOS dictation and the Character Viewer, and only if
     needed before our macOS backend.
7. **The plan** (`plan.md`):
   - **Foundations first:** 4 headless crates plus 3 compiler and runtime packages, one cloud batch of
     6–8 lanes.
   - **Then an M1 skeleton** with a **measured go/no-go** on the user's Windows machine and MacBook.
   - **Then M2–M6:** widgets, the shell on skelter with egui islands for unported panels, OS layers
     and the present path, own platform code, and finally removing egui.

   Planning scale: 70–90k lines new or rewritten, 23k moved, 15–25k of cadatomic touched.

## Open questions for the user

Each has a recommendation. Four of them reverse or change decisions recorded in
`docs/plan/DECISIONS.md` as the user's; the table says which.

| # | Question | Recommendation | Changes |
|---|---|---|---|
| **Q1** | The name | **skelter** ("hey skelter": the cadabra family; means fast) or **keel**/**folio** (beside berth and quire). Clash check on crates.io and GitHub before committing | — |
| **Q2** | Build our own toolkit, gated by the M1 go/no-go? | **Yes:** run the foundations batch (plan §2.1) and the M1 skeleton; decide at M1 on measured latency, pass cost, idle and text quality | DECISIONS "UI framework: svelters and egui": "cadatomic's UI is svelters on egui 0.37"; "egui is only the engine" (it would become skelter) |
| **Q3** | Where skelter lives | **A new repository, developed by cadatomic sessions like berth and quire.** Licence: none declared (like cadatomic and berth) until you decide; GPUI-derived files carry their Apache-2.0 headers either way. `svelters-skelter` in svelters; `skelter_berth`/`skelter_quire` in berth and quire | DECISIONS: berth and quire are "standalone egui-only" crates (they would gain a skelter renderer beside the egui one) |
| **Q4** | Fork rquickjs/QuickJS-NG for zero-serdes integration? | **No, not now.** Option (a), raw `rquickjs-sys` fast ops plus compiler templates and ids; W-J0 measures; reopen if the crossing is still above ~30% of a batched update after W-C1/W-C3/W-J1 | keeps DECISIONS' "no rquickjs fork" |
| **Q5** | Fork winit? | **No general fork.** winit 0.30.13 plus side-channel shims behind skelter's platform trait, then own backends per OS (Windows, Wayland, macOS; X11 stays on winit). A one-patch bridge fork for macOS text input only if needed. A ~600-line experiment checks whether Wayland tear-off works from our own queue without a fork | may retire D7 ("Wayland tear-off may use a winit fork") if the experiment works |
| **Q6** | JS on the UI thread at event time (not a JS thread)? | **Yes** (`architecture.md` §2; `notes/js-runtimes.md`): synchronous `preventDefault` and layout reads, atomic transactions, and the industry trend (React Native, Flutter's thread merge) | confirms "first-party Svelte runs in process on the UI thread" |
| **Q7** | One text rasteriser everywhere (skrifa), deterministic in tests? | **Yes,** with the DirectWrite gamma/contrast and raster policy on Windows and dilation on macOS; LCD text as a later option gated on a 96-DPI screenshot review (Zed made LCD its default in Jan 2026) | — |
| **Q8** | A wgpu fork for present-with-damage, acquire timeouts and Metal pacing (P1–P4)? | **Yes,** each patch upstreamed; P1 tracks upstream PR #10152 | — |
| **Q9** | `svelte/motion` aliased to a native, frame-rate-independent `svelters:motion` | **Yes:** exact at 60 Hz, correct at 120/144 Hz where Svelte's integrator overshoots differently | a deliberate departure from stock Svelte at refresh rates other than 60 Hz |
| **Q10** | Layout transitions (`slide`) run real layout per frame, or keep svelters' clip approximation? | **Real layout by default,** the clip approximation opt-in | — |
| **Q11** | Wheel scrolling feel | **Per OS:** Windows instant per notch, or a short 30–50 ms ease as a setting; macOS 1:1 with OS momentum; Linux browser-like (Chromium's curve) rather than GTK's | overlaps the deferred input-latency question (c) |
| **Q12** | When the deferred egui latency work resumes, who owns ids, lazy nodes, passive and event-time dispatch? | **This plan**, since they help egui and skelter alike; one owner per package | — |

## What is not decided here

- The licence of skelter, which follows the cadatomic and berth licence decision.
- The design-system look, which stays DECISIONS' VS Code Modern with affordances.
- Extension security (deferred; skelter leaves the principal id and `#[non_exhaustive]` room the
  ARCH-SECURITY decision asks for).
