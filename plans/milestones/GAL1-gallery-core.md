# GAL1: the gallery library, headless renderer and CLI (lane core, batch gallery-b1)

Branch `ci/gb1-core`. Brief: `plans/cloud/gallery-b1/BRIEF.md` and `LANE-core.md`. Base: `master` `01e8f67`.
Status: **items 1 to 7 done.** CI: see the end of this record.

## Scope

`crates/skelter-gallery` (new), with its rows in the root `Cargo.toml` and `Cargo.lock`, and `docs/skelter/gallery.md`.
This lane edited no existing crate, `gallery/`, or `crates/skelter-gallery-app`.

## What landed

| Item | Commit | What |
|---|---|---|
| 1 data model | `479e35f` | `Story`, `Variant`, `State`, `Theme`, `Cell`, `StoryId`/`VariantId`; `story.toml` parsing that reports every error with its file and field (`variant[1].args.size`); `discover`/`discover_with`; `Catalog::{stories, get, family, coverage}`; `Matrix::new(story, &Axes)` in the stable order; `Extensions` |
| 2 shot | `16ce5f2` | `Renderer` (one QuickJS engine, document, headless window and GPU device for every cell; each variant file is its own embed entry; a cell change remounts); `shot`; `png`/`save_png`; CLI `list` and `shot` |
| 3 sheet | `6ee4970` | `Renderer::sheet` → `Sheet { image, cells: [SheetCell { cell, label, rect, header, applied, warnings, error }] }`; labels drawn with skelter-text's layout and rasteriser; CLI `sheet <story> \| --family \| --all` with `--index` |
| 4 inspect | `88eaf73` | `Renderer::inspect(cell, selector)`: kind, anchor, boxes, computed style that differs from CSS's initial values, matched rules (selector, sheet, specificity, source file and line), forced and own state, AccessKit node; CLI `inspect` |
| 5 list, check | `559327e` | `Renderer::check` → `CheckReport` (problem kinds `story`, `load`, `runtime`, `no-subject`, `empty`, `render`); a load error carries the compiler's full diagnostics; CLI `check` |
| 6 extensions | `ad8e04e` | a host's kinds (registered after the built-ins, `Kinds::with_app`, and handed to the compiler as a spec fragment), story roots, families, setup hooks, token sets; `cli_main(ext)`, `cli::run(ext, argv)`, `cli::registry(ext)` |
| 7 docs | (this push) | `docs/skelter/gallery.md`: the agent loop, each command with an example, the JSON shapes, how a host adds itself |

## Public API (additions to the brief's contract; nothing renamed)

- `model`: the brief's types. In addition: `Theme` is an enum (`DarkModern`, `LightModern`, `HcDark`, `HcLight`,
  `Extra(name)`, `#[non_exhaustive]`); `Story.root` (the absolute gallery root) next to the root-relative `dir` and
  `Variant.file`; `Variant.props` (the names the component destructures from `$props()`) and `takes_disabled()`;
  `Cell::new(story, variant)`, `Cell::label()`, `density_named`, `FAMILIES`.
- `Axes { variants, args, states, themes, densities, zooms }`. `Default` is the brief's default axes.
- `Renderer`: `new(&Extensions)`, `shot`, `sheet(&Matrix, SheetLayout)`, `inspect`, `check`, plus `mount(&Cell) ->
  Mounted` (no draw), `select(selector)`, `catalog()`, `rediscover()`, `set_base_css()`, `backend()`, `embed()`,
  `harness()`, `fonts()`, `current()`. `Error` has the variants `Stories`, `Request`, `Load`, `Runtime`, `Setup`, `Render`.
- `Extensions` builder: `story_root`, `kinds`, `on_setup`, `theme`, `family`; `theme_named`, `theme_names`.
  **Deviation:** `setup` is `Vec<Rc<dyn Fn(&mut SetupCx)>>` (the `SetupHook` alias), not `Box`: the renderer keeps the
  hooks, and `Renderer::new` takes `&Extensions`. Added `families` (a host's stories may use families beyond the ten).

## Decisions (inside the agreed design)

- **One entry per variant file**, loaded once per renderer. A file that doesn't compile fails only its own cells, and
  `shot` compiles only what it renders. QuickJS caches modules by path, so the Svelte runtime is loaded once. One
  consequence: a renderer does not see edits to a `.svelte` file it has already loaded. The CLI starts a fresh
  process per command, so this never shows there. The app lane needs a reload path (question Q1).
- **Theme** = a theme-origin sheet: `Theme::css()` (tokens) plus `base.css`, named `gallery theme <name> + base.css`.
  The renderer also sets the theme service (`skelter_widgets::host::set_theme`), `prefers-contrast`, and **reduced
  motion**, so transitions jump and a cell settles at its end state. The CLI reads `base.css` from
  `crates/skelter-widgets/src/theme/base.css` when it runs inside the checkout, so the agent loop needs no rebuild for
  a `base.css` edit.
- **Zoom** is the window's scale factor. A cell's image is (story size × zoom) device pixels.
- **The cell's slot** fills the window on `var(--sv-bg)`. Headless, the `window` kind paints no background, so without
  this every theme's cells sat on white.
- **Disabled:** the prop when the variant destructures `disabled` from `$props()` (read from its source, best effort);
  otherwise the forced bit, and the label says `(forced bit)`.
- **Settling:** each step runs the embed's logic, then one harness step. A cell waits at most 120 frames for both the
  embed and the harness to settle. After that it is drawn anyway, with a `not settled` warning (a spinner).
- **Fonts:** skelter-text's bundled test fonts (`FontConfig::testing()`), so shots are hermetic. Scripts these fonts
  don't cover (Arabic, Hebrew) draw as boxes (question Q3).
- `inspect` prints JSON with or without `--json`, because the JSON is also the readable form.

## Tests

`cargo nextest run -p skelter-gallery` (or `cargo test`). GPU tests need `SKELTER_GPU_TESTS=1` (skelter's rule: CI sets
it), with `WGPU_BACKEND=vulkan` on Linux. `GALLERY_SHOTS=<dir>` writes the test images for a look. No test compares an
image with a stored one.

| File | Tests | What |
|---|---|---|
| `tests/model.rs` | 6 | discovery order and fields; every error names its file and field; duplicate ids across roots; coverage against `register_builtin`; matrix order and counts (100 cells: 5 rows × 5 states × 4 themes); `$props()` parsing |
| `tests/render.rs` | 5 (GPU) | a shot is the cell's size (×2 at zoom 2), and the same cell twice gives the same bytes; `disabled` goes through the prop (the node is really disabled, nothing forced) and is forced when there is no prop; a forced hover is on the subject and is gone on the next cell; a sheet places and indexes every cell and is deterministic; `inspect` shows the `:hover` rule only under the forced state, with its `.svelte` line, the computed background and the AccessKit node |
| `tests/check.rs` | 2 (1 GPU) | `check` names `broken/syntax` (load, with its file) and `nosubject/bare` (no-subject), and passes the good fixture; CLI exit codes (0, 1 for a story.toml error, 2 for an unknown command, story, theme or state) |
| `tests/extension.rs` | 2 (1 GPU) | a host kind, story root, family, setup hook and token set: the kind renders (its pixels), `inspect` finds it, the setup hook ran, the host theme's `--sv-bg` is the cell's background, `check` passes |

Locally (4-core container, lavapipe): all 15 pass, last run after the clippy fixes.

**Speed** (cloud, not a gate; debug build, lavapipe, 4 cores): a 40-cell sheet (fixture button, 4 args × 5 states ×
2 themes) renders at **17 cells/s**. On the stories lane's 31 stories (`origin/ci/gb1-stories` at `29ebdbf5`), `sheet
--all --themes dark-modern,hc-light` rendered about 1,500 cells in about 70 s, at 15 to 28 cells/s per story, with no
errors; `check` passed all 135 variants in 5 s. One engine per run is enough at this speed. If more is needed later,
the next steps are a release build of the binary and drawing only once per cell (each cell draws one frame and reads
it back from the CPU rasteriser).

## Styling bugs the stories show (for the polish batch, not fixed here)

Each was seen on the stories lane's sheets. Reproduce with `skelter-gallery sheet <story> --themes dark-modern,hc-light`
from a checkout with `gallery/`:

1. **`focus-visible` draws no focus ring** on button and checkbox (the two sheets checked): the cell looks like
   `normal`. The other kinds' sheets are worth the same look. Example: `skelter-gallery shot button --state focus-visible`.
2. **`disabled` looks like `normal`** for button and checkbox in every theme, although the node is disabled for real
   (`inspect … --state disabled` shows `state.own: ["disabled"]`). Example: `skelter-gallery shot checkbox --state disabled`.
3. RTL text draws as boxes. This is the gallery's fonts, not a kind bug (question Q3).

## Requests (smallest change; none made in this batch)

- **R1 (skelter-core / svelters-style): a rule's layer in `MatchedRuleInfo`.** Add `layer: Option<String>`, filled
  from the stylist's layer of the rule. `inspect` prints `"layer": null` until then.
- **R2 (svelters-compiler): inline `style=` on an app kind.** A host kind's `style="width: 40px"` fails with
  `style_not_applicable` because the style table lists only the built-in kinds. Smallest change: treat app kinds from a
  spec fragment like `Widget` (they all extend it) when checking inline style. Workaround: a class in `<style>`.
- **R3 (svelters-host): module-load messages are cut short** (`…error[js_parse_error]: Un`). The gallery works around
  it with `SourceProvider::check`. The message passed through QuickJS's load error seems to be truncated.

## Questions (each with a recommendation)

- **Q1 (app lane): reloading an edited variant.** A renderer keeps the modules it has loaded, so an edited `.svelte`
  file needs a new `Renderer` (not timed), or the host's hot-reload path (`Host::apply_changes` for the entry's
  provider). *Recommendation:* the app recreates the `Renderer` on save for now; core adds `Renderer::reload(paths)`
  over `apply_changes` in a later item if that proves too slow.
- **Q2: host themes are Dark Modern plus overrides.** `Extensions::themes` has no base scheme. *Recommendation:* keep
  it, and add `Extensions::theme_on(name, base: Theme, tokens)` when cadatomic needs a light set.
- **Q3: fonts.** Hermetic test fonts make shots identical on every machine, but they lack the CJK, Arabic and Hebrew
  glyphs. *Recommendation:* keep them as the default, and add a `--system-fonts` flag in the polish batch for looking
  at RTL and CJK samples.

## Handoff

- Code: `crates/skelter-gallery/src/` has `model`, `catalog`, `matrix`, `ext`, `render`, `sheet`, `inspect`, `check`,
  `cli`, and `bin/skelter-gallery.rs`. Fixtures are in `tests/fixtures/` (`gallery`, `bad`, `bad-toml`, `ext`).
- The stories lane's `gallery/` works unchanged with this code (`check` passes all 135 variants).
- Not run locally: the full workspace tests and the other four workflows (GitHub CI is the gate).

## CI

(filled in after the push)
