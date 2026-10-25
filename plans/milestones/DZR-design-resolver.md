# DZR: the designer's resolver, v1 (`svelters_compiler::design`)

**Status:** built and tested on branch `claude/practical-lamport-y8z11o`, on top of DZ0. Not merged; CI not run.
Requested by the user on 2026-10-25 ("ok lets do the stuff we can now"). This is designer wave 1's second lane. The
other lane is `plans/cloud/designer-wave1/LANE-dz0-followups.md`.

The design is cadatomic `docs/research/2026-10-25-visual-ui-designer.md`:
- §3.1, the edit path: "the resolver lives in svelters as a library";
- §3.3, the editable subset;
- §3.5, styles.

cadatomic's DZ2 (`designer.set_prop`, `set_style`, `clear`) wraps this in typed commands. Each one parses at the
document's `TextHead`, plans, and sends the edits as `text.edit` splices. DZ1's outline pane reads the same tree.

## What it is

`Design::parse(src)` reads a `.svelte` file into:
- **`nodes`**: an outline of template nodes. Whitespace-only text and comments are left out. Each node has:
  - kind, name, span, parent and branch (body, else, pending, then, catch, fallback);
  - children, and its path from the root;
  - its attributes;
  - an `Editability` (editable, structural or opaque) with a `Why`.
- **`rules`**: the rules of the component's `<style>`. Each has its selector text and offset, its block span, its
  declarations (with value spans) and an `Editability`.

`Design::plan(&Intent)` returns a `Plan` or a `Refusal`.
- A `Plan` has:
  - `edits`: sorted `TextEdit`s in the old file's offsets, empty when the file already says it;
  - `text`: the new file;
  - `target`: the node's or rule's offset in the new file.
- A `Refusal` has:
  - `kind`: `Parse`, `NoNode`, `NoRule`, `NotEditable(Why)`, `Missing`, `BadValue` or `Reparse`;
  - a message;
  - the span to edit by hand, which the designer's "edit in code" jumps to.

Every plan is checked by parsing the result again. The check confirms that the target is still at `target` and that
the attribute or declaration reads back as asked. A failure is `Reparse`, which means a resolver bug.

`rebase(offset, &edits)` maps a held offset through edits, the way an editor maps cursors (§3.2: identity follows the
text). It returns `None` when the edit replaced that byte.

Nodes are named by their start offset, which is DZ0's `source-loc` element offset (`file#offset`). Rules are named by
their selector's offset, which is DZ0's `MatchedRuleInfo.source.style_offset`.

## Intents (v1)

| Intent | Writes |
|---|---|
| `SetAttr { node, name, value: Literal }` | Replaces the value span and keeps its quote. A missing attribute is inserted after the last one with the tag's own separator: a newline at the same indent, or a space. With no attributes it goes after the name. `style:x` directives work the same way. |
| `ClearAttr { node, name }` | Removes the attribute and the whitespace before it. |
| `SetDecl { rule, property, value }` | Replaces the value span. A missing declaration is inserted after the last one with the block's separator. If the last one has no `;`, the resolver adds it. An empty block gets an indented line. |
| `ClearDecl { rule, property }` | Removes each declaration of the property, with its `;` and the whitespace before it. |

How literals are written:
- `Str` is written as `"text"`, in the tag's quote.
- A string containing `{`, `}`, `&` or both quote characters is written as a JS string, `{"…"}`, so it can't turn into
  an expression or an entity.
- `Num` is written as `{12}`, `{0.5}`, `{-4}`.
- `Bool` is written as `{true}`. A bare attribute stays bare when it is set to true.

Names and values are checked first:
- an attribute name is `[A-Za-z_-][A-Za-z0-9_-]*`, optionally with a `style:` prefix;
- a declaration value has no `;`, braces, comments or newlines.

## What is editable (§3.3)

- **Elements and components** are editable, except:
  - a node with `{...spread}` is structural (`Spread`), because the spread may override any attribute;
  - `<svelte:element>` and `<svelte:component>` are structural (`Dynamic`).
- **Text** is editable.
- **Not part of the layout:**
  - `{expr}` and `{@html}` are structural (`Expression`);
  - blocks and `{@render}` are structural (`Block`);
  - `{@const}`, `<svelte:head>` and the like are opaque (`Special`).
- **Attributes:**
  - literal values and bare attributes are editable;
  - `{expression}` is `Expression`;
  - `on*={…}`, `bind:` and `on:` are `Binding` (DZ4);
  - `"a {b}"` is `Interpolated`;
  - directives are `Directive`;
  - spreads are `Spread`.
- **Rules:** a rule is editable when each of its selectors is one compound of kinds, classes and pseudo-classes, such
  as `:hover` or `:state(cad.pending)`.
  - A rule with combinators, ids, attributes, `:global` or a pseudo-class with arguments is opaque (`Selector`).
  - A rule inside an at-rule or nested in another rule is opaque (`AtRule`).

Over cadatomic's 25 UI files (cadatomic master `0058d40`, `ui/`):
- every file parses;
- 361 of 529 outline nodes are editable;
- 494 of 1038 attributes are literals;
- all 20 rules are editable.

Setting each of the 494 literals to its own value planned no edit. Inserting a new attribute on each of the 361
editable nodes, a new declaration in each rule, and clearing each declaration all planned and re-parsed. The sweep was
an ad-hoc test and isn't committed.

## Decisions

- **No `loose` parsing in v1.** A file that doesn't parse is refused whole. A half-typed file is the code editor's
  job, and loose parses would make the re-parse check weaker.
- **Byte order marks are refused.** The parser's offsets skip the mark, so every offset would be off by three.
- **No string diagnostic codes.** Refusals are an enum (`RefusalKind`, `Why`), not compiler codes, so `codes.rs` is
  untouched.

## Tests

`crates/svelters-compiler/src/design/tests.rs` has 8 tests:
- classification of nodes, attributes and rules;
- in-place replacement;
- the separators and quotes for new attributes;
- clearing;
- refusals with spans;
- declarations;
- `rebase`.

Run: `tools/dev/cargo-slot cargo test -p svelters-compiler --lib design::`, 8 passed. `cargo clippy -p
svelters-compiler --tests` is clean for `design/`. The full workspace was not run.

## Next

- **cadatomic DZ2:** command wrappers over `Design::plan`, after the svelters pin advance.
- **DZ3 adds structural intents:** insert a child at a boundary with the sibling's indentation, delete a node, move a
  node by its span (re-indented by line shift), and wrap and unwrap in a `<box>`.
- `Text` content edits, and token-aware values: offering `var(--sv-*)` before raw values is policy for cadatomic's
  side.
