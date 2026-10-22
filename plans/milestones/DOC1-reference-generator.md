# DOC1: widget reference generator

Lane DOC1, started 2026-10-22 on master `ed3d0a5f`. Spec: `plans/research/DOCS-PLAN.md` §5.4 (widget
reference), §6.1 (metadata additions), §6.2 (generators), §7.1 (the DOC1 row). Target format: the sample
`plans/research/docs-samples/reference-text-edit.md`.

## What landed

### 1. Docs fields in `egui.json`

`metadata/semantics/egui.json` has, for every kind, the four fields the plan names plus two:

| Field | Values | Source |
|---|---|---|
| `category` | `inputs`, `buttons`, `display`, `containers`, `layout`, `views`, `menus`, `overlays`, `pickers`, `media`; plus `base` (the abstract `Widget`, `MenuEntry`) and `custom` (provider kinds) | `kinds/docs.rs` |
| `states` | `[{name, pseudo, negated, source, when}]`: the built-in state bits the walk sets on the kind, the pseudo-classes that read each bit, and who sets it (`response`, `prop`, `scene`, `kind`) | the kind's walk class in `kinds/docs.rs` (leaf, container, none) plus its own prop states, mapped through `svelters_style::STATE_PSEUDO_CLASSES` |
| `a11y` | `{role, variants, name_from, value, actions, author_id, invalid, note}` | `kinds/docs.rs` |
| `paint` | `stock`, `box`, `provider`, `none` | `kinds/docs.rs` |
| `css_note` | one line for the CSS section (what CSS doesn't reach, the token to use instead) | `kinds/docs.rs` |
| `css_hidden` | paint properties a kind's own drawing covers (only `panel` today) | `kinds/docs.rs` `CSS_HIDDEN` |

`style.pseudo_classes` (top level) lists every state pseudo-class with the state bit it reads and whether it
is negated. DOC5 can use it for `css/selectors.md`.

- The facts live in a Rust table, `crates/svelters-egui/src/kinds/docs.rs` (`KIND_DOCS`, one row per kind),
  not in hand-edited JSON: the binary stays the source of truth (EGUI.md §6). `spec.rs` writes them
  (`add_docs`), and `tests/spec.rs` checks freshness as before plus `every_kind_has_one_docs_row` (a new kind
  without a row fails, and so does a row for a kind that's gone).
- The rows were filled by reading the walk and the paint code: `style.rs` (`before_leaf`,
  `before_container`, `prop_bits`, `is_stock`, `widget_visuals`), `walk.rs`, `leaf.rs`,
  `elements/{mod,inputs,media,panes}.rs`, `menus.rs`, `overlays.rs`, `views/{mod,split,tabs,table,toast}.rs`,
  `kinds/{pickers,virtuallist}.rs`, `a11y.rs`, and egui 0.37's widget `WidgetInfo` roles and AccessKit actions.
- They describe the element's API (selectors, states, roles, actions), not its drawing library. Page text
  never says "wraps egui::X"; the generator also drops doc-string parentheticals that name a drawing-library
  type (`(\`egui::Window\`)`, `(\`Response::clicked\`)`), keeping those that name a `svelters` item.
- Provider kinds (`#[svelters::widget]`) get the same fields in the dumped fragment (`kinds.json`):
  category `custom`, paint `provider`, the leaf states (`:focus` when the kind has `Caps::FOCUSABLE`), and an
  a11y note that the role is what its `Widget::a11y` sets. `examples/egui-kinds/app/.svelters/app/kinds.json`
  was regenerated (its staleness test, `checked_in_kinds_json_and_typings_are_current`, would fail otherwise).
- The compiler ignores the new fields (no `deny_unknown_fields` on its spec structs); `egui.d.ts` is unchanged.

Facts the table records that differ from the plan's sample page (`reference-text-edit.md`):

- **`:invalid` never matches a built-in kind.** The `invalid` prop draws the error outline and sets AccessKit
  `invalid`, but no code sets the `INVALID` state bit (`prop_bits` sets only `disabled`, `exiting`, and
  `checked`/`selected` for `checkbox`/`button`). The sample's `:invalid`/`:valid` row is wrong at this sha.
  Likewise `:open`, `:expanded`, `:read-only`, `:dragging` and `:busy` are set by no built-in kind, and
  `:checked` only on `checkbox` (not `toggle-button`, `radio-button`, `switch`). Every page lists the
  pseudo-classes that never match it. This is a code gap worth a small lane (set `INVALID` from the prop,
  `CHECKED` for the toggles, `OPEN`/`EXPANDED` for `collapsing-header`, `popup`, `modal`), not a docs fix.
- **`:focus` never matches `combo-box`** (its response isn't read under the id the walk reads), nor any
  container (only leaves get `focused`; containers get `:focus-within`).
- **Most containers have no AccessKit node** (`box`, `grid`, `frame`, `panel`, `stack`, `sides`,
  `scroll-area`, `tab-view`, `popup`, `modal`). On those, and on `window`, `tab`, `virtual-list`,
  `table-column` and `toast-host`, `anchor` keys the ids but isn't an author id, and `invalid` isn't
  announced (only nodes that go through `a11y::decorate` get them). Pages say so.
- **`panel` paints `--sv-panel-bg` over the CSS box**, so `background-color` and `border-*` rules on a panel
  don't show (`css_hidden`).

### 2. The generator: `tools/docs/egui/gen_reference.py`

Writes `docs/egui/reference/widgets/<tag>.md` for every kind (abstract kinds as `widget.md`,
`menu-entry.md`) and `index.md` grouped by category: 49 files for the 48 kinds on master `d242d83c` (WG5b's `tree-view` included). Path as in DOCS-PLAN §5.4
(`reference/widgets/`), not `reference/<kind>.md`.

- **Managed regions.** The generator owns only the text between
  `<!-- BEGIN GENERATED: <region> of kinds.<Id> (...) -->` and `<!-- END GENERATED -->`, and the front-matter
  keys `title`, `tag`, `kind`, `category`. Regions: `summary`, `props`, `events`, `methods`, `css`, `states`,
  `a11y` (and `index` on `index.md`). Everything else (hand sections, `status`, `verified_at`, `sources`,
  `cases`) is kept. A region removed by hand comes back under its heading.
- **New pages** come from a template: front matter (`status: stub`, `verified_at: ~`, `generated: partial`,
  `sources`), the regions, and `<!-- TODO(DOC7): ... -->` markers for the hand sections (when to use, Example,
  Behaviour you need to know, Testing, See also), in the sample's order.
- **Each page covers** props (type, default, `bind:` and the event that carries a bound value, required for
  provider props), props inherited from intermediate bases (`box` for `collapsing-header`, ...), the `Widget`
  props by name; events (attribute, handler signature, sync handlers with their `SvlEvent`), inherited events;
  methods (call through `bind:this`); CSS (selector, how it's painted, which paint properties apply, the
  theme default from `theme/base.css` and its state rules, the layout setters and CSS-only layout
  properties for that tag, the `css_note`); states and the pseudo-classes that never match it; accessibility
  (role and variants, name source order, value, actions, `invalid`, author id, note).
- **`--fragment kinds.json --out <dir> [--widget-link <path to svelters' widget pages>]`** documents a spec
  fragment's kinds only (cadatomic's `quantity-field`, `code-editor`), with links to the built-in `Widget`
  page through `--widget-link`.
- **`--check`** writes nothing and exits 1 when a page is missing, a region or a generated front-matter key
  is stale, or a generated page documents a kind that no longer exists.

Commands:

```sh
SVELTERS_UPDATE=1 tools/dev/cargo-slot cargo test -p svelters-egui --test spec        # egui.json after a kind change
SVELTERS_UPDATE=1 tools/dev/cargo-slot cargo test -p egui-kinds-example --test kinds  # the example's kinds.json
python3 tools/docs/egui/gen_reference.py           # or: pnpm docs:egui
python3 tools/docs/egui/gen_reference.py --check
python3 -m unittest discover -s tools/docs -p 'test_egui*.py'
```

### 3. Freshness check in CI

- `js.yml`, job `js`: a step "egui widget reference --check" before `pnpm docs:test`.
- `tools/docs/test_egui_reference.py` runs under `pnpm docs:test` (its `unittest discover` picks it up):
  the checked-in pages are current, every kind has a page with all regions and egui.json has the four
  fields; plus the deliberate failures the plan asks for (a stale region, a missing page, a stale front-matter
  key, a page for a removed kind each fail `--check`), hand text and `status` survive a run, a lost region
  comes back, and the fragment mode on `examples/egui-kinds`' `kinds.json`.
- The chain is the plan's: a kind change makes `egui.json` stale (`tests/spec.rs`, linux-gtk), and a new
  `egui.json` makes the pages stale (`gen_reference --check`, js).

### 4. Not done: the `svelters docs egui` CLI pack (follow-up DOC1b)

The plan's DOC1 row includes an egui IR frontend and `svelters docs egui <tag>`. It isn't small:
`tools/docs/build_ir.py` runs every `tools/docs/<tk>/frontend.py`, so an egui frontend also makes the
Starlight site render egui pages, and CL0's pack/site consistency test (`crates/svelters-cli/tests/
site_consistency.rs`) then requires the site and the pack to agree on egui. That touches the site's
`PLANNED_TOOLKITS`/`TOOLKIT_ORDER` (DOC11's scope). DOC1b: `tools/docs/egui/frontend.py` (egui.json →
docs IR, with the new fields as `extensions.egui`), egui out of `PLANNED_TOOLKITS`, the consistency test green,
and the reference pages' hand sections as entity descriptions. Until then `grep -rl "tag: <tag>" docs/egui`
finds a page.

## Not run

- Locally I ran only: `tests/spec.rs` of svelters-egui (with and without `SVELTERS_UPDATE`), the
  `checked_in_kinds_json_and_typings_are_current` test of egui-kinds-example (with and without update), the
  new Python tests (`test_egui_reference.py`, 17 tests), `gen_reference.py --check`,
  `tools/metadata/style/gen_props.py --check`, `svelters-compile --backend egui --check` on a small file (the
  compiler loads the new egui.json), and `cargo clippy -p svelters-egui --all-targets` (as linux-gtk runs it; no warnings in the lane's files).
- Not run: the rest of `pnpm docs:test` (`test_ir.py`, the site tests; pnpm isn't installed on this
  machine), any other nextest suite, conformance. CI runs them.

## Environment

Briefed as a cloud session, this lane in fact ran as a worktree agent on the WSL PC (the remote launch fell back to a local worktree; the coordinator confirmed)
(`~/projects/svelters/.claude/worktrees/agent-a0fcbdd01da786661`, WSL2), so it used `tools/dev/cargo-slot`
and the machine's slots. A fresh worktree `target/` built `svelters-egui`'s `spec` test in about 6 minutes;
each later spec regeneration took 1 to 2 minutes. Xvfb wasn't needed: `egui.json` comes from a plain cargo
test, not a GUI run. `pnpm` isn't on this machine's PATH, so `pnpm docs:test` wasn't run as a whole.

## Follow-ups

- **DOC1b**: the egui IR frontend and `svelters docs egui <tag>` (above).
- **States code gap** (above): set `INVALID`, `CHECKED` (toggle-button, radio-button, switch), `OPEN`/`EXPANDED`
  and `:focus` on `combo-box`; then update the rows in `kinds/docs.rs`. Until then pages say "never set".
- **Kind doc strings** in `kinds/*.rs` still carry lane ids and drawing-library names (`X8 display list`,
  `M5 model-view protocol`, `SVP6`, "The egui frame drawn around the content: `egui::Frame::group`"). The
  generator drops parentheticals that name library types; the rest should be reworded at the source (DOC7).
- **`#[svelters::widget(category = "...")]`**: provider kinds are all `custom` today; an attribute would let
  cadatomic file its kinds under `inputs` etc.
- DOC7a/b fill the hand sections (TODO markers) and flip `status: stub` → `available` with `verified_at`.
- Links to `../../css/properties.md` etc. resolve once DOC0/DOC5 create those pages.
