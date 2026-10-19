# `svelters` CLI: design proposal

Status: research proposal, 2026-10-17; implementation language decided 2026-10-19 (Rust, see §5 and the Decisions list). C0 (`svelters docs`) is implemented in `crates/svelters-cli` (`plans/milestones/CL0-cli-docs.md`). Evidence, file references and prior-art links are in `docs/notes/cli-research.md`.

## Summary

- **One command, `svelters`.** It is a Rust binary (`crates/svelters-cli`), decided 2026-10-19. It spawns the other Rust pieces (one runner binary per toolkit, `svelters-compile`) and, until the QuickJS dev module loader lands, drives Vite as a child process for `dev`/`build`. npm stays the package manager only.
- **Distribution.**
  - Users get it with `npm create svelters@latest` or `npx svelters`.
  - Runner binaries come as per-platform npm packages, the way esbuild and biome ship theirs. When no prebuilt binary fits, they are built from source with cargo.
- **One toolkit per app**, recorded in `svelters.toml`. Both the commands and the docs probe read it.
- **`svelters docs`** is the interface for agents.
  - Addresses look like `go doc`: `svelters docs [<toolkit>] <element>[.<member>]`.
  - Output is terminal markdown by default, with `--brief` and `--json`.
  - Its data is a pack built from the same docs IR as the site, so the CLI and the site cannot drift.
  - No MCP server for now (decision 7); `--json` is the agent interface.
- **MVP.**
  - `docs` can be built today, on the current IR.
  - `create`, `dev`, `build`, `run` and `doctor` work inside the repo once `examples/counter` becomes a generic runner.
  - Publishing, MCP, `add`, inspect and package come later.

## 1. What exists today, and what the CLI wraps

| Piece | Today | CLI role |
| --- | --- | --- |
| Bundle build (stock Svelte) | `packages/build/index.js` `svelteConfig()`, `bin/svelters-build.mjs [--watch] [dir]` -> `dist/app.js` + map | `build`, the build half of `dev` |
| Bundle build (native compiler, PR #3) | `svelters-compile --backend gtk\|qt`, `@svelters/native-runtime/vite` `nativeConfig` + `svelteNative({backend, spec})` | selected by `frontend = "native"` |
| Runner | `examples/counter` bin: `--backend gtk\|qt --fixture <dir> [--watch] [--script --screenshot --report]` | generalised into `svelters-run-{gtk,qt}`, used by `run` and `dev` |
| Hot reload | host `reload.rs`: inotify on `dist/`, load new then destroy old, mapped errors | `dev` = Vite watch + runner `--watch`, logs multiplexed |
| Typings | `packages/renderer/types/{gtk,qt}.d.ts` (generated) | template `tsconfig` picks one toolkit; `check` |
| Op-log tests | `packages/testhost/bin/svelters-test.mjs`, `script.json` + `expected.oplog` | `test` (later) |
| Docs IR | `tools/docs/build_ir.py` -> `docs/site/.generated/ir/{gtk,qt}.json` (schema `tools/docs/ir/schema.json`) | source of the docs pack |
| Style tables | ST1: `metadata/style/{gtk,qt}.yaml` (`via: css\|setter\|child-attr`) | the docs `style` section, via the IR |
| Galleries | `fixtures/gallery-{gtk,qt}` | `svelters gallery <tk>` (later) |

There are three gaps the CLI has to close.

1. **There is no generic runner.** `counter` resolves `fixtures/<name>/dist/app.js` and links both toolkits.
2. **Nothing is published.** The Svelte fork is a vendored tarball, and every `@svelters/*` package is private at `0.0.0`.
3. **The bundle is toolkit-neutral today** (portable tags). The native compiler, and N1 after it, make the bundle per-toolkit. The CLI treats the toolkit as a build input from day one.

## 2. Command tree

```text
svelters
├── create [dir] --toolkit gtk|qt [--template app|minimal|gallery] [--frontend svelte|native]
│                 [--pm pnpm|npm] [--agents|--no-agents] [--yes]          # no prompts with --yes or no TTY
├── add <what> …                                                         # file-editing add-ons, idempotent
│   ├── component <Name> [--dir src/lib]
│   ├── window <Name> [--dialog]                                         # toplevel declared in a component (X7)
│   ├── lists | canvas                                                   # @svelters/lists, canvas package + imports
│   ├── test <name>                                                      # script.json + oplog TestHost test
│   ├── agents [--mcp]                                                   # AGENTS.md, CLAUDE.md, .mcp.json
│   └── native                                                           # eject a Rust leaf crate for custom native code / release build
├── dev  [--toolkit] [--no-window] [--inspect]                           # Vite watch + runner --watch, one log stream
├── build [--release] [--json]                                           # dist/app.js (+ map); --release: minified, no map
├── run  [--bundle path] [--script s.json --screenshot out.png --report r.json]
├── check [--json]                                                       # svelte-check (one toolkit's typings) + style/compile diagnostics
├── test                                                                 # TestHost op-log tests (later)
├── doctor [--json]                                                      # toolchain checklist with fix commands (also `info`)
├── docs [<tk>] [<address>] [--brief|--json|--all|--section s] [-k query] [--open]
├── mcp                                                                  # stdio MCP server over the docs/check backend
├── inspect tree|props <id>|screenshot|eval (later; talks to a `dev --inspect` socket)
├── gallery <tk>                                                         # run the shipped widget gallery
└── package [--format dir|tar|appimage|flatpak] (later; needs M6)
```

The following rules apply across commands.

- **Machine output.** Every command that produces data takes `--json`. Long-running commands take `--json-events`, which writes NDJSON lines such as `{"event":"rebuilt","ms":412}` and `{"event":"error","file":"src/App.svelte","line":12,"col":4,"message":…}`. Agents can then drive `dev` without scraping logs.
- **Interaction.** Every prompt has a matching flag. When stdin is not a TTY, nothing prompts (as in `sv`).
- **No toolkit switch.** No `add toolkit` command exists, because apps don't port between toolkits. Supporting a second toolkit means a second app in the same workspace, sharing plain `.svelte.js`/`.ts` logic modules. `create` can scaffold that as a pnpm workspace (see the open questions).

## 3. Project layout and config

```text
my-app/
├── svelters.toml          # the one config the CLI, the runner and the docs probe read
├── package.json           # svelte (fork), @svelters/runtime-<tk>, @svelters/cli (dev)
├── tsconfig.json          # "types": ["@svelters/types-<tk>"]   (one toolkit only)
├── src/
│   ├── main.ts            # mount(App)
│   ├── App.svelte         # the main window
│   ├── windows/           # `add window`
│   └── lib/               # `add component`
├── tests/                 # `add test` (script.json + expected.oplog)
├── AGENTS.md              # `create --agents` (default on) / `add agents`
├── CLAUDE.md              # one line: @AGENTS.md
├── vite.config.js         # optional; only for user plugins. The CLI supplies svelteConfig() itself
└── native/                # optional, `add native`: Rust leaf crate (custom widgets, release binary)
```

The config is `svelters.toml`, not `svelters.config.js`.
- **Both runtimes read it.** The Rust runner, Node and Python tools all need it, and TOML parses natively in each.
- **It is static data, which suits agents.** An agent can read it and edit it safely.
- **The build hook stays.** Anything programmatic belongs in the optional `vite.config.js`, which is merged over the CLI's config, so no capability is lost.

```toml
[app]
name = "my-app"
id = "org.example.MyApp"        # GApplication id / Qt organization+app name
toolkit = "qt"                  # gtk | qt  (one per app)
frontend = "svelte"             # svelte (stock + renderer fork, default) | native (Rust compiler, experimental)
entry = "src/main.ts"

[style]
strict = false                  # unknown properties are errors instead of warnings (style plugin options)
tokens = "src/tokens.css"

[run]
runner = "prebuilt"             # prebuilt | source (cargo build of the runner) | path:/abs/runner
args = []

[package]                       # later
icon = "assets/icon.svg"
```

### Templates (per toolkit; element names follow N1)

| Template | GTK | Qt |
| --- | --- | --- |
| `minimal` | `<application-window>` + `<box>` + label/button counter | `<main-window>` + central `<widget>` + counter |
| `app` (default) | header bar, menu (X4), a dialog window (X7), a list (X5), scoped `<style>` with CSS + setter properties | menubar, toolbar, status bar, dock (parent:area), dialog, list view, QSS + setter properties |
| `gallery` | a copy of `fixtures/gallery-gtk` | a copy of `fixtures/gallery-qt` |

Templates are generated from the tag table, never hand-written tag names (the ST1 rule). The template test builds each one and runs it under the TestHost and the real toolkit with `--screenshot`. After N1 that keeps them correct when tags are renamed.

### Frontend selection

`[app].frontend` chooses between the two:
- `svelte` uses `svelteConfig()`, the stock Svelte path with the renderer fork;
- `native` uses `nativeConfig()` + `svelteNative({backend: toolkit})` and needs the `svelters-compile` binary, which is fetched like the runner.

`create --frontend native` stays hidden behind `--experimental` until ST1 lands and the compiled conformance baseline is published. `doctor` reports which frontend is in use and its version.

### Dependencies and how they are fetched

| Dependency | Proposal |
| --- | --- |
| Svelte fork | Publish `@svelters/svelte@5.57.2-cr.<n>` from `vendor/svelte`. Templates depend on `"svelte": "npm:@svelters/svelte@…"` plus a `pnpm.overrides`/`overrides` entry so vite-plugin-svelte sees the same copy. Inside the repo, use the workspace override as today. |
| JS packages | Publish `@svelters/{build,renderer,style,lists,canvas,native-runtime,cli}` and `@svelters/types-{gtk,qt}` in lockstep under one version. |
| Runner (host + backend) | `svelters-run-gtk` and `svelters-run-qt` are generalised from `examples/counter`. Each links one toolkit and takes `--bundle <path>`. They ship as `@svelters/runner-<tk>-<os>-<arch>` optionalDependencies. Qt is built against the oldest supported Qt 6 minor, because Qt 6 is forward-compatible within 6.x. GTK links the system GTK 4. |
| Source fallback | With `runner = "source"`, or when no prebuilt binary matches, the CLI runs `cargo build --release -p svelters-run-<tk>` from a pinned crate (crates.io or a git tag). The result is cached in `~/.cache/svelters/<version>/<tk>/`. This is a one-time cost, so an edit to `.svelte` never invokes cargo (PLAN rule 5). |
| `svelters-compile` | Same scheme as the runner. It is needed only for `frontend = "native"`. |
| Custom native code | `add native` creates the leaf crate (the `examples/counter-qt` pattern), and the runner becomes that crate's binary through `runner = "path:…"`. |

## 4. The docs probe

### 4.1 Addressing

```text
svelters docs                               # toolkits + project toolkit + data version
svelters docs qt                            # element index: tag, native class, one-line summary, tier
svelters docs qt main-window                # the whole element card
svelters docs qt main-window --section style   # one section: props|events|slots|methods|style|examples|lifetime
svelters docs qt main-window.title          # one member, resolved across props/events/methods/slots/style
svelters docs qt main-window.prop:title     # explicit kind when a name is ambiguous
svelters docs qt main-window.event:close-request
svelters docs qt main-window.slot:menubar   # slot + its child attrs (parent:<attr>)
svelters docs qt box.attr:stretch           # child attribute a <box> gives its children
svelters docs qt style                      # the toolkit's whole style table
svelters docs qt style.minimum-width        # one style property: via, target, type, kinds, note
svelters docs qt enum:Qt::AlignmentFlag     # enums / value types (native spelling or js name)
svelters docs qt QMainWindow                # native class names resolve too (GtkBox, Gtk.Box, Box)
svelters docs main-window                   # toolkit from svelters.toml; outside a project see 4.4
svelters docs -k dock                       # search names, tags, summaries (all toolkits, or the project's)
svelters docs guide styling                 # guides from docs/site (markdown, same source)
```

These are the resolution rules.

- **Matching.** Matching is case-insensitive. An address can be a tag, an old (pre-N1) tag, a native class or an entity id. Old tags print a "renamed to `main-window`" line, which helps migration after N1.
- **Members.** A member name is tried as a prop, an event (`close-request`, the handler attribute `onclose-request`, or an alias such as `onclick`), a method, a slot, a child attr, and a style property.
- **Ambiguity.** If a name matches more than one, the CLI prints the candidates with their `kind:` prefix and exits with status 2, like `ri`. It never guesses.
- **Not found.** An address that matches nothing falls back to `-k` search and exits with status 1. The output shows "did you mean" suggestions ranked by edit distance.

### 4.2 Output formats

The human format is the default when stdout is a TTY. It is compact markdown, rendered with ANSI when on a TTY and plain when piped. The sections follow the site page (`docs/site/scripts/lib/render.mjs`) so the two look alike. The listing below is illustrative: it uses post-N1 names and the current IR facts for QMainWindow.

```text
$ svelters docs qt main-window
<main-window>  QMainWindow (QtWidgets, qmainwindow.h)        tier: annotated, tested
Main application window.                                       [GFDL brief; doc.qt.io/qt-6/qmainwindow.html]
Inherits: QWidget > QObject                                    (63 inherited props: --all)

PROPS
  title               string            rw   QWidget::windowTitle
  document-mode       boolean           rw   default false
  animated            boolean           rw
  tab-shape           TabShape          rw   enum:QTabWidget::TabShape
  tool-button-style   ToolButtonStyle   rw
  …
EVENTS
  close-request       onclose-request   sync: preventDefault() vetoes the close
SLOTS (children)
  central   (default)  one QWidget       setCentralWidget; old child deleted
  menubar              one QMenuBar      setMenuBar
  statusbar            one QStatusBar    setStatusBar
  toolbar              many QToolBar     addToolBar      attrs: parent:area = top|bottom|left|right
  dock                 many QDockWidget  addDockWidget   attrs: parent:area
STYLE  (QSS for type selector `main-window`; setters take type/.class/#id selectors only)
  css:     color, background, border*, padding*, margin*, font-*, min-width, …   (QSS; subcontrol ::separator)
  setter:  minimum-width → minimumWidth, maximum-width, minimum-height, maximum-height, enabled
METHODS
  show()  present()  close()  resize(w, h)  … 49 more (--section methods)
EXAMPLE
  <main-window title="Hello" onclose-request={…}> <label>Child content</label> </main-window>
See also: dialog, dock-widget, tool-bar · guide styling · site: /api/qt/main-window/
```

- `--brief` keeps the header, the member names and the types only, with no docs text. It is about 20 lines and is aimed at an agent's first look.
- `--all` expands inherited members, grouped by the base that declares them (`go doc -all`). By default the inherited ones are collapsed into counts, and `--section props --all` shows them.
- `--json` returns a stable, versioned object, `{"schema":"svelters-docs/1", …}`. It is a projection of the IR entity, not the raw IR.

```json
{"schema":"svelters-docs/1","toolkit":"qt","data_version":"0.4.0+6aaa642",
 "element":{"tag":"main-window","native":"QMainWindow","module":"QtWidgets","tier":"annotated","tested":true,
  "summary":"Main application window.","bases":["QWidget","QObject"],"url":"https://…/api/qt/main-window/"},
 "props":[{"name":"title","type":"string","native":"windowTitle","access":"rw","bindable":false,
           "default":null,"declared_in":"QWidget","verified":true,"doc":"…"}],
 "events":[{"name":"close-request","attr":"onclose-request","params":[],"sync":true,"result":"preventDefault"}],
 "slots":[{"name":"central","default":true,"cardinality":"one","accepts":["widget"],"attrs":[]},
          {"name":"toolbar","cardinality":"many","accepts":["tool-bar"],"attrs":[{"name":"area","type":"enum","values":["top","bottom","left","right"]}]}],
 "style":[{"name":"minimum-width","via":"setter","target":"minimumWidth","type":"length"},
          {"name":"color","via":"css","dialect":"qss"}],
 "methods":[…],"examples":[{"lang":"svelte","code":"…","tested":false}],
 "inherited":{"QWidget":{"props":63,"events":12,"methods":140}}}
```

- **One format for one member.** A member query (`main-window.event:close-request`) returns a single object in the same shape as the full card.
- **Every format flags its source.** All formats carry `verified` / `tested` / `annotated`, so an agent can tell a verified fact from upstream metadata alone.

### 4.3 Data: one source, shipped offline

*As built in C0 (2026-10-19); the 2026-10-17 proposal (a Python `build_pack.py` writing `pack.json.gz`) was replaced by the following. Details and numbers: `plans/milestones/CL0-cli-docs.md`.*

- **The pack is built from the site's IR, by the CLI's own code.** `svelters docs-pack` (`pnpm docs:pack`) reads `docs/site/.generated/ir/<tk>.json`, the file the site renders, and writes `docs/site/.generated/pack/<tk>.svpack`: a directory plus one raw-deflate JSON blob per entity (GTK 0.92 MB, Qt 0.59 MB from 7.3 / 8.2 MB of IR). The projection structs are the pack format, so the writer and the reader share one definition. A lookup inflates only the entity and its ancestors (11–18 ms per release-build call). `tests/site_consistency.rs` renders the site from the same IR and checks that every page, tag, member anchor and inherited link resolves in the pack to the same route and anchor, so the CLI and the site can't drift.
- **Inside the repo it needs no extra step.** The CLI finds the repo by `docs/site/.generated/ir` and rebuilds a missing or stale pack (older than the IR) on the fly, in about 0.5 s.
- **Shipping.** A release binary can carry the packs (`SVELTERS_DOCS_EMBED_DIR` at build time, `include_bytes!`) or find them next to itself. Either way it works offline and links neither GTK nor Qt. `--pack`/`SVELTERS_DOCS_PACK` override both. The project-locked `node_modules/@svelters/docs-<tk>` lookup waits for C3 (publishing).
- **The style data comes in through the IR.** ST1 design item 6 makes the docs generator read `metadata/style/<tk>.yaml`. That data has to arrive as an IR field, `entity.style` or a top-level `style` table, and not through a separate side channel. The CLI then gets it for free. As of C0 the site still reads the style table through `packages/style/src/props.generated.js`, so the pack has no style section yet (C1.5).
- **The Qt license holds offline.** Qt text is GFDL with a `brief` text policy, and the IR already carries only briefs. The pack keeps them plus `toolkit.doc_license` and the notices, which the markdown output prints under any doc text and `--json` returns as `notices`.

### 4.4 With and without a project

- **Inside a project.** The toolkit comes from `svelters.toml` and is optional on the command line. Passing a different toolkit prints a one-line note ("this app targets gtk").
- **Outside a project, unique name.** If the name exists in only one toolkit, the CLI answers with a note saying which toolkit it is.
- **Outside a project, shared name.** If the tag exists in several toolkits (`box`, or `window` before N1), the CLI lists one line per toolkit and asks for one. It never merges them, because there is no portability layer.

### 4.5 MCP (`svelters mcp`)

`svelters mcp` is a stdio server using the same resolver and pack. It follows the Svelte MCP two-step pattern: list first, then fetch, which keeps context small.

| Tool | Args | Returns |
| --- | --- | --- |
| `list_elements` | `toolkit?`, `query?` | tag, native class, summary, tier |
| `get_element` | `toolkit?`, `element`, `sections?`, `brief?`, `include_inherited?` | the `--json` card |
| `get_member` | `toolkit?`, `address` | one member object |
| `search_docs` | `query`, `toolkit?`, `limit?` | ranked addresses |
| `get_style` | `toolkit?`, `property?` | style table rows |
| `get_guide` | `name` | guide markdown |
| `check` | `code` or `path` | diagnostics from svelte-check, the style plugin and (native) the compiler: the counterpart of `svelte-autofixer` |

Each tool's description names the CLI equivalent. MCP is phase 2, because agents with a shell can already use `svelters docs --json` and the MCP server stays a thin wrapper over it. `add agents --mcp` writes `.mcp.json` with `{"svelters":{"command":"npx","args":["svelters","mcp"]}}`.

### 4.6 AGENTS.md written by `create`

```markdown
# Agent notes for <app>

This is a Svelters app: Svelte 5 rendered as native **Qt 6 Widgets** (toolkit fixed in svelters.toml).
It is not a web app: there is no DOM, no HTML elements, and no browser CSS.

- Elements are native classes in kebab-case without the prefix: QMainWindow → <main-window>,
  QPushButton → <push-button>. Props, events and style properties use native names in kebab-case.
- Before using an element or property, look it up:  `npx svelters docs <element> --brief`
  (full card: drop --brief; one member: `<element>.<name>`; search: `svelters docs -k <word>`;
  machine-readable: add --json). Do not guess names from HTML or from the other toolkit.
- Styling: `npx svelters docs style` lists every property and whether it is QSS or a widget setter.
  Setter properties only accept type, .class and #id selectors.
- Check your work: `npx svelters check --json` (types, styles); run: `npx svelters dev`;
  screenshot: `npx svelters run --screenshot out.png`.
- Children go in named slots; see the SLOTS section of the parent's card (`parent:<attr>` for child attrs).
```

`CLAUDE.md` contains `@AGENTS.md`. The file is generated from the tag table, so the example names are real ones for the chosen toolkit.

## 5. Implementation language and packaging

**Decision (user, 2026-10-19): the CLI is written in Rust.** The 2026-10-17 proposal recommended a Node/TypeScript CLI; it is kept below, collapsed, for the record.

Why Rust:
- **The pieces it drives are Rust.** The runner (host + backend), `svelters-compile` and the planned QuickJS dev module loader are all Rust. A Rust CLI can call into them as libraries or exec them, with no Node process in between.
- **The Node constraint is gone.** The proposal's main argument was that "every build already needs Node" for Vite. `plans/research/QUICKJS-RUNTIME.md` §3.6 removes it: with the dev module loader, the runner is host + loader + compiler + watcher, and `dev`/`build` can be Rust-only.
- **npm remains, as the package manager only.** Apps still have a `package.json` and `node_modules`; `npm create`/`npx` can still be the entry points by shipping the binary in per-platform npm packages (the esbuild/biome model), with `cargo install` as the second channel.

Consequences:
- **Until the dev module loader lands, `dev` and `build` still drive Vite** (or `svelters-compile` through `@svelters/native-runtime/vite`) as a child process, exactly as `pnpm watch` does today. C0 does not depend on either, and the C1 command layer must keep "which bundler" behind one function so the loader can replace Vite without touching the commands.
- **`svelters docs` is self-contained.** The docs pack is built from the site's IR by the same Rust code that reads it (`svelters docs-pack`), so the pack schema has one definition. The binary links neither GTK nor Qt and works offline.
- **The runners stay separate binaries**, so the CLI never links a toolkit: `docs` and `create` work on a machine with neither installed. That part of the proposal is unchanged.
- **Build cost is watched.** The CLI crate depends only on serde, serde_json and miniz_oxide (pure Rust deflate); argument parsing is hand-written for C0 (see the milestone file for when `clap` would pay off).

<details><summary>Superseded proposal (2026-10-17): a Node/TypeScript CLI</summary>

**Recommendation: a TypeScript (Node ≥ 22) CLI orchestrating Rust binaries, shipped through npm. `cargo install` is a secondary channel for the Rust pieces only.**

The reasons for Node:
- **Every build already needs Node.** Both frontends run through Vite, including the native compiler's `svelteNative` plugin. A Rust CLI would still have to spawn Node for every `build` and `dev`. A Node CLI calls `vite.build()` in-process, as `svelters-build.mjs` already does.
- **The renderers are already JavaScript.** The docs page renderer (`render.mjs`) and the reference MCP SDK are both JS. A Node CLI can reuse them, so the human output stays close to the site.
- **The project is a Node project.** Every app has a `package.json`, and `npm create` and `npx` are the scaffolding conventions users expect (as with `sv` and `create-tauri-app`).

These parts stay in Rust: the runners, `svelters-compile`, and later the M6 release linker.

**What was rejected:** a single Rust binary embedding the host. It would either link both toolkits, or ship two CLIs. It would still need Node for Vite, and it would make `docs` depend on Qt or GTK shared libraries. This can be revisited if Vite is ever replaced (for example, if the native compiler gains its own bundler).

</details>

## 6. Phasing

| Phase | Scope | Size | Depends on |
| --- | --- | --- | --- |
| **C0 docs probe** | `crates/svelters-cli` (Rust, decided 2026-10-19) with `svelters docs` (address resolver, md/brief/json, `-k`, inheritance), reading a docs pack built from `docs/site/.generated/ir` (`svelters docs-pack`); pack/site consistency test | ~2–3 agent-days | **first** fix or verify the Qt IR tag gap (8 tagged entities vs 57 override tags, notes §1) |
| **C1 MVP loop** | generic runner `crates/svelters-run` (from `examples/counter`, `--bundle`, one toolkit per feature/binary); `create` (minimal template per toolkit, in-repo workspace linking), `build`, `run`, `dev` (Vite watch + runner `--watch`, `--json-events`), `doctor`, `check`; `svelters.toml` | ~4–6 agent-days | M2b (done); runner refactor touches `examples/counter` |
| **C1.5 style + names** | `style` section/table in docs; templates and AGENTS.md on native kebab tags; old-tag aliases in the resolver | ~1–2 days each | **ST1** (style table in IR), **N1** (tag rename) |
| **C2 agents** | `svelters mcp`, `add agents`, `add component/window/test/lists/canvas`, `app` templates | ~3–4 days | C0, C1 |
| **C3 distribution** | publish the fork + `@svelters/*`, prebuilt runners in CI (linux x64/arm64 first), cargo source fallback, `docs-<tk>` packs | ~4–6 days + CI | release decision; Qt minimum version |
| **C4 later** | `inspect` (dev socket from the host: tree, props, screenshot, eval), `test`, `gallery`, `package` (AppImage/Flatpak, M6 release binary), `frontend = "native"` out of experimental | — | M6, ST1, host inspect protocol |

**Recommended order: C0 → C1 → C1.5 → C2 → C3.** C0 is useful to agents in this repo straight away. Its resolver is written against the tag table, so N1 only changes data, not code.

## 7. Open questions for the user

1. **Prop and event names after N1.** The IR still uses camelCase JS prop names (`documentMode`). Do element props also move to native kebab-case (`document-mode`), as style properties do? This decides what `docs` prints and what templates write. The examples above assume yes.
2. **Publishing the Svelte fork.** Should it be an npm alias package (`@svelters/svelte`) or tarballs on GitHub releases? Where does the fork branch live publicly? `vendor/svelte/PIN` says it isn't pushed.
3. **Prebuilt runners vs source only for v0.** Prebuilt Qt runners pin a minimum Qt 6 minor version (6.5 LTS?). Would building from source with cargo on first run be acceptable at first?
4. **Config format.** `svelters.toml` (recommended) or `svelters.config.js`?
5. **Two toolkits for one product.** Should `create` offer a workspace template (shared logic package plus one app per toolkit), or leave that to users?
6. **The native frontend in `create`.** Hidden behind `--experimental` until ST1 and the compiled conformance baseline are done, or offered from the start?
7. **MCP timing.** Ship MCP in C0 alongside `docs --json`, or wait for C2 as proposed?
8. **libadwaita (`ad-…`).** No Adw metadata is ingested today. Does the docs probe need to plan a `gtk` sub-namespace for it now, or is the flat tag table with the `ad-` prefix enough?

## Decisions (user, 2026-10-17)

1. Element props and events become native kebab-case in N1, as tags already are. CSS element (type) selectors use the same kebab-case tags.
2. Drop the Svelte fork. The Rust compiler (`svelters-compiler` + `@svelters/native-runtime`) is the frontend, so the stock Svelte + `@svelters/renderer` path is retired, and nothing about the fork needs publishing.
3. svelters ships with one bundled Qt version by default. The generators, once ported to Rust (M6), must be able to target Qt back to 6.0, and possibly 5.x later. Prebuilt runners are a release concern, not v0.
4. The config file is `svelters.toml`.
5. `create` gets no template with both toolkits.
6. The native compiler is not behind `--experimental`; it is the only frontend.
7. No MCP server for now. Agents with a shell get everything through `svelters docs … --json`. MCP would only matter for clients without a shell, so it waits until someone needs that.
8. The `ad-` namespace needs no planning now.

## Decisions (user, 2026-10-19)

9. The CLI is written in Rust, not Node/TypeScript (§5). The runner, the compiler and the future module loader are Rust, and `plans/research/QUICKJS-RUNTIME.md` §3.6 removed the "needs Node for Vite" constraint. npm remains the package manager only. The QuickJS dev module loader is deferred, so C1's `dev`/`build` still drive Vite or `svelters-compile` for now.
