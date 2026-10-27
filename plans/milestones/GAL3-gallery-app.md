# GAL3: the interactive gallery (`skelter-gallery-app`)

Lane `app` of svelters batch gallery-b1 (`plans/cloud/gallery-b1/LANE-app.md`), branch `ci/gb1-app`, base master
`01e8f677` plus the batch brief `34ed54b9`. Merges `origin/ci/gb1-core` (GAL1) as its API landed; never edits its files.

**Status:** done. Items 1–5 landed; core's R1 and R2 landed and are used; CI green on `b4256a78`.

## Scope

`crates/skelter-gallery-app/` (new): a library (`skelter_gallery_app::run(Extensions)`, so a host such as
`cadatomic-gallery` starts it with its own `Extensions`) and the binary `skelter-gallery-app`. Its rows in the root
`Cargo.toml`/`Cargo.lock`: the member, `skelter_quire` and `quire-core` at quire `ed0665b`, and a
`[patch."https://github.com/akiselev/svelters"]` for the three skelter crates `skelter_quire` names by git.

## Decisions

1. **quire builds as pinned.** `skelter_quire` at `ed0665bdb50a8a3b4b8fd8d20bd9bbde43f18f9f` (the rev cadatomic pins)
   builds against this master with the patch pointing `skelter-base`/`-core`/`-text` at the workspace paths (one
   svelters in the graph, as cadatomic's root manifest does). Nothing broke; no fallback pane was needed.
   Two `quire-text` copies are in the graph: skelter-widgets' (`35b5063a`, by https) and skelter_quire's (`ed0665b`).
   They share no types here, so it is only build time; see Q2.
2. **The chrome is Rust on skelter, not Svelte.** Each preview cell must be a Svelte root of its own (its own fault,
   reload and theme), and embed slots are host-owned nodes the host places in its own tree; the source pane is quire's
   `code-editor`, a Rust kind. With Svelte chrome the slots would have to live under JS-owned nodes (whose lifetime the
   chrome's JS controls) or float over them by copied geometry. Rust owns the frame around the roots, as cadatomic's
   shell does. The chrome is ~30 skelter-widgets nodes (`ui.rs`, the ids cadatomic's native panes use), rebuilt per
   section when the model changes; events come back through the story backend's event tap, matched by node.
3. **One embed per selected story.** Selecting a story makes a fresh `Embed` (its own QuickJS runtime) over the
   gallery's document, with one `SourceProvider` (`Entry::Embed`, an export `V<i>` per variant, `dev` + `hmr`). The host's
   reloader then watches exactly that story's module graph (it keeps one provider: several stories' entries in one host
   would leave all but the last unwatched, and every entry shares the id `svelters:entry`). A variant that does not
   compile at load is left out of the entry and its cells show the error; once it compiles, the story reloads.
4. **Live edit is the host's in-place update (H1).** Typing previews through `provider.overlay().set(file, text)` +
   `Host::apply_changes([file])` (no save); Ctrl+S (Cmd+S) writes the file and clears the overlay; a change on disk comes
   from the host's own watcher (`Host::watch`, woken through the backend's `WakeState`). A compile error (checked first
   with `SourceProvider::check`) or a failed swap keeps the last good render and shows `file:line:col` in the cell, over
   the source pane, and as a diagnostic on the line in quire. No svelters-modules change was needed.
5. **Per-cell themes are a class per theme** (`theme.rs`): `.gal-theme-<name>` redeclares the theme's whole token sheet
   (`Theme::sheet_tokens`, component tokens as `var()` of their roles) at the theme origin, on the cell's frame node; the
   stock kinds' UA tokens and `base.css` read the cascade first, so the cell follows. The chrome keeps Dark Modern on
   `:root`. Limit: what a kind reads from the `Theme` *service* (scheme, contrast, density outside tokens) follows the
   chrome's theme; the CLI renders each theme in its own document and is the reference there.
6. **Sheets as skelter-gallery's renderer installs them**: tokens + `base.css` at `Origin::Theme`, host kinds through
   `Kinds::with_app` (JS can mount them), locale `en-US`. The chrome's `gal-*` classes are author rules on chrome nodes
   only.
7. **`base.css` is read from source** (`crates/skelter-widgets/src/theme/base.css` above the story roots, as the CLI
   finds it; `--base-css <file>` overrides). It is not a module of any story, so a small thread stats it and wakes the
   loop; the frame reinstalls the sheet (`Gallery::poll_base_css`).
8. **Zoom is the window's scale** (skelter's UI zoom: points × zoom = device pixels), chrome included, as
   `skelter-gallery --zoom` scales a shot. Per-subtree zoom does not exist in skelter.
9. **Inspect shows `skelter-gallery inspect`'s JSON for the live cell**: a click in inspect mode hit-tests the cell,
   picks the nearest anchor at or above the node (`#subject`), else the kind name, and builds the JSON with core's
   `inspect::select_in` + `inspect::node_json` on the cell's backend (unsaved edits included). `accesskit` is `null`:
   the app keeps no AccessKit snapshot per cell (the CLI's harness does). "Copy shot command" / "Copy inspect command"
   put the matching CLI line on the clipboard (and show it in the inspect pane).
10. **One bad `story.toml` hides only itself in the app** (`gallery::discover`): `skelter_gallery::discover` refuses
    the whole catalog, so the app falls back to core's `parse_story` per manifest and lists the errors in the sidebar.
    (Core's `Renderer::new` still refuses the whole catalog; the app no longer uses a renderer.)
11. **Host extensions**: kinds through `Kinds::with_app` and, for the compiler, the same spec fragment core writes
    (`svelters_skelter::spec::fragment_text`, a temp file removed on drop); setup hooks through
    `Extensions::run_setup` before each cell mounts; host token sets as extra theme checkboxes (off by default: the
    brief's default is the four built-ins).

## What landed

| File | What |
|---|---|
| `src/app.rs` | `run(ext)`: the window on skelter's app runner (`GalleryApp`, an `AppHandler`), `document(ext, fonts)`, `shot(..)` (`--shot`), `write_png` |
| `src/document.rs` | `GalleryDocument: DocumentApi`: input through the story's backend, Ctrl+S, inspect clicks, frames, zoom, the window root |
| `src/gallery.rs` | `Gallery`: catalog, settings, the chrome, events, the canvas, the source pane, inspect, copy, base.css; `kinds(ext)`, `discover`, `story_roots`, `shot_command`, `inspect_command`, `selector_for` |
| `src/live.rs` | `LiveStory`: one story's embed, cells (`CellMount`), forced states, previews, `SourceError` |
| `src/theme.rs` | the four themes plus a host's token sets as cell classes; `CHROME_CSS` |
| `src/ui.rs` | the chrome's skelter-widgets nodes by id |
| `src/main.rs` | `skelter-gallery-app` = `run(Extensions::default())` |

The window: a controls bar (theme and state checkboxes, density and zoom selects, one select per arg key, Inspect,
Copy shot command, Copy inspect command), the sidebar (filter field; families → stories → the selected story's
variants; kinds with no story under "No story yet"; story.toml errors), the canvas (rows of variant × args, cells of
state × theme in the story's size, each a themed root; a cell's caption picks it), and on the right the source pane
(file, Save, Revert, errors, quire) over the inspect pane. Every control has an anchor (`story:<id>`,
`variant:<story>/<id>`, `theme:<name>`, `state:<name>`, `density`, `zoom`, `arg:<key>`, `inspect-toggle`, `copy-shot`,
`copy-inspect`, `filter`, `source`, `save`, `revert`, `source-errors`, `canvas`, `inspect`, `cell:<i>`).

### Public API (lib)

```rust
pub fn run(ext: skelter_gallery::Extensions) -> std::process::ExitCode;   // a host's binary is three lines
pub mod app { pub fn document(ext, fonts) -> Result<GalleryDocument, String>; pub fn shot(doc, fonts, size, pick, out) -> Result<(), String>; }
pub struct GalleryDocument { pub gallery: Gallery }                       // skelter::seams::DocumentApi
impl Gallery {
    fn select(&mut self, story: &str, variant: Option<&str>); fn cells(&self) -> impl Iterator<Item = (&Cell, NodeId)>;
    fn pick_cell(&mut self, i: usize); fn inspect_cell(&mut self, i: usize, selector: &str) -> Result<(), String>;
    fn inspect_live(&self, i: usize, selector: &str) -> serde_json::Value; fn set_inspector(&mut self, f: InspectFn);
    fn save(&mut self); fn revert(&mut self); fn unsaved(&self) -> bool; fn source(&self) -> Option<(PathBuf, String)>;
    fn set_base_css_path(&mut self, p: Option<PathBuf>); fn poll_base_css(&mut self) -> bool;
    fn live(&self) -> Option<&LiveStory>; fn catalog(&self) -> &Catalog; fn load_errors(&self) -> &[String];
    pub settings: Settings, pub inspected: Inspected, pub copied: Option<String>, pub watch: bool, ..
}
```

### Running it

```sh
cargo run -p skelter-gallery-app                      # the first story of gallery/ (found from the working directory up)
cargo run -p skelter-gallery-app -- button/look       # opens on a story or variant
# headless (a container, CI, an agent): the whole window as a PNG, through lavapipe
cargo run -p skelter-gallery-app -- checkbox/value --shot target/gallery/app.png --size 1500x900 --pick 5 --inspect '#subject'
# the window under Xvfb in a container
WGPU_BACKEND=vulkan xvfb-run -a -s "-screen 0 1600x1000x24" cargo run -p skelter-gallery-app -- label
```

Needs `pnpm install` (Svelte resolves through `@svelters/native-runtime`; the stories lane adds `gallery` to the pnpm
workspace for the packages its stories import).

## Tests

`cargo test -p skelter-gallery-app` (10: `tests/app.rs` 9, `tests/host.rs` 1; headless `Harness`, fixture gallery `tests/fixtures/gallery` copied
to `target/gallery-app-tests/<test>/` because the tests edit and save it; no symlinks, Cmd on macOS):

| Test | Checks |
|---|---|
| `the_sidebar_lists_the_fixture_stories_by_family` | stories by anchor, family headings, a coverage gap (`checkbox`) |
| `selecting_a_story_mounts_its_cells` | 3 rows × 3 states × 4 themes = 36 roots, each showing its args; light and dark cells resolve different tokens; hover forced on the subject; the source pane shows the first variant |
| `a_preview_edit_changes_the_cell_without_touching_disk` | Ctrl/Cmd+A and typing in quire changes the cell; the file is unchanged; Ctrl/Cmd+S writes it |
| `a_compile_error_keeps_the_last_good_render_and_shows_where` | the cell keeps the last good text; one error at `plain.svelte:1:<col>` shown over the source pane; fixing it applies |
| `a_file_changed_on_disk_reloads_through_the_host` | a disk edit applied through the host (`apply_changes`, what the watcher calls) updates the cell and the source pane |
| `picking_a_cell_readies_its_shot_command_and_inspect_picks_the_clicked_node` | the copied `shot` line; inspect mode: a click on the cell's button gives core's node JSON (`#subject`, the cell, forced `hover`) and the `inspect` line |
| `the_controls_narrow_the_matrix` | theme and state toggles, a variant from the sidebar, the filter |
| `an_edit_to_base_css_restyles_the_cells` | a changed `base.css` file is reinstalled and reaches a cell's label |
| `a_hosts_setup_hooks_run_before_each_cell_mounts` | `Extensions::on_setup` runs once per cell, in cell order |
| `host::a_hosts_kind_family_and_theme_work_in_the_app` | a host kind compiles (spec fragment) and mounts in every cell, its family lists, its token set is a theme toggle that reaches the cell, inspect describes the host node |

Last local run: 10 passed, after the last code change. Also run by hand: the windowed app under Xvfb on the
stories lane's gallery (31 stories), and a disk edit of `stories/controls/label/plain.svelte` while it ran: every cell
and the source pane showed the edit within 4 s (the host's watcher). Not run locally: anything outside this crate.

### Screenshots (container paths, not committed)

- `/tmp/claude-0/-home-user-svelters/7c0cf11f-2fad-58ab-b252-1b45815f02e8/scratchpad/app-checkbox.png`: the Checkbox
  story, variant Value; the four themes side by side per state; cell 3 (normal · hc-light) picked, its `skelter-gallery
  shot` line in the inspect pane; the variant's file in quire.
- `/tmp/claude-0/-home-user-svelters/7c0cf11f-2fad-58ab-b252-1b45815f02e8/scratchpad/app-inspect.png`: the same story
  with cell 5 (hover · light-modern) inspected with `#subject`: core's inspect JSON (applied `forced :hover on 1
  subject`, the cell, the node's AccessKit node, box, computed style, matched rules) under the `inspect` command line.
- Also `xvfb.png` (the windowed app under Xvfb, Button) and `xvfb-edit.png` (Label after the on-disk edit) there.

## CI

All six workflows green on `b4256a78` (the last code change; this record's later commit is docs only):
js `40579894054`, linux-gtk `40579894200`, linux-qt `40579894030`, macos-appkit `40579894045`, skelter `40579894037`
(ubuntu, windows, macos), windows-winui `40579894081`.

Earlier: `f50c64c6`'s skelter windows `cargo test` failed (core's and this crate's tests: gallery roots carried the
`\\?\` verbatim prefix from `std::fs::canonicalize` into module specifiers, so no story loaded). Fixed by routing every
path through `skelter_gallery::canonicalize` (core `8350d0b`, merged) in `b4256a78`.

Not run locally: anything outside `skelter-gallery-app` (its 10 tests and clippy were run after the last code change).

## Requests

To **core (GAL1)**: R1 (`SetupCx::new` / `Extensions::run_setup`) and R2 (`inspect::node_json`, `select_in`) landed
in `5d69749` and are used. Open, nice to have:
- **R3** `Renderer::new` refuses the whole catalog when one story.toml is bad. The app no longer needs a renderer, so
  this only matters to the CLI's own users; recommendation: leave it (`check` wants the strict answer).

To **stories (GAL2)**: none. (Its first push had `views/tooltip` with empty `kinds`, which core's format refuses;
fixed in `29ebdbf5`.)

## Questions (with a recommendation each)

- **Q1** Two `quire-text` copies (skelter-widgets at `35b5063a`, skelter_quire at `ed0665b`). Recommendation: a
  maintenance lane advances skelter-widgets' pin to `ed0665b` (cadatomic already patches it to that rev); out of this
  batch's scope ("don't edit existing crates").
- **Q2** Theme fidelity in side-by-side cells is token-level only (decision 5). Recommendation: accept; the CLI's
  per-document render is the reference for scheme/density-dependent painting.

## Styling notes for the polish lanes (seen in the app; reproduce with the CLI)

- Disabled label looks like the normal one in every theme: `skelter-gallery shot label/plain --state disabled --theme
  dark-modern -o target/gallery/label-disabled.png`.
- Checkbox hover is hard to see in Dark Modern: `skelter-gallery shot checkbox/value --args value=unchecked --state
  hover --theme dark-modern -o target/gallery/checkbox-hover.png`.

## Handoff

Nothing open in the lane. Next steps someone may take: an AccessKit snapshot per cell for inspect's `accesskit`
(the app's window publishes one tree; per-cell subtrees would need the harness's `A11yTree` over the app's document),
and Svelte syntax colouring in the source pane (quire-syntax has no Svelte grammar yet).
