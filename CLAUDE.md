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

Decided by the user (2026-10-22): **leave room for extension security policies, build none now.**
cadatomic may later serve high-security fields (aerospace) where extensions get deny-by-default,
audited grants, but "right now we're developing": no enforcement, policy, audit or signing work,
and security must not slow development. APIs only leave room: `#[non_exhaustive]` types and a
principal id where ARCH-SECURITY §5 shows it is cheap now and breaking later. ARCH-SECURITY.md
(SEC1–SEC4, SEC-O, SEC-C) is deferred until cadatomic loads its first third-party extension.

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

## Who decides (user, 2026-10-22)

**The coordinators decide; the user is not asked.** The user has delegated decisions to the two
coordinators and does not want sessions asking them questions.

- **Worker sessions and lane agents never ask the user.** They send the question to their
  coordinator (`svelters-agent` for svelters, `main-cadatomic` for cadatomic) with a recommendation,
  keep working on something else meanwhile, and follow the answer.
- **The coordinators decide**, citing the reason in the answer:
  - each coordinator decides routine and procedural questions in its own repo: lane assignment
    and order, scheduling, merges and conflict resolution, pins and pin advances, build and test
    rules, rules files (`CLAUDE.md`), implementation details inside an agreed design, and recording
    in the decision register the user's decisions that the other coordinator relays;
  - the two coordinators settle cross-repo seams between themselves. If they disagree, svelters-agent
    decides for svelters-side APIs, main-cadatomic for cadatomic-side ones;
  - a coordinator's message to a session in its repo carries this delegated authority. Act on it.
- **Escalate to the user only** (the coordinator does it, nobody else):
  - architecture and design choices: the open questions in `plans/research/ARCH-*.md` and similar
    design docs, brought together with a recommendation each;
  - reversing or contradicting a decision the user made (the list above, or a register entry
    marked as the user's);
  - destructive actions on work that isn't yours: force-push, deleting branches or repos with
    unmerged work, discarding uncommitted changes;
  - anything public or paid: publishing crates, making repos public, new paid services.
- Record each decision where the next session will find it: svelters in `STATUS.md` or the
  relevant `plans/research/*.md` ("Decided (coordinator, date)"), cadatomic in its register.

## Building and testing: as little as possible locally

Rust builds are the bottleneck. Every local build or test run takes a shared slot and slows every
other agent on the machine. **GitHub CI is the gate, not your machine.** The user's rule
(2026-10-22): no build or test run that doesn't answer a question you have right now.

- **Every cargo command goes through `tools/dev/cargo-slot`.** It takes one of the machine's
  build slots, `${XDG_RUNTIME_DIR:-/tmp}/build-slots/slot-<n>.lock`, shared with cadatomic's and
  cadabra3's wrappers. Slot count and cores per slot come from `~/.config/build-slots.env`; don't
  override them. There are **no other locks** (no test lock): never wrap a slot-taking command in
  another lock. Nested calls are safe (`BUILD_SLOT_HELD`). Builds and tests take a slot;
  `cargo check`/`clippy` take **none** (they run niced with 3 jobs), so never wait for a slot to
  type-check. The coordinator checks a merge locally only when it combines lanes that touched the
  same crate (`cargo check -p <that crate> --all-targets`, once); docs and disjoint merges get
  nothing.
- **Disk (WSL PC):** `df /` lies (the WSL disk is a growing file on C:); watch `df -h /mnt/c`.
  When a lane is merged, delete its worktree's `target/`.
- **While writing code:** `tools/dev/cargo-slot cargo check -p <crate you changed>` only, and only
  when you need the compiler's answer. Use `--workspace --all-targets` only after changing an API
  that other crates use.
- **When the feature and its tests are written:** run only the tests you added or touched,
  filtered (`tools/dev/cargo-slot cargo nextest run -p <crate> <filter>`, `pnpm --filter <pkg>
  test`; GTK/Qt under `tools/dev/headless`). Fix all failures in one batch, then rerun those
  tests once.
- **Never run locally:** the full workspace nextest, the full gate list, or a full conformance run
  (see below). Push `ci/<lane>`; the five GitHub workflows run everything. Fix what CI reports, in
  one batch, and push again.
- **Never rerun anything on unchanged code.** Run nothing for docs-only changes, milestone records,
  or a merge of `origin/master` that conflicts nowhere in code. A result from before your last code
  edit is not a pass, so say "not run" instead.
- **Conformance** runs in CI (the `conformance` job of `js.yml`, `--check` against `baseline.json`;
  its `results.json`/`REPORT.md` are uploaded as an artifact). Locally, run only the affected
  samples (`node run.mjs <name-filter>`, see `docs/notes/cf1-handoff.md`). When a lane raises the
  score, it commits `--update-baseline` output from one full run at the end.
- **Research and docs lanes build nothing.**
- **Stop when told.** When the coordinator or user says stop, kill your running builds and tests,
  commit, push and report.
- `tools/dev/headless` runs GTK/Qt tests under Xvfb. macOS needs `flock` (`brew install flock`).
- Start Claude sessions inside a repo, never in `~/projects` itself: rust-analyzer there loads every
  project (23 GB, ~50 parallel `cargo check`s).

## Before reporting a lane done

1. Merge `origin/master` into your branch; on generated-file conflicts rerun only that generator.
2. `pnpm install --frozen-lockfile` if the lockfile changed, then `tools/dev/cargo-slot cargo check
   -p <crates you changed> --all-targets`, plus the filtered tests above.
3. Push `ci/<lane>` and get all five CI workflows green. Wait for CI with one
   `gh run watch <id> --interval 300` per run (or a `gh run list` loop sleeping 300 s); never poll faster. Every
   session on all three machines shares one GitHub API limit (5,000 requests an hour), and faster polling has used
   it up. Report the sha, the CI run ids, and what you
   did not run.

## Git safety

- **Never commit build artifacts.** Any `target/` directory (at any depth) is ignored and rejected:
  `.githooks/pre-commit` runs `tools/dev/check-no-artifacts --staged` (enable once per checkout with
  `git config core.hooksPath .githooks`), and CI's js job runs it on every push. Run cargo from the checkout root.
  Before every push, check `git diff --stat origin/master...HEAD` lists only your lane's files.
- Never use destructive git to undo your own edits (`git checkout <file>`, `git restore`,
  `git reset --hard`, `git clean`); make the inverse edit. Run `git status --porcelain` before
  editing and treat dirty files as someone else's work. Never force-push.
- A new compiler diagnostic code needs a row in `crates/svelters-compiler/src/codes.rs` (then `UPDATE=1` on the
  `cx1_codes` test regenerates the docs page); an unregistered code fails that test and panics in debug builds.
- Generated files (`*.generated.js`, `types/*.d.ts`, metadata outputs, op logs, snapshots): on
  conflicts, rerun that generator; don't hand-merge.
- Commit messages end with the attribution trailers your session is given.
