# NC1 compiler: deliberate deviations from upstream

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

## acorn regexp validator (`acorn/regexp.rs`, agent B)

None change behaviour (90,940 / 90,940 verdicts agree with acorn 8.19.0, messages included):
- `wordsRegexp` is a word set instead of a generated RegExp.
- BranchIDs live in a list and are compared by index.
- Two dead initial values were dropped (clippy).
- Error offset: acorn raises every regexp error at the pattern start, so `validate_pattern` returns 0; the caller adds the pattern's absolute position.

## Scope analysis (`svelte/scope*`, agent E)

- Node metadata is returned in side tables (`CreatedScopes`), not stored on the AST; `Fragment.metadata.transparent` is recomputed (element fragments are transparent unless they directly contain a `DeclarationTag`).
- Not ported: `Binding.blocker`, `Scope.tracing`, the `importKind === 'type'` exception (no TS nodes).
- `extract_paths` returns only `node`, `is_rest`, `has_default_value`.
- `Evaluation` of a cyclic expression returns the initial in-progress state; cases where upstream throws inside JS (BigInt arithmetic, `in`/`instanceof` on primitives, out-of-range `fromCodePoint`) evaluate to UNKNOWN; lone surrogates become U+FFFD; `Math.*` may differ from V8 in the last ulp.
- `analyze.rs` omits warnings and the legacy-mode binding kinds set by other analysis visitors (`export let` props).

## Native lowering of template blocks (`lower`, `codegen/component.rs`, agent T)

Semantics are specified in `runtime-contract.md` "Template blocks"; departures from Svelte 5:

- **`{#await}` pending in a microtask**: Svelte batches it through `queue_micro_task`; the runtime
  uses one promise job per input. Same order relative to the promise's own reactions (registered
  first), so an already-settled promise never shows `pending` in both.
- **Snippet params, `{@const}`, `{:then}`/`{:catch}`**: plain identifiers only (Svelte supports
  destructuring and parameter defaults via `$.derived`/`$.fallback`): `unsupported_feature`.
- **Top-level snippets** follow Svelte: before the instance script, or at module level when
  they reference nothing of the instance (`can_hoist_snippet`).
- **Template effect order**: Svelte creates one `$.template_effect` per fragment after its blocks
  and components, so a parent's template expressions run after its children initialised. Here
  each reactive prop write is its own render effect created in element order
  (`codegen/component.rs` `DEFER_TEMPLATE_EFFECTS`, off: turning it on reorders GTK/Qt op logs).
  Observable only through side effects in template expressions (`lifecycle-render-order-*`).
- **Dynamic components** (`<Comp />` with `Comp` state/prop/each item/snippet param, or
  `<obj.Comp />`) render through `$n.snippet` (Svelte's `$.component`), re-created when the
  function changes.
- **Legacy `on:event={handler}`** on native elements is a listener, as `onevent={handler}` (both
  run, in attribute order). Svelte attaches it with `addEventListener` (not delegated), so its
  order relative to delegated `onevent` handlers can differ. Modifiers, bare forwarding
  (`on:click`) and `on:` on components are rejected (`unsupported_feature`).
- **`<svelte:boundary>`** (`$n.boundary`, runtime-contract.md): the `pending` snippet is ignored
  (no async mode); `failed` renders in the flush after the task that ran `onerror` (Svelte renders
  it inside that task); no fork batches; dev-mode error adjustments (component stack in the
  message) are Svelte's own and only apply where its `handle_error` runs.
- **`bind:this` on a component** binds its exports (`$.bind_this(Child(...), set, get)`), as Svelte.
- **`{@render}` dynamic test**: `$n.snippet` is used when the callee reads reactive state
  (codegen's `is_reactive`); Svelte uses `node.metadata.dynamic` (callee not a non-reassigned
  snippet declaration). Imported or plain-variable snippet functions are called directly here.
- **`<sv-slot name>`** is not Svelte: a slot proxy for native slots (a custom-element-like tag the
  upstream parser accepts). Spread attributes on native elements are rejected.
- **Style matching through snippets**: a snippet body is matched with the ancestors of its
  declaration site (Svelte's CSS pruning considers render sites).
- **Text-content `{#if}`**: only `{#if}` (no `{#each}`/`{#key}`) folds into a text prop; branch
  edges are trimmed.
- `transform.rs` hook: `BindingKind::Template` reads as `$.get(x)` (read-only), `BindingKind::Snippet`
  as a thunk `x()` (`constant_assignment` on writes).

## Rune transform (`transform/*`, `compile_module`; agent R2)

Follows Svelte 5.57.2's client transform (`phases/3-transform/client`) and emits the same
runtime calls (checked by hand against `svelte/compiler` for every `tests/runes` case), but as
span edits over the user's source instead of an AST reprint.

1. **Text, not AST**: output keeps the user's formatting; generated members (class accessors,
   destructuring declarators, `$$exports`) are written on one line. Destructuring and `$inspect`
   output inside an edited expression is assembled as text (no finer source mapping).
2. **Generated names** (`tmp`, `$$array`, `$$d`, `rest_excludes`) are unique per compilation
   rather than per scope (`scope.generate`), so suffixes can differ from upstream.
3. **`return [a, b] = x`**: upstream treats any `*Statement` parent as standalone and drops the
   value (returns `undefined`); we only treat an `ExpressionStatement` as standalone.
4. **`dev`** keeps `$inspect` / `$inspect().with` / `$inspect.trace` and adds `$.tag` /
   `$.tag_proxy` labels; the other dev-only instrumentation is not emitted (`$.push(.., App)`,
   `$.legacy_api()`, `$.check_target`, `App[$.FILENAME]`, `$.log_if_contains_state`,
   `$.track_reactivity_loss`, `$.assign` mutation warnings, ownership validation).
   `$inspect.trace`'s location label is `file:line:column` (1-based line, 0-based column).
5. **Not supported** (diagnostics): async `$derived` (`await` inside, `unsupported_feature`),
   `$host()` (`host_invalid_placement`: no custom elements), re-exports from the instance
   script, destructuring assignments through store/prop member paths (assigned plainly).
6. `$.push($$props, true)` is always emitted (upstream omits it when no context is needed).
7. `$props.id()` is evaluated in place, not hoisted above `$.push` (only matters for hydration).
8. Validation is the subset listed in `language-subset.md`, reported as diagnostics; analysis
   errors from the scope port still abort at the first error, transform errors accumulate.
9. Template reactivity (`is_reactive`) approximates upstream's `has_state`: any identifier bound
   to something other than a function, a never-reassigned literal or an each index counts as
   stateful (so `{obj.field}` with class state fields or imported proxies updates).
