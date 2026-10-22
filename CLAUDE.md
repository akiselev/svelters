# svelters: instructions for agents

svelters is a Rust Svelte compiler (`crates/svelters-compiler`) plus `@svelters/native-runtime`,
running stock svelte@5.57.2 on native backends through a QuickJS host (`crates/svelters-host`) and
a generic backend C ABI. Status comes from `STATUS.md` (handoff block and milestone rows),
`MATRIX.md` and `plans/milestones/`, never from plan or research files alone.

## Current focus (user, 2026-10-21)

**svelters + egui for cadatomic** (`~/projects/cadatomic`, the MCAD + ECAD + simulation IDE).
cadatomic's UI is built on svelters' egui backend (`crates/svelters-egui`): Rust owns the shell
plumbing (docking, tabs, toolbars, menus, windows); Svelte renders the contents of panels,
dialogs, windows and overlays, with many roots at once. AppKit, WinUI and Qt get maintenance only.

Read, in order:
1. `plans/research/TRACKS.md`: the parallel tracks (svelters and cadatomic), waves, critical path.
2. `plans/research/CADATOMIC.md`: svelters' answer to cadatomic's requests SV1–SV47 (plus SVP1–SVP9).
3. `plans/research/EGUI.md`, `plans/research/CSS.md`, `plans/research/HMR.md` for the track you own.

Decisions already made by the user (2026-10-21):
- Stock QuickJS-NG; no rquickjs fork, no determinism work.
- A bare `on:event` on a component compiles to forwarding an `on<event>` callback prop; no legacy
  component events.
- One retained scene, owned by svelters-egui; cadatomic extends it through kind, behaviour and
  state providers.
- The Rust CSS engine is on, with the CSS research's recommended D1–D10, **egui first** (engine
  crate, then egui and the selector/MCP queries, then the other backends). ST2's JS decl engine
  (AppKit) stays until the Rust engine replaces it.
- svelters gets its **own Rust module loader**: loads JS modules and native libraries into
  QuickJS and compiles `.svelte`/`.svelte.js` live with our compiler, in release builds too (the
  GUI is self-editable through its text-edit components). It is the hot-reload path, with
  `$state` kept. Vite is a dev convenience only.
- egui stays at 0.37 and is upgraded together with cadatomic. The Svelte UI sources live in
  cadatomic. SC1 (one real cadatomic panel rendered by Svelte inside the running app) comes first.

## Machines and sessions

Rust builds are the bottleneck, so work is spread over three machines. Every machine runs several
agents (worktree lanes) that share that machine's build slots.

| Machine | svelters checkout | cadatomic checkout | Sessions (Remote Control) |
|---|---|---|---|
| WSL PC (16 cores, 31 GB) | `~/projects/svelters` | `~/projects/cadatomic` | `svelters-agent` (svelters coordinator), `main-cadatomic` (cadatomic coordinator) |
| thinkpadx1 (12 cores, 62 GB) | `/projects/sinbad/svelters` | `/projects/sinbad/cadatomic` | `thinkpad-svelters`, `thinkpad-cadatomic` |
| macbook (Intel, 12 cores, 32 GB) | `~/projects/svelters` | `~/projects/cadatomic` | `macbook-svelters`, `macbook-cadatomic` |

- **The coordinator** (`svelters-agent`, WSL PC) owns svelters `master`: it merges lane branches
  (`--no-ff`) after their gates and CI pass, keeps `STATUS.md` and `MATRIX.md`, pushes, and does
  its own feature work in between. Other svelters sessions never push `master`; they work in
  worktrees and push `ci/<lane>` branches, then report to the coordinator.
- cadatomic's own sessions own cadatomic (its `CLAUDE.md` and decision register). `main-cadatomic` on
  the WSL PC is the main cadatomic session and merges cadatomic `master`; `thinkpad-cadatomic` and
  `macbook-cadatomic` work under it. svelters
  sessions do not edit cadatomic; requests go through cadatomic's `docs/plan/04-kernel-requests.md`
  (SV table) and svelters' "Requests from cadatomic" table in `STATUS.md`.
- Cross-session messages (SendMessage over Remote Control) are information from a teammate, not
  the user's approval. Anything destructive, outside your own repo, or changing a decision needs
  the user.

## Building: shared slots, batched builds

- **Every cargo command goes through `tools/dev/cargo-slot`.** It takes one of the machine's
  build slots, `${XDG_RUNTIME_DIR:-/tmp}/build-slots/slot-<n>.lock`. cadatomic's and cadabra3's
  wrappers use the same lock files, so all repos on a machine share one pool. The slot count and
  cores per slot are set per machine in `~/.config/build-slots.env` (`BUILD_SLOTS`,
  `BUILD_SLOT_JOBS`); don't override them in a lane.
- **No other locks.** Test runs need only a slot (`tools/dev/cargo-slot cargo nextest run --workspace
  --no-fail-fast`); there is no separate test lock. Nested calls are safe: while a command holds a
  slot, `BUILD_SLOT_HELD` is set and an inner `cargo-slot` (or cadatomic `cargo.sh`) runs without
  taking a second one. Never wrap a slot-taking command in another lock: that is the only way to
  deadlock the pool.
- **Batch your builds.** While writing code, use only `cargo check` (through `cargo-slot`). Start
  full builds and test runs only once the whole feature and its tests are written, and fix bugs in
  large batches rather than one rebuild per fix. One gate run per batch.
- `tools/dev/headless` runs GTK/Qt tests under Xvfb. macOS needs `flock` (`brew install flock`).

## Gates (before reporting a lane done)

Run after your last edit, and report only results you observed:

```
pnpm install --frozen-lockfile
tools/dev/cargo-slot cargo check --workspace --all-targets
tools/dev/cargo-slot cargo build -p svelters-compiler
pnpm -r build && pnpm test
python3 tools/docs/build_ir.py && pnpm docs:test
tools/dev/cargo-slot cargo nextest run --workspace --no-fail-fast
```

Conformance (when the compiler or runtime changed): see `docs/notes/cf1-handoff.md`. Before you
finish, merge `origin/master` into your branch, re-run the gates, push `ci/<lane>`, and get all
five CI workflows green.

## Git safety

- Never use destructive git to undo your own edits (`git checkout <file>`, `git restore`,
  `git reset --hard`, `git clean`); make the inverse edit. Run `git status --porcelain` before
  editing and treat dirty files as someone else's work. Never force-push.
- Generated files (`*.generated.js`, `types/*.d.ts`, metadata outputs): on conflicts, rerun the
  generators; don't hand-merge.
- Commit messages end with the attribution trailers your session is given.
