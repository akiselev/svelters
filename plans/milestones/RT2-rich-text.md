# RT2: inline rich text (egui)

User decision (WIDGET-PARITY Q4, 2026-10-22): inline rich text through child elements (`<b>`, `<i>`, `<code>`,
`<a>`, `<span class>`) styled by CSS, with no HTML or Markdown parser. Lane RT2 (thinkpad), branch `ci/rt2`,
written against `master` (not `ci/ow0`; see "OW0 and OW1a" below).

## Decided (RT2 lane, 2026-10-22): where the lowering lives

**Runs are scene nodes, lowered by the compiler, and flattened into one galley by the label at paint.**

- The compiler keeps the static structure: a text-content element whose slot takes the inline-text kind (spec
  `inline_text`, `{"kind": "span", "prop": "text"}` on egui) lowers text mixed with run elements into run
  nodes; bare text and `{expressions}` become anonymous `span` runs (the same lowering as the DOM backend's
  `text_node`). Text alone still folds into the `text` prop, so existing labels compile as before.
- The `label` kind (`crate::rich`) walks its run subtree in order and builds one egui `LayoutJob`, a section per
  run, from each run's computed style.

Reason: Q4 says the runs are styled by CSS (`label span.cls`, `label a:hover`, scoped classes). The CSS engine
(CS5e) already matches selectors, scoped hashes, `:hover`/`:focus` and inheritance on scene nodes; a run list as
a prop would need a second matcher for objects that are not nodes. Nodes also give reactivity for free
(`{expr}`, `{#if}`, `{#each}` inside a label are ordinary `$n.set` and blocks) and give each `a` its own node
id for focus, AccessKit and its `clicked` signal.

## What was built

| Piece | Where |
|---|---|
| Run kinds `span`, `b`, `i`, `code`, `a` (abstract base `Run`: `text`, `font-weight` (0 inherits); `a`: `href`, `accessible-name`, `clicked`), appended to `KINDS` | `crates/svelters-egui/src/kinds/rich.rs` |
| `label` takes a `runs` slot (runs only); `W::Label::rich` holds the collected runs (measure, AccessKit text); `W::Run` | `kinds/builtin.rs`, `scene/node.rs`, `scene/tree.rs`, `scene/props.rs` |
| Collect, layout job, paint, per-link interaction | `crates/svelters-egui/src/rich.rs` |
| A run's text/weight/href write, insert/remove, or restyle remeasures its label | `scene/access.rs`, `scene/tree.rs`, `style.rs` |
| Engine properties `font-style`, `font-family`, `font-weight`, `text-decoration-line` (egui table; OW-CSS's canonical names) | `style.rs` `ENGINE_PROPS` (generated: `egui.json`, `tables.generated.rs`, `props.generated.js`) |
| Theme defaults: `b` 700, `i` italic, `code` mono on `--sv-control-bg`, `a` `--sv-link`, `a:hover` underline | `theme/base.css` (the same defaults hold in Rust without a theme) |
| Spec `inline_text`; lowering | `crates/svelters-compiler/src/semantics/{spec,mod,backend,overlay}.rs`, `lower/mod.rs` |

Each run's format: `color`, `background-color`, `font-size` (when it differs from the label's), `font-style`,
`font-family` (monospace when it names one), `text-decoration-line` (decorations of an enclosing run stay), and the
`font-weight` prop; what a run does not set it inherits from the enclosing run.

Links: every `a` gets `ui.interact` at its glyph bounds under its own egui id, so it is hoverable (pointer
cursor, `:hover`, underline), focusable (Tab; the focus ring), activated by click, Enter or Space, and an
AccessKit `Link` node. Activation fires the run's `clicked` (`onclick` on `<a>`); with no handler, a non-empty
`href` opens as `hyperlink`'s `url` does.

## OW0 and OW1a

Written against master's `egui::Label` path. The seam for OW1a's own label is `rich::layout_job(ctx, style,
&Rich, colors, wrap, underline) -> LayoutJob` and `W::Label::rich`: OW1a lays the job out where OW0's
`paint/text::text_galley` lays out a single-format text, and keeps `rich::paint`'s link loop. OW1a's matches on
`W::Label` need `..` for the new field.

## The shared paragraph API (`crate::rich`; for OW1a, OW3's tooltip and OW7's toasts)

Decided (coordinator, 2026-10-22): the layout is a shared module, not part of the label kind.

```rust
pub struct RunFmt { weight: i64, italic: bool, mono: bool, size: Option<f32>, color: Option<Color32>,
                    background: Option<Color32>, underline: bool, strike: bool }
pub struct RunSeg { node: Option<NodeId>, start: usize, end: usize, fmt: RunFmt, link: Option<NodeId>, href: Option<String> }
pub struct Rich   { plain: String, runs: Vec<RunSeg> }   // runs cover `plain` in order, byte ranges
pub struct Colors { text: Color32, link: Color32 }

pub fn collect(s: &Scene, label: usize) -> Option<Rich>          // runs as nodes: per-run CSS
pub fn from_value(v: &Value) -> Result<Rich, String>              // plain runs: string | [string | {text, b, i, code, href}]
pub fn layout_job(ctx, style: &egui::Style, rich: &Rich, colors: Colors,
                  wrap: Option<f32>, underline: Option<NodeId>) -> LayoutJob
```

`layout_job` contract: one section per run, in order; a run with no `color` takes `colors.link` when it is a link
(`link` or `href`) and `colors.text` otherwise; `size: None` is the body size; `mono` is
`FontFamily::Monospace`, otherwise the body font at `weight` (`fonts::text_font`); `underline` names the link
drawn underlined (hover or focus, R18); `wrap: None` is no wrapping (the caller may set `job.wrap` itself, as
`truncate` does). It reads no scene state, so a caller without nodes (a toast) uses it with `from_value`; when
toasts or tooltips become nodes (OW7, OW3) they call `collect` on their run children and get per-run CSS. Painting
and link interaction for node runs are `rich::paint` (the label's).

## Tooltip and toast

- **tooltip** (coordinator): goes to OW3, whose Tooltipable takes text or a snippet; a snippet of a rich `label`
  gives a rich tooltip with no new kind. Not built here.
- **toast** (coordinator): `toast-host.show(text, …)` also takes a plain run list (`rich::from_value`), drawn as
  one paragraph with the run tag defaults; a click on a run with an `href` opens it. No per-run CSS (toasts are
  not nodes yet; OW7). Unknown fields are errors: there is no markup.

## Not in RT2 (follow-ups)

- Selecting text across runs in a `selectable` rich label (OW1a's read-only TextInput).
- `:visited` (OW-P2); reading runs through OW-CSS's `StyleView` once it lands.

## R-rules, parity rows, VIS1

- DESIGN **R18** (links get the `link` colour and an underline on hover), **R10** (pointer cursor on links),
  **R8** (focus visible: a focus ring around the focused link, keyboard only through egui's focus).
- WIDGET-PARITY §2.10 `textFormat` RichText (Q4), `linkActivated` (inline links: `clicked` on the `a` run),
  styling row "inline spans (rich text)" (`label span.cls`, `label a:hover`); QT-SURVEY QW13 (rich text with
  links).
- VIS1: no new snapshot gate; the link colour is the `link` token (checked by the token mapping), hover adds an
  underline (R18 check).

## Tests

- `crates/svelters-compiler/tests/rt2_rich.rs` (golden `tests/rt2/*.txt`): mixed text and runs, nested runs with
  scoped classes, text-only labels unchanged, `{#if}`/`{#each}` inside a label, a non-run child is an error,
  a `button` keeps `text_content_children`.
- `crates/svelters-egui/tests/rt2_rich.rs` (kittest): one paragraph with per-run CSS (`.warn` colour, `code`
  background, `a` decoration), one `Label` and one `Link` AccessKit node; a run's text change remeasures the
  label and a removed run leaves it; a toast takes a plain run list; the link fires `clicked` from an AccessKit click, a pointer click and Enter
  after focus, and `a:hover` applies while hovered.
