# Lane core: the gallery library, headless renderer and CLI (GAL1)

Branch `ci/gb1-core`; record `plans/milestones/GAL1-gallery-core.md`. Read `BRIEF.md` first: the story format,
the library API and the CLI there are your spec. You own `crates/skelter-gallery/` (new) and its rows in the root
`Cargo.toml` and `Cargo.lock`. You don't own `gallery/` (stories lane) or `crates/skelter-gallery-app/` (app lane).

What exists to build on: `svelters-skelter-run --entry <dir> --dev` (`crates/svelters-skelter/src/bin/`) loads an
app from source; `SourceConfig::for_path` / `SourceProvider` and its `overlay()` and `Assets::watch_files`
(`crates/svelters-modules`); the embed (`svelters_skelter::embed`, many roots; package.json `svelters.embed`
tables); `svelters_skelter::app::{SkelterDocument, install_widget_services}`; `skelter_test::{Harness, render,
Image}` (headless platform, virtual clock, lavapipe pixels; `crates/svelters-skelter/tests/run.rs` and
`common/` show the mount); `designer.rs` (`force_state`, `set_override`, `matched_rules`, `by_source`,
`hit_stack`); `skelter_widgets::theme` (the four VS Code sets, densities, `TokenMap`). cadatomic's
`crates/cadatomic-ui/src/skelter/testing.rs` and `crates/cadatomic-app/tests/gallery.rs` mount a Svelte page in
four themes headlessly; read them for shape (`add_repo` `akiselev/cadatomic`, read only), don't depend on them.

Items, in order, pushing after each:

1. **Data model** (first push, about an hour in): the crate, `Story`/`Variant`/`State`/`Cell`/`Matrix`,
   `story.toml` parsing with errors that name file and field, `discover`, matrix expansion in a stable order
   (story, variant, args in key order, state, theme, density, zoom), `Extensions`. Unit tests on a fixture
   story under `crates/skelter-gallery/tests/fixtures/`.
2. **`shot`**: mount one variant from source into a headless window of the story's size × zoom, apply the
   theme and density, pass args (and `disabled`), force the state on the `subject` anchors, run to settled on
   the virtual clock, render to PNG. One `Renderer` keeps the JS host and module cache across cells: a cell
   change remounts, never restarts the engine. Measure cells per second on a sheet of 40 (record it, "cloud,
   not a gate"); if one engine per run is not enough, record why and what you'd change.
3. **`sheet`**: rows = variant × args × state, column groups = theme (then zoom, density), each cell labelled
   with its ids in a small header, one PNG plus an `--index` JSON of cell rects and ids. `--family` and `--all`
   render one sheet per story into a directory with an `index.json`.
4. **`inspect`**: for a selector in a cell, JSON with the node's kind, anchor, layout box, computed style
   (every property that differs from the kind's initial value), matched rules (selector, sheet name, layer,
   source file and line when known), the forced state bits, and the AccessKit node (role, name, states).
5. **`list` and `check`**: `list --json` with stories, variants, kinds and coverage (registered kinds with no
   story; story kinds the registry lacks). `check` renders one cell of every variant and fails on compile errors,
   runtime errors, a missing `subject` when states are declared, or an empty render.
6. **Extensions and `cli_main`**: a host binary adds story roots, kinds, setup hooks and token sets. Test it with
   a test-only extension that registers a kind and a story root under `tests/fixtures/`.
7. **Docs**: `docs/skelter/gallery.md`: the agent loop (edit the variant file or `base.css` → `sheet` → look →
   repeat), each command with an example, the JSON schemas, and how a host adds itself.

Tests: discovery and errors; matrix order and counts; a shot is the right size and deterministic within a run
(the same cell twice gives the same bytes); a forced hover changes the subject's matched rules in `inspect`;
`disabled` goes through the prop; `check` fails on a broken fixture and names it; the extension's kind renders.
No test compares against a stored image (decision 2). GPU tests follow skelter's rule for where an adapter exists.

Refuse rather than guess: a state the style engine has no bit for (record it), a theme name outside the four,
any change to an existing crate (a request in your record instead).
