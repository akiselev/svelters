# DZR2: the designer's structural intents and forced states

**Status:** built and tested on branch `claude/practical-lamport-y8z11o`, on svelters master `1d6656c2`. Not merged;
CI not run. This covers designer phases DZ2b (forced states) and DZ3 (structural edits). The design is cadatomic
`docs/research/2026-10-25-visual-ui-designer.md`:
- §3.3 and §3.4, structural edits;
- §3.7, editing a `:hover` rule while hover is forced.

cadatomic's designer commands are being written against these shapes in parallel.

## Resolver v2 (`svelters_compiler::design`)

### New intents

Each new intent is planned like v1's: `TextEdit`s in the file's own style, checked by parsing the result again.

| Intent | Writes | `Plan::target` |
|---|---|---|
| `Insert { parent: Option<u32>, index, markup }` | `markup` (trimmed; exactly one element or component, parsed to check) as child `index`. A node alone on its line gets its own line at the siblings' indent, before the sibling at `index` or after the last one. Siblings on one line keep the new node on that line, with the separator they use (else a space). `<box />` becomes `<box>`, the child one step in, `</box>`. `<box></box>` gets the child on its own line. An empty template gets it before the `<style>`, else at the end. The markup's continuation lines keep their indent relative to its first line. | the new node |
| `Remove { node }` | the node, with its whole line when it is alone on it, else with the whitespace before it (after it when it starts its line) | the parent, 0 at the top level |
| `Move { node, parent, index }` | a removal plus an insertion, as above, re-indented for the new place. `index` counts the new parent's children without the node. Where it already is: no edits. | the moved node |
| `Wrap { first, last, tag, attrs }` | contiguous siblings (same parent and branch, in outline order) in `<tag attrs…>`…`</tag>`. Siblings on lines of their own: the tags go on their own lines and every line in between goes one step in. Inline: the tags go around them on the same line. | the wrapper |
| `Unwrap { node }` | removes the tags, and their lines when they are alone on them. Lines in between go one step out (the child's indent past the element's). | the first unwrapped child |
| `AddClass { node, class }` | appends the class to a static `class="…"` (already there: no edits), or adds `class="…"` after the last attribute | the node |
| `AddRule { selector, decls }` | the rule after the last top-level rule or at-rule. It has a blank line before it when the last two rules have one, and the declaration indent of an existing multi-line rule (else the file's step). A `<style>` with no rules gets it one step in. With no `<style>`, `\n<style>\n  sel {\n    p: v;\n  }\n</style>\n` is appended in the file's step. | the new rule's selector |

The step is the template's own: a child's indent past its parent's line. Failing that, it is the `<style>`'s, and
failing that, two spaces.

### What is refused

| Refused | Kind |
|---|---|
| A new parent that is a block | `NotEditable(Block)` |
| A new parent that is a dynamic element | `NotEditable(Dynamic)` |
| A new parent that is another non-element | `NotEditable(why)` |
| Removing, moving or wrapping an opaque node | `NotEditable(why)` |
| Moving a node into itself or its own descendant | `BadValue` |
| Wrapping nodes that aren't siblings in one branch, or `first` after `last` | `BadValue` |
| Unwrapping a component (its children are its content) or an element with no children | `BadValue` |
| Unwrapping a block, a spread or a dynamic element | `NotEditable(why)` |
| Markup that isn't one element or component | `BadValue` |
| An index past the children | `BadValue` |
| A class that isn't a CSS identifier | `BadValue` |
| `class={expr}` | `NotEditable(Expression)` |
| `class="a {b}"` | `NotEditable(Interpolated)` |
| A selector the designer doesn't edit (the same check `Design::parse` gives rules) | `NotEditable(Selector)` |
| A selector that can't be written | `BadValue` |
| Bad declarations (`check_property`, `check_decl_value`) | `BadValue` |

### Verification

`verify` now gets the old outline. For each structural intent it checks:
- the target is a node of the right kind and name;
- the outline's node count changed by what the intent adds or removes. Text nodes are not counted, because they merge
  and split.

It also checks that a wrapper has the wrapped nodes as its children, that the class reads back, and that the new rule
is at the target, is editable, has its declarations, and is the only rule added.

### `Design::plan_all(&[Intent])`

Each intent is planned on the text the previous ones left, so its offsets name that text. The result is one `Plan`:
- `edits` in the original file's offsets. An edit that touches text an earlier step wrote can't be mapped back, so
  the plan falls back to one edit from the common prefix to the common suffix.
- `text`: the final file.
- `target`: the last intent's target.

A refused step refuses the whole plan. cadatomic uses `[AddClass, AddRule]` as one undo group for "override here".

## Forced states (svelters-egui)

- `EguiBackend::force_state(h, StateBits) -> Result<(), String>` forces bits on a node. `StateBits::NONE` clears them.
- `EguiBackend::forced_state(h) -> StateBits` reads the forced bits back.
- `svelters_egui::style::pseudo_state(name) -> Option<StateBits>` maps a pseudo-class to its bits. It reads the
  engine's `STATE_PSEUDO_CLASSES` (non-negated rows), adds `focus-visible`, and resolves `state(ns.name)` through
  `ProviderStates`, the registry. There is no second table.

The forced bits live in `SceneStyle::forced_states` (by `NodeId`). `Scene::shown_state(i)` ORs them over the node's own
bits. It is the one place they enter:
- `Element::state` and `is_focus_visible`, which is all selector matching;
- the walk's `StyleView`s in `before_container` and `before_leaf`;
- an own widget's `WidgetCx::style`.

The walk keeps writing `core.state` every frame, so the forced bits survive it. They never reach the journal, events,
a11y or `NodeRef::state`.

For stock buttons and text fields, a forced hover or press makes their lower per-state styles that state's style, so
they paint it whatever egui's own state is.

Setting forced bits restyles what the change reaches, the same way a state change does. `clear_overrides` clears them
(`clear_forced`), and a freed node drops them (`SceneStyle::forget`).

Not covered: the UA-table states of the built-in container views (table, toast, list rows) read their own bits.

## Tests run

- `cargo test -p svelters-compiler --lib design::`: 16 passed (8 v1, 8 new).
  - The new tests are:
    - `insert_follows_the_siblings_layout`;
    - `insert_refusals`;
    - `remove_takes_the_line`;
    - `move_reorders_and_reparents`;
    - `wrap_and_unwrap`;
    - `add_class_writes_the_static_class`;
    - `add_rule_follows_the_sheets_layout`;
    - `plan_all_is_one_plan_in_the_original_offsets`.
  - They cover:
    - insert into a self-closing box, at index 0, at the end and into an empty top level;
    - remove the only child;
    - move up, down and into another parent;
    - moving into a node's own descendant, refused;
    - wrap one and several siblings, then unwrap;
    - `AddClass` with an existing, absent and expression class;
    - `AddRule` with and without a `<style>`;
    - `plan_all([AddClass, AddRule])` producing edits in the original offsets, and the fallback;
    - refusals.
- `cargo test -p svelters-egui --test style`: 4 passed. The new one is `forced_states_show_a_states_rules`:
  - a forced `:hover` on a label and on a box, and a forced `:state(cadt.pending)` on a button, apply with the
    pointer off the window;
  - the node's own bits stay unforced;
  - `NONE` clears one node;
  - `clear_overrides` clears the rest.
- `cargo clippy -p svelters-compiler --tests` is clean for `design/`. `cargo clippy -p svelters-egui --tests` shows
  nothing new on the changed lines.

## Not run

- The workspace test suite and CI.
- `tests/overrides.rs` (DZ0's JS-loaded test, which needs `pnpm install`).
- A sweep over cadatomic's `ui/` files.
