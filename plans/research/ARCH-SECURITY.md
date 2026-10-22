# ARCH-SECURITY: a deny-by-default, auditable policy for extensions

> **Deferred (user, 2026-10-22):** "this is too complicated this early on in our prototype" /
> "leave room for security policies later but right now we're developing". Nothing here is built
> now; only §5's cheap API-shape items (principal ids, `#[non_exhaustive]`) go into running lanes.
> SEC-O and SEC-C stay open until cadatomic loads its first third-party extension.

Status: architecture proposal (research, no code). 2026-10-22. svelters master `39ec3558`, branches `ci/ld1` (`b14023c6`),
`ci/q1`, `ci/h1` as pushed on 2026-10-22; cadatomic master `4a5decc`. Scope: the **mechanism** svelters gives its
embedders so that every capability an extension can reach is granted by an explicit policy, checked at one
enforcement point, and recorded. The **policy content** (which extension gets what), its UI, signing and
distribution are cadatomic's (§4). Open questions are split into SEC-O (svelters mechanism, for the user) and
SEC-C (cadatomic policy, routed through main-cadatomic) in §6.

Refines: ARCH-EMBED (`RootOwner`, `MountSpec`, §6.2 failure modes, §8.5 behaviours), ARCH-SCENE (§5.1 owners,
§5.3 `Origin`, §6 access, §6.5 journal, §7.3 passes, §7.5 overlays, §11 snapshots/MCP), LD1 (`ModuleHooks`,
`Visibility`, `SchemeHook`, `DylibModules`), TRACKS ISOLATES (ISO1–ISO3), CUSTOM-WIDGETS (`svl_pack`),
QUICKJS-RUNTIME item 9, HMR.md §3.2. Answers the svelters half of cadatomic's U5 ("which extensions, which
capabilities"), D11 (extension windows under U5), js-api A5 (checks at import and at call, revocable) and MQ5
(extension data into Rust tables under a budget).

Short names for cadatomic sources (under `~/projects/cadatomic/docs/`), as in ARCH-EMBED:

| Name | File |
|---|---|
| decisions | `plan/00-decisions.md` (U5, D4, D11, U3, 10/16, "Native extensions", "Extensions: a deny-by-default…") |
| requests | `plan/04-kernel-requests.md` (SV, OB, SY tables) |
| js-api | `research/2026-10-22-js-api-publishing-and-live-ide.md` (§3.7 capabilities, §3.9 lifetime, §6.3 layers, §6.7 permissions) |
| framework | `research/2026-10-22-component-framework.md` (§5.1 isolates, "what extensions can reach") |
| docking | `research/2026-10-22-docking-windows-and-svelters-roots.md` (§6.1 `Root { owner, caps: GrantedCaps }`) |
| native-ext | `research/2026-10-22-native-extensions-abi-and-engines.md` (N9, §7.1 tiers, §8 outboard/switchyard, NX10, OB3–OB6) |
| shell | `research/2026-10-22-ide-shell-and-ux.md` (U5, §7 agent permissions) |
| models | `research/2026-10-22-models-filtering-and-dynamic-data.md` (MQ5) |
| commands | `research/2026-10-16-command-architecture.md` (one command path, `Invocation`, §3.8 observability) |
| scripting | `best-practices/scripting-and-extensions.md` (§6 capability declaration and trust) |

## Binding inputs

- **User, 2026-10-22** (svelters CLAUDE.md; decisions, last entry): extensions run under a deny-by-default,
  auditable policy. Every capability svelters gives an extension host is granted by an explicit policy; an
  unconfigured extension gets nothing; new host, loader and scene APIs an extension can reach have a policy hook.
  svelters provides the mechanism (hooks, fail-closed defaults, audit events); cadatomic owns policy content, UI
  and signing.
- **U5** (decisions, shell §9): one policy layer, a settings layer between defaults and user, distributed by the
  hub or a self-hosted server, lockable keys, visible provenance; it carries extension policies and remote
  features. This document builds on U5 and does not add a second policy layer.
- **D4**: first-party Svelte runs in process on the UI thread; third-party extensions run in isolates. **D11**:
  extensions may open OS windows and tear off panes, under U5.
- **Native extensions** (decisions): no native hot reload; engines static; third-party native code only out of
  process (outboard workers) under U5. **10/16**: static Rust plugins only for now.
- **The one command path** (commands §1): no document changes except through a registered command; every
  invocation carries `source`, `trace_id`, `parent`. It is the audit point for document edits (js-api A10, §3.10).
- **LD1, decided (coordinator)**: `ModuleHooks::default()` serves all `svelters:app/*` (primary host);
  `ModuleHooks::isolate()` defaults to `Allow([])`; a hidden module looks absent; `DylibModules` behind the
  `dlopen` feature, which cadatomic leaves off.
- **C-1** stock QuickJS-NG through rquickjs 0.14, no fork (svelters CLAUDE.md).

## 0. Summary

1. **Two placements, one gate.** An extension's JS runs in an isolate: a QuickJS runtime of its own that sends op
   batches and API calls across a boundary. Every policy check sits on the **trusted side of that boundary**
   (the UI-thread batch applier, the module loader, the export/API router), never inside the isolate. The same
   gate then works for a thread isolate and for a sandboxed **process** isolate, and moving an extension out of
   process changes no policy code (§1.4, §3.6).
2. **In-process QuickJS isolation is not enough for the high-security tier.** It stops a JS extension that plays
   by the engine's rules; it does not survive one memory-safety bug in QuickJS-NG (C), rquickjs or a host op's
   conversion code, after which the attacker holds the whole process: every open document, every credential,
   the network. High-security operators need out-of-process, OS-sandboxed isolates for untrusted extensions
   (Chrome's renderer model). Thread isolates stay the default for first-party and vetted code (§1.4, SEC-O1).
3. **Principals, not code locations.** Every runtime, root, handle, journal origin, module load and audit event
   carries a `PrincipalId`. One principal per runtime: no two principals ever share a QuickJS runtime
   (SEC-O7). The first-party realm is a principal too, with a broad built-in grant, so it is audited by the
   same path (SEC-O4).
4. **One hook trait, implemented by cadatomic.** `svelters_host::policy::Policy::check(&Request) -> Decision`
   is called at each enforcement point in §2 with the principal, a typed capability, a resource and a mode
   (read, write, observe, use). The default implementation is `DenyAll`. svelters ships `StaticPolicy` (a table)
   for tests and svelters apps. cadatomic implements `Policy` over U5.
5. **Object capabilities for objects, grants for ambient things.** Handles, slots, stores and API objects are
   unforgeable per-principal capabilities: an isolate can only name what it created or was handed (ISO1's
   per-isolate handle space). Ambient capabilities (importing a module, instantiating a kind or behaviour,
   opening a window, the clipboard, timers past a budget) are checked against grants.
6. **Declared ∩ granted.** An extension's manifest declares what it requests; the operator's policy grants a
   subset; the effective set is the intersection. A capability that is granted but not declared is not usable,
   so an update that adds a request needs a new grant (Chrome's rule).
7. **Checked at resolve and at use, revocable.** ES module records cannot be unloaded, so a module import is a
   first check only; every native call re-checks against the grant epoch. Revoking a structural capability (a
   kind, a window, a slot) restarts the principal's isolate; revoking a data capability fails the next call.
8. **Audit is a stream of typed events** (`AuditEvent`) to an `AuditSink` the embedder supplies: every deny,
   first use of each grant, every write-class use, and every read of a labelled data class. A capability can be
   marked audit-required, so it is denied when the sink cannot accept the record (SEC-O5).
9. **Integrity: the code that runs is the code that was approved.** The loader verifies each module's content hash
   against a host-supplied lock; extension isolates have no `eval`/`Function`; QuickJS bytecode is never read
   from an untrusted source; live edit of a signed extension makes it a dev principal with no grants (§3.5).
10. **Milestones** SEC1–SEC4, 12–17 lane-days in all, none on SC1's path; what running lanes must do now is
    §5 (mostly: carry a principal through the APIs they are adding).

## 1. Threat model

### 1.1 Principals and tiers

| Tier | What | Placement (decided or proposed) | Trusted for |
|---|---|---|---|
| **T0** app core | Rust compiled into the binary: svelters, cadatomic, engines, providers (`#[svelters::widget]`, behaviours, `StateProvider`, `ScenePass`), E0 exports | in process | everything; it *is* the reference monitor |
| **T1** first-party realm | cadatomic's own Svelte panels, from the release bundle (D4) | one QuickJS runtime on the UI thread | what its built-in grant says (SEC-O4); integrity of the shipped bundle |
| **T1o** overlays | user or project edits of first-party sources (js-api §6.3, §6.7) | today: the first-party realm | **not trusted in high-security profiles** (SEC-C5): project overlays are someone else's code |
| **T2** JS extension | third-party Svelte/JS from an extension package, including its bundled npm dependencies | isolate: thread (default) or process (high-security, SEC-O1) | nothing beyond its grants |
| **T2d** dev extension | an extension being edited in the live IDE, or a "fork to dev" copy | isolate | nothing; no grants until granted again (§3.5) |
| **T3** third-party compute | WASM component or outboard worker (native-ext §7.1, §8) | Wasmtime in process, or a worker process | nothing; enforced by cadatomic (NX10), outside this document's mechanism |
| **T4** agents | MCP/RPC clients, the in-app assistant (shell §7, U3) | out of process | their grants; svelters sees them only as readers of scene snapshots and selector queries |

### 1.2 Attacker capabilities assumed

| Id | Attacker | Can | Wants |
|---|---|---|---|
| **A1** | malicious JS extension (T2) | run any JS in its isolate; emit any op sequence, any handle integer, any prop value, any module specifier (relative, absolute, `../`, schemes, `data:`); loop, allocate, spawn timers; pollute its own realm's prototypes; render any pixels inside its slots; hold focus and read keys typed into its own widgets | read data it was not granted (other documents, other extensions' state, first-party UI contents, files); exfiltrate (network, clipboard, files, a crafted document, an agent's context); act as the user (spoofed dialogs, clickjacking overlays, fake first-party chrome); escalate (edit first-party overlays, use a privileged API as a confused deputy, persist past revocation); deny service |
| **A2** | compromised dependency inside an extension | same as A1, under that extension's principal | same as A1; it inherits the extension's grants, which is why grants are narrow and per-extension |
| **A3** | engine exploit from A1 | turn a memory-safety bug (QuickJS-NG C code, rquickjs `unsafe`, a host op's value conversion, the bytecode reader) into native code execution | everything the process can do; **only process placement plus an OS sandbox contains it** (§1.4) |
| **A4** | malicious native pack (`svl_pack` code pack, `DylibModules` library) loaded in process | anything | everything. There is no in-process containment of native code; the only control is not loading it (cadatomic: never in process, decisions "Native extensions"; svelters: refused under a policy, SEC-O11) |
| **A5** | hostile input to trusted code | crafted `.svelte`/CSS/JSON source to the compiler (CX1, Q1 live compile, Q3), crafted op batches from a process isolate, crafted bytecode | crash or exploit T0 parsers; a Rust stack overflow aborts the process |
| **A6** | over-curious agent (T4) | call MCP `ui.tree`, `ui.inspect`, `screenshot.*`, selector queries | read controlled data shown in any root |

### 1.3 Out of scope

- **The operator and the local OS account.** The policy store, the binary and the user's files are protected by
  the OS and cadatomic's packaging. A user with write access to the binary is not an attacker we model.
- **T0 supply chain.** A compromised crate in the app is a build-provenance problem (cadatomic PB16: signed
  releases, SBOM, reproducible builds), not a runtime policy problem.
- **Covert and side channels** in the thread placement (timing, cache, Spectre-class reads across isolates in one
  address space). QuickJS has no JIT and svelters does not expose `SharedArrayBuffer` across isolates, which makes
  them harder, not impossible. Process placement is the answer where they matter.
- **Availability guarantees.** Budgets (§3.7) bound what one extension can take; they do not promise a frame
  rate under attack.
- **Pixels.** An extension can draw anything inside its slots, including text that imitates first-party UI. We
  bound *where* it draws (§2, slots and overlays) and label extension surfaces (SEC-C), we do not inspect
  content.
- **Third-party compute (T3) and worker sandboxing**: cadatomic's NX10/OB6, outside svelters.

### 1.4 Is in-process QuickJS isolation enough?

**No, not for untrusted code in the high-security tier. Yes, for policy enforcement over code whose engine
behaviour we trust.**

- **What thread isolation gives.** A separate `rquickjs::Runtime` per isolate (QUICKJS-RUNTIME item 9) has its
  own heap, GC, globals, interrupt handler and memory limit. JS objects cannot cross runtimes. If every
  capability is reached through host ops and the gate checks them, a JS extension that does not break the engine
  is confined to its grants. That is the Deno model (permissions enforced at the op layer in one process), and it
  is enough for first-party code, vetted extensions and protection against *mistakes*.
- **What it does not give.** The engine is C, the binding has `unsafe`, and svelters' own value conversion
  (`svelters-host/src/convert.rs`, `quickjs/conv.rs`, `quickjs/buffers.rs` for external buffers, and `svelters-abi/src/safe.rs` for `svl_value`) contains `unsafe` FFI. One exploitable bug gives native
  execution with the process's authority: every open document, keys in memory, the network, the filesystem. A
  browser does not rely on its JS engine for this: Chrome puts each site in a sandboxed renderer process
  precisely because engine bugs are expected. QuickJS and QuickJS-NG have had memory-safety bugs reported from
  fuzzing [verify the current OSS-Fuzz status of QuickJS-NG before quoting it]. A high-security operator should
  assume an attacker who ships an extension will look for one.
- **What closes the gap.** Run the untrusted isolate in **a separate OS process** whose only resource is the
  transport to the host: no files, no network, no other process (Linux: seccomp-bpf plus Landlock or a user
  namespace; Windows: AppContainer or a restricted token plus a Job object; macOS: App Sandbox or
  `sandbox_init`-style profiles [mechanisms per platform to be verified in SEC4]). An engine exploit then holds
  only what the sandbox holds: the extension's own state and its transport, which the host still checks batch by
  batch. This is the same boundary cadatomic already chose for third-party *native* code (decisions, "Native
  extensions") and NX10 chose for workers.
- **Cost.** The ISO1 design is already a serializable op-batch protocol over a transport trait (framework §5.1,
  TRACKS ISO1), so process placement is a transport plus an isolate-host binary, not a redesign, **provided the
  protocol never relies on shared memory** (no `Arc` crossing, `Shared<[T]>` views copied or moved through
  shared-memory segments with a copy fallback, §5 ISO1). Latency: one extra hop per batch; batches are per
  checkpoint, not per op.
- **Recommendation (SEC-O1 b).** Both placements, chosen per extension by policy: `Thread` by default,
  `Process { sandbox }` required by a high-security profile for any extension not on the operator's trusted list.
  The gate is identical; only the transport differs.

## 2. Inventory: what svelters exposes, and where the check goes

Code facts are svelters master `39ec3558` unless a branch is named; `host/` is `crates/svelters-host/src/`,
`egui/` is `crates/svelters-egui/src/`, `modules/` is `crates/svelters-modules/src/` (Q1/LD1 branches).

### 2.1 What exists today, and what it means

| Fact | Where | Consequence |
|---|---|---|
| Nothing on master checks a permission. The containment that exists: the bundle-directory jail of the master resolver, the per-host handle table, the scene's owner rule (JS cannot destroy `Owner::Host` slots), and QuickJS limits (256 MiB memory, 1 MiB stack, 10 s turn budget) | `host/quickjs.rs:1171` (`BundleResolver`), `:1214`/`:1236` (`BundleLoader`, symlink check); `host/core.rs:47` (`HostConfig`); `egui/scene/access.rs:703-710` | the gate is new code; nothing has to be unpicked |
| One host is one runtime and one context, built with `Context::full` (eval, `Function`, dynamic `import()`) | `host/quickjs.rs:160`, `:197` | SEC-O10 needs an alternative constructor for extension hosts |
| The op surface is `globalThis.__svelters_host`, an ordinary writable global; every op goes through `core_of` → `handle_of` (a number check) → a `Core::op_*` method | `host/quickjs.rs:755-1161`, `:603` | JS can always reach the ops; the check must be in the Rust `Core::op_*` methods, never in JS |
| The runtime's entry hooks (`__svelters_dispatch`, `__svelters_dispatch_sync`, `__svelters_changed`, `__svelters_gone`, `__svelters_embed`; `__svelters_hot` on `ci/h1`) are writable globals that native code looks up per call | `packages/native-runtime/src/index.js:2142`, `embed.js:328`; `ci/h1` `hot.js` | harmless across principals once one runtime is one principal (SEC-O7); in the first-party realm, any overlay can hook every event, which is one more reason overlays are first-party code (SEC-C5) |
| **The module filter can be bypassed**: `construct('app:mod.T')` and `callStatic('app:mod', …)` reach every E0 export with no import, through `op_kind_id` → `app_kind` | `host/core/ops.rs:186-199`, `host/core/apps.rs:29`, `:158`, `:198` | LD1's `Visibility` is necessary, not sufficient: the op path needs the same check (R-B) |
| **Root messages are addressed by a root id JS supplies**: any root's JS can fault, retitle or emit as another root | `host/quickjs.rs:877-904` (`rootFault`/`rootInfo`/`rootEmit`); `egui/embed/mod.rs:1487-1505` | the host must stamp the root from the calling context, not trust the argument (§5 EM1) |
| One handle table per host, shared by every root of that host; handles found in backend results are adopted | `host/core.rs:272`; `host/core/ops.rs:104`, `:117`, `:622` (`adopt_handles`); `host/core.rs:1278` | per-principal handle spaces need one runtime per principal (R-C) |
| `persist(key, …)` is one map per host: any root reads any key | `host/core/reactive.rs:303`, `:314` | namespace by principal (§5 E0/E3) |
| Offload op ids (`#[js(worker)]`, `#[js(tokio)]`) are not scoped to a root | `host/core/offload.rs:147`, `:449`, `:559`; `host/core/asyncs.rs:138` | cancel and stream-ack must check the op's principal |
| `alloc(TypedArray, n)` creates external buffers up to 2 GiB per call, **outside** QuickJS's memory limit | `host/quickjs/buffers.rs:208` | a budget (§3.7) |
| `setTimeout`/`setInterval` have no count cap; the console buffer is unbounded | `host/core.rs:789` (`timer_set`), `:489` (`console`); `host/prelude.js:16-54` | budgets |
| The export schema is process-global (`OnceLock` over `inventory`) | `host/exports/schema.rs:150` | per-principal views are filters over one schema, not separate schemas |
| No fetch, files, network, clipboard or primary selection exist in the host or the egui backend | grep of `host/`, `egui/` | today the ambient surface for egui is small; keep it that way: new ones arrive as gated ops or as cadatomic APIs |
| **GTK reaches all of GObject Introspection**: any GType or `Gio.*` name, any typelib method, construct and static calls, so JS on GTK has files, processes and network through Gio | `crates/svelters-gtk/src/backend.rs:346`, `:363`, `:410`, `:1017` | policy mode is **egui only**: a GTK, Qt, AppKit or WinUI host must not run extension principals unless `Backend::resolve` is gated (out of scope: those backends get maintenance only) |
| "Dangerous members" are typings and docs, not enforcement | `docs/notes/dangerous-members.md`, `metadata/dangerous/*.yaml` | — |
| `Embed::mount` refuses `RootOwner::Isolate` | `egui/embed/mod.rs:621-623`, `egui/embed/roots.rs:22` | there is no extension placement yet: the mechanism can land before ISO1 |
| Q1's `SourceProvider` loads any path it resolves (absolute, `..` above the root, assets as absolute paths) | `ci/q1` `modules/source/mod.rs:352-369`; noted in `eab37e97` | an extension must never reach it except through a composed, confined provider (§5 Q1) |
| Q1's `ModuleCode::Cached` loads bytecode with `unsafe { Module::load(bytes) }` | `ci/q1` QuickJS adapter | crafted bytecode is A3 (SEC-O12) |
| LD1 (tip `b14023c6`) adds `ExtensionPolicy { visibility, native_modules }`, `open()` and `deny_all()`; `ModuleHooks::isolate()` = `deny_all()`; `load` re-checks only added natives, not app-export visibility | `ci/ld1` `modules/compose.rs:27`, `:51`, `:77`, `:168`, `:187`; `host/hooks.rs:25`, `:47` | the first slice of this mechanism; SEC1 grows it into `Policy` (§5 LD1) |
| A source-map lookup joins a module name parsed from error text that JS controls onto the bundle root | `host/sourcemap.rs:29-35` | minor read; confine to the package root like the loader |

### 2.2 Capability inventory

Status: **now** on master, **br** on a pushed branch, **plan** in a planned lane. "Gate" is where the check
goes; "Cap" is the §3.2 capability.

| # | Capability | Where | Gives JS | Gate | Cap | Status |
|---|---|---|---|---|---|---|
| M1 | import `svelters:core` | `host/exports/codegen.rs:609`; `ci/ld1` `compose.rs` | the object core the generated modules use | none: always visible (it is the op surface to the principal's own objects) | — | now |
| M2 | import `svelters:app/<module>` | `codegen.rs:609` (`builtin_module`); `ci/ld1` `ComposedProvider::resolve`/`load` | an E0 module's classes, functions, stores, events | `ComposedProvider::resolve` **and** `load` (LD1 checks only resolve for app exports) | `Module` | now, gate br |
| M3 | `svelters:behaviors/<ns>`, `svelters:embed`, `svelters:<toolkit>/*` | aliased at build time by Vite (`packages/native-runtime/vite.js:130-143`); module source from `egui/spec.rs:573` | ordinary JS over host ops | when the Rust loader serves them (Q1/LD1), the same resolver gate; until then they are bundle code, gated by the ops they use | `Module` | br/plan |
| M4 | host schemes (`cadatomic:api/*`) | `ci/ld1` `SchemeHook` (`compose.rs:10`) | whatever the hook returns | `ComposedProvider` before calling the hook; the hook gets the principal (§5 LD1) | `Scheme`, `Ext("api",id)` | br |
| M5 | relative and package imports, JSON and assets | master `BundleResolver`/`BundleLoader`; `ci/q1` `SourceProvider` | file contents as modules | package-root confinement in the provider; module lock hash check before evaluation (§3.5) | — (confinement), integrity | now / br |
| M6 | dynamic `import()` | same loader | as M2–M5, at run time | same gates; nothing extra | as M2–M5 | now |
| M7 | `eval`, `new Function` | `Context::full` (`quickjs.rs:197`) | code generation | not created in extension runtimes (SEC-O10) | — | now |
| M8 | native module sets (`NativeRegistry`, `DylibModules`) | `ci/ld1` `modules/native.rs:24`, `dylib.rs` (feature `dlopen`) | JS source from a dlopen'd library | `NativeAccess` (LD1); refused under any policy but `AllowAll` (SEC-O11) | — | br |
| M9 | bytecode (`ModuleCode::Cached`) | `ci/q1` QuickJS adapter | precompiled code | only host-produced, hash-keyed (SEC-O12) | integrity | br |
| M10 | HMR swap, `import.meta.hot` | `ci/h1` `quickjs.rs:1335`, `hot_update` `:311`; `Host::apply_changes` `lib.rs:283` | new versions of modules | the loader gates again; off for locked principals (§5 H1) | integrity | br |
| O1 | create a node of a kind | `Core::op_create` `host/core/ops.rs:229`, `op_kind_id` `:187` → `EguiBackend::resolve` `egui/ops.rs:67` | any built-in or provider kind (`#[svelters::widget]`) | `op_kind_id` (cached per principal) | `Kind` | now |
| O2 | construct / call static on `app:<module>.<Type>` | `host/core/statics.rs:21`, `:98` → `apps.rs:158`, `:198` | every E0 class and static, **bypassing imports** | `op_kind_id`/`app_kind` (`apps.rs:29`) and `app_invoke` (`apps.rs:120`) | `Module`, `Export` | now |
| O3 | call a method | `op_call` `ops.rs:474` → backend `call` (`egui/ops.rs:157`: `present`, `close`, `request-focus`) or `app_call` `apps.rs:178` | native and export methods | `app_invoke` per member (exports); backend call table (`present` → `Window`) | `Export`; `Window` | now |
| O4 | set/get props, child attributes, style, bounds | `ops.rs:397`, `:461`, `:304`, `:347`, `:658`, `:669`; `apps.rs:224`, `:235` | node state on its own handles; export object props | handle ownership (R-C); export props by mode | `Export` (objects) | now |
| O5 | insert/remove/destroy | `ops.rs:263`, `:289`, `:363`; scene owner check `egui/scene/access.rs:703` | tree edits on its own nodes; mount into a slot it was given | handle ownership; slot class at mount | `Slot` | now |
| O6 | listen / observe | `ops.rs:678`, `:709`; `reactive.rs:102`, `:161` | events of its nodes; E3 store and object observation | handle ownership; store observe by mode | `Export` (Observe) | now |
| O7 | offloaded calls (`#[js(worker)]`, `#[js(tokio)]`), cancel, stream ack | `offload.rs:147`, `:449`, `:559`; `asyncs.rs:138` | background Rust work | at start (as O3) and at cancel/ack by op owner | `Export` | now |
| O8 | external buffers (`alloc`, `Buffer<T>`, E2 `Shared<[T]>`) | `quickjs/buffers.rs:208` | memory outside the QuickJS limit; zero-copy views of Rust data | budget at `alloc`; a `Shared` view is an export result, gated as O3 | `Budget` | now |
| O9 | `persist(key, make)` | `reactive.rs:303`, `:314` | state that survives reload | key namespaced by principal | `Persist` | now |
| O10 | root messages (`rootFault`, `rootInfo`, `rootEmit`) | `quickjs.rs:877-904` → `egui/embed/mod.rs:1487` | fault, title, dirty, emit to the embedder | stamp the root from the calling context; reject ids of other principals' roots | — (integrity of identity) | now |
| O11 | handle reach | `ops.rs:104`, `:117`, `:622`; `core.rs:1278` | any handle the host table knows | per-principal table (R-C); ISO1 `(isolate, handle)` map | — | now |
| E1 | timers, microtasks, rAF | `prelude.js:25-82`; `core.rs:789`, `:946` | scheduling | budgets (count, rAF only while visible: ARCH-EMBED §5.3) | `Budget` | now |
| E2 | console | `prelude.js:16` → `core.rs:489` | logging, stderr echo | bounded buffer; log lines tagged with the principal (log injection is a forensic problem) | `Budget` | now |
| E3 | event delivery (pointer, keys, focus) | `egui/events.rs`, walk | input to its own widgets | none beyond R-F: input reaches a principal's nodes only where they are drawn, inside its slots | — | now |
| S1 | scene access from Rust (`SceneHandle::read/write/defer/observe`, `Query`, `hit_test`, `ExtKey`) | `egui/scene/access.rs:159-285`, `query.rs:157-202`, `hit.rs:124`, `ext.rs`, `journal.rs` | **not reachable from JS**; T0 only | none for T0; deputies check with `cx.principal()` (R-E) | — | now |
| S2 | behaviours (`#[svelters::behavior]`, attached kinds) | `egui/provider/behavior.rs:56`, `:77`, `:126` | native per-frame behaviour on its own node | `create` of the attached kind (O1), and the behaviour reads the host node's principal (R-E) | `Behavior` | now |
| S3 | provider kinds bound to data (`<model-tree>`, `<property-grid>`, ARCH-SCENE §10) | RA1/SG4/SG5 | data shown by a Rust widget whose props the extension sets | the widget checks the data reference against `cx.principal()` (R-E); the kind itself is O1 | `Kind` + embedder `Ext` | plan |
| S4 | state providers, scene passes, overlay roots (SV7) | SG4 (ARCH-SCENE §7.3, §7.5) | Rust only; overlays from an extension root | overlay roots of an extension principal bounded (R-F) | `Slot(Overlay)` | plan |
| S5 | snapshots, selector queries, F12 inspector, MCP `ui.*` | SG7, CS2, TK2 | readers that see every root | reader principal + labels (R-G) | embedder `Ext` | plan |
| W1 | present a `window` (OS viewport), tear-off (D11) | `egui/windows.rs:40` (`present` on `window`) | a top-level OS window | backend `call("present")` and EM2's tear-off path | `Window` | now |
| W2 | mount into slots of a class (pane, section, dialog, window, overlay, popover) | `Embed::create_slot` `egui/embed/mod.rs:531`, `mount` `:621` | where its UI appears | `Embed::mount` with the principal; slot class on `SlotOptions` | `Slot` | now (class plan) |
| W3 | clipboard, primary selection | none yet (egui `ctx.copy_text`, paste events) | copy, paste | when added: a gated op; paste delivered only on a user gesture to a focused node of the principal (SEC-O16) | `Clipboard`, `PrimarySelection` | plan |
| N1 | native packs (`svl_pack`, dlopen) | design only, `CUSTOM-WIDGETS.md` §4 | — (native authority, A4) | not loaded under a policy (SEC-O11); cadatomic: never in process | — | plan |
| N2 | native backends (C ABI `svl_backend`) | `crates/svelters-abi/src/lib.rs:256`, `:287`; `safe.rs:1360` | — (T0) | not an extension surface | — | now |
| I1 | isolates (proxy backend, isolate host, compiler isolate target) | ISO1–ISO3 | an isolate's whole surface | UI-side batch applier, `(isolate, handle)` map, API router | all of the above | plan |
| I2 | compile `.svelte` in process (CX1, Q1 live compile) | `crates/svelters-compiler` | turns source into code | robustness (A5); compiled output runs as the source's principal | — | plan |

## 3. The policy model

### 3.1 Principals

```rust
// crates/svelters-host/src/policy.rs  (new; no egui, no cadatomic types)
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct PrincipalId(NonZeroU32);                 // process-local, minted by the host when a runtime is created

#[derive(Clone, Debug)]
pub struct PrincipalInfo {
    pub id: PrincipalId,
    pub kind: PrincipalKind,                         // App (T0) | FirstParty (T1) | Extension (T2) | Dev (T2d) | Agent (T4)
    pub name: Arc<str>,                              // "acme.gears": the extension id, for audit and MCP
    pub version: Option<Arc<str>>,                   // "1.4.2"
    pub content: Option<ContentHash>,                // hash of the approved package (lock), None for dev
    pub placement: Placement,                        // InProcess | Thread | Process
    pub labels: LabelSet,                            // data classes this principal is cleared for (opaque to svelters)
}
```

- **One principal per runtime.** A QuickJS runtime is created for exactly one principal; every op, call and load
  from it is that principal's. Contexts sharing a runtime (QuickJS supports several) are not used as a security
  boundary, because objects passed between them keep their identity (SEC-O7).
- **T0 Rust is `PrincipalKind::App`** and is not checked (it is the monitor). When T0 code acts *for* another
  principal (a provider widget instantiated by an extension, an export called by an isolate), it receives that
  principal and must use it (§3.4 deputies).
- **Transitive calls** follow js-api §3.7: a provider runs with its own grants; the call record names caller and
  callee; a member may declare `requires_caller`.

### 3.2 Capabilities

svelters defines the vocabulary for what *it* exposes. cadatomic adds its own (`api:<id>`, `doc.read`,
`doc.edit`, `doc.propose`, `fs.read{roots}`, `net{hosts}`, js-api §3.7) through the same `Cap::Ext` variant, so
there is one request type and one audit stream.

```rust
#[non_exhaustive]
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Cap {
    Module(ModuleId),          // import a module: "svelters:app/task", "svelters:behaviors/cad", "cadatomic:api/mcad.measure"
    Export(ModuleId, Member),  // call/construct/read one export member (default: the module grant covers members)
    Kind(KindName),            // instantiate a provider or built-in kind: "cad.model-tree", "egui.window"
    Behavior(KindName),        // attach a native behaviour: "cad.pick"
    Slot(SlotClass),           // be mounted into a slot class: PaneBody | Section | Dialog | Window | Overlay | Popover
    Window,                    // present a top-level window / viewport (D11), incl. tear-off of its own panes
    Clipboard(Rw),             // ctx.copy_text / paste events delivered to its widgets
    PrimarySelection(Rw),      // X11/Wayland primary selection, where a backend exposes it
    Persist(Rw),               // `persist(key, …)` storage, always namespaced by principal
    Budget(BudgetKind),        // raise a resource budget above the policy default (§3.7)
    Scheme(Arc<str>),          // resolve through a host scheme hook at all ("cadatomic")
    Ext(Arc<str>, Arc<str>),   // embedder vocabulary: ("api", "mcad.measure"), ("doc", "read"), ("net", "host:example.com")
}

pub enum Mode { Use, Read, Write, Observe }

pub struct Request<'a> {
    pub principal: &'a PrincipalInfo,
    pub cap: &'a Cap,
    pub mode: Mode,
    pub resource: Resource<'a>,   // None | Root(RootKey) | Document(&str) | Labels(&LabelSet) | Path(&Path) | Host(&str)
    pub trace: Option<TraceId>,   // the command path's trace id when there is one
}

pub enum Decision {
    Allow { audit: AuditLevel },  // None | First | Every
    Deny  { reason: Reason },     // stable reason id, e.g. "policy.not_granted", "policy.not_declared", "policy.revoked"
}
```

- **Scopes.** A grant is scoped by principal × capability × mode × resource. The resource dimension carries what
  cadatomic needs: a document set, a data class (`Labels`), a path root, a host. svelters never interprets
  resources beyond equality; cadatomic's `Policy` does.
- **Read vs write vs observe.** Exports and stores distinguish them (an E3 store field read, write, observe; an
  export method marked `query` vs `command` in E0 metadata). Scene access from JS is always on the principal's
  own handles, so the mode matters for exports, stores, persist and the clipboard.
- **Data classes.** svelters carries opaque `LabelSet`s on principals and roots (§3.4). It never decides what a
  label means; it filters snapshots and selector results by asking the policy (SEC-O13).

### 3.3 The hook and the default

```rust
pub trait Policy: Send + Sync + 'static {
    fn check(&self, req: &Request<'_>) -> Decision;
    /// Bumped by the embedder on any change (hub update, user revoke). Cached allows carry the epoch they were made in.
    fn epoch(&self) -> u64;
}
pub struct DenyAll;          // the default for every isolate host: fails closed
pub struct StaticPolicy { /* principal name → [(Cap pattern, Mode, Resource pattern, AuditLevel)] */ }

pub struct HostConfig { /* … */ pub policy: Arc<dyn Policy>, pub audit: Arc<dyn AuditSink>, pub principal: PrincipalInfo }
```

- **Fail closed.** `HostConfig::default()` for an isolate host is `DenyAll` plus `ModuleHooks::isolate()`
  (LD1). The primary host keeps today's behaviour through an explicit `AllowAll` policy for the `FirstParty`
  principal in svelters apps (`svelters-egui-run`, tests), which audits as such. A host created without choosing
  a policy cannot reach anything an extension could misuse.
- **Fast path.** Checks are on hot paths (every `call`, every `create` of a kind). The host caches `Allow`
  decisions per (principal, cap, mode, resource class) with the epoch; a changed epoch drops the cache. `Deny`
  is never cached (a grant may appear).
- **Panics in a policy** are treated as `Deny { reason: "policy.error" }` and audited.

### 3.4 Enforcement points, by rule

| Rule | Mechanism |
|---|---|
| **R-A** module visibility | the loader (LD1 `ComposedProvider`) asks `Policy` for `Cap::Module` with the importing principal before routing; a deny looks like an absent module (LD1's rule), and the audit event names the capability so the IDE can offer "Grant…" (js-api §3.7) |
| **R-B** calls re-check | the export dispatcher checks `Cap::Export`/`Cap::Ext` on every call against the current epoch, because the module record outlives a revocation (js-api §3.9) |
| **R-C** handles are per principal | an isolate names only handles it minted or was handed (slot targets, objects returned by granted calls). ISO1's `(isolate, handle) → NodeId` map is the check: an unknown pair is "stale handle", indistinguishable from a destroyed one. In-process, the first-party realm keeps today's global handle space, which is why no extension ever runs in it |
| **R-D** structural caps at creation | `create(kind)` checks `Cap::Kind`; attaching a behaviour checks `Cap::Behavior`; `mount` into a slot checks `Cap::Slot(class)`; presenting a `window` or tearing off checks `Cap::Window` |
| **R-E** deputies get the principal | T0 code acting for a principal (a provider widget instantiated by it, an export it calls, a behaviour attached by it) gets `cx.principal()` and must check data access with it; `NodeRef::principal()` returns the owning root's principal |
| **R-F** surfaces are bounded | an extension's nodes draw only inside its slots; overlay and popover roots from an extension are clipped to its slot's rect or the anchor it was given, and are not hit-test-opaque over other principals' roots (no clickjacking); the host labels extension surfaces (SEC-C) |
| **R-G** readers are filtered | snapshots, selector queries, F12 inspector data and MCP `ui.*` answers include a root only if the *reader's* principal may read the root's labels |
| **R-H** origins are recorded | the journal's `Origin::Js` becomes `Origin::Js(PrincipalId)`; command invocations from an isolate carry `source: Script { principal }` (commands §3.8) |

### 3.5 Declaration, approval, integrity

- **Manifest.** The extension package's manifest (cadatomic's format; outboard-style `needs`, scripting §6)
  declares requested capabilities in the §3.2 vocabulary plus cadatomic's. svelters reads none of it; cadatomic
  passes the declared set into its `Policy`, which enforces **declared ∩ granted**.
- **Approval.** Operator policy through U5 (locked keys from the hub or a self-hosted store); user prompts only
  where the policy allows them (SEC-C2). A high-security profile has no prompts: what the operator's policy does
  not grant, nobody can grant at run time.
- **Signing and identity.** cadatomic verifies the package signature and the operator's allow-list (SEC-C3), then
  hands svelters a **module lock**: `ModuleLock { principal, entries: [(ModuleId, ContentHash)] }`. svelters'
  loader computes the hash of every source it is about to evaluate for that principal and refuses a mismatch
  (`loader.integrity`), audited. This makes "the code that ran is the code that was approved" a loader property,
  not a packaging hope.
- **No code generation in extension isolates.** Extension runtimes are created without `eval` and the `Function`
  constructor (rquickjs `Context::custom` with the intrinsics minus eval [verify the exact intrinsic set in
  rquickjs 0.14]), so the locked modules are all the code that runs (CSP's "no unsafe-eval"; Chrome MV3's
  "no remotely hosted code"). Dynamic `import()` stays, through the same resolver and lock (SEC-O10).
- **Bytecode.** QuickJS's bytecode reader is not a validator; reading crafted bytecode is an A3 path. Extension
  runtimes load source only, or bytecode the host itself produced from locked source and stored in a cache it
  controls, keyed by content hash (SEC-O12).
- **Live edit.** The live IDE (js-api §6) may edit an extension only as a dev copy under a new principal
  (`PrincipalKind::Dev`, no content hash, no grants until granted, js-api §6.7). An edit to a locked module
  through Q1's source overlay is refused for a locked principal: the overlay is per principal, and a locked
  principal has none.

### 3.6 Placement

```rust
pub enum Placement {
    InProcess,                         // the first-party realm on the UI thread (T1)
    Thread,                            // ISO1: an isolate runtime on its own thread
    Process { sandbox: SandboxSpec },  // ISO1's protocol over a pipe/socket to `svelters-isolate` (SEC3)
}
```

The policy names the placement per principal (`Decision` for `Cap::Ext("placement", …)` or a dedicated
`Policy::placement(&PrincipalInfo) -> Placement`, SEC-O1). The isolate host is the same code in both cases; in
process placement it runs in a minimal `svelters-isolate` binary that links QuickJS, the runtime and the proxy
backend, and no app code: no exports, no providers, no documents. Exports are reached as calls over the
transport and gated on the host side (R-B).

### 3.7 Budgets

Budgets are policy values with defaults, not constants (SEC-O14): memory (QuickJS `set_memory_limit` per
runtime), per-entry CPU (ARCH-EMBED's 8 ms warn / 50 ms interrupt), live nodes per principal, ops per batch and
batches per second (SV21 queue bounds), live timers, `Table.fromColumns` bytes (MQ5), persist bytes, windows open.
Exceeding one is an audited event; the response is the existing fault path (ARCH-EMBED §6.2: interrupt, fault the
root, restart the isolate with backoff).

### 3.8 Audit

```rust
pub struct AuditEvent {
    pub seq: u64, pub time: SystemTime,
    pub principal: PrincipalId, pub principal_name: Arc<str>,
    pub cap: Cap, pub mode: Mode, pub resource: OwnedResource,
    pub outcome: AuditOutcome,      // Allowed | Denied(Reason) | Budget(BudgetKind) | Integrity | Lifecycle(Started | Stopped | Revoked | Faulted)
    pub trace: Option<TraceId>, pub epoch: u64,
}
pub trait AuditSink: Send + Sync + 'static {
    /// Must not block the UI thread. Returns false if the record could not be accepted.
    fn record(&self, ev: AuditEvent) -> bool;
}
```

- **What is recorded:** every deny; the first allow per (principal, cap, mode, resource, epoch); every allow with
  `AuditLevel::Every` (write-class uses, labelled reads, window opens, clipboard reads); every lifecycle change
  of a principal; every integrity failure and budget hit. Volume is the policy's choice through `AuditLevel`
  (SEC-O6).
- **Audit-or-deny.** A grant may say audit is required; if `record` returns false the use is denied (Linux audit's
  failure mode, NIST SP 800-171 3.3.4 "alert in the event of an audit logging process failure"). Default for
  required caps: deny (SEC-O5).
- **Document edits are audited by the command path**, not here: an isolate's command invocation is one
  `Cap::Ext("doc","edit")` check (here) and one `Invocation` record with `source: Script { principal }` (cadatomic).
  Both carry the same `trace_id`, so one query joins them.
- **Storage, tamper-evidence, retention and export** (hash chain, SIEM, syslog) are cadatomic's sink (SEC-C6).
  svelters ships `MemorySink` (tests) and `JsonLinesSink` (svelters apps).

### 3.9 Revocation

| Revoked | Effect |
|---|---|
| a data or API capability (`Export`, `Ext`, `Clipboard`, `Persist`) | the next call fails with `policy-denied` (R-B); open subscriptions and observers of that principal on that capability are closed; mounted UI stays |
| a structural capability (`Module`, `Kind`, `Behavior`, `Slot`, `Window`) | the principal's isolate is stopped, its roots show the fault UI ("disabled by policy"), and it restarts only if the new policy still lets it start; modules cannot be unloaded from a runtime, so a restart is the only clean revoke |
| the principal (kill switch) | isolate stopped (thread: interrupt flag and drop; process: kill), roots unmounted, handles released, `Lifecycle(Revoked)` audited |

The embedder signals a change by bumping `Policy::epoch()`; the host checks it at the start of each `logic` and
before each call into an isolate's API.

### 3.10 Defaults

| Host | Policy | Modules | Reaches |
|---|---|---|---|
| primary host of a svelters app | `AllowAll` for its `FirstParty` principal | `ModuleHooks::default()` | as today |
| primary host in cadatomic | cadatomic's `Policy`, first-party principal with a built-in broad grant (SEC-O4) | cadatomic `Allow` list (LD1) | its grant |
| any isolate host (thread or process) | `DenyAll` unless the embedder passes a policy | `ModuleHooks::isolate()` = `Allow([])` | `svelters:core` only, and no slot to render into: an unconfigured extension draws nothing and calls nothing |
| dev extension | the embedder's policy for `PrincipalKind::Dev` (cadatomic: none granted) | `isolate()` | nothing until granted |

### 3.11 Comparison with existing systems

| System | Model | What we take | What we do not |
|---|---|---|---|
| **VS Code** workspace trust + extension host | extensions run in a separate Node process with full Node access; "Restricted Mode" disables workspace-defined tasks, debugging and some settings in untrusted folders; Marketplace signature checks | separate extension host process; trust per workspace for *project* content (our SEC-C5 overlays) | its extension host is not a sandbox: any extension can read files and use the network. That is the gap this design closes |
| **Deno** permissions | deny by default; `--allow-read=<paths>`, `--allow-net=<hosts>`, `--deny-*` overrides; runtime prompts; checks at the op layer; `Deno.permissions.query/request/revoke` | enforcement at the op layer; path and host scoping; deny overrides allow; revocation | per-process, not per-module: one program, one permission set. We need per-principal, which is why one principal per runtime |
| **WebAssembly component model / WASI 0.2** | no ambient authority; a component can call only the imports its world declares; resources are handles; filesystem access through preopened directories | the linker is the policy point: our resolver is the linker (R-A); handles as capabilities (R-C) | WASM's memory safety boundary. QuickJS does not have one, hence §1.4 |
| **Chrome extensions** (MV3) | manifest `permissions` and `host_permissions`; optional permissions requested at run time; install-time warnings; an update that adds a permission disables the extension until accepted; enterprise `ExtensionSettings` (allow/block lists, `runtime_blocked_hosts`); no remotely hosted code | declared ∩ granted; update re-approval; enterprise allow/block lists as the operator layer (U5); no eval in extension contexts | content scripts in web pages (we have no equivalent) |
| **Fuchsia** component framework | capabilities are routed explicitly in manifests (`use`, `offer`, `expose`); nothing is ambient; routing errors are diagnosable | slots and API objects are *offered* to a principal, not discovered (R-C, `Cap::Slot`) | a microkernel; we are one process plus isolates |
| **seL4 / object capabilities** (and E, Cap'n Proto) | authority = possession of an unforgeable reference; no global names; attenuation by wrapping | per-principal handle spaces; deputies receive the caller's principal (R-E) to avoid confused-deputy bugs | formal verification; ambient caps (modules, windows) still need grants in our model |
| **macOS entitlements** + hardened runtime | capabilities are signed claims in the code signature, granted by provisioning, enforced by the OS; changing code invalidates the signature | grants tied to a content hash (module lock); any edit drops the identity (§3.5 live edit) | kernel enforcement, except in process placement where the OS sandbox does it |

### 3.12 Aerospace and defence context (what this supports, not what it certifies)

- **DO-178C / DO-330.** DO-178C covers software in airborne systems; cadatomic is a design tool, not airborne
  software. Where a tool's output is not independently verified, DO-178C's tool qualification (DO-330) can apply
  to the tool. Extensions that change outputs would then be part of the qualified configuration. What helps: the
  module lock and audit stream identify exactly which extensions (by hash) were active when an output was made,
  and the policy can pin that set. Whether cadatomic is ever qualified is not this document's claim.
- **DO-326A / ED-202A** (airworthiness security process) concern aircraft systems; their relevance here is
  indirect, as part of an operator's development-environment and supply-chain controls.
- **ITAR / EAR.** Technical data must not reach unauthorised persons or destinations; for an IDE the realistic
  export paths are network egress, the clipboard, files written outside controlled storage, agents and cloud
  services. The mechanism supports this with deny-by-default network and file capabilities (cadatomic's `net`,
  `fs` vocabulary, never ambient in an isolate), data-class labels on documents and roots with filtered readers
  (R-G), and an audit record of who read labelled data. Classification, marking rules and who counts as
  authorised are the operator's.
- **CMMC 2.0 / NIST SP 800-171** (CUI). Controls this mechanism supports (an organisation is assessed, not a
  library): 3.1.1/3.1.2 limit access to authorised users, processes and functions; 3.1.5 least privilege; 3.3.1/
  3.3.2 create audit records and trace actions to a principal; 3.3.4 alert on audit failure (audit-or-deny);
  3.3.8 protect audit information (cadatomic's sink); 3.4.8 deny-all, permit-by-exception for software execution
  (the module lock and the extension allow-list); 3.4.9 control user-installed software (locked U5 keys).

## 4. The split: svelters mechanism, cadatomic policy

| Concern | svelters provides | cadatomic owns |
|---|---|---|
| Identity | `PrincipalId`, `PrincipalInfo`, one principal per runtime, principal on roots, handles, journal origins and audit events | extension ids, versions, publisher identity, the mapping from package to principal |
| Decision | `Policy` trait, `Request`, `Decision`, `Cap` (svelters vocabulary + `Ext`), epoch-based cache, `DenyAll`, `AllowAll`, `StaticPolicy` | the `Policy` implementation over U5: layers, locks, provenance, prompts, profiles ("high-security") |
| Enforcement | checks at every point in §2; fail-closed defaults (§3.10); per-principal handle spaces; surface bounds (R-F); filtered readers (R-G) | checks in its own APIs (`cadatomic:api/*`, commands, jobs, `fs`, `net`), using the same `Policy` and `AuditSink` |
| Integrity | `ModuleLock` hash checks in the loader; eval-free extension runtimes; bytecode rules; refusal of native packs under a policy | package format, signatures, allow-lists, distribution (hub, self-hosted, air-gapped) |
| Isolation | thread isolates (ISO1), the `svelters-isolate` process host and its transport (SEC3), budgets as policy values | the OS sandbox profile and launcher (shared with NX10/OB6), supervision policy, which extensions go out of process |
| Audit | `AuditEvent`, `AuditSink`, audit-or-deny, `MemorySink`, `JsonLinesSink` | the production sink: storage, hash chain, retention, SIEM export, review UI |
| Revocation | `Policy::epoch`, call-time re-checks, isolate stop and restart, roots' "disabled by policy" fault state | when and why to revoke; the UI that does it |
| UI | the fault state text and a `RootInfo::principal` the host can label | permission prompts, extension manager, "Grant…" from diagnostics, extension-surface labelling |

## 5. What running and planned lanes must do now

Concrete API shape only; none of these needs the policy to exist yet. Each is a field or parameter that is cheap
now and a breaking change later.

| Lane | Do now | Why |
|---|---|---|
| **LD1** (`svelters-modules` `compose.rs`, `svelters-host` `hooks.rs`; tip `b14023c6` already has `ExtensionPolicy`, `deny_all()`) | (1) Give `ModuleHooks` a `principal: PrincipalId` (or `Arc<PrincipalInfo>`) and pass it to `Visibility::Filter` and to `SchemeHook::{resolve, load}` alongside the importer: an importer path is not an identity. (2) Keep `ExtensionPolicy` `#[non_exhaustive]` (it is) and let SEC1 add a `gate: Option<Arc<dyn Policy>>` field that, when set, is asked before `visibility`, so call sites do not change. (3) `ComposedProvider::load` re-checks visibility for app exports too, not only added natives (a module id reached by another route must not load). (4) Return a typed denial internally even though the JS message stays "no such module", so the audit event can name the capability. (5) Report every module evaluated (id, principal, content hash) through one callback: SEC1's audit and lock checks attach there | SV22 asks for "each realm or isolate resolves with its own principal" (requests); module visibility is R-A |
| **E0 op path** (`host/core/ops.rs` `op_kind_id`, `apps.rs` `app_kind`, `app_invoke`) | Apply the same module visibility in `app_kind` (and so `construct`/`callStatic` on `app:` kinds) as in the resolver, now, with LD1's `ExtensionPolicy`: today an isolate host with `deny_all()` still reaches every export through `construct('app:mod.T')` | §2.1 bypass; R-B |
| **Q1** (`svelters-modules` providers, source overlay, watcher) | (1) Confine `SourceProvider` resolution to a **package root** per principal: no `..` escape, no absolute paths, symlinks resolved and re-checked. Otherwise an extension can `import data from '/home/u/secret.json'` and read any file through the JSON and asset transforms. (2) Key the source overlay by principal; a locked principal has none. (3) Expose the content hash of each loaded source (for the lock). (4) The watcher watches only package roots | the loader is the most direct file-read path an extension has |
| **Q2/Q3** (bytecode cache; CSS scoping in Rust) | Bytecode cache: key by content hash and principal, store only host-produced bytecode in a host-owned directory, never read bytecode from a package for an extension principal. CSS: no network or file fetch from stylesheets (`url()`, `@import`, fonts) except through the loader and policy; `:global(…)` rules from an extension's components apply only within that principal's roots, so an extension cannot restyle first-party chrome (hide a warning, fake a dialog) | A3 via bytecode; R-F via styles |
| **H1** (component HMR on the Rust loader) | A swap re-enters the loader, so it re-checks the lock and visibility; a versioned re-import keeps the principal of the module it replaces; HMR is off for locked principals (their code is immutable) and on for `Dev`; HMR delivery must not use `eval` in extension runtimes (versioned `import()` is fine) | §3.5 live edit; SEC-O10 |
| **CX1** (compiler as a library, diagnostics only) | Treat source as hostile: recursion depth limits in parser and analysis (a Rust stack overflow aborts the process), a size cap, a time budget, `catch_unwind` at the entry; add the entry point to the fuzz list. Diagnostics carry spans, not file contents of other files | A5; the in-app editor compiles whatever is typed or opened |
| **SG4** (provider passes, `WidgetCx::scene`, `StateProvider`, `ScenePass`, overlay roots) | (1) `NodeCore` (or the root table) records the root's `PrincipalId`; `NodeRef::principal()` and `WidgetCx::principal()` (R-E). (2) `Origin::Js` becomes `Origin::Js(PrincipalId)` before the journal is stable (ARCH-SCENE §16 lists `Origin` as stable). (3) Overlay roots carry an owner principal; an extension's overlay is clipped to its anchor/slot and is hit-transparent over other principals' roots unless the host opts in (R-F). (4) `SceneSnapshot` and selector queries take a reader principal and filter roots by labels (R-G) | ARCH-SCENE's stability contract would otherwise freeze principal-less types |
| **EM1/EM2** (embed core, viewports) | `rootFault`/`rootInfo`/`rootEmit` take the root from the calling context (the mount's root context the runtime already sets), not from a JS argument, or the host checks that the id belongs to the caller's principal (§2.1); `MountSpec` gains `principal` (today `owner: RootOwner`, `InProcess | Isolate`); `RootInfo` reports it; `SlotOptions` gains a `class: SlotClass` (PaneBody, Section, Dialog, Window, Overlay, Popover, docking §6.1) for `Cap::Slot`; presenting a `window` element from a root checks `Cap::Window` | R-D, D11 |
| **ISO1** (planned: proxy backend, isolate host) | (1) The transport trait carries bytes or `svelters_abi::Value` only, never `Arc`/pointers; `Shared<[T]>` crosses as a copy or a named shared-memory segment with a copy fallback, so `Process` placement is a transport, not a redesign. (2) Handles minted per isolate, mapped on the UI side (R-C), unknown pairs indistinguishable from stale. (3) Every op batch is validated on the UI side (kinds, slots, prop types, sizes) before it touches the scene: a process isolate is A5. (4) No `call_sync` to isolates (framework §5.1 already) | §1.4 |
| **E0/E3/RA3** (exports, stores, services, dynamic exports SV23) | Every export member carries a mode in its metadata (`query` read, `command` write, `observe`), and optionally `cap = "…"`; the dispatcher is the one place calls are checked (R-B). `persist(key, …)` keys are namespaced by principal | R-B; cross-principal state collisions |

### 5.1 Milestones

| Id | Content | Size (lane-days) | Depends | Acceptance |
|---|---|---|---|---|
| **SEC1** | `svelters-host::policy`: `PrincipalId/Info`, `Cap`, `Request`, `Decision`, `Policy` (+ `DenyAll`, `AllowAll`, `StaticPolicy`), epoch cache, `AuditEvent`/`AuditSink` (+ `MemorySink`, `JsonLinesSink`); wired into LD1's module gate, the export dispatcher (R-B), `persist` namespacing, native-module refusal | 4–5 | LD1, Q1 merged | an isolate host with no policy imports nothing but `svelters:core` and calls nothing; a revoked export fails the next call; every deny produces one audit event; audit-or-deny denies when the sink refuses |
| **SEC2** | Principal on roots, nodes, journal origins, snapshots; `Cap::Kind/Behavior/Slot/Window/Clipboard` checks; overlay bounds (R-F); filtered readers (R-G); `ModuleLock` hash checks; eval-free extension runtimes | 4–5 | SEC1, SG4, EM2 | kittest: an extension root cannot create an ungranted kind, open a window, or overlay a first-party root's hit area; a snapshot for a reader without the label omits the root; a modified locked module is refused |
| **SEC3** | `Process` placement: `svelters-isolate` binary, a pipe/socket transport under ISO1's trait, UI-side batch validation hardening, crash and kill handling; sandbox hooks the embedder fills (no sandbox code in svelters, SEC-O15) | 3–5 | ISO1 | the same extension test suite passes in `Thread` and `Process` placement; killing the isolate process faults only its roots; a malformed batch is rejected and audited |
| **SEC4** | Fuzzing and red-team: op-batch fuzzer against the UI-side applier, compiler entry fuzz (CX1), module resolver escape tests (`..`, symlinks, schemes, `data:`), bytecode-cache tamper test | 1–2 | SEC1–SEC3 | fuzzers run in CI nightly; no escape found by the resolver suite |

None of these is on SC1's path. SEC1 can start as soon as LD1 and Q1 are merged.

## 6. Open questions

### 6.1 SEC-O: svelters mechanism (for the user, through svelters-agent)

| # | Question | Options | Recommendation |
|---|---|---|---|
| **SEC-O1** | Isolation for untrusted extensions in the high-security tier | (a) thread isolates only; (b) thread by default, sandboxed process placement chosen per extension by policy, one protocol and one gate; (c) every third-party extension out of process | **(b)**: (a) does not contain an engine exploit (§1.4); (c) costs latency and memory for vetted extensions that don't need it. The gate on the trusted side makes (b) cheap |
| **SEC-O2** | Shape of the hook | (a) one `Policy::check(Request) -> Decision` trait implemented by the embedder, svelters ships `DenyAll`/`AllowAll`/`StaticPolicy`; (b) a svelters-owned declarative policy engine; (c) one hook per subsystem | **(a)**: U5 is cadatomic's policy layer (decisions); one trait gives one audit stream; (b) would be a second policy layer |
| **SEC-O3** | Capability vocabulary | (a) typed `Cap` enum (`#[non_exhaustive]`) for svelters' surface plus `Cap::Ext(ns, name)` for the embedder; (b) strings everywhere | **(a)**: typed at the enforcement points, string ids in audit records |
| **SEC-O4** | Is the first-party realm checked? | (a) implicit all-access (js-api §3.7 today); (b) it is a principal with a broad built-in grant, checked and audited by the same path | **(b)**: operators can restrict first-party features too (no network APIs at all), audit is uniform, and the cost is a cached allow |
| **SEC-O5** | When the audit sink cannot accept a record | (a) best effort (drop and count); (b) deny the use; (c) per grant: audit-required caps deny, others best effort | **(c)**, with high-security profiles marking every write-class and labelled-read cap audit-required |
| **SEC-O6** | Audit volume for allows | (a) every check; (b) denies + first use per (principal, cap, mode, resource, epoch) + per-use for `AuditLevel::Every`; (c) denies only | **(b)**: (a) floods on per-frame paths; (c) fails "trace actions to a principal" |
| **SEC-O7** | Several principals in one QuickJS runtime (separate contexts) | (a) never: one principal per runtime; (b) allow contexts as a cheap boundary | **(a)**: objects passed between contexts keep identity and prototypes; the boundary would be by convention only |
| **SEC-O8** | Revoking a structural capability while roots are mounted | (a) stop and restart the principal's isolate; (b) leave mounted UI, deny new uses only | **(a)** for `Module/Kind/Behavior/Slot/Window` (module records cannot be unloaded); **(b)** for data and API caps (call-time deny) |
| **SEC-O9** | Who checks module integrity | (a) svelters' loader checks content hashes from a host-supplied `ModuleLock`; (b) cadatomic's scheme hook does it all | **(a)**: generic, covers relative imports and HMR re-imports that never pass through a scheme hook |
| **SEC-O10** | `eval` and `Function` in extension runtimes | (a) removed (intrinsics without eval); (b) allowed | **(a)**: otherwise the lock does not bound the code that runs. First-party realm keeps eval in dev only (Vite-style HMR delivery, HMR.md §3.2) |
| **SEC-O11** | Native code loaded in process (`DylibModules`, future dlopen'd `svl_pack`) when a policy is active | (a) refused at run time unless the policy is `AllowAll`, in addition to the cargo feature; (b) cargo feature only | **(a)**: defence in depth; cadatomic already decided "third-party native only out of process" |
| **SEC-O12** | QuickJS bytecode for extension principals | (a) only host-produced bytecode from locked source, cached by content hash in a host-owned directory; (b) also signed bytecode shipped in packages; (c) source only | **(a)**; (b) trusts a reader that was not built for hostile input |
| **SEC-O13** | Data-class labels inside svelters | (a) opaque `LabelSet` on principals and roots; readers (snapshots, selectors, MCP, inspector) filtered through the policy; (b) none in svelters, cadatomic filters above | **(a)**: the scene snapshot is the only place that sees every root at once; filtering above it means re-implementing the walk |
| **SEC-O14** | Resource budgets | (a) policy values with defaults (memory, CPU per entry, nodes, ops/s, timers, table bytes, windows); (b) fixed host config | **(a)**: MQ5 already asks for a budget under U5 |
| **SEC-O15** | Who owns OS sandboxing for process isolates | (a) svelters ships `svelters-isolate` designed to need nothing but its transport fd, and an embedder hook to wrap the launch; cadatomic (with NX10/OB6) applies the platform sandbox; (b) svelters ships per-OS sandbox code | **(a)**: one sandbox launcher for workers and isolates in cadatomic; svelters stays free of platform security code |
| **SEC-O16** | Clipboard and primary selection for extensions | (a) write and read are separate grants; read is delivered only to a focused widget of the principal on a user paste gesture; (b) one `clipboard` grant | **(a)**: reading the clipboard in the background is the classic exfiltration-in path of copied controlled data |

### 6.2 SEC-C: cadatomic policy (for main-cadatomic)

| # | Question | Options | Recommendation |
|---|---|---|---|
| **SEC-C1** | Grant format in U5 | (a) `extensions.<id>.grants` as a key tree with patterns, effective = declared ∩ granted, explicit deny wins, locked layers final (js-api §3.7); (b) a separate policy file | **(a)**: U5 already has layers, locks and provenance |
| **SEC-C2** | User prompts | (a) a policy key `extensions.prompts = never | install | first-use`; high-security profile `never`; (b) always prompt | **(a)** |
| **SEC-C3** | Signing and identity | (a) publisher signature (ed25519) over a content-addressed package plus an operator allow-list by content hash, verifiable offline; (b) X.509 code signing; (c) a transparency log (Sigstore) | **(a)**, with (c) as an option where networks allow; air-gapped sites must verify offline |
| **SEC-C4** | Data classes | (a) document-level labels first (e.g. public, proprietary, CUI, export-controlled), entity-level later; principals cleared per label; (b) entity-level from the start | **(a)**; svelters only carries the labels (SEC-O13) |
| **SEC-C5** | First-party overlays in high-security profiles | (a) user and project overlays off by a locked key; project overlays never in the first-party realm unless signed into the lock; (b) trust prompt per project and hash (js-api J7) | **(a)** for high-security, (b) elsewhere: an overlay runs with first-party grants |
| **SEC-C6** | Audit storage | (a) append-only JSON lines with a hash chain, local, exported to syslog/SIEM, retention by policy; (b) hub-only | **(a)**: works air-gapped; the hub can collect copies |
| **SEC-C7** | Agents and MCP | (a) agents are principals with grants and labels; `ui.*`/`screenshot.*` answers filtered by labels (R-G); (b) agents see everything the user sees | **(a)**; screenshots of labelled roots are denied or redacted |
| **SEC-C8** | Network egress for extensions | (a) none in the high-security profile; elsewhere a brokered `fetch` in the host checking `net{hosts}`; (b) direct sockets in workers only | **(a)**: isolates never get sockets; process isolates are sandboxed without network |
| **SEC-C9** | Extension surfaces | (a) every extension root is drawn with a host-owned label or frame naming the extension (not drawable by JS), extension dialogs cannot be modal over first-party dialogs; (b) no marking | **(a)** against spoofing (R-F) |
| **SEC-C10** | Policy distribution | (a) signed policy bundles from the hub or a self-hosted server, with an offline import path; (b) hub only | **(a)** (U5 already names self-hosted) |
| **SEC-C11** | Third-party compute in high-security profiles | (a) WASM components only (memory-safe, host-function grants); (b) WASM or sandboxed workers; (c) also unsandboxed workers | **(b)**, with NX10's launcher mandatory |

## Sources

- svelters: `CLAUDE.md` (user decisions 2026-10-21/22); `plans/research/{ARCH-EMBED,ARCH-SCENE,TRACKS,HMR,
  CUSTOM-WIDGETS,CADATOMIC,QUICKJS-RUNTIME}.md`; `origin/ci/ld1:plans/milestones/LD1-loader-hooks.md`;
  `origin/ci/ld1` (`b14023c6`: `modules/compose.rs`, `host/hooks.rs`), `origin/ci/q1` (`modules/source/mod.rs`, QuickJS adapter), `origin/ci/h1` (`hot.js`, `quickjs.rs` HMR paths); `crates/svelters-host/src/{lib,core,quickjs,convert,engine,
  reload}.rs`, `crates/svelters-host/src/exports/`, `crates/svelters-egui/src/{backend,windows,events,ops}.rs`,
  `crates/svelters-egui/src/embed/`, `crates/svelters-egui/src/scene/` (see §2 for the exact symbols).
- cadatomic `4a5decc` (read-only): `docs/plan/00-decisions.md` (9, 10/16, U3, U5, D4, D11, "Native extensions",
  "Extensions: a deny-by-default…"); `docs/plan/04-kernel-requests.md` (SV22, SV25, OB3–OB6, SY3);
  `docs/research/2026-10-22-{js-api-publishing-and-live-ide,component-framework,docking-windows-and-svelters-roots,
  native-extensions-abi-and-engines,ide-shell-and-ux,models-filtering-and-dynamic-data}.md`;
  `docs/research/2026-10-16-command-architecture.md`; `docs/best-practices/scripting-and-extensions.md` §5–§6;
  `crates/cadatomic-cli/src/{server,paths}.rs` (RPC socket and token).
- External (from memory; verify before quoting outside this repo): Deno permissions documentation; WASI 0.2 and
  the component model; Chrome extension manifest and enterprise `ExtensionSettings` policy; Fuchsia component
  capability routing; VS Code workspace trust; Apple entitlements and hardened runtime; NIST SP 800-171 r2
  (3.1.x, 3.3.x, 3.4.8, 3.4.9); RTCA DO-178C, DO-330, DO-326A; 22 CFR 120–130 (ITAR).
