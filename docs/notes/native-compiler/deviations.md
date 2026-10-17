# X1 compiler: deliberate deviations from upstream

Every entry corresponds to a `// SVELTERS:` comment in the code. Behaviour not listed here is
meant to match upstream exactly (checked by the oracle, `tools/oracle/run.sh`).

## Svelte parser (`svelte::parse`, `svelte::errors`; agent D)

Port of `packages/svelte/src/compiler/phases/1-parse` of Svelte 5.57.2 @ f3de3f2.

### By design

1. **`<style>` is captured raw** (`read/style.rs` replaces `read/style.js`). The top-level block
   becomes `ast::StyleBlock { attributes, content: { start, end, styles } }`; the native style
   parser (`crate::style`) parses `styles`. Consequences: no `css_*` parse errors; the content
   ends at the first `</style` (upstream only looks for it between rules, so `</style` inside a
   CSS string/comment does not end the block there); `content.comment` (the HTML comment before
   the block) is not kept.
2. **TypeScript is rejected.** acorn-typescript is not ported. When upstream's
   `regex_lang_attribute` detects `<script lang="ts">`, `parse` fails with the new code
   `svelters_typescript_unsupported` (position: the `<script` tag). Consequently
   `remove_typescript_nodes`, the TS branches of `read_type_annotation`, the `TSAsExpression`
   unwrapping in `{#each}` and snippet generics are dead/omitted.
3. **`ParseOptions.native`** turns off the rules that interpret tag names as HTML. With
   `native == false` behaviour equals upstream. With `native == true`:
   - no void elements: `<input>`, `<br>`, `<img>`... (and `<!doctype>`) need closing like any
     element, and `</br>` is not `void_element_invalid_content`;
   - no implicit closing on an opening tag (`closing_tag_omitted`: `<p><div>`, `<li><li>`,
     `<td>`, `<option>`...), hence no `element_implicitly_closed` warnings and no
     `element_invalid_closing_tag_autoclosed`;
   - no implicit closing on a mismatched end tag (`<a><b></a>` closes `<b>` in HTML mode): it is
     `element_invalid_closing_tag`, as for components and blocks upstream;
   - `<textarea>` is not RCDATA (upstream reads its content as text + `{expressions}` up to
     `</textarea>`), and a non-top-level `<script>`/`<style>` is not raw text;
   - `<slot>` inside a declarative-shadow-root `<template shadowrootmode>` is still a
     `SlotElement` (the exception is HTML-only).
   Kept in native mode (Svelte-level, not HTML): `svelte:*` meta tags, `<title>` inside
   `<svelte:head>` (`TitleElement`), `<slot>` (`SlotElement`), component-name detection, tag
   name validity, entity decoding in text and attribute values.
4. **Entities table is generated** (`parse/gen/gen_entities.py` reads upstream
   `utils/entities.js` and writes `parse/utils/entities.rs`, keeping key order, which is the
   order upstream's regex alternation tries names).

### Representation-forced

5. **Positions are byte offsets** (upstream: UTF-16). Places where upstream does arithmetic in
   UTF-16 units over non-delimiter text use bytes: the `<script>` prefix blanking (each
   non-newline character becomes as many spaces as it has UTF-8 bytes) and
   `read_type_annotation`'s `index - 5` (snapped to a char boundary). Identical whenever the
   text involved is ASCII.
6. **`end: -1`** of nodes left open in loose mode is `u32::MAX` (`parse::OPEN_END`).
7. **Comments attached to nodes are not kept**: `add_comments` (`leadingComments` /
   `trailingComments` on ESTree nodes) has nowhere to go in `crate::estree`, nor does the
   `leadingComments` upstream puts on a `<script>`'s Program for the preceding HTML comment
   (used by `svelte-ignore`). `Root.comments` (the `onComment` list) is complete.
8. **`typeAnnotation`** objects that JS-mode `read_type_annotation` can produce (it always runs
   when a `:` follows a pattern) are not stored; their effect on the parser position is.
9. **`customElement`** in `<svelte:options>` is fully validated but only its span is stored
   (`SvelteOptions.custom_element`). For an invalid `tag` property upstream reports
   `svelte_options_invalid_tagname` without a position (the node is a `[name, value]` tuple);
   we report the attribute's span.
10. **Loose-mode crash**: `</div>` closing past the root in loose mode makes upstream throw a
    `TypeError`; we return `element_invalid_closing_tag`.
11. **Warnings** (`element_implicitly_closed`, `script_unknown_attribute`,
    `svelte_element_invalid_this`) are returned by `parse_with_warnings` without
    `svelte-ignore` filtering (upstream's ignore stack is empty while parsing anyway).
12. `CompileError.message` omits upstream's trailing `\nhttps://svelte.dev/e/<code>` line
    (`CompileError::docs_url` rebuilds it).
