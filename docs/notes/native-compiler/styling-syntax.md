# X1 native styling: accepted `<style>` syntax

`<style>` in a native component is not browser CSS. `style::parse_style` (crate
`svelters-compiler`, `src/style.rs`) parses only the subset that the lowering pass can map to
native widget properties (the `style:` sections of `metadata/semantics/rules/*.yaml`). Everything
else is an error with a precise span; the parser recovers and continues with the next
declaration or rule. Spans are absolute byte offsets into the `.svelte` source.

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
| `style_unsupported_selector` | `#id`, `*`, `[attr]`, `::pseudo-element`, `:not(...)` / `:global(...)` and other functional pseudo-classes, `&` |
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
