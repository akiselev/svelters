# NC1 native styling: accepted `<style>` syntax

**ST1 (2026-10-17).** A `<style>` holds two kinds of declarations, decided by the backend's style
table (`metadata/style/<backend>.yaml`, the `style` section of `metadata/semantics/*.json`):
setter / child-attr properties are the compiler's, CSS properties go to the X6 translator in the
Vite plugin (the compiler scopes them with its port of Svelte's CSS pipeline, `--css`, Q3w; stock Svelte
still does for egui; `--css-scope` gives the compiler the scope class).
`style::parse_style` reads the whole sheet but reports a problem below only when it touches a
setter declaration: a CSS-only rule, value or at-rule is never reported here. `#id` selectors are
accepted (specificity is `(ids, classes + states, types)`), units are `px` or none, and a rule with
setter declarations must use one compound selector without states (`style_unsupported_combinator`,
`style_state_unsupported` from the lowering). `@media` or nesting around a setter property is
`style_unsupported_at_rule` / `style_unsupported_nesting`. See docs/notes/st1-style-unification.md.

`<style>` in a native component is not browser CSS. `style::parse_style` (crate
`svelters-compiler`, `src/style.rs`) parses the subset that the lowering pass can map to native
widget properties. The parser recovers and continues with the next declaration or rule. Spans are
absolute byte offsets into the `.svelte` source.

## Grammar

```
sheet       = (rule | comment | whitespace)*
rule        = selector-list "{" (declaration? ";")* declaration? "}"
selector-list = complex ("," complex)*
complex     = compound ((ws | ">") compound)*          descendant / child only
compound    = element? (("." ident) | (":" ident))*    at least one part
element     = letter (letter | digit | "-")*
declaration = ident ":" value                          --custom: raw text kept as is
value       = token+
token       = ident | number unit? | string | "#" hex{3,4,6,8} | ident "(" raw ")"
unit        = "%" | letters                            e.g. 8dp, 50%, 1.5, 0
```

Comments `/* */` are allowed anywhere whitespace is. `:state` names are not checked here
(`:hover`, `:focused`, `:disabled`, `:checked`, ...); the backend lowering validates them, as it
does property names and units (`dp`). Function tokens keep their argument text raw.
Specificity is `(classes + states, element names)`, compared lexicographically, summed over the
compounds of a selector; `ComplexSelector::subject()` is the last compound.

## Diagnostics

| Code | Cause |
| --- | --- |
| `style_unsupported_selector` | `*`, `[attr]`, `::pseudo-element`, `:not(...)` / `:global(...)` and other functional pseudo-classes, `&` |
| `style_unsupported_combinator` | `+`, `~` |
| `style_unsupported_at_rule` | any `@rule` (`@media`, `@import`, `@keyframes`, ...), at top level or in a block |
| `style_unsupported_nesting` | a rule inside a rule's block |
| `style_unsupported_important` | `!important` (the declaration is dropped) |
| `style_unsupported_value` | a value character outside the token set, e.g. `,` or `/` |
| `style_syntax` | missing `{`, `:`, value, selector; stray `}`; unclosed block, comment, string or `(`; bad `#color` |

Recovery: a bad selector drops only that selector (the rule is kept if another selector in the
list parsed); a bad declaration is dropped up to its `;`; an at-rule or nested rule is skipped
with its balanced `{ ... }` block. `;`, `{` and `}` always end a declaration, even inside an
unbalanced `(`.
