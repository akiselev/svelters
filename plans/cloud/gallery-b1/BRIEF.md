# svelters cloud batch gallery-b1: the component gallery (common brief)

The user asked for this batch on 2026-10-27 ("lets work on polishing all of the components in svelters and
cadatomic … a separate widget gallery that shows every variant and stuff, with code for each example that's
editable and viewable in the gallery live. Basically everything an agent needs to rapidly iterate. It should be
a separate crate so it doesn't bring everything in cadatomic"; then "write it and start. Accept all
recommendations for the crates and sheet golden thing"). The coordinator is the cloud session that wrote this
brief (`session_01HBbGRgGENFKUjsqyGryCSr`). A separate coordinator runs MAL (cadatomic's malleability batches,
which also merge svelters lanes); this batch stays out of its way (below).

svelters' `CLAUDE.md` binds you, with these specifics:
- **Coordinator:** the session above. It merges your branch into `master` (`--no-ff`) once all CI workflows
  are green on it. Lanes never ask the user; questions go in your milestone record with a recommendation.
- **Your branch** is `ci/gb1-<lane>` (the `ci/**` pattern runs the workflows). Base: `master` `01e8f67` or
  later. Push only there; no pull requests; never push `master`.
- **Your record** is a new milestone file (the table below): scope, decisions, what landed, public API, tests
  with counts and commands, CI run ids, questions with a recommendation each, handoff. Don't edit `STATUS.md`,
  `MATRIX.md` or `plans/HANDOFF.md`; the coordinator does at the merge.
- **Read first:** `CLAUDE.md`, `plans/HANDOFF.md`, `plans/research/skelter/README.md` and `architecture.md`
  (§16 for the test harness), `docs/skelter/`, `plans/research/DESIGN.md` §1.5, §4 and §6 (themes, tokens,
  the visual-conformance loop VIS1 that this gallery replaces for skelter), then your lane file.
- **Setup (cloud container, 4 cores, ~39 GB disk):** rustc 1.97.0; `corepack enable` and
  `pnpm install --frozen-lockfile`; `git config core.hooksPath .githooks` once; cargo with
  `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0`. Pixel rendering needs lavapipe:
  install the packages `.github/workflows/skelter.yml` installs (`apt-get install -y libwayland-dev
  libxkbcommon-dev libxkbcommon-x11-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libfontconfig1-dev
  libfreetype-dev mesa-vulkan-drivers libvulkan1 vulkan-tools xvfb`).
- **Kinds:** the registry the gallery renders with is what the app runner registers
  (`svelters_skelter::kinds::register_builtin`, which covers `skelter_widgets::host::register_tier0` and the
  rest); list kinds from that registry, not from a hand-written list.
- **Build little:** `cargo check -p <crate>` while writing; your own tests filtered at the end; GitHub CI is
  the gate. Wait with `tools/dev/ci-wait <sha> --jobs`; never poll faster than every 5 minutes.
- **Commit and push often:** after every item and at least every 30–45 minutes. Near 400k tokens of context,
  stop, commit and write the handoff into your record.

## Decisions in force (user, 2026-10-27)

1. **The gallery lives in svelters** and depends on nothing in cadatomic. cadatomic adds its own components
   later through a small crate of its own, `cadatomic-gallery`, which depends only on `cadatomic-ui` and
   `cadatomic-core` (wave 2, a cadatomic lane; not this batch). So the gallery has an extension API (below).
2. **Contact sheets are for looking at, not goldens.** No test compares a sheet or a shot against a stored
   image, and no PNG is committed. Existing snapshot tests stay as they are. The gallery's tests check
   structure (stories found, every cell rendered, the image has the right size, inspect output has the fields)
   and that rendering is deterministic within one run.
3. **The source pane is quire on skelter (`skelter_quire`, github.com/akiselev/quire)**, not skelter-widgets'
   `text-edit` (that is the form field, also built on quire-text).
4. **Edits save to disk.** The file shown is the file rendered; the existing module watcher reloads it. An
   unsaved buffer previews through `SourceProvider::overlay()` (`svelters-modules/src/source/mod.rs`).
5. **Agents first.** Every view the app shows can be produced headlessly by the CLI, as a PNG or JSON.

## Shared contracts (every lane codes against these; change them only through the coordinator)

### Story files (`gallery/` at the repository root)

```
gallery/
  package.json            # a svelters source package: { "name": "@svelters/gallery-stories", "private": true,
                          #   "svelters": { ... } }, so SourceConfig::for_path finds a root
  README.md               # the format, for humans and agents (stories lane writes it)
  stories/<family>/<id>/
    story.toml
    <variant>.svelte      # one self-contained component per variant: exactly what the gallery renders and shows
```

`story.toml`:

```toml
id = "button"                 # stable id (lowercase, `-`); never a label
title = "Button"              # display text
family = "controls"           # controls | text | pickers | views | menus | tabs | dialogs | layout | data | feedback
kinds = ["button"]            # the element kinds this story covers (the coverage check reads it)
size = [320, 96]              # each cell's logical size in px
states = ["normal", "hover", "active", "focus-visible", "disabled"]   # optional; this is the default set
notes = "..."                 # optional: what to look at

[[variant]]
id = "primary"                # stable id
title = "Primary"
file = "primary.svelte"
args = { size = ["small", "normal", "large"] }   # optional matrix: each combination is passed as props
```

- A variant file is a plain Svelte 5 component. It takes the matrix args as props (`let { size = 'normal' } =
  $props()`) and marks the node that states are forced on with `anchor="subject"` (several nodes may carry it).
  With no `subject`, states apply to the root.
- `normal` forces nothing. `hover`, `active`, `focus-visible`, `focus` and `checked` are forced with
  `svelters_skelter::style::pseudo_state` and `SkelterBackend::force_state` (`designer.rs`). `disabled` is
  passed as the prop `disabled = true` when the variant declares it, so the kind's real disabled state and its
  accessibility state are what render; the gallery forces the `disabled` bit only when the variant has no such
  prop, and says so in the cell's label.
- A story never imports app state, the network or the clock; a story that animates is rendered with the virtual
  clock at a fixed time.

### Axes

A cell is (story, variant, args combination, state, theme, density, zoom). Themes: VS Code's `dark-modern`,
`light-modern`, `hc-dark`, `hc-light` (`skelter_widgets::theme`). Density: the names `Density::name` gives.
Zoom: a scale factor (`1`, `1.5`, `2`). Defaults: all four themes, the default density, zoom 1.

### The library (`crates/skelter-gallery`, lane core)

Names are the contract; the core lane may add, not rename. Paths are relative to the gallery root.

```rust
pub struct StoryId(pub String);                       // and VariantId
pub struct Story { id, title, family, kinds, size, states, notes, variants: Vec<Variant>, dir: PathBuf }
pub struct Variant { id, title, file: PathBuf, args: BTreeMap<String, Vec<serde_json::Value>> }
pub enum State { Normal, Hover, Active, Focus, FocusVisible, Checked, Disabled }   // #[non_exhaustive]
pub struct Theme { scheme, contrast }                 // or skelter_widgets' own type, with the four names above
pub struct Cell { story, variant, args: BTreeMap<String, serde_json::Value>, state, theme, density, zoom }
pub struct Matrix { ... }                             // a story × the chosen axes, expanded in a stable order

pub fn discover(roots: &[PathBuf]) -> Result<Catalog, Vec<StoryError>>;   // every story, errors named by file
impl Catalog { fn stories(&self) -> &[Story]; fn get(&self, id: &str) -> Option<&Story>;
               fn coverage(&self, registry: &KindRegistry) -> Coverage }   // kinds with no story, stories with unknown kinds

pub struct Renderer { ... }                           // headless: skelter-test's Harness + render, svelters-skelter, from source
impl Renderer {
    fn new(ext: &Extensions) -> Result<Renderer, Error>;
    fn shot(&mut self, cell: &Cell) -> Result<Shot, Error>;            // Shot { image: skelter_test::Image, warnings }
    fn sheet(&mut self, matrix: &Matrix, layout: SheetLayout) -> Result<Sheet, Error>;   // one PNG + an index of cell rects
    fn inspect(&mut self, cell: &Cell, selector: &str) -> Result<serde_json::Value, Error>;
}

pub struct Extensions {                               // what a host such as cadatomic-gallery adds
    pub story_roots: Vec<PathBuf>,
    pub register_kinds: Vec<fn(&mut KindRegistry) -> Result<(), String>>,
    pub setup: Vec<Box<dyn Fn(&mut SetupCx)>>,        // per-mount host state (cadatomic's grid sources, pick board)
    pub themes: Vec<(String, TokenMap)>,              // extra token sets on top of the four
}
pub fn cli_main(ext: Extensions) -> std::process::ExitCode;   // the CLI below, so a host's binary is three lines
```

### The CLI (`skelter-gallery`, lane core; the JSON is the contract agents use)

```
skelter-gallery list [--json]                                   # stories, variants, kinds, coverage gaps
skelter-gallery shot <story>[/<variant>] [--args k=v,..] [--state s] [--theme t] [--density d] [--zoom z] -o out.png
skelter-gallery sheet <story>|--family f|--all [--states ..] [--themes ..] [--zoom ..] -o out.png [--index out.json]
skelter-gallery inspect <story>/<variant> <selector> [cell flags] --json
        # computed style, matched rules with their sheet and source, layout box, AccessKit node, the forced state
skelter-gallery check [--json]                                  # every story parses, compiles and renders one cell; exit 1 on any error
```

Selectors are anchors (`#subject`) or a kind name; never visible text. Output goes where `-o` says (the default
is `target/gallery/`, ignored). Exit codes: 0 ok, 1 a story failed, 2 bad arguments.

## Lanes

| Lane | Branch | Record | Scope | Merge order |
|---|---|---|---|---|
| core | `ci/gb1-core` | `plans/milestones/GAL1-gallery-core.md` | `crates/skelter-gallery`: the format, discovery, matrix, headless renderer, sheets, inspect, the CLI, extension API | 1 |
| stories | `ci/gb1-stories` | `plans/milestones/GAL2-stories.md` | `gallery/`: a story for every skelter kind, the format README, the coverage list | 2 |
| app | `ci/gb1-app` | `plans/milestones/GAL3-gallery-app.md` | `crates/skelter-gallery-app`: the interactive gallery (browse, variant grid, quire source pane, controls, live reload) | 3 |

**Order inside the batch.** core pushes the data model (`Story`, `Variant`, `Cell`, `Matrix`, `discover`,
`Extensions`) as its first item, within about the first hour, and the `shot` command as its second. stories and
app may merge `origin/ci/gb1-core` into their branches whenever they need the newer API (a merge, never a
rebase); they never edit core's files, and list anything they need from it as a request in their record. stories
writes its files before it can render them, then renders them once `shot` lands.

**Shared files.** The root `Cargo.toml` (`members` and `[workspace.dependencies]`) and `Cargo.lock`: core adds
`skelter-gallery`, app adds `skelter-gallery-app` and the quire git dependencies with any `[patch]` lines;
keep every row from both sides at a merge. Nothing else is shared.

**Stay out of MAL's way.** Don't edit existing crates (`skelter-*`, `svelters-*`) in this batch. When the gallery
needs a change in one (a missing hook, a bug a story exposes), write it in your record as a request with the
smallest change you'd make, and work around it or leave the cell marked. Styling bugs the stories show are the
next batch's work (the polish lanes), not this one's: list them, with the `shot` command that shows each.

**Not implemented means not shipped.** No placeholder commands or empty panes; a command the CLI lists works.
Security: leave room only (`#[non_exhaustive]`); build no policy.
