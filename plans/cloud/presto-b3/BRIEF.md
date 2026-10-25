# presto batch 3, early lanes: own platform backends

Date: 2026-10-25. Coordinator: `session_01CqZ417NuvkRujZaYSm7Pwo`. **`plans/cloud/presto-b1/BRIEF.md` applies**
(minimal building and testing, rules, attribution) and `plans/cloud/presto-b2/BRIEF.md`'s note on coding
against wave-1 APIs. Read `plans/research/presto/platform.md` (§5 is your scope), architecture §11, and the notes
`round2/winit.md`, `round2/ime-a11y-os.md`, `round2/gpui-port.md` (items 7–14 for what to take from GPUI's
Apache-2.0 platform crates: `gpui_windows`, `gpui_macos`, `gpui_linux` at zed a5c4ca8).

Each backend is **its own crate** implementing the platform trait lane PLATFORM defines in `presto-platform`
(`AppHandler`, `PlatformCx`, `run`, `SurfaceTarget`, `FrameSource`; signatures in the b1 brief). Until
PLATFORM's skeleton is in `ci/presto-base`, code against the brief's signatures in a `shim` module and switch
when told. presto-platform will select your backend with a cargo feature and fall back to winit; that wiring
is the consolidation's. Your crate compiles only on its OS: CI's `presto` workflow checks it there
(windows-latest, macos-latest; Wayland on ubuntu). Use `cfg(target_os)` at the crate root so other OSes build
an empty crate.
