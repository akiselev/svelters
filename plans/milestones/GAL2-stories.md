# GAL2: the gallery's stories (batch gallery-b1, lane stories)

Branch `ci/gb1-stories`. Brief: `plans/cloud/gallery-b1/BRIEF.md` and `LANE-stories.md`. Status: done.

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
- **The `tooltip` story names `kinds = ["button"]`**: it shows the common `tooltip` prop on a button, and
  core's format requires a non-empty `kinds` (GAL1 item 1).
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

- None open. (`kinds = []` was answered by core's format: not allowed.)

## Polish findings

Rendered with core's CLI on `ci/gb1-core` `559327e` (merged): one sheet per family, all states and the four
themes (`skelter-gallery sheet --family <f> -o target/gallery/<f>`), plus normal-state sheets in dark-modern and
light-modern for reading. No cell has an error or a warning. The layout family (box, grid, sides, stack)
looks right. Rows are ordered by impact; the first three affect every interactive kind. None is fixed here
(BRIEF: the polish lanes' input).

| Story/variant | State | Theme | What is wrong | Command |
|---|---|---|---|---|
| button/look, checkbox/value, text-edit/value (every interactive kind) | focus-visible | all four | No focus ring: the subject renders exactly as `normal`, in hc-dark and hc-light too. `inspect` shows the state reached the node (`state.forced = [focus-visible]`) but the computed style has no outline and the paint has none. | `skelter-gallery shot button/look --args look=plain --state focus-visible --theme hc-dark` |
| button/look, checkbox/value, text-edit/value | disabled | all four | `enabled={false}` changes nothing visible: no dimmed text or border. `inspect` shows `state.own = [disabled]` and AccessKit `disabled: true`, but the computed `color` and `background` equal `normal`'s. | `skelter-gallery shot checkbox/value --args value=checked --state disabled --theme light-modern` |
| checkbox/value, text-edit/value | hover, active | all four | No hover or pressed look (button's hover changes its background only slightly: `#313131` → `#3b3b3b` in dark-modern). | `skelter-gallery shot text-edit/value --args text=R12 --state hover --theme dark-modern` |
| button/look | normal | all four | `default` and `selected` look identical to `plain`: no accent fill for the default button and no toggle look for `selected`. | `skelter-gallery shot button/look --args look=default --theme dark-modern` |
| dialog/destructive | normal | all four | `dialog-role="destructive"` has no styling; the `default` button (Keep) is not emphasised either. | `skelter-gallery shot dialog/destructive --theme light-modern` |
| button/long-text, checkbox/long-text, switch/long-text, collapsing-header/long-text | normal | all four | Captions never wrap or elide: the control grows past its parent's width (box `width=336`) and is clipped by the cell. | `skelter-gallery shot checkbox/long-text --theme dark-modern` |
| progress-bar/text | normal | all four | Text wider than the bar overflows both ends of the bar, unclipped. | `skelter-gallery shot progress-bar/text --args "text=Building the netlist for the main board and every sub-board"` |
| progress-bar/percentage, progress-bar/text | normal | light-modern | Text over the fill is dark on accent blue: poor contrast (the part over the fill should switch colour). | `skelter-gallery shot progress-bar/percentage --theme light-modern` |
| progress-bar/fraction | normal | light-modern | The trough is near-white on the near-white background: an empty bar is almost invisible. | `skelter-gallery shot progress-bar/fraction --args fraction=0.0 --theme light-modern` |
| progress-bar/indeterminate | normal | all four | Nothing but the trough at the gallery's fixed virtual time: no moving segment or busy look. | `skelter-gallery shot progress-bar/indeterminate` |
| slider/marks | normal | all four | `marks="0:Low,50:Medium,100:High"` draws no marks and no labels. | `skelter-gallery shot slider/marks` |
| rich-text/runs, rich-text/nested | normal | all four | `<code>` runs draw as plain text: no monospace font and no field fill (the doc says both). | `skelter-gallery shot rich-text/runs` |
| label/weight | normal | all four | `font-weight=300` draws like 400 (no light face registered, no fallback). | `skelter-gallery shot label/weight --args weight=300` |
| text-edit/* and multiline/* | normal | all four | No inner padding: the text touches the field's left border (and the top-left corner in multi-line fields); the single-line field's height leaves no space above and below the text. | `skelter-gallery shot multiline/wrapping --theme light-modern` |
| text-edit/invalid, password/invalid | normal | all four | `invalid` draws no error border or colour. | `skelter-gallery shot text-edit/invalid --theme dark-modern` |
| text-edit/read-only, multiline/read-only | normal | all four | Read-only fields look identical to editable ones. | `skelter-gallery shot text-edit/read-only` |
| multiline/overflow | normal | all four | Lines past the field's height are cut mid-glyph with no scroll bar. | `skelter-gallery shot multiline/overflow` |
| dialog/form | normal | light-modern | The autofocused field's selection is dark text on accent blue: poor contrast. | `skelter-gallery shot dialog/form --theme light-modern` |
| combo-box/open | normal | light-modern | The highlighted row in the popup is dark text on accent blue (poor contrast); dark-modern uses light text. | `skelter-gallery shot combo-box/open --theme light-modern` |
| scroll-area/scrolled | normal | all four | `scroll-y={200}` moves the thumb to the middle but the content still shows Row 1 at the top: the content is not offset. | `skelter-gallery shot scroll-area/scrolled` |
| scroll-area/direction | normal | all four | The scroll bars overlay the text with no track or gutter; with `direction=both` both thumbs sit over content. | `skelter-gallery shot scroll-area/direction --args direction=both --theme light-modern` |
| virtual-list/plain, table/overflow | normal | all four | virtual-list shows no scroll bar for 200 rows; table `overflow` (stretch-last off, four columns) hides the fourth column with no horizontal scroll bar. | `skelter-gallery shot table/overflow` |
| table/overflow | normal | all four | `elide="middle"` on Description elides at the end. | `skelter-gallery shot table/overflow --theme light-modern` |
| table/sorted-selected | normal | all four | Selected rows are a faint grey (not the selection colour); the cursor row (`current={2}`) has no visible cursor. | `skelter-gallery shot table/sorted-selected --theme dark-modern` |
| tree-view/expanded | normal | all four | `root-decorated={false}` still draws level-0 twisties. | `skelter-gallery shot tree-view/expanded --args decorated=false` |
| tree-view/rtl | normal | all four | Right-to-left rows align right but the twistie stays at the left edge (not mirrored). | `skelter-gallery shot tree-view/rtl` |
| frame/checkable | normal | all four | An unchecked checkable frame leaves its content looking enabled (the doc says unchecked disables it). | `skelter-gallery shot frame/checkable --args checked=false` |
| frame/style | normal | all four | `canvas` has a smaller inner margin than the other styles: the caption almost touches the border. | `skelter-gallery shot frame/style --args style=canvas --theme light-modern` |
| collapsing-header/end-slot | normal | all four | The `end` slot's button sits above the header row's centre line. | `skelter-gallery shot collapsing-header/end-slot` |
| button/dense, icon/toolbar | normal | all four | A vertical `separator` in a row draws nothing (the gap is there, the line is not). | `skelter-gallery shot button/dense --theme light-modern` |
| radio-button/single | normal | light-modern | The checked radio's dot is dark grey while the checked checkbox uses the accent: inconsistent. | `skelter-gallery shot radio-button/single --args checked=true --theme light-modern` |
| every `rtl` variant with Arabic | normal | all four | Arabic text renders as tofu boxes: the renderer's font set has no Arabic coverage (Hebrew renders). Likely the gallery/harness font set, not the widgets. | `skelter-gallery shot label/rtl` |
| icon/glyph, icon/button, icon/toolbar | normal | all four | Symbol glyphs (★ ⚙ ◀ ✎ ✂) render as tofu: no symbol coverage in the font set. | `skelter-gallery shot icon/glyph --args size=32` |
| icon/registry, button/icon | normal | all four | An unregistered icon key draws as its key text (`file.save`) in `icon`, and as an empty box in `button`'s `icon`. The gallery host registers no icons (request 4). | `skelter-gallery shot icon/registry` |
| image/*, icon/source | normal | all four | An imported SVG (`asset://` ref) does not load: the placeholder box is drawn. Confirms request 3. | `skelter-gallery shot image/fit --args fit=cover` |
| multiline/value (core lane) | — | — | The sheet's cell header prints the arg `First line\nSecond line` with its newline, so the label's second line overlaps the next header line. A core-lane sheet issue: escape or cut arg values in headers. | `skelter-gallery sheet multiline --variants value` |

## Tests and CI

- `skelter-gallery check` (core's CLI, debug build): `31 stories, 135 variants, 0 problems`.
- `skelter-gallery list`: 34 kinds registered, 33 covered; `window` without a story (recorded above).
- `svelters-compile --backend skelter --check` on every story file: no diagnostics.
- Sheets: 7 families, 2,892 cells across all states and the four themes, no errors or warnings.
- No tests of this lane's own (it ships no code); the rest is GitHub CI.

CI on `47adaec` (the last code sha: stories plus `ci/gb1-core` `8350d0b` merged), all six workflows green: js
40579883283, linux-gtk 40579883341, linux-qt 40579883216, macos-appkit 40579883273, skelter 40579883278,
windows-winui 40579883355. Earlier: `29ebdbf` (stories only) green; `c163a06` red in skelter (windows) on
core's `skelter-gallery` tests (Windows `\\?\` root paths), fixed by core's `8350d0b`, merged in `47adaec`.
This record's last edit is docs only (CI not rerun for it).

Not run locally: the workspace tests, core's own tests, conformance (CLAUDE.md: CI is the gate).

## Handoff

Done: stories, coverage, findings. Next (a later lane): when request 1 lands, add stories for the view, tab
and picker kinds (split, panel, popup, tooltip, toast, status bar, modal, inner window, dialog buttons, menu
bar, tabs, colour and date pickers, calendar); when request 2 lands, show `context-menu` open. The polish
lanes take the findings table as their list.
