# GAL2: the gallery's stories (batch gallery-b1, lane stories)

Branch `ci/gb1-stories`. Brief: `plans/cloud/gallery-b1/BRIEF.md` and `LANE-stories.md`. Status: in progress.

## Scope

`gallery/` (new): the svelters source package of the skelter component gallery's stories, the format README,
a story for every element kind the app runner registers, the coverage list below, and the polish findings
from rendering them. No crate is touched (BRIEF: stay out of existing crates; `crates/skelter-gallery*` are
the core and app lanes').

## What landed

- `gallery/package.json`: `@svelters/gallery-stories`, `"svelters": { "toolkit": "skelter" }`, depending on
  `svelte` 5.57.2, `@svelters/native-runtime` and `@svelters/lists` (workspace). `SourceConfig::for_path`
  finds the root from any story file, and the modules resolve through `gallery/node_modules`.
- `pnpm-workspace.yaml` lists `gallery` and `pnpm-lock.yaml` has its importer (pnpm install's output, not
  hand-edited), so `pnpm install --frozen-lockfile` links `gallery/node_modules`. These two shared files are
  outside the BRIEF's shared-file list; they are needed for a story to resolve `svelte`. One line each.
- `gallery/README.md`: the format, the axes, the `subject` rule, `disabled`, adding a story in five lines.
- 31 stories, 135 variant files, 216 args combinations (before states, themes, densities and zooms), in
  seven families: controls 15, text 3, views 2, menus 2, dialogs 1, data 3, layout 4.

## Decisions (lane)

- **`kinds` uses registry names**, which are the tags except `combo-box` (kind `select`) and `action` (kind
  `menu-item`) (`svelters_skelter::kinds::CORE_TAGS`). The coverage check compares against a `KindRegistry`.
- **Every variant with an interactive subject declares `disabled = false` and passes `enabled={!disabled}`**,
  so the gallery's `disabled` state renders the kind's real disabled state. Layout-only stories (`box`,
  `grid`, `sides`, `stack`, `separator`, `image`) declare neither and list no `disabled` state.
- **Inline runs take no `anchor`** (it is a Widget prop; runs are `Run`s), so `rich-text` marks its label.
- **`kinds = []`** for the `tooltip` story: it shows the common `tooltip` prop, not a kind. If core's format
  requires a non-empty list, this becomes `kinds = ["button"]`; see Questions.
- **Data views use `@svelters/lists`** (`<ListView>`, `<TreeView>`), as apps do; `table` uses its own
  `range`/`set-rows` protocol directly.
- Every story file passes `svelters-compile --backend skelter --check` (no diagnostics; the check rejects an
  unknown prop or element, verified with a bad file).

## Coverage: every kind the app runner registers

The registry is `svelters_skelter::kinds::register_builtin`: skelter-widgets' tier-0 kinds
(`host::register_tier0`), the tier-1 views it hosts (scroll-area, collapsing-header, table and
table-column, tree-view, virtual-list, select, context-menu, menu-item), `text-edit`, and a placeholder for
every surface tag without one (`window`). The Svelte surface is the 34 tags of
`metadata/semantics/skelter.json`.

| Kind (registry) | Tag | Family | Story | Notes |
|---|---|---|---|---|
| button | `button` | controls | `button` (+ `tooltip`, `dialog`) | look, weight, icon, long, empty, rtl, dense |
| toggle-button | `toggle-button` | controls | `toggle-button` | checked × frame, segmented, long, rtl |
| checkbox | `checkbox` | controls | `checkbox` | three values, no caption, long, rtl, dense |
| radio-button | `radio-button` | controls | `radio-button` | group, single, row, rtl |
| switch | `switch` | controls | `switch` | value, no caption, long, rtl |
| slider | `slider` | controls | `slider` | value, no value box, ticks, marks, caption, vertical, inverted |
| progress-bar | `progress-bar` | controls | `progress-bar` | fraction, percentage, text, format, indeterminate, vertical, rtl |
| spinner | `spinner` | controls | `spinner` | size, row height, stopped, beside a label |
| hyperlink | `hyperlink` | controls | `hyperlink` | text, url only, long, rtl, in a row |
| label | `label` | controls | `label` | plain, weight, wrap modes, elide, selection, empty, rtl, form |
| span, b, i, code, a | `span` `b` `i` `code` `a` | controls | `rich-text` | every run, nested, wrapped, rtl |
| icon | `icon` | controls | `icon` | glyph sizes, icon button, registry key, imported SVG, toolbar |
| image | `image` | controls | `image` | fit, natural size, missing source |
| separator | `separator` | controls | `separator` | horizontal, vertical |
| collapsing-header | `collapsing-header` | controls | `collapsing-header` | open, `end` slot, accordion, long, rtl |
| frame | `frame` | controls | `frame` (+ layout stories) | seven frame styles, caption, checkable, rtl |
| text-edit | `text-edit` | text | `text-edit`, `password`, `multiline` | placeholder, read-only, invalid, char limit, long, rtl, number, dense; password; multi-line wrap/overflow |
| scroll-area | `scroll-area` | views | `scroll-area` | three directions, scrolled, fits, empty |
| select | `combo-box` | menus | `combo-box` | selected and hint, long option, open, rtl, dense |
| context-menu | `context-menu` | menus | `context-menu` | every item type; opens on a secondary click only (see findings) |
| menu-item | `action` | menus | `context-menu` | plain, shortcut, disabled, check, radio, long |
| box | `box` | layout | `box` | direction, justify, align, wrap, grow |
| stack | `stack` | layout | `stack` | by index, by name, homogeneous |
| grid | `grid` | layout | `grid` | templates, justify, form |
| sides | `sides` | layout | `sides` | two, groups, crowded |
| table | `table` | data | `table` | striped, sorted + multi-selection, overflow + sticky, empty, rtl |
| table-column | `table-column` | data | `table` | align, header-align, elide, widths |
| tree-view | `tree-view` (via `<TreeView>`) | data | `tree-view` | expanded × decorated, collapsed, long labels, rtl |
| virtual-list | `virtual-list` (via `<ListView>`) | data | `virtual-list` | plain + selection, rich rows, empty, rtl |
| window | `window` | — | none | Skipped: the top-level viewport (a placeholder kind on skelter). A gallery cell is mounted into a slot of the renderer's own root, so a story cannot open a window; `dialog` shows `frame-style="window"` instead. |

### Kinds skelter-widgets has that Svelte cannot mount

These exist in `crates/skelter-widgets/src` but are neither registered by `register_builtin` nor tags in
`skelter.json`, so `svelters-compile --backend skelter` rejects them (`unknown_element`) and the runner has no
kind for them. No story; each is a request (below).

| Kind (skelter-widgets) | Source | Family | Notes |
|---|---|---|---|
| split view | `views/split.rs` | views | registered only by `views::register_all` (skelter-test's own tests) |
| panel | `views/panel.rs` | views | same |
| popup | `views/popup.rs` | views | same; select's popup is its internal part |
| overlay | `views/overlay.rs` | views | a helper, not a registered kind |
| tooltip | `views/tooltip.rs` | views | same as split; the common `tooltip` prop is covered by the `tooltip` story |
| toast host | `views/toast.rs` | views | same as split |
| status bar | `views/status_bar.rs` | views | same; the common `status-tip`/`status-section` props have no bar to show in |
| modal | `views/modal.rs` | dialogs | same; `dialog` shows dialog content in a `frame` |
| inner window | `views/inner_window.rs` | dialogs | same |
| dialog buttons | `views/dialog_buttons.rs` | dialogs | same; `dialog-role` on buttons compiles but nothing orders them |
| menu bar, menu, menu separator | `views/menu.rs` | menus | registered by `register_all` only; `context-menu`'s spec accepts `Menu` and `MenuSeparator` children but there are no tags |
| scrollbar | `views/scroll.rs` | views | an internal part of scroll-area: skipped, not a request |
| tab view, tab | `tabs/kinds.rs` | tabs | `tabs::kinds::register` is not called by the runner |
| colour picker, date picker, calendar | `pickers/kinds.rs` | pickers | `pickers::kinds::register` is not called by the runner |
| number and quantity fields | `text/number.rs` | text | an editing helper of `text-edit`, not a kind: `text-edit/number` shows a narrow field |
| native dialogs | `dialogs/` | dialogs | platform file and message dialogs, not element kinds |

## Requests (to the coordinator; the smallest change each)

1. **Register the view, tab and picker kinds for Svelte** (svelters-skelter, not this batch): call
   `skelter_widgets::views::register_all` (minus what `register_hosted` already registers),
   `tabs::kinds::register` and `pickers::kinds::register` from `register_hosted`, and add their surface rows
   (tags and props) to `surface/tier1.rs`, then regenerate `metadata/semantics/skelter.json`. Stories for
   split, panel, popup, tooltip, toast, status bar, modal, inner window, dialog buttons, menu bar, tabs and the
   three pickers follow in a day once they mount.
2. **A way to show a context menu open** for a shot: an `open` prop on `context-menu` (as `combo-box` has),
   or a gallery state `open` that calls its `popup` method. Until then the story renders the owner only.
3. **`asset://` sources on skelter**: `image` and `icon`'s `source` take a file path or a registered name in
   skelter-widgets (`controls/image.rs`), not the `asset://` ref an import gives. svelters-skelter should
   resolve the ref through the host's `Assets` (as svelters-egui's loader does). Confirmed or refuted by
   rendering (below).
4. **A demo icon set in the gallery host** (core lane): the runner registers no icons and no icon font, so
   `icon name=…` and `button icon=…` show nothing. `Extensions::setup` could register a few registry icons.

## Questions

- `kinds = []` (the `tooltip` story): allowed by core's format? Recommendation: allow it for stories about a
  common prop, and have `coverage` ignore them.

## Polish findings

Pending: core's `shot` and `check` had not landed when the stories were written.

## Tests and CI

Not run yet (no renderer). Compile check: `svelters-compile --backend skelter --check` on every story file,
no diagnostics.

## Handoff

Next: merge `origin/ci/gb1-core` once `shot` and `check` land; run `skelter-gallery check`, fix every story
it fails; render one sheet per family; fill "Polish findings".
