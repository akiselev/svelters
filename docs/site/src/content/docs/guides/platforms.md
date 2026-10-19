---
title: Platforms
description: Per-platform and per-engine status for svelters, separating shipped backends from research decisions and planned work.
---

Two toolkit backends run today (GTK4 and QtWidgets). Everything else on this page is research or planned, and is marked that way.

## Status

| Platform | Status | Evidence or decision | Next |
| --- | --- | --- | --- |
| GTK4 | **done** (P3, P5) | 443 Gtk kinds ingested; 298 kinds, 1304 props, 349 signals in generated tables; 22 tests plus op-log replay; counter runs | M2 timers and lifetime; M3 dynamic method invocation |
| QtWidgets | **done** (P4, P5) | 192 classes ingested; 8 elements annotated; 41 generated tests; leaf-crate rebuild verified; counter runs | M2; fix slow reorders (M5); `headerbar` equivalent |
| WinUI 3 | research | `docs/notes/windows-gui-backend.md`: primary Windows target via Rust and windows-rs | prototype on Windows CI (estimate 2 to 3 agent-lane weeks) |
| Win32 + comctl32 v6 | research | Fallback that builds and runs on Linux CI under Wine | only if WinUI 3 is blocked |
| AppKit (macOS) | research | `docs/notes/apple-gui-backend.md`: Rust with `objc2` | prototype on macOS runners (estimate 2.5 to 3.5 weeks) |
| UIKit (iOS, iPadOS) | research | same crate family as AppKit; interpreter-only engine on iOS | after macOS (estimate +1.5 to 2 weeks) |
| egui | **planned** (M7) | `PLAN.md`: depends on M4 | M7 |
| SwiftUI | not a backend | needs an inverted observable-model design; hosted leaf components only after M4 | none |

:::note[What "done" means]
"Done" is the milestone state in `STATUS.md`: the counter, keyed list, keyed stress, timers (and if-toggle on GTK) run from one JS bundle on the backend, with tests. Only the prototype kinds are annotated. See [Metadata and annotations](/guides/annotation-workflow/).
:::

## GTK4

- Rust backend over `gtk4-rs` 0.11, tables generated from `metadata/gtk`. Tested on GTK 4.22.4.
- Runs under Xvfb. First frame about 1.16 s in the P5 baseline (0.69 s with `GSK_RENDERER=cairo`), dominated by GTK and GL initialisation rather than JS.
- Known gaps: no dynamic GI method invocation (only `Window.present` is a method; M3); `handler_result: bool_stop` for bool-returning signals is a heuristic default; ABI v2 lifetime work is not applied (see [Ownership](/concepts/ownership/#abi-v2-and-implementation-state)).
- Element pages: [GTK elements](/api/gtk/).

## QtWidgets

- C++ generated from metadata and compiled by `cc` in a leaf crate's `build.rs`, linked via `pkg-config Qt6Widgets`. Tested on Qt 6.11.
- First frame about 0.20 s; peak RSS 55 MB in the counter baseline.
- Known gaps: moves reparent to null and re-show, which makes reorders very slow; no `header-bar` (the if-toggle fixture is GTK-only); native close does not report `on_destroyed`.
- Element pages: [Qt elements](/api/qt/).

## WinUI 3 (research)

Decision from `docs/notes/windows-gui-backend.md` (desk research written on Linux; items marked verified there were checked against downloaded metadata or under Wine, the rest need a Windows machine):

- **Primary: WinUI 3 (Windows App SDK), backend in Rust over windows-rs bindings generated from `Microsoft.UI.Xaml.winmd`.** It is the only Microsoft-backed native toolkit with Fluent styling, built-in UI Automation, a retained tree, layout and real virtualization.
- Its winmd is the richest metadata source of any toolkit: 1852 `XxxProperty` statics across 900 classes, 322 events, collections, default child slots (`ContentProperty`), attached properties and overridable virtuals.
- Codegen size: windows-bindgen over the whole namespace emits 1.28 M lines (67 MB of Rust). Member-level filters cut the prototype set to 8.6 k lines, which cargo-checks for `x86_64-pc-windows-gnu` on Linux.
- ABI fit is good: `wake` through `DispatcherQueue.TryEnqueue`, `schedule_wake` through a `DispatcherQueueTimer`.
- **Fallback: Win32 + comctl32 v6.** Builds and runs under Wine, needs no runtime, but has no layout, no Fluent look and almost no structured metadata.
- Excluded: UWP XAML (maintenance mode), WPF and WinForms (need a CLR in process), Direct2D custom drawing (duplicates egui).

## AppKit and UIKit (research)

Decision from `docs/notes/apple-gui-backend.md`:

- macOS uses AppKit; iOS and iPadOS use UIKit (Mac Catalyst and visionOS are later bonus targets). SwiftUI is not a backend.
- Language: Rust with `objc2` plus one small ObjC helper file for `NSInvocation` and delegate trampolines. One shared `svelters-apple` crate holds the run loop, handles, dynamic bridge and accessibility.
- Dev mode is fully dynamic through the ObjC runtime (the equivalent of GObject introspection); release uses typed `objc2` calls for tree-shaken roots.
- Event loop: `wake` is a `CFRunLoopSource` signal, `schedule_wake` one re-armed `CFRunLoopTimer`, frames through `CADisplayLink`. Modal calls are exposed as async sheets only.
- iOS: interpreter only; any JIT is macOS and desktop only.
- Linux can `cargo check` the Apple backend without an SDK; GUI tests need macOS runners and the iOS simulator.

## egui (planned)

M7, after M4 (native semantics). `PLAN.md` lists egui as the third L3 backend (Rust). egui is immediate mode: there is no widget state to echo, so two-way values round-trip each frame, and virtualized lists use `ScrollArea::show_rows`. Nothing is implemented.

## JS engines

| Engine | Status | Notes |
| --- | --- | --- |
| QuickJS-NG (rquickjs 0.14.0) | **default, implemented** | Execution only: no TypeScript or Svelte compilation and no npm resolution at runtime. |
| V8 (`deno_core`) | **planned if performance demands** | Measured about 20x on Svelte signal propagation and 27x on app glue versus QuickJS. The op layer stays engine-neutral. |
| JavaScriptCore | optional, **research** | An optional engine after P5 and M2 (estimate 1.5 to 2 weeks). On iOS there is no JIT; its LLInt is 1.6 to 1.8 times QuickJS-NG (measured). Microtasks drain automatically at the end of each top-level API call, so `run_jobs` becomes a no-op. No public module loader, interrupt or heap limit. |
| Stock Node (N-API), Deno FFI | research | Node works with GTK4 and Qt6 through a small addon that lets the toolkit loop own the main thread. Needs optional ABI members (`foreign_fd`, `foreign_timeout_ms`, `poll_prepare`/`poll_dispatch`). Priority after QuickJS: stock Node, then embedded `deno_core`, then stock Deno. Bun is not supported. |
| QuickJS JIT (quickjs-jit, Cranelift) | **rejected for now** | Not adopted and not built: a tie with its own interpreter and about 25 percent slower than the stock NG 0.16.2 on the real Svelte runtime. Revisit if it rebases onto NG 0.16 or newer. |

P5 measurements point the same way: on a 1000-row keyed list, QuickJS time dominates while host and ABI cost is under 20 ms. See [Lists and snippets](/guides/lists-and-snippets/) and [Architecture](/guides/architecture/).

Sources: `STATUS.md`, `PLAN.md`, `docs/notes/windows-gui-backend.md`, `docs/notes/apple-gui-backend.md`, `docs/notes/quickjs-jit-cranelift.md`, `docs/notes/node-deno-integration.md`, `docs/notes/p3-gtk-lane.md`, `docs/notes/p4-qt-lane.md`, `docs/notes/p5-integration.md`.
