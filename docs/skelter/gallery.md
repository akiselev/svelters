# The component gallery (`skelter-gallery`)

`crates/skelter-gallery` renders every skelter kind's stories headlessly in every state, theme, density and zoom,
as PNGs and JSON. Everything the gallery app shows can be produced from the command line, so an agent can change a
component and look at the result in seconds. The story format is `gallery/README.md`. The batch brief is
`plans/cloud/gallery-b1/BRIEF.md`.

Contact sheets are for looking at. They are not goldens: no test compares them to a stored image.

## The agent loop

1. Edit a variant file (`gallery/stories/<family>/<id>/<variant>.svelte`), a kind's look in
   `crates/skelter-widgets/src/theme/base.css`, or a story's `story.toml`.
2. Render the story: `skelter-gallery sheet button --themes dark-modern,hc-light -o /tmp/button.png`.
3. Look at the PNG. To ask why a cell looks the way it does, run `skelter-gallery inspect button/primary '#subject' --state hover`
   (matched rules, computed style, the forced state, the AccessKit node).
4. Repeat. Before pushing, run `skelter-gallery check`.

Each command starts a fresh process, so it always reads the files as they are on disk. When the gallery runs inside
the svelters checkout, it reads `base.css` from its source file, so an edit to `base.css` shows without a rebuild
(`--base-css <file>` names another file). Build the binary once with `cargo build -p skelter-gallery`. It is
`target/debug/skelter-gallery`. Pixels need a wgpu adapter: lavapipe on Linux (`WGPU_BACKEND=vulkan`, the packages
`.github/workflows/skelter.yml` installs), WARP on Windows, Metal on macOS.

## Commands

The global flags come first: `--root <dir>` adds a gallery root, and may be given more than once. With no
`--root`, the gallery uses `gallery/` in the current directory or the nearest directory above it that has one.
`--base-css <file>` is described above. `--system-fonts` is described under "How a cell is rendered".

Exit codes: `0` ok, `1` a story failed (a compile, load or runtime error, or a check problem), `2` bad arguments
(an unknown story, variant, state, theme or flag). Outputs go where `-o` says. The default is `target/gallery/` in
the workspace (ignored by git).

Cell flags pick one cell: `--args k=v,..` (values are JSON when they parse, else strings), `--state <s>`,
`--theme <t>`, `--density <d>`, `--zoom <z>`. A cell's defaults are each arg's first value, `normal`,
`dark-modern`, `default`, and zoom `1`.

Axis flags pick a matrix: `--variants a,b`, `--args k=v,..` (fixes those args), `--states s,..`, `--themes t,..`,
`--densities d,..`, `--zooms z,..`. By default a matrix has every variant, every args combination, the story's states,
all four themes, the default density, and zoom 1.

States: `normal hover active focus focus-visible checked disabled`. Themes: `dark-modern light-modern hc-dark
hc-light`, plus a host's. Densities: `compact default comfortable`.

### `list [--json]`

```sh
skelter-gallery list --json
```

```json
{
  "roots": ["…/gallery"],
  "stories": [{ "id": "button", "title": "Button", "family": "controls", "kinds": ["button"], "size": [320, 96],
                "states": ["normal", "hover", "active", "focus-visible", "disabled"], "notes": null,
                "dir": "stories/controls/button", "manifest": "…/story.toml",
                "variants": [{ "id": "primary", "title": "Primary", "file": "stories/controls/button/primary.svelte",
                               "args": { "size": ["small", "normal"] }, "props": ["size", "disabled"] }] }],
  "kinds": ["box", "button", "…"],
  "coverage": { "missing": ["…"], "unknown": [["story", "kind"]], "covered": ["…"] },
  "themes": ["dark-modern", "light-modern", "hc-dark", "hc-light"],
  "densities": ["compact", "default", "comfortable"],
  "states": ["normal", "hover", "…"]
}
```

`kinds` comes from the registry the gallery renders with (`svelters_skelter::kinds::register_builtin` and the host's
kinds), not from a hand-written list. `coverage.missing` lists registered kinds that no story names.
`coverage.unknown` lists the kinds stories name that the registry lacks (a story kind is matched under its core name:
`combo-box` is skelter-core's `select`). `props` lists the names the variant destructures from `$props()`, read from
its source.

### `shot <story>[/<variant>] [cell flags] [-o out.png] [--json]`

```sh
skelter-gallery shot button/primary --args size=large --state hover --theme light-modern --zoom 2 -o /tmp/b.png --json
```

```json
{ "file": "/tmp/b.png", "label": "primary size=large hover light-modern x2",
  "cell": { "story": "button", "variant": "primary", "args": { "size": "large" }, "state": "hover",
            "theme": "light-modern", "density": "default", "zoom": 2.0 },
  "size": [640, 192], "applied": "forced :hover on 1 subject", "warnings": [] }
```

The image is the story's `size` × zoom, in device pixels. `applied` says how the state was applied:

| `applied` | Meaning |
|---|---|
| `forced :hover on 1 subject` | The state's bits were forced on the subject anchors. |
| `forced :hover on the root (no subject anchor)` | The variant has no `subject` anchor, so the bits were forced on its top-level nodes. |
| `disabled prop` | The variant takes a `disabled` prop, so it rendered with `disabled = true`: its real disabled state and accessibility state. |
| `forced :disabled on … (the variant has no disabled prop)` | The variant takes no `disabled` prop, so only the style bit was forced. |

`warnings` lists what the cell logged without failing: console warnings and errors, handler exceptions, and "not
settled" (an animation or timer that kept going after 120 frames; the cell is drawn anyway).

### `sheet (<story> | --family <f> | --all) [axis flags] [-o out] [--index index.json] [--json]`

```sh
skelter-gallery sheet button --states normal,hover,disabled -o /tmp/button.png --index /tmp/button.json
skelter-gallery sheet --family controls --themes dark-modern,hc-light -o /tmp/controls/
skelter-gallery sheet --all -o /tmp/all/
```

On a sheet, each row is a variant × args combination × state, in the matrix's order. Columns are grouped by theme,
then zoom, then density. Each cell has a two-line header: `variant args` and `state theme [density] [xZ]`. A disabled
cell whose bit was forced says `(forced bit)`. A cell that fails is drawn as its error and listed with it; the other
cells still render.

For one story, `-o` names the PNG. With `--family` or `--all`, `-o` names a directory: the gallery writes one sheet
per story (`<id>.png`) and an `index.json`. With `--family` or `--all`, `--states` keeps the states each story lists
among those asked, and skips stories that list none of them. Speed is reported on stderr (`N cells in S s`).

The index of one sheet, also printed with `--json` and written with `--index`:

```json
{ "story": "button", "title": "Button", "file": "/tmp/button.png", "size": [1032, 1544], "seconds": 1.9,
  "cells": [{ "cell": { "story": "button", "variant": "primary", "args": {}, "state": "normal", "theme": "dark-modern",
                        "density": "default", "zoom": 1.0 },
              "label": "primary / normal dark-modern",
              "rect":   { "x": 8, "y": 54, "w": 320, "h": 96 },
              "header": { "x": 8, "y": 22, "w": 320, "h": 32 },
              "applied": null, "warnings": [], "error": null }] }
```

`rect` is where the cell's image sits in the sheet, in pixels. `header` is the label band above it. A directory's
`index.json` is `{ "sheets": [<index>, …], "cells": N, "seconds": S }`.

### `inspect <story>/<variant> <selector> [cell flags] [--json]`

A selector is an anchor (`#subject`) or a kind name (`button`, or a core name such as `select`), never visible
text. The command prints JSON, with or without `--json`:

```json
{ "cell": { … }, "label": "…", "selector": "#subject", "applied": "forced :hover on 1 subject", "warnings": [],
  "nodes": [{
    "handle": 4, "kind": "button", "core_kind": "button", "anchor": "subject",
    "box": { "x": 8, "y": 8, "w": 144, "h": 28 }, "visible_box": { … }, "layout_box": { … },
    "computed": { "background": "#ff0000", "color": "#cccccc", "border-width": "1px 1px 1px 1px", … },
    "matched_rules": [{ "selector": ".b.svelte-fy749k:hover", "sheet": "app/stories/controls/button/primary.svelte",
                        "layer": null, "specificity": 3072, "sheet_line": 3,
                        "source": { "file": "…/primary.svelte", "line": 11, "style_offset": 29 },
                        "declarations": [{ "name": "background-color", "value": "#ff0000", "important": false,
                                           "winner": true, "source": { … } }] }],
    "state": { "forced": ["hover"], "own": [] },
    "accesskit": { "role": "Button", "name": "Run small", "value": null, "author_id": "subject", "disabled": false,
                   "hidden": false, "toggled": null, "bounds": { … } } }] }
```

- `box` is the border box in window points from the last frame. `layout_box` is relative to the parent.
- `computed` holds every property that differs from CSS's initial value.
- `matched_rules` lists rules in cascade order. Theme rules come from the sheet named `gallery theme <name> + base.css`
  (`sheet_line` is the line in that sheet). Component rules carry the `.svelte` file and line of their `<style>` rule.
  `layer` is `null` until skelter-core reports it.
- `state.forced` lists the bits the gallery forced. `state.own` lists the node's real state (`disabled` through the prop).
- `accesskit` is the node skelter published, or `null` for a node it prunes (a plain box).

### `check [--json]`

```sh
skelter-gallery check --json
```

```json
{ "ok": false, "stories": 31, "variants": 135, "warnings": ["button/primary: console.warn: …"],
  "problems": [{ "kind": "load", "story": "broken", "variant": "syntax", "file": "…/syntax.svelte",
                 "message": "does not load: …/syntax.svelte:2:11: Unexpected token [js_parse_error]" }] }
```

`check` renders one cell of every variant: its first args, `normal`, and Dark Modern. Problem kinds:

| Kind | Meaning |
|---|---|
| `story` | A `story.toml` error, naming the file and the field. |
| `load` | A compile error (the compiler's diagnostics, with line and column) or an exception while loading. |
| `runtime` | The variant threw while mounting or rendering, or reported an error. |
| `no-subject` | The story shows states other than `normal`, but no node has `anchor="subject"`. |
| `empty` | Nothing with a size was mounted. |
| `render` | No pixels (no adapter). |

## How a cell is rendered

One `Renderer` keeps one QuickJS engine, one skelter document and one headless window (skelter-test's `Harness` on the
virtual clock, drawn by skelter-render) for all its cells. Each variant file is its own embed entry, compiled live by
svelters' compiler (dev mode, so rules carry their source) and loaded once per run. A file that doesn't compile fails
only its own cells. When the cell changes, the variant is remounted in the window's slot. For each cell, the renderer:

1. Installs the theme and density as a theme-origin sheet: the theme's token sheet (`Theme::css`) plus `base.css`.
   It also sets the theme service, `prefers-contrast`, and reduced motion, so transitions jump to their end.
2. Sizes the window to the story's size × zoom (zoom is the window's scale factor).
3. Runs the host's setup hooks.
4. Mounts the variant with the args as props (plus `disabled = true` for the `disabled` state when the variant takes
   that prop).
5. Steps the embed's logic and the harness until both are settled (at most 120 frames).
6. Forces the state's bits on the `subject` anchors (`SkelterBackend::force_state`), settles again, and draws.

**Fonts** are bundled and system fonts are off (hermetic: a shot is the same on every machine). The set
(`crates/skelter-gallery/fonts/README.md`, OFL subsets) is the theme's own families, Inter (300, 400, 600, 700) and
JetBrains Mono, plus Noto fallbacks: Arabic, Hebrew, a CJK sample (common UI words in Chinese, Japanese and Korean,
`fonts/cjk-sample.txt`), symbols (★ ⚙ ◀ ✂ ⌘ ⋯) and three emoji. A character outside the set draws as a box;
`--system-fonts` adds the OS's fonts behind the bundled ones (for looking only: not the same on every machine). The
same cell rendered twice in one run gives the same bytes.

**Icons and assets.** The gallery registers a demo icon set on the process's registry (`skelter_gallery::icons::DEMO`:
`file.new`, `file.open`, `file.save`, `edit.copy`, `edit.cut`, `edit.paste`, `edit.undo`, `edit.redo`, `search`,
`settings`, `close`, `add`, `remove`, `check`, `run.start`, `run.stop`, `refresh`, `folder`, `warning`, `info`), so
`<icon name="file.save">` and `<button icon="file.save">` draw. A key a host registered first is kept. A story's
image files (`.svg`, `.png` next to its variants) are registered as image sources under the `asset://app/…` refs their
imports evaluate to, so `import gear from './gear.svg'` works as an `image` or `icon` `source`.

## Adding a host (cadatomic-gallery)

A host's binary is three lines. Its stories live in its own gallery root (the same format), and its kinds register
after the built-ins:

```rust
use skelter_gallery::{Extensions, cli_main};

fn main() -> std::process::ExitCode {
    cli_main(
        Extensions::new()
            .story_root(concat!(env!("CARGO_MANIFEST_DIR"), "/gallery"))
            .family("cad")                                    // families beyond the built-in ten
            .kinds(|reg| cadatomic_ui::skelter::register(reg).map_err(|e| e.to_string()))
            .on_setup(|cx| { /* grid sources, the pick board: cx.with_doc(|d| …), cx.cell, cx.story */ })
            .theme("cad-dark", tokens),                       // a TokenMap of overrides on Dark Modern
    )
}
```

- **Kinds:** the host's kinds are registered after the built-in ones. They reach JS through
  `svelters_skelter::Kinds::with_app`, and reach the compiler as a spec fragment
  (`svelters_skelter::spec::fragment`) that the gallery writes to a temporary file.
- **Setup hooks** run before every mount. They get the cell, its story and the backend.
- **Themes:** a host theme is named on the command line like the four built-ins. It is Dark Modern with the host's
  tokens as overrides.
- **Library use:** `Renderer::new(&ext)`, then `shot`, `sheet`, `inspect`, `check` and `mount` on it. `Renderer::catalog`
  lists the stories, and `discover`/`Matrix::new` work without a renderer.

A host kind's inline `style="width: …"` is refused by the compiler: its style table lists only the built-in kinds. Use
a class in the story's `<style>` until that changes. The kind's `record` must paint its own box (`cx.draw_box()`).

## The interactive gallery (`skelter-gallery-app`)

The same stories in one window, for a person: browse, compare themes side by side, edit a variant and see it live,
click a node to inspect it, and copy the CLI line that reproduces a cell for an agent.

```sh
cargo run -p skelter-gallery-app                    # the first story of gallery/ (found from the working directory up)
cargo run -p skelter-gallery-app -- button/look     # opens on a story or a variant
# in a container: under Xvfb (lavapipe draws), or headless as a PNG of the whole window
WGPU_BACKEND=vulkan xvfb-run -a -s "-screen 0 1600x1000x24" cargo run -p skelter-gallery-app -- label
cargo run -p skelter-gallery-app -- checkbox/value --shot target/gallery/app.png --size 1500x900 --pick 5 --inspect '#subject'
```

Flags: `--base-css <file>` (as the CLI's), `--shot <png>` with `--size <w>x<h>`, `--pick <cell index>` and
`--inspect <selector>` (needs `--pick`). Run `pnpm install` first.

- **Sidebar:** a filter, families → stories → the selected story's variants, `story.toml` errors (one bad file hides
  only itself), and the registered kinds no story covers ("No story yet").
- **Canvas:** one row per variant × args combination, one cell per state × theme. Each cell is its own Svelte root in
  a slot of its own, themed by a class that redeclares that theme's token sheet, so the four themes sit side by side
  while the gallery's own chrome stays Dark Modern. What a kind reads from the theme *service* rather than its tokens
  (scheme or density outside tokens) follows the chrome; the CLI renders each theme in its own document. A cell's
  caption picks it.
- **Controls:** themes, states, density, zoom (the window's scale: the whole window zooms, as `--zoom` scales a shot),
  one select per arg key, Inspect, Copy shot command, Copy inspect command.
- **Source pane:** the picked variant's file in quire. Typing previews at once without saving (the module loader's
  overlay and the host's in-place update); Ctrl+S (Cmd+S on macOS) or Save writes the file; Revert drops the edits.
  An edit on disk (an agent's, or `crates/skelter-widgets/src/theme/base.css`) reloads through the host's file
  watcher. A compile error keeps the last good render, shows `file:line:col` in the cell and over the editor, and
  marks the line.
- **Inspect:** with Inspect on, a click in a cell picks the nearest `#anchor` at or above the node (else its kind
  name) and shows what `skelter-gallery inspect` prints for its nodes (`skelter_gallery::inspect::node_json` on the
  live cell, so unsaved edits are included; `accesskit` is `null` there), with the matching `inspect` command line.
- **Hosts:** `skelter_gallery_app::run(Extensions)` takes the same `Extensions` as `cli_main`: the host's story roots,
  families, kinds (compiled through the same spec fragment), setup hooks (before each cell mounts) and token sets.
- **Anchors** for tests and automation: `story:<id>`, `variant:<story>/<id>`, `theme:<name>`, `state:<name>`,
  `density`, `zoom`, `arg:<key>`, `inspect-toggle`, `copy-shot`, `copy-inspect`, `filter`, `source`, `save`, `revert`,
  `source-errors`, `canvas`, `inspect`, `cell:<i>` (a cell's frame).
