# Node.js / Deno / Bun as alternative runtimes

Status: research complete (2026-10-16). Web facts in §1 and §3 were gathered from primary sources (GitHub source, release pages, docs) on this date. Items marked *unverified* were not confirmed.

**TL;DR.** Stock Node works on Linux with GTK4 and Qt6 without qode. A small N-API addon lets the toolkit loop own the main thread and watch `uv_backend_fd()` + `uv_backend_timeout()`, which is node-gtk's technique. This was demonstrated for GTK4 and for Qt6 (both dispatchers), and nested `QDialog::exec` kept Node timers running. The costs are two stock-Node warts: a manual microtask drain (`process._tickCallback`), and re-entrant `uv_run` inside nested modal loops. Stock Deno works for GTK through pure `Deno.dlopen` with a split GLib poll (only `poll(2)` runs off-thread), but it is blocked during nested loops. Bun cannot do loop integration (no libuv). The `svl_backend` ABI needs one small addition: the backend must be able to watch a host-supplied fd and timeout, plus an optional non-owning "pump" mode. With that, QuickJS, Node, Deno-FFI and an embedded deno_core can all drive the same backends.

## 0. Local environment

| Tool | Version |
| --- | --- |
| node | v22.22.2 (nvm; libuv 1.51.0, statically linked, `uv_*` symbols exported from the `node` binary) |
| deno | 2.7.14 (V8 14.7) |
| bun | 1.3.14 (canary build) |
| GTK | 4.22.4, GLib 2.88 |
| Qt | 6.11.0 |
| Xvfb | `/usr/bin/xvfb-run` |

N-API addons were built with plain `gcc -shared -fPIC` against `~/.cache/node-gyp/22.14.0/include/node` (no node-gyp run, about 2 s per build). Scratch sources are in `/tmp/claude-1000/-projects-svelters/10c0a3c6-7a5c-4d9b-9688-b5af0249a13a/scratchpad/node-deno/` and are not in git.

## 1. Prior art: GUI loops with stock runtimes

### NodeGui / qode
- `nodegui/qode` was archived (2024-09) and replaced by `nodegui/qodejs`. The latest release is `v24.12.0-rc19-qode` (2026-04-20), and nodegui 0.74.2 (2026-05) pins it. One main maintainer; issues include "Is this project dead?" (#1078) and "Node 24?" (#1071).
- **Why a custom build** (commit `a2e043c851` in qodejs): it patches `SpinEventLoopInternal` (`src/api/embed_helpers.cc`) to call `qode_run_gui_loop()` instead of `uv_run(DEFAULT)`, and enters through `qode::Start` (`node_main.cc`). nodegui's `InjectCustomRunLoop` then runs `QApplication::exec()` with **no JS frame underneath**.
- It also patches **libuv**: a `qode_on_watcher_queue_updated` hook fired from `uv__io_start/stop/feed`. The hook wakes the poller thread when watchers change.
- The loops merge Electron-style. An `EmbedThreadRunner` thread blocks in `PollEvents()` (Linux: `epoll_wait(uv_backend_fd, uv_backend_timeout)`; macOS: `select()`; Windows: `GetQueuedCompletionStatus` + re-post). It then posts `uv_run(NOWAIT)` to the main thread (Linux via `g_idle_add_full`, so it relies on Qt's GLib dispatcher; macOS via `dispatch_async`; Windows via `PostMessage`).
- Issue #1092 (2026-09) asks for "event loop integration without qode", proposing `eventloop-graft` (the app runs in a worker thread, the toolkit on main). The maintainer said it would need message passing throughout NodeGui. No maintained Qt-on-stock-Node N-API project was found.
- **What has changed:** nothing fundamental. qode still exists mainly to avoid having a JS frame under the GUI loop and to get the libuv watcher hook. Our experiment (§4) shows that the same-thread fd watch removes the need for the hook. The JS frame under the loop costs one microtask-drain workaround.

### node-gtk (stock Node, no custom build)
- `src/loop.cc` attaches a custom **GSource** to the default context. It does `g_source_add_unix_fd(uv_backend_fd)`. `prepare()` calls `uv_update_time`, **drains nextTick/microtasks**, and returns `uv_backend_timeout()`. `dispatch()` calls `uv_run(NOWAIT)`. When `!uv_loop_alive()` it masks the fd, to avoid spinning (we hit exactly that spin in §4).
- `lib/loop.js` wraps the timer functions and defers blocking `run()` to a macrotask under ESM, because the ESM top-level body is itself a microtask (issue #442).
- On Windows, the fd watch is `#ifdef`'d out (FIXME → nodejs/node#36015), so it depends on timeouts and wakeups only.
- Active: v4.1.1 (2026-07), GTK4/libadwaita, prebuilts for Node 22/24/26.
- It calls `uv_run` from inside Node's `uv_run`, which libuv documents as unsupported.

### Electron
- `shell/common/node_bindings*.cc`: an `EmbedThreadRunner` thread polls `uv_backend_fd`. Linux uses `epoll_wait`, macOS now uses `poll()`, Windows uses `GetQueuedCompletionStatus` + `PostQueuedCompletionStatus` on `loop->iocp`. `UvRunOnce` posts `uv_run(NOWAIT)` to the Chromium main loop.
- Electron also patches libuv (`uv_loop_interrupt_on_io_change`, from libuv PR #3308) for the same reason as qode: a separate poller thread sleeps on a stale timeout or watcher set unless libuv tells it about changes.
- libuv docs: "uv_run is not reentrant. It must not be called from a callback." `uv_backend_fd` exists only for kqueue, epoll and event ports, so **there is no Windows fd**.

### Deno
- FFI (`Deno.dlopen`, `UnsafeCallback`, `nonblocking: true`) is **stable** behind `--allow-ffi`. A nonblocking symbol runs on a blocking thread and returns a Promise.
- `UnsafeCallback` can be invoked from any thread but doesn't wake the loop. `.threadSafe()` = `ref()`, which keeps the loop alive and makes foreign-thread calls wake it.
- `Deno.UnsafeWindowSurface` (WebGPU surface on a native window handle) is still unstable. It needs FFI permission since PR #36080.
- Deno 2.9 (2026-06) added an experimental `deno desktop` / `Deno.BrowserWindow`. It is webview/CEF-based, so not relevant to native widgets.
- Community projects pump from JS:
  - `deno_sdl2`: `SDL_PollEvent` in an async generator with `setTimeout(0)` yields.
  - `webview_deno`: blocks the JS loop in `webview_run`.
  - `sigmaSd/jsGTK` (GTK4 over Deno/Bun/Node FFI): `g_main_context_iteration(FALSE)` in a loop, then `setTimeout` backoff from 16 to 250 ms. That is polling, with latency up to 250 ms when idle.

### Bun
- `bun:ffi` is experimental. "Async functions are not supported" (no nonblocking calls). `JSCallback({threadsafe:true})` marshals calls to the JS thread.
- On POSIX, `napi_get_uv_event_loop` returns Bun's own loop. Only a handful of `uv_*` are polyfilled (mutex, once, hrtime, getpid); the others are stubs that abort. Confirmed in §4: our addon aborted with `unsupported_uv_function`.
- So libuv-based integration is impossible on Bun Linux/macOS. Only FFI polling or a worker-thread `poll(2)` is possible.

## 2. Event-loop integration strategies

Notation: "GUI" = GLib (GTK; also Qt on Linux by default) or Qt's dispatcher. "Runtime" = libuv (Node) or tokio (Deno).

| | (a) GUI owns main, watches runtime fd | (b) Runtime owns main, pumps GUI | (c) Watcher thread (Electron/qode) | (d) GUI on another thread | (e) Embed libnode / deno_core in our host |
| --- | --- | --- | --- | --- | --- |
| Stock Node via addon | **yes** (tested GTK+Qt) | **yes** (tested) | yes, but stale-timeout race without libuv patch | GUI thread yes on Linux (GTK/Qt allow a non-main GUI thread there); JS ↔ GUI only by message passing | needs libnode.so (Debian ships `libnode127/137`; **Arch does not**; build `--shared` ourselves) |
| Stock Deno via FFI | **no** (no tokio fd, can't run Deno's loop from inside an FFI call) | **yes** (split poll, tested) | n/a | yes, but every op is cross-thread | deno_core/deno_runtime crates |
| Latency | sub-ms both ways (fd-driven) | sub-ms when fd-driven (uv_poll on GLib fds / split poll); 16–250 ms if timer-polled | sub-ms + one thread hop | one thread hop per op; sync ops (`get_prop`, `call_sync`) block | sub-ms, no hops |
| Idle CPU | ~0 (Qt 0.6 ms/2 s) | ~0 if fd-driven; constant wakeups if timer-polled | ~0 | ~0 | ~0 |
| Nested loops (`QDialog::exec`, `QMenu::exec`, X11/Win `QDrag::exec`) | runtime keeps running, but **`uv_run` re-entered** (unsupported; worked in test). GLib needs `G_SOURCE_CAN_RECURSE` | **runtime frozen** for the modal's duration | runtime keeps running; `uv_run` still nested on main | runtime unaffected; GUI-side modals block only the GUI thread | as (a), but we control the outermost frame |
| JS frame under the GUI loop | yes → microtasks need manual drain (`process._tickCallback`, node-gtk does the same in `prepare`) | no | yes (stock) / no (qode, Electron) | no | **no** (we write the loop) |
| Linux | epoll fd pollable directly | GLib `prepare/query/check/dispatch` → `uv_poll`; Qt only via GLib dispatcher or polling | epoll_wait thread | ok | ok |
| macOS | kqueue fd: `poll()`-able (Electron does this); Cocoa main loop via CFRunLoop source / `QSocketNotifier` | Cocoa must own the main thread: Node can't block in `[NSApp run]` from (b) without nesting → effectively (a) | `poll()` thread + `dispatch_async` | **forbidden**: AppKit and Qt's cocoa plugin require the process main thread | ok |
| Windows | **no backend fd**: need a thread on `loop->iocp` (GetQueuedCompletionStatus + re-post), i.e. (c) for I/O and (a) for timers | `MsgWaitForMultipleObjects`-style; GLib/Qt fds are HANDLEs, so Node can't `uv_poll` them; needs threads | the standard Windows answer | allowed for Qt (but QApplication must be on the thread that created it) | ok |

Notes:
- **(a) is the best stock fit for "each toolkit's event loop is authoritative"** (PLAN rule 6).
  - The timeout side must be recomputed on the GUI thread just before it blocks: GLib `prepare`, Qt `QAbstractEventDispatcher::aboutToBlock`. Doing that on the GUI thread avoids the stale-timeout race that forced qode and Electron to patch libuv.
  - On Windows, combine (a)'s timeout handling with a thread that only waits for IOCP readiness ("c-lite"). Timers are never decided on that thread, so the race doesn't apply.
- **(b) is fine as long as the backend never runs nested loops.** GTK4 has no blocking dialogs; Qt has `open()`/`popup()` variants. But X11/Windows drag-and-drop and some platform dialogs (`QFileDialog` native) nest internally, so (b) will freeze timers and I/O during DnD on Qt.
- **(d)** is legal for GTK (any single thread) and for Qt on Linux/Windows (`QApplication` on a non-main thread works with a warning on some platforms), but **not on macOS**. It also turns every sync op into a cross-thread round trip.
  - That conflicts with `call_sync` (M4) and `get_prop`.
  - Avoid it, except as a fallback for runtimes that can't integrate (Bun) or for `eventloop-graft`-style "JS in a worker" designs. In those, JS in a Node `Worker` talks to the GUI on the main thread; the inverse layout also works on macOS.
- **(e) Embedded libnode** is the qode result without a fork. The embedder API (`CommonEnvironmentSetup`, `LoadEnvironment`) does not force `SpinEventLoop`. Our host's `backend->run()` can own main and do `uv_run(NOWAIT)` + `platform->DrainTasks()` from a GSource or `QSocketNotifier`, with no JS frame below. That removes the microtask hack.
  - Nested loops still re-enter `uv_run`, but only from our own frame. We can guard it and degrade to "don't run uv while nested" if needed.
  - Cost: shipping or locating `libnode.so` (Debian yes, Arch no, macOS Homebrew *unverified*) and the C++ embedder API, which is not ABI-stable across Node majors. Rust wrappers (`edon`, `libnode_sys`) are early-stage.

## 3. Deno (stock CLI via FFI, and embedded deno_core/deno_runtime)

### 3.1 Stock `deno` + `Deno.dlopen` of our C ABI
- Deno owns the main thread (tokio current-thread + V8). There is no way to hand it to GTK/Qt and still run Deno's loop: no `uv_backend_fd` equivalent is exposed. Strategy (a) is impossible.
- What works (tested in §4) is **(b) split poll** for GLib: prepare/query/check/dispatch on the JS thread, and `poll(2)` as a `nonblocking: true` symbol.
  - Signal callbacks are plain synchronous `UnsafeCallback`s fired inside `g_main_context_dispatch` on the JS thread, and microtasks drained correctly.
  - For our ABI this becomes `backend->pump()`-style entry points (see §5), exported from the backend `.so` and called via `Deno.dlopen`.
- `UnsafeCallback.threadSafe()` is the right tool for `post_event` and `wake` from a non-JS thread: a Qt worker, or a backend that runs its loop on another thread under (d). It keeps the process alive and wakes the loop.
- Qt via Deno FFI is harder:
  - On Linux, Qt uses the GLib dispatcher, so the same split poll works.
  - Elsewhere it's either a periodic `processEvents()` (polling) or Qt on a non-main thread (not macOS).
- FFI marshalling costs ~tens of ns per call for simple signatures, so `svl_value` structs need a buffer-packing convention. That is fine for an op-per-call ABI.

### 3.2 Embedding `deno_core` / `deno_runtime` in our Rust host
- Current versions: `deno_core` 0.412 / `deno_runtime` 0.267 (2026-09), `v8` crate 152.2. deno_core now lives in the main deno repo (`libs/core`).
- `JsRuntime::poll_event_loop(&mut self, cx, PollEventLoopOptions)` is a plain `poll` fn, so it can be driven from our checkpoint with a custom `Waker` whose `wake()` calls `backend->wake()`. This fits `svl_host.checkpoint` exactly.
- **The catch is tokio.** The default `reactor-tokio` feature means timers and async ops expect a tokio context, and tokio's I/O and timer drivers only make progress when a runtime parks or drives them. Working design:
  - a **multi-thread tokio runtime** whose worker threads run the I/O and timer drivers;
  - the GUI thread holds `rt.enter()` while calling `poll_event_loop` in each checkpoint;
  - wakers fire on tokio threads and call the thread-safe `backend->wake()`.
  - A new `Reactor` trait in deno_core (currently only used for timers) may later allow a GLib-backed reactor.
- `deno_runtime::MainWorker` (Node compat, Web APIs, permissions) is public and embeddable in principle, but it is heavy and tracks Deno CLI internals. Plain `deno_core` + our own ops is the sensible scope: we need timers, console, module loading and our host ops, not `fetch` and `node:*`.
- Cost:
  - The rusty_v8 prebuilt static lib is ~40 MB compressed download; a source build is ~30 min. Reported binary growth is ~20–30 MB (*unverified*).
  - It adds a tokio dependency and a longer link; incremental Rust builds of the host crate get slower mainly from link time.
  - deno_core also gained an experimental **QuickJS backend** (PR #36194, 2026-08). Worth tracking: one op layer could serve both engines.

## 4. Experiments (all under Xvfb, 2026-10-16)

Sources: `/tmp/claude-1000/-projects-svelters/10c0a3c6-7a5c-4d9b-9688-b5af0249a13a/scratchpad/node-deno/gtkloop.c` + `test.cjs` (GTK4/Node), `qtloop.cc` + `qtest.cjs` (Qt6/Node), `deno_gtk.ts` (GTK4/Deno, pure FFI).

Each run creates a window with a button, then checks the following: `setTimeout`, promises, `fs.promises`, `setImmediate`, a native `clicked` signal calling into JS (the microtask in the handler mutates the widget), timer overshoot, idle CPU over 2 s, a nested modal loop for 300 ms while a JS `setInterval(20)` runs, and a `worker_threads` message (a cross-thread uv wake).

### 4.1 Results

| Runtime / toolkit / strategy | Works | setTimeout(10) overshoot p50 / max | Idle CPU per 2 s | JS ticks during 300 ms nested loop |
| --- | --- | --- | --- | --- |
| Node + GTK4, **A**: GLib owns main, GSource on `uv_backend_fd` → `uv_run(NOWAIT)` | yes | 0.4 / 9–15 ms | 9 ms | **0** without `G_SOURCE_CAN_RECURSE`; **12–14** with it |
| Node + GTK4, **B**: Node owns main, uv_prepare/uv_poll/uv_timer/uv_check embed `GMainContext` | yes | 0.5 / 9 ms | 9 ms | **0** (Node starved) |
| Node + Qt6, **A**: `QApplication::exec`, `QSocketNotifier(uv_backend_fd)` + `QTimer` re-armed on `QAbstractEventDispatcher::aboutToBlock` | yes, with both the Glib and the Unix dispatcher (`QT_NO_GLIB=1`) | 0.3 / 2 ms | **0.6 ms** | **14** (`QDialog::exec`) |
| Node + Qt6, B-crude: `processEvents()` from uv_check + a 16 ms uv_timer | yes | 0.5 / 2–7 ms | 8 ms (the polling timer) | **0** |
| Deno + GTK4 via `Deno.dlopen` only, **split poll**: prepare/query/check/dispatch on the JS thread, `poll(2)` as a `nonblocking: true` FFI call | yes | 1.5 / 4 ms (Deno timer granularity) | 8 ms, 0 GLib iterations while idle | **0** (the JS thread is blocked in the FFI call) |
| Bun + the same N-API addon | **crash**: `Features: ... unsupported_uv_function` (Bun's N-API does not implement `uv_backend_fd`, `uv_prepare_init` and similar) | – | – | – |

The first frame takes 90–130 ms in every configuration (llvmpipe GL init under Xvfb). One Deno run showed 1.8 s once, a cold shader cache. Worker-message latency was 0.25–0.4 ms in all Node modes.

### 4.2 Things the experiment exposed

1. **Microtasks do not drain while JS is blocked in `run()` (Node, strategy A).** The main script, and every `setImmediate` or timer callback, runs inside Node's `InternalCallbackScope`. Nested `MakeCallback`s (`napi_make_callback`, and Node's own timer callbacks run from our nested `uv_run`) skip the tick/microtask drain when the scope depth is > 1. The symptom: the first timer fired, but its `await` continuation never ran. The loop then busy-spun, because `uv_backend_timeout()` returns 0 when the loop is not alive. **Fix used:** after each `uv_run(NOWAIT)` and each signal-to-JS call, the addon calls `process._tickCallback()` (DEP0134, documentation-only deprecated, still present in Node 22). This is the main stock-Node wart. qode and Electron avoid it because they patch `node::Start` so that no JS frame sits under the GUI loop.
2. **Nested loops imply re-entrant `uv_run`.** Keeping Node alive during a modal `QDialog::exec` or a nested `GMainLoop` means calling `uv_run` from inside a uv callback. libuv documents `uv_run` as non-reentrant. Timers, fs and async handles worked in the test, but the Linux `uv__io_poll` stashes its event array in `loop->watchers[nwatchers]`, and a nested poll clears it. If an fd is closed in the nested loop, the outer iteration can then dispatch a stale event. Treat this as "works in practice, unsupported". GLib also needs `g_source_set_can_recurse(TRUE)`, otherwise the uv source cannot be dispatched again while it is already dispatching (0 ticks).
3. **Strategy B (runtime owns main) starves the runtime during any nested toolkit loop.** This is the same for Node B and Deno split-poll. It applies to `QDialog::exec`, `QMenu::exec`, `QDrag::exec`, and GTK nested `GMainLoop`s. Mitigation: the backend uses only async variants (`QDialog::open`, `QMenu::popup`, GTK4 async dialogs). GTK4 has no blocking dialog API anyway.
4. **A uv_backend_fd watcher needs no thread on Linux.** The epoll fd is pollable by GLib (`g_source_add_unix_fd`) and by Qt (`QSocketNotifier`). The timer side is `uv_backend_timeout()`, re-armed from GLib `prepare` or Qt `aboutToBlock`. Cross-thread wakeups (worker messages, threadpool completions) arrive via uv_async → eventfd → epoll fd readable, so latency stays sub-ms.
5. **Qt on Linux uses `QXcbGlibEventDispatcher` by default.** So one GLib integration also covers Qt on Linux. The Qt-native `QSocketNotifier` path also works with `QT_NO_GLIB=1`, so a GLib dependency is not required.
6. **Bun:** N-API loads, but libuv functions abort the process. `bun:ffi` has no nonblocking calls. The Bun path is therefore either pure `bun:ffi` with a timer-driven `g_main_context_iteration(FALSE)` pump (polling: costs CPU and adds latency), or a Worker running `poll(2)` and posting back. Neither was tested.
7. **Deno split poll is GLib-specific.** It relies on GLib exposing `prepare/query/check/dispatch` so that only the `poll(2)` leaves the JS thread. Qt has no public equivalent, apart from Qt-on-GLib on Linux, which can use the same trick. JS may call GTK between `prepare` and `check`, which differs from how GLib's own iterate is used. It worked here, but the GLib docs do not cover it. A wakeup is needed to cancel the pending poll: `g_main_context_wakeup`. Toolkit mutations from JS wake it automatically through the frame clock.

## 5. Recommendation: runtime abstraction in the host

### 5.1 Split the host core from the engine

Today L2 is "Rust host on rquickjs". Make it three pieces:

1. **`svelters-core` (engine-agnostic Rust).** Handle table, name→ID cache, event queue, the `SveltersHost` op implementations, and the `svl_host` callbacks. It knows nothing about JS values beyond a small `HostValue` enum mirroring `svl_value`.
2. **`JsEngine` trait.** In-process engines implement it. Sketch:
   ```rust
   trait JsEngine {
       fn install_host_ops(&mut self, core: CoreHandle);              // expose SveltersHost to JS
       fn dispatch_events(&mut self, ev: &mut dyn Iterator<Item = Event>); // __svelters_dispatch
       fn run_jobs(&mut self, budget: usize) -> JobsState;           // microtasks/jobs, bounded
       fn next_deadline(&self) -> Option<Instant>;                   // timers owned by the engine
   }
   ```
   - The QuickJS implementation is the existing rquickjs code.
   - The deno_core implementation is `poll_event_loop` with a waker → `backend->wake()`, plus a tokio multi-thread runtime for drivers.
3. **External-runtime fronts.** `svelters-core` is also built as a **cdylib** with:
   - a **N-API front** (napi-rs) for Node. It provides the `SveltersHost` object, and its `checkpoint` is `uv_run(NOWAIT)` + microtask drain.
   - a plain **C front** for Deno/Bun FFI (`svl_rt_*` functions taking packed buffers).

   In these fronts the JS engine is the external runtime, and the core is only the op/handle/event layer.

### 5.2 ABI additions (go through `backend-abi.md` first)

`run()`/`wake()`/`quit()` assume the backend owns the loop and the host is woken only by `wake()`. That is enough for QuickJS and deno_core, where all host-side async work funnels into `wake()`. Node needs the backend to **watch a foreign fd and deadline**, and Deno-FFI needs a **non-owning pump**. Proposed optional additions (versioned, NULL = unsupported):

```c
/* svl_host additions: an external event source the backend loop must watch */
int     (*foreign_fd)(void *self);          /* -1 = none (QuickJS, deno_core); uv_backend_fd for Node */
int64_t (*foreign_timeout_ms)(void *self);  /* called right before the toolkit loop blocks; -1 = infinite */
/* the backend calls checkpoint() when foreign_fd is readable or the timeout elapses */

/* svl_backend additions: run the toolkit loop without owning the thread (Deno/Bun FFI, Node mode b) */
int  (*poll_prepare)(void *self, struct svl_pollfd *fds, size_t cap, size_t *n, int *timeout_ms);
void (*poll_dispatch)(void *self, const struct svl_pollfd *fds, size_t n);   /* after poll(2) returns */
```

- **GTK backend:**
  - `foreign_fd` becomes a GSource with `G_SOURCE_CAN_RECURSE`, and `foreign_timeout_ms` is read in `prepare`.
  - `poll_prepare`/`poll_dispatch` wrap `g_main_context_prepare/query` and `check/dispatch`.
- **Qt backend:**
  - `foreign_fd` becomes a `QSocketNotifier`, with the timeout read on `aboutToBlock`.
  - `poll_*` exist only when the GLib dispatcher is active; otherwise NULL, and the front falls back to (a).
- **Windows:** `foreign_fd` = -1. The Node front runs an IOCP-readiness thread that calls `backend->wake()`, while timeouts still come from `foreign_timeout_ms` on the GUI thread.
- Document in the ABI that **nested toolkit loops may re-enter `checkpoint`**. The host must tolerate this; it already must for `call_sync`.

### 5.3 Priorities and costs

1. **QuickJS (default).** Unchanged. Do the core/engine split in P2 now so that nothing rquickjs-specific leaks into `svelters-core`. Cost: small, and it pays off immediately because the P1 Node TestHost and the native host converge on one op layer.
2. **Stock Node via an N-API addon, strategy (a).** Best value: npm, V8, Chrome DevTools via `--inspect`, no custom Node.
   - Cost: the napi-rs front (~1–2 weeks after P5) plus the `foreign_fd`/`foreign_timeout_ms` hooks (a day per backend).
   - Accept the `process._tickCallback` drain (an internal API; pin a Node range and test it in CI), and possible `uv_run` re-entry during modals.
   - Linux first. Windows needs the IOCP thread. macOS needs (a) with a `poll()`-thread or CFRunLoop source for the kqueue fd.
3. **Embedded deno_core (V8 inside our binary).** When QuickJS performance or debugging becomes the bottleneck. It keeps "one binary, our loop" and avoids every stock-runtime wart.
   - Cost: +~20–30 MB binary, V8 prebuilt download, tokio, and slower links.
   - Keep it a cargo feature, off by default.
   - Revisit if deno_core's QuickJS backend matures; it could replace rquickjs and unify the op layer.
4. **Stock Deno via FFI (split poll).** Cheap once the C front and `poll_*` exist: the experiment is ~100 lines of TS. GTK only, or Qt on Linux. JS freezes during nested loops, so it suits the dev/scripting story more than production.
5. **Embedded libnode (e).** The cleanest Node semantics, but libnode packaging is uneven (no Arch, unknown elsewhere) and the C++ embedder ABI changes per major. Only if (2) hits a wall.
6. **Bun.** Skip. There is no libuv, and FFI has no async calls. The only options are timer polling or a worker-thread `poll(2)` with `JSCallback({threadsafe})` → (b)-only, Linux-only. Not worth maintaining.
