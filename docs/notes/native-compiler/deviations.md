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
2. **TypeScript** (`lang="ts"`) is parsed with the acorn-typescript port (`acorn::typescript`,
   see its section below) exactly where upstream uses `TSParser`; there is no deviation in
   `svelte::parse` itself. (Before agent TS, `lang="ts"` was rejected with the non-upstream code
   `svelters_typescript_unsupported`; that code no longer exists.)
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
   when a `:` follows a pattern) are not stored; their effect on the parser position is. In a
   `lang="ts"` component they are stored (`TsPatternExtra::type_annotation`) when the
   `_ as <type>` trick yields a `TSAsExpression`, which is always the case for input it accepts.
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

## acorn-typescript (`acorn/typescript/`, agent TS)

Port of @sveltejs/acorn-typescript 1.0.13 (`node_modules/@sveltejs/acorn-typescript/index.js`),
the plugin Svelte's `TSParser` uses (default options: no JSX, no `dts`), as overrides of the acorn
port switched on by `AcornOptions::typescript` (plain JS never takes a TypeScript path). Checked by
the oracle's TypeScript cases (`options.typescript`, see tools/oracle/README.md).

1. **Speculative parsing** (`tryParse`, `tsTryParse`, `tsLookAhead`): the plugin's
   `ParseEffects` undo log becomes a `Checkpoint` (a copy of the token state, context stack,
   scopes, labels, private names, undefined exports, plugin state and the comment count) that is
   restored on rollback; comments reported by a rolled-back branch are dropped and re-reported by
   `selectTryParseResult`, as the plugin's event buffer does.
2. **Typed nodes**: properties the plugin adds to ESTree nodes live in their `ts` field
   (`estree::ts`); `toAssignable` results that upstream discards (it converts in place) are kept
   as `TSPatternWrapper` patterns (`[(x as T)] = v`), which serialize identically.
3. **Unrepresentable error paths**: when a `<T>` type assertion fails the plugin returns a
   `TSTypeParameterDeclaration` as an *expression*; here the type parameters are parsed and the
   following token is reported as unexpected. Likewise `(a,,b) => T` in a type and decorators
   followed by `;` in a class body (TypeErrors in the plugin) are "Unexpected token".
   `TSTypeCastExpression` passed to `toAssignable` outside a parameter list is converted to the
   annotated pattern (the plugin returns the bare expression).
4. **Decorator member expressions** (`@a.b`) get `optional: false` (the plugin builds them without
   `optional`); the only acorn-typescript corpus mismatch. Decorators are rejected by
   `remove_typescript_nodes` anyway. Decorators on object-literal members are not stored.
5. **Error positions the plugin passes as `loc.column`** (modifier order errors,
   `UnexpectedParameterModifier`) use the UTF-16 column as a byte offset, which is the same
   number for ASCII lines. `extra.trailingComma` of type parameter lists is a byte offset.

## remove_typescript_nodes (`svelte/parse/remove_typescript_nodes.rs`, `svelte/ts.rs`, agent TS)

Upstream runs it in `compile()`; here `svelte::ts::strip` runs it from `compile.rs` before scope
analysis and also records the source ranges of the removed TypeScript, because the compiler emits
the user's script and template text through span edits instead of reprinting the AST (the
rewriter adds the ranges as empty overwrites: `ScopeRewriter::strip_typescript`).

1. Removed statements are `EmptyStatement`s with their original span (upstream `b.empty` has no
   position); removed class members (abstract methods, `declare` fields) are dropped from the
   class body instead of becoming `EmptyStatement`s.
2. Nodes upstream leaves for esrap to print as TypeScript are removed too: method overloads
   (`TSDeclareMethod` values), class index signatures and `TSTypeCastExpression` (`(x: T)` that
   is not a parameter list); `import x = require()`, `export =` and `export as namespace` are
   `typescript_invalid_feature` errors (feature names "`import =` declarations", "`export =`
   assignments", "`export as namespace` declarations").
3. A nested `namespace A.B {}` is checked like a flat one (upstream throws a TypeError).
4. `import(source, options)` parsed by the TypeScript parser (`arguments: [options]`) gets its
   `options` back so later phases see one shape.

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

- **Destructuring template bindings** (snippet parameters with patterns or defaults, `{#each}`
  contexts, `{@const}`, `{:then}`/`{:catch}`): Svelte's `extract_paths` as in its visitors, but
  every leaf of an each item / `{@const}` / await value is a `$.derived_safe_equal` (Svelte: a thunk
  re-evaluated per read, or one derived object for `{@const}`); snippet leaves are thunks as in
  Svelte. A keyed each destructures inside its key function instead of in its parameter list.
- **Top-level snippets** follow Svelte: before the instance script, or at module level when
  they reference nothing of the instance (`can_hoist_snippet`).
- **Template effects** follow Svelte since CF2: one `$.template_effect` per fragment, created
  after its blocks and components, then the events and bindings (Svelte's `after_update`), with
  the fragment's call expressions memoized as its dependencies (runtime-contract.md "Shape of
  generated code"). Departures: the init order of an element is native (its children are inserted
  into it, then it into its parent, before the template effect: Svelte clones the whole template
  first, then appends the fragment last); a toplevel shown at the root is shown when the outermost
  build returns (Svelte has no toplevels). A reactive `{@const}`/snippet scope inside an element
  is its own fragment (Svelte: the element's children in a block with their own effect).
- **Dynamic components** (`<Comp />` with `Comp` state/prop/each item/snippet param, or
  `<obj.Comp />`) render through `$n.snippet` (Svelte's `$.component`), re-created when the
  function changes. `<svelte:component this={expr}>` likewise (always dynamic).
- **Legacy `on:event={handler}`** on native elements is a listener, as `onevent={handler}` (both
  run, in attribute order). Svelte attaches it with `addEventListener` (not delegated), so its
  order relative to delegated `onevent` handlers can differ. On a component, `on:event={h}` is the
  prop `onevent={h}`.
- **Bare `on:event` forwards one callback prop** (FW1, ARCH-EMBED §8.7). Inside a component, a bare
  `on:clicked` on an element is the listener `onclicked={$$props.onclicked}` and on a child component
  the prop `onclicked={$$props.onclicked}` (`$$props`: the compiler's own props parameter). Svelte 4
  re-dispatched a component event to every `$on` listener; here the one handler the parent passes
  gets it, and there are no component events (`createEventDispatcher`, `$on`). A note,
  `legacy_forwarding_lowered`, says so once per component. A bare `on:x` next to another handler of
  `x` (`onx={h}`, `on:x={h}`) and two `onx` props on a component, one from `on:`, are
  `duplicate_event` errors. A snippet with a bare `on:` is not hoisted to module level. Modifiers
  (`on:click|once`) stay `unsupported_feature`, with the `onclick={...}` rewrite as help.
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
- **`<ListView>` from `@svelters/lists`** (runtime-contract.md "Virtualized lists") is the
  native runtime's own implementation (`src/lists.js`; since RT1 the package only re-exports it).
  Same props, exports and op log as the retired JS pipeline's `ListView.svelte` except the destroy
  order inside a teardown (content, then outlet; the JS pipeline destroyed the outlet first). Each row is a separate root mounted into its
  outlet, with the list's contexts copied in (Svelte's `mount(RowHost, { context })` in the JS
  pipeline). Rest props reach the native `virtuallist` unchecked (it is not in the specs).
- `transform.rs` hook: `BindingKind::Template` reads as `$.get(x)` (read-only), `BindingKind::Snippet`
  as a thunk `x()` (`constant_assignment` on writes).

## Child attributes, phases, sync handlers (`lower/native_attrs.rs`, agent I1)

- **Spreads with static keys** are expanded at compile time only for an object literal
  (`{...{ 'onnotify::x': f }}`). Since SP1 a top-level `const` object goes through the runtime
  spread like any other expression, so keys added to it later are applied, as in Svelte (the old
  deviation is gone). Runtime spreads: runtime-contract.md "Spread attributes".
- **`listen()` import**: from `@svelters/native-runtime` (RT1: `@svelters/renderer` is gone),
  which provides `listen`, `NativeEvent`, `StaleHandleError`, `LISTEN_*`.
- **Explicit default slot names** (`slot="center"` naming the default slot) are passed by name to
  `insert`, as the renderer does (they used to be normalised to the default slot).
- **`track` is passive** (renderer parity): a one-way `text={x}` with no listener no longer makes the
  host listen to `changed`.
- **Child attribute op order** within one update follows the fragment's template effect (element
  order, as Svelte's output the JS pipeline ran).

## Runtime spreads and `<svelte:element>` (SP1)

- **Unknown keys** of a runtime spread (or any attribute of `<svelte:element>`) are skipped with a
  DEV warning carrying the compiler's code (`unknown_property`, `unknown_event`, ...). Svelte would
  set an attribute; a native element has no free-form attributes.
- **A string for a typed prop** is coerced as a static attribute is (`"120"` to an integer); a value
  that does not coerce is skipped with a DEV `bad_attribute_value` warning.
- **A key that goes away** resets the prop to the spec default, else to the native value read before
  the spread first wrote it. Svelte removes the attribute; a native prop cannot be removed.
- **`<svelte:element>` bindings**: only `bind:this`. Other `bind:` directives need the kind's binding
  plan at compile time (`dynamic_element_bind`).
- **`this`** names a native tag of the backend; a tag it does not have renders nothing and warns in
  DEV (Svelte would create an unknown HTML element).

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
   Snippets get Svelte's dev shape: `$.wrap_snippet(App, function ($$anchor, ...) {
   $n.validate_snippet_args(...arguments); ... })`, the implicit `children` snippet wrapped too, so
   `{snippet}` throws `snippet_without_render_tag` and a direct call `invalid_snippet_arguments`
   (the anchor must be a runtime container, not a DOM `Node`). `$n.snippet` throws
   `invalid_snippet` on a nullish snippet when esm-env's `DEV` is set, as Svelte's snippet block.
   A non-inline event handler goes through `$n.apply(() => handler, this, $$args, event, loc, ...)`
   (Svelte's `$.apply`, `event_handler_invalid`), with the event name and `file:line:column`
   static instead of read from the DOM event and `Component[FILENAME]`.
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
