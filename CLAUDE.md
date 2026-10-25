# svelters: instructions for agents

svelters is a Rust Svelte compiler (`crates/svelters-compiler`) plus `@svelters/native-runtime`,
running stock svelte@5.57.2 on native backends through a QuickJS host (`crates/svelters-host`) and
a generic backend C ABI. Status comes from `STATUS.md` (handoff block and milestone rows),
`MATRIX.md` and `plans/milestones/`, never from plan or research files alone.

## Current focus (user, 2026-10-21)

**svelters + egui for cadatomic** (the MCAD + ECAD + simulation IDE, `github.com/akiselev/cadatomic`).
cadatomic's UI is built on svelters' egui backend (`crates/svelters-egui`): Rust owns the shell
plumbing (docking, tabs, toolbars, menus, windows); Svelte renders the contents of panels,
dialogs, windows and overlays, with many roots at once. AppKit, WinUI and Qt get maintenance only.

Read, in order:
1. `plans/research/TRACKS.md`: the parallel tracks (svelters and cadatomic), waves, critical path.
2. `plans/research/CADATOMIC.md`: svelters' answer to cadatomic's requests SV1–SV47 (plus SVP1–SVP9).
3. `plans/research/EGUI.md`, `plans/research/CSS.md`, `plans/research/HMR.md` for the track you own.
4. `docs/egui/START-HERE.md`: the egui backend's docs (own widgets in Rust and CSS, icons, automation,
   loading code, hot reload), with when to read each page. Read it before writing egui widget or embed code.

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

## How work runs (user, 2026-10-25)

Work runs as Claude Code cloud batches: each lane is its own cloud session in its own container,
on its own `ci/<lane>` branch, with a written brief (`plans/cloud/<batch>/`). **The coordinator**
(the cadatomic batch coordinator session, which also coordinates svelters) plans the batch,
answers lanes and merges `master`. cadatomic's `CLOUD.md` describes the procedure.

- The coordinator owns svelters `master`: it merges lane branches (`--no-ff`) once all five
  GitHub CI workflows are green on them, and keeps `STATUS.md` and `MATRIX.md`. Lanes never push
  `master`; they push their branch and write a record in `plans/milestones/`.
- svelters sessions do not edit cadatomic. Requests go through cadatomic's
  `docs/plan/04-kernel-requests.md` (SV table) and svelters' "Requests from cadatomic" table in
  `STATUS.md`.
- **A fresh agent for new work;** never reuse a finished or long-context agent. Hand off at about
  400k tokens, with a handoff note in the milestone file. At most eight subagents at once.

## Who decides (user, 2026-10-22)

**The coordinator decides; the user is not asked.** The user has delegated decisions to the
coordinator and does not want sessions asking them questions.

- **Lanes never ask the user.** They write the question with a recommendation into their record
  (or send it to the coordinator), keep working on something else meanwhile, and follow the answer.
- **The coordinator decides**, citing the reason in the answer:
  - routine and procedural questions in svelters: lane scope and order, merges and conflict
    resolution, pins and pin advances, build and test rules, this file, implementation details
    inside an agreed design;
  - cross-repo seams between svelters and cadatomic that fix a contradiction or a bug and choose
    no design;
  - the coordinator's message to a session carries this delegated authority. Act on it.
- **Escalate to the user only** (the coordinator does it, nobody else):
  - architecture and design choices: the open questions in `plans/research/ARCH-*.md` and similar
    design docs, brought together with a recommendation each;
  - reversing or contradicting a decision the user made (the list above, or a register entry
    marked as the user's);
  - destructive actions on work that isn't yours: force-push, deleting branches or repos with
    unmerged work, discarding uncommitted changes;
  - anything public or paid: publishing crates, making repos public, new paid services.
- Record each decision where the next session will find it: `STATUS.md` or the relevant
  `plans/research/*.md` ("Decided (coordinator, date)").

## Building and testing: as little as possible

**GitHub CI is the gate, not your container.** No build or test run that doesn't answer a question
you have right now (user, 2026-10-22).

- Plain `cargo` is fine in a cloud container (one lane per container). `tools/dev/cargo-slot`
  is optional there; set `CARGO_PROFILE_DEV_DEBUG=0` to save disk.
- **While writing code:** `cargo check -p <crate you changed>` only, and only when you need the
  compiler's answer. Use `--workspace --all-targets` only after changing an API other crates use.
- **When the feature and its tests are written:** run only the tests you added or touched,
  filtered (`cargo nextest run -p <crate> <filter>`, `pnpm --filter <pkg> test`; egui, GTK and Qt
  tests under `tools/dev/headless`). Fix all failures in one batch, then rerun those tests once.
- **Never run locally:** the full workspace nextest, the full gate list, or a full conformance run.
  Push your branch; the five GitHub workflows run everything. Fix what CI reports, in one batch,
  and push again.
- **Never rerun anything on unchanged code.** Run nothing for docs-only changes, milestone records,
  or a merge of `origin/master` that conflicts nowhere in code. A result from before your last code
  edit is not a pass, so say "not run" instead.
- **Conformance** runs in CI (the `conformance` job of `js.yml`, `--check` against `baseline.json`;
  its `results.json`/`REPORT.md` are uploaded as an artifact). Locally, run only the affected
  samples (`node run.mjs <name-filter>`, see `docs/notes/cf1-handoff.md`). When a lane raises the
  score, it commits `--update-baseline` output from one full run at the end.
- **Research and docs lanes build nothing.**
- **Stop when told:** kill your running builds and tests, commit, push and report.

## Before reporting a lane done

1. Merge `origin/master` into your branch; on generated-file conflicts rerun only that generator.
2. `pnpm install --frozen-lockfile` if the lockfile changed, then `cargo check -p <crates you
   changed> --all-targets`, plus the filtered tests above.
3. Push `ci/<lane>` and get all five CI workflows green. Wait with `tools/dev/ci-wait <sha> --jobs`
   (no credentials) or one `gh run watch <id> --interval 300` per run; never poll faster. Every
   session shares one GitHub API limit (5,000 requests an hour). Report the sha, the CI run ids,
   and what you did not run.

## Git safety

- **Committed files never use our credentials** (user, 2026-10-22): no `gh auth` calls, tokens, credential
  helpers or account-specific auth in scripts, build files or CI config. Agents may use `gh` interactively.
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
