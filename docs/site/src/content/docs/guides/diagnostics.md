---
title: Compiler diagnostics
description: Every diagnostic code the svelters compiler reports, with its stable namespaced id, severity and fix-it.
---

<!-- Generated from crates/svelters-compiler/src/codes.rs by `UPDATE=1 cargo nextest run -p svelters-compiler --test cx1_codes`. Do not edit. -->

Every diagnostic has a short **code** (`bind_partially_observed`, printed as `error[code]` by `svelters-compile`) and a stable namespaced **id** (`svelters::bind::partially_observed`) that tools should key on. Ids are never renamed or reused. Codes from Svelte's own compiler keep Svelte's code under `svelte::` and are documented on svelte.dev; the native compiler's own codes are `svelters::<area>::<name>`.

Severities: **error** fails the build; **warning** does not (the observability warnings become errors under `--strict-native`); **note** explains how something was lowered and never fails a build.

`svelters-compile --check --json <file>` reports diagnostics without generating code, with byte and line/column ranges and fix-its as text edits; `svelters-compile --explain <code or id>` prints one row of this page. In Rust, `svelters_compiler::check::check` does the same in process, with incremental state per file.

## svelters codes

### element

| Id | Code | Severity | Meaning | Fix-it |
| --- | --- | --- | --- | --- |
| `svelters::element::unknown` | `unknown_element` | error | The tag is not an element of the backend's semantic spec (nor a component in scope). | "did you mean": replace the name with the suggestion |
| `svelters::element::children_not_allowed` | `children_not_allowed` | error | The element takes no child elements on this backend. |  |
| `svelters::element::text_not_allowed` | `text_not_allowed` | error | Text or an expression tag where the parent cannot hold text; only elements whose spec has text content (e.g. `<label>`) can. |  |
| `svelters::element::text_content_children` | `text_content_children` | error | A text-content element has both text and child elements; it takes one or the other. |  |
| `svelters::element::show_invalid` | `show_invalid` | error | A toplevel's `show` is not a static `modal`, `modeless` or `none`. |  |
| `svelters::element::bad_attribute_value` | `bad_attribute_value` | error | A static attribute value cannot be converted to the property's type. |  |

### property

| Id | Code | Severity | Meaning | Fix-it |
| --- | --- | --- | --- | --- |
| `svelters::property::unknown` | `unknown_property` | error | The element has no property of that name on this backend. | "did you mean": replace the name with the suggestion |
| `svelters::property::read_only` | `read_only_property` | error | The property cannot be written (or, for `bind:`, is write-only or not readable and writable). | set it one way: `bind:x={v}` becomes the attribute the help names |

### method

| Id | Code | Severity | Meaning | Fix-it |
| --- | --- | --- | --- | --- |
| `svelters::method::unknown` | `unknown_method` | error | The element has no annotated method of that name. | "did you mean": replace the name with the suggestion |

### event

| Id | Code | Severity | Meaning | Fix-it |
| --- | --- | --- | --- | --- |
| `svelters::event::unknown` | `unknown_event` | error | The element has no event of that name on this backend. | "did you mean": replace the name with the suggestion |
| `svelters::event::phase_unsupported` | `listen_phase_unsupported` | error | The event's signal does not support the requested phase (`capture`, ...). |  |
| `svelters::event::duplicate` | `duplicate_event` | error | A bare `on:x` (forwarding) is not the only handler of `x`, or a component gets the prop `onx` twice. |  |
| `svelters::event::forwarding_lowered` | `legacy_forwarding_lowered` | note | A bare `on:x` forwards the component's own `onx` callback prop (FW1); once per component. |  |
| `svelters::event::sync_handler_arguments` | `sync_handler_arguments` | warning | A handler of a sync signal declares a parameter for a signal argument, but listeners get `(event, ...args)`. |  |
| `svelters::event::sync_handler_return` | `sync_handler_return` | warning | The return value of a sync signal's listener is ignored. |  |

### bind

| Id | Code | Severity | Meaning | Fix-it |
| --- | --- | --- | --- | --- |
| `svelters::bind::no_plan` | `bind_no_plan` | error | `bind:` on a property with no binding plan: whether its native changes are announced is unknown. | "did you mean": replace the name with the suggestion |
| `svelters::bind::not_observable` | `bind_not_observable` | error | `bind:` on a property whose native changes are never announced (or that never changes). | set it one way: `bind:x={v}` becomes the attribute the help names |
| `svelters::bind::set_only` | `bind_set_only` | error | `bind:` on a set-only property: only the app's own writes change it, so there is nothing to bind back. | set it one way: `bind:x={v}` becomes the attribute the help names |
| `svelters::bind::partially_observed` | `bind_partially_observed` | warning | `bind:` on a property only some of whose native changes are announced; the bound value can go stale (error under `--strict-native`). |  |
| `svelters::bind::observation_unverified` | `bind_observation_unverified` | warning | `bind:` on an `exact` property whose change delivery no generated row has seen (error under `--strict-native`). |  |
| `svelters::bind::modifier_invalid` | `bind_modifier_invalid` | error | An unknown, repeated or meaningless `bind:` modifier (only `\|nonreactive`, on native element properties). | remove the modifier |
| `svelters::bind::nonreactive_redundant` | `nonreactive_redundant` | warning | `bind:x\|nonreactive` on a property that is set-only or constant, so `\|nonreactive` changes nothing (error under `--strict-native`). | set it one way: `bind:x={v}` becomes the attribute the help names |
| `svelters::bind::observability_differs` | `observability_differs` | warning | Portability mode: the bound property is observed differently on the other platforms (error under `--strict-native`). |  |
| `svelters::bind::and_prop` | `bind_and_prop` | error | A property is both bound with `bind:` and set explicitly on the same element. |  |
| `svelters::bind::dynamic_element` | `dynamic_element_bind` | error | `bind:` on `<svelte:element>`: the binding plan needs the element's kind at compile time. |  |

### reactivity

| Id | Code | Severity | Meaning | Fix-it |
| --- | --- | --- | --- | --- |
| `svelters::reactivity::native_read_not_observable` | `native_read_not_observable` | warning | A reactive context reads a native property whose changes are not announced, so it does not rerun when the property changes (error under `--strict-native`). |  |

### slot

| Id | Code | Severity | Meaning | Fix-it |
| --- | --- | --- | --- | --- |
| `svelters::slot::unknown` | `unknown_slot` | error | The parent element has no slot of that name. |  |
| `svelters::slot::no_default` | `no_default_slot` | error | The parent element has no default slot; name one with `slot="..."`. |  |
| `svelters::slot::rejects_child` | `slot_rejects_child` | error | The slot does not accept this kind of child. |  |
| `svelters::slot::cardinality` | `slot_cardinality` | error | A single-child slot already has a child. |  |
| `svelters::slot::conflict` | `slot_conflict` | error | An element or snippet asks for a slot other than the one its `<sv-slot>` names. |  |
| `svelters::slot::not_applicable` | `slot_not_applicable` | error | The child attaches to its parent in another way and takes no slot. |  |
| `svelters::slot::not_static` | `slot_not_static` | error | `slot` (or `<sv-slot name>`) is not a static name. |  |
| `svelters::slot::proxy_attribute` | `slot_proxy_attribute` | error | `<sv-slot>` takes exactly one static `name` attribute. |  |
| `svelters::slot::proxy_placement` | `slot_proxy_placement` | error | `<sv-slot>` is not a direct child of a native element, or is nested in another `<sv-slot>`. |  |
| `svelters::slot::mixed_in_block` | `block_mixed_slots` | error | The elements of one block (or the roots of one snippet) go to different slots of their parent. |  |

### child_attr

| Id | Code | Severity | Meaning | Fix-it |
| --- | --- | --- | --- | --- |
| `svelters::child_attr::unknown` | `unknown_child_attr` | error | The slot has no child attribute (`parent:<name>`) of that name. | "did you mean": replace the name with the suggestion |
| `svelters::child_attr::bad_value` | `child_attr_bad_value` | error | A static `parent:<name>` value cannot be converted to the child attribute's type. |  |

### style

| Id | Code | Severity | Meaning | Fix-it |
| --- | --- | --- | --- | --- |
| `svelters::style::syntax` | `style_syntax` | error | A syntax error in `<style>`. |  |
| `svelters::style::unsupported_selector` | `style_unsupported_selector` | error | A selector form native styles do not support (`*`, pseudo-elements, ...). |  |
| `svelters::style::unsupported_combinator` | `style_unsupported_combinator` | error | A combinator other than descendant (whitespace) and child (`>`). |  |
| `svelters::style::unsupported_at_rule` | `style_unsupported_at_rule` | error | A setter property inside an at-rule (`@media`, `@native`); setter properties are written with setProp, not CSS. |  |
| `svelters::style::unsupported_nesting` | `style_unsupported_nesting` | error | A setter property in a nested rule. |  |
| `svelters::style::unsupported_important` | `style_unsupported_important` | error | `!important` is not supported; native styles apply by specificity and source order. | remove `!important` |
| `svelters::style::unsupported_value` | `style_unsupported_value` | error | A value form native styles do not support (`,` or `/` lists, ...). |  |
| `svelters::style::unknown_declaration` | `style_unknown_declaration` | error | A declaration no setter property or CSS property of the backend knows (reserved; not reported yet). |  |
| `svelters::style::not_applicable` | `style_not_applicable` | error | The declaration applies to other elements (or to the children of other elements) on this backend. |  |
| `svelters::style::bad_value` | `style_bad_value` | error | A declaration's value cannot be converted to the property's type (or uses an unsupported unit). |  |
| `svelters::style::conflicts_binding` | `style_conflicts_binding` | error | A style sets a property that is bound with `bind:` on the element. |  |
| `svelters::style::overridden` | `style_overridden` | warning | A style sets a property that the element also sets explicitly; the explicit value wins. |  |
| `svelters::style::unused_selector` | `style_unused_selector` | warning | The selector matches no element of the component, so its declarations are not checked. |  |

### list

| Id | Code | Severity | Meaning | Fix-it |
| --- | --- | --- | --- | --- |
| `svelters::list::children` | `list_children` | error | A virtualized list has children other than its `row` snippet. |  |
| `svelters::list::row_missing` | `list_row_missing` | error | A virtualized list has no `row` snippet. |  |
| `svelters::list::row_params` | `list_row_params` | error | The `row` snippet takes more than `(item, index)`. |  |
| `svelters::list::row_toplevel` | `list_row_toplevel` | error | A row of a virtualized list is a toplevel element. |  |
| `svelters::list::prop_invalid` | `list_prop_invalid` | error | A list property has a value outside its allowed set. |  |
| `svelters::list::bind_invalid` | `list_bind_invalid` | error | A list binds something other than `bind:selected` or `bind:this`. |  |

### transition

| Id | Code | Severity | Meaning | Fix-it |
| --- | --- | --- | --- | --- |
| `svelters::transition::modifier_invalid` | `transition_modifier_invalid` | error | A transition modifier other than `local` or `global`. | remove the modifier |

### feature

| Id | Code | Severity | Meaning | Fix-it |
| --- | --- | --- | --- | --- |
| `svelters::feature::unsupported` | `unsupported_feature` | error | A Svelte feature the native compiler does not support (legacy syntax, event modifiers, re-exports, async `$derived`, ...); see the language subset. | event modifiers: the handler rewritten without them, when that is exact |

## Svelte codes

The parser, scope analysis and rune transform are ported from Svelte 5.57.2 and report Svelte's codes (with svelters' wording where the native compiler differs). The meaning column is Svelte's message template.

| Id | Severity | Meaning | Fix-it |
| --- | --- | --- | --- |
| [`svelte::animation_duplicate`](https://svelte.dev/e/animation_duplicate) | error | An element can only have one 'animate' directive |  |
| [`svelte::animation_invalid_placement`](https://svelte.dev/e/animation_invalid_placement) | error | An element that uses the `animate:` directive must be the only child of a keyed `{#each ...}` block |  |
| [`svelte::animation_missing_key`](https://svelte.dev/e/animation_missing_key) | error | An element that uses the `animate:` directive must be the only child of a keyed `{#each ...}` block. Did you forget to add a key to your each block? |  |
| [`svelte::attribute_duplicate`](https://svelte.dev/e/attribute_duplicate) | error | Attributes need to be unique |  |
| [`svelte::attribute_empty_shorthand`](https://svelte.dev/e/attribute_empty_shorthand) | error | Attribute shorthand cannot be empty |  |
| [`svelte::bind_invalid_target`](https://svelte.dev/e/bind_invalid_target) | error | `bind:%name%` can only be used with %elements% |  |
| [`svelte::bind_invalid_value`](https://svelte.dev/e/bind_invalid_value) | error | Can only bind to state or props |  |
| [`svelte::bindable_invalid_location`](https://svelte.dev/e/bindable_invalid_location) | error | `$bindable()` can only be used inside a `$props()` declaration |  |
| [`svelte::block_duplicate_clause`](https://svelte.dev/e/block_duplicate_clause) | error | %name% cannot appear more than once within a block |  |
| [`svelte::block_invalid_continuation_placement`](https://svelte.dev/e/block_invalid_continuation_placement) | error | {:...} block is invalid at this position (did you forget to close the preceding element or block?) |  |
| [`svelte::block_invalid_elseif`](https://svelte.dev/e/block_invalid_elseif) | error | 'elseif' should be 'else if' | replace `elseif` with `else if` |
| [`svelte::block_invalid_placement`](https://svelte.dev/e/block_invalid_placement) | error | {#%name% ...} block cannot be %location% |  |
| [`svelte::block_unclosed`](https://svelte.dev/e/block_unclosed) | error | Block was left open |  |
| [`svelte::block_unexpected_close`](https://svelte.dev/e/block_unexpected_close) | error | Unexpected block closing tag |  |
| [`svelte::const_tag_invalid_expression`](https://svelte.dev/e/const_tag_invalid_expression) | error | {@const ...} must consist of a single variable declaration |  |
| [`svelte::const_tag_invalid_placement`](https://svelte.dev/e/const_tag_invalid_placement) | error | `{@const}` must be the immediate child of `{#snippet}`, `{#if}`, `{:else if}`, `{:else}`, `{#each}`, `{:then}`, `{:catch}`, `<svelte:fragment>`, `<svelte:boundary>` or `<Component>` |  |
| [`svelte::constant_assignment`](https://svelte.dev/e/constant_assignment) | error | Cannot assign to %thing% |  |
| [`svelte::css_empty_declaration`](https://svelte.dev/e/css_empty_declaration) | error | Declaration cannot be empty |  |
| [`svelte::css_expected_identifier`](https://svelte.dev/e/css_expected_identifier) | error | Expected a valid CSS identifier |  |
| [`svelte::css_global_block_invalid_combinator`](https://svelte.dev/e/css_global_block_invalid_combinator) | error | A `:global` selector cannot follow a `%name%` combinator |  |
| [`svelte::css_global_block_invalid_declaration`](https://svelte.dev/e/css_global_block_invalid_declaration) | error | A top-level `:global {...}` block can only contain rules, not declarations |  |
| [`svelte::css_global_block_invalid_list`](https://svelte.dev/e/css_global_block_invalid_list) | error | A `:global` selector cannot be part of a selector list with entries that don't contain `:global` |  |
| [`svelte::css_global_block_invalid_modifier`](https://svelte.dev/e/css_global_block_invalid_modifier) | error | A `:global` selector cannot modify an existing selector |  |
| [`svelte::css_global_block_invalid_modifier_start`](https://svelte.dev/e/css_global_block_invalid_modifier_start) | error | A `:global` selector can only be modified if it is a descendant of other selectors |  |
| [`svelte::css_global_block_invalid_placement`](https://svelte.dev/e/css_global_block_invalid_placement) | error | A `:global` selector cannot be inside a pseudoclass |  |
| [`svelte::css_global_invalid_placement`](https://svelte.dev/e/css_global_invalid_placement) | error | `:global(...)` can be at the start or end of a selector sequence, but not in the middle |  |
| [`svelte::css_global_invalid_selector`](https://svelte.dev/e/css_global_invalid_selector) | error | `:global(...)` must contain exactly one selector |  |
| [`svelte::css_global_invalid_selector_list`](https://svelte.dev/e/css_global_invalid_selector_list) | error | `:global(...)` must not contain type or universal selectors when used in a compound selector |  |
| [`svelte::css_nesting_selector_invalid_placement`](https://svelte.dev/e/css_nesting_selector_invalid_placement) | error | Nesting selectors can only be used inside a rule or as the first selector inside a lone `:global(...)` |  |
| [`svelte::css_selector_invalid`](https://svelte.dev/e/css_selector_invalid) | error | Invalid selector |  |
| [`svelte::css_type_selector_invalid_placement`](https://svelte.dev/e/css_type_selector_invalid_placement) | error | `:global(...)` must not be followed by a type selector |  |
| [`svelte::css_unused_selector`](https://svelte.dev/e/css_unused_selector) | warning | Unused CSS selector "%name%" |  |
| [`svelte::debug_tag_invalid_arguments`](https://svelte.dev/e/debug_tag_invalid_arguments) | error | {@debug ...} arguments must be identifiers, not arbitrary expressions |  |
| [`svelte::declaration_duplicate`](https://svelte.dev/e/declaration_duplicate) | error | `%name%` has already been declared |  |
| [`svelte::declaration_duplicate_module_import`](https://svelte.dev/e/declaration_duplicate_module_import) | error | Cannot declare a variable with the same name as an import from `<script module>` |  |
| [`svelte::declaration_tag_invalid_type`](https://svelte.dev/e/declaration_tag_invalid_type) | error | Declaration tags must be `let` or `const` declarations |  |
| [`svelte::derived_invalid_export`](https://svelte.dev/e/derived_invalid_export) | error | Cannot export derived state from a module. To expose the current derived value, export a function returning its value |  |
| [`svelte::directive_invalid_value`](https://svelte.dev/e/directive_invalid_value) | error | Directive value must be a JavaScript expression enclosed in curly braces |  |
| [`svelte::directive_missing_name`](https://svelte.dev/e/directive_missing_name) | error | `%type%` name cannot be empty |  |
| [`svelte::dollar_binding_invalid`](https://svelte.dev/e/dollar_binding_invalid) | error | The $ name is reserved, and cannot be used for variables and imports |  |
| [`svelte::dollar_prefix_invalid`](https://svelte.dev/e/dollar_prefix_invalid) | error | The $ prefix is reserved, and cannot be used for variables and imports |  |
| [`svelte::each_item_invalid_assignment`](https://svelte.dev/e/each_item_invalid_assignment) | error | Cannot reassign or bind to each block argument in runes mode. Use the array and index variables instead (e.g. `array[i] = value` instead of `entry = value`, or `bind:value={array[i]}` instead of `bind:value={entry}`) |  |
| [`svelte::effect_invalid_placement`](https://svelte.dev/e/effect_invalid_placement) | error | `$effect()` can only be used as an expression statement |  |
| [`svelte::element_implicitly_closed`](https://svelte.dev/e/element_implicitly_closed) | warning | This element is implicitly closed by the following `%tag%`, which can cause an unexpected DOM structure. Add an explicit `%closing%` to avoid surprises. |  |
| [`svelte::element_invalid_closing_tag`](https://svelte.dev/e/element_invalid_closing_tag) | error | `</%name%>` attempted to close an element that was not open |  |
| [`svelte::element_invalid_closing_tag_autoclosed`](https://svelte.dev/e/element_invalid_closing_tag_autoclosed) | error | `</%name%>` attempted to close element that was already automatically closed by `<%reason%>` (cannot nest `<%reason%>` inside `<%name%>`) |  |
| [`svelte::element_unclosed`](https://svelte.dev/e/element_unclosed) | error | `<%name%>` was left open |  |
| [`svelte::expected_attribute_value`](https://svelte.dev/e/expected_attribute_value) | error | Expected attribute value |  |
| [`svelte::expected_block_type`](https://svelte.dev/e/expected_block_type) | error | Expected 'if', 'each', 'await', 'key' or 'snippet' |  |
| [`svelte::expected_identifier`](https://svelte.dev/e/expected_identifier) | error | Expected an identifier |  |
| [`svelte::expected_pattern`](https://svelte.dev/e/expected_pattern) | error | Expected identifier or destructure pattern |  |
| [`svelte::expected_tag`](https://svelte.dev/e/expected_tag) | error | Expected 'html', 'render', 'attach', 'const', or 'debug' |  |
| [`svelte::expected_token`](https://svelte.dev/e/expected_token) | error | Expected token %token% |  |
| [`svelte::expected_whitespace`](https://svelte.dev/e/expected_whitespace) | error | Expected whitespace |  |
| [`svelte::global_reference_invalid`](https://svelte.dev/e/global_reference_invalid) | error | `%name%` is an illegal variable name. To reference a global variable called `%name%`, use `globalThis.%name%` |  |
| [`svelte::host_invalid_placement`](https://svelte.dev/e/host_invalid_placement) | error | `$host()` can only be used inside custom element component instances |  |
| [`svelte::inspect_trace_invalid_placement`](https://svelte.dev/e/inspect_trace_invalid_placement) | error | `$inspect.trace(...)` must be the first statement of a function body |  |
| [`svelte::js_parse_error`](https://svelte.dev/e/js_parse_error) | error | %message% |  |
| [`svelte::legacy_export_invalid`](https://svelte.dev/e/legacy_export_invalid) | error | Cannot use `export let` in runes mode — use `$props()` instead |  |
| [`svelte::module_illegal_default_export`](https://svelte.dev/e/module_illegal_default_export) | error | A component cannot have a default export |  |
| [`svelte::props_duplicate`](https://svelte.dev/e/props_duplicate) | error | Cannot use `%rune%()` more than once |  |
| [`svelte::props_id_invalid_placement`](https://svelte.dev/e/props_id_invalid_placement) | error | `$props.id()` can only be used at the top level of components as a variable declaration initializer |  |
| [`svelte::props_illegal_name`](https://svelte.dev/e/props_illegal_name) | error | Declaring or accessing a prop starting with `$$` is illegal (they are reserved for Svelte internals) |  |
| [`svelte::props_invalid_identifier`](https://svelte.dev/e/props_invalid_identifier) | error | `$props()` can only be used with an object destructuring pattern |  |
| [`svelte::props_invalid_pattern`](https://svelte.dev/e/props_invalid_pattern) | error | `$props()` assignment must not contain nested properties or computed keys |  |
| [`svelte::props_invalid_placement`](https://svelte.dev/e/props_invalid_placement) | error | `$props()` can only be used at the top level of components as a variable declaration initializer |  |
| [`svelte::render_tag_invalid_expression`](https://svelte.dev/e/render_tag_invalid_expression) | error | `{@render ...}` tags can only contain call expressions |  |
| [`svelte::render_tag_invalid_spread_argument`](https://svelte.dev/e/render_tag_invalid_spread_argument) | error | cannot use spread arguments in `{@render ...}` tags |  |
| [`svelte::rune_invalid_arguments`](https://svelte.dev/e/rune_invalid_arguments) | error | `%rune%` cannot be called with arguments |  |
| [`svelte::rune_invalid_arguments_length`](https://svelte.dev/e/rune_invalid_arguments_length) | error | `%rune%` must be called with %args% |  |
| [`svelte::rune_invalid_spread`](https://svelte.dev/e/rune_invalid_spread) | error | `%rune%` cannot be called with a spread argument |  |
| [`svelte::rune_invalid_usage`](https://svelte.dev/e/rune_invalid_usage) | error | Cannot use `%rune%` rune in non-runes mode |  |
| [`svelte::script_duplicate`](https://svelte.dev/e/script_duplicate) | error | A component can have a single top-level `<script>` element and/or a single top-level `<script module>` element |  |
| [`svelte::script_invalid_attribute_value`](https://svelte.dev/e/script_invalid_attribute_value) | error | If the `%name%` attribute is supplied, it must be a boolean attribute |  |
| [`svelte::script_invalid_context`](https://svelte.dev/e/script_invalid_context) | error | If the context attribute is supplied, its value must be "module" |  |
| [`svelte::script_reserved_attribute`](https://svelte.dev/e/script_reserved_attribute) | error | The `%name%` attribute is reserved and cannot be used |  |
| [`svelte::script_unknown_attribute`](https://svelte.dev/e/script_unknown_attribute) | warning | Unrecognised attribute — should be one of `generics`, `lang` or `module`. If this exists for a preprocessor, ensure that the preprocessor removes it |  |
| [`svelte::snippet_conflict`](https://svelte.dev/e/snippet_conflict) | error | Cannot use explicit children snippet at the same time as implicit children content. Remove either the non-whitespace content or the children snippet block |  |
| [`svelte::snippet_parameter_assignment`](https://svelte.dev/e/snippet_parameter_assignment) | error | Cannot reassign or bind to snippet parameter |  |
| [`svelte::state_field_duplicate`](https://svelte.dev/e/state_field_duplicate) | error | `%name%` has already been declared on this class |  |
| [`svelte::state_invalid_export`](https://svelte.dev/e/state_invalid_export) | error | Cannot export state from a module if it is reassigned. Either export a function returning the state value or only mutate the state value's properties |  |
| [`svelte::state_invalid_placement`](https://svelte.dev/e/state_invalid_placement) | error | `%rune%(...)` can only be used as a variable declaration initializer, a class field declaration, or the first assignment to a class field at the top level of the constructor. |  |
| [`svelte::store_invalid_scoped_subscription`](https://svelte.dev/e/store_invalid_scoped_subscription) | error | Cannot subscribe to stores that are not declared at the top level of the component |  |
| [`svelte::store_invalid_subscription`](https://svelte.dev/e/store_invalid_subscription) | error | Cannot reference store value inside `<script module>` |  |
| [`svelte::store_invalid_subscription_module`](https://svelte.dev/e/store_invalid_subscription_module) | error | Cannot reference store value outside a `.svelte` file |  |
| [`svelte::style_duplicate`](https://svelte.dev/e/style_duplicate) | error | A component can have a single top-level `<style>` element |  |
| [`svelte::svelte_component_invalid_this`](https://svelte.dev/e/svelte_component_invalid_this) | error | Invalid component definition — must be an `{expression}` |  |
| [`svelte::svelte_component_missing_this`](https://svelte.dev/e/svelte_component_missing_this) | error | `<svelte:component>` must have a 'this' attribute |  |
| [`svelte::svelte_element_invalid_this`](https://svelte.dev/e/svelte_element_invalid_this) | warning | `this` should be an `{expression}`. Using a string attribute value will cause an error in future versions of Svelte |  |
| [`svelte::svelte_element_missing_this`](https://svelte.dev/e/svelte_element_missing_this) | error | `<svelte:element>` must have a 'this' attribute with a value |  |
| [`svelte::svelte_meta_duplicate`](https://svelte.dev/e/svelte_meta_duplicate) | error | A component can only have one `<%name%>` element |  |
| [`svelte::svelte_meta_invalid_content`](https://svelte.dev/e/svelte_meta_invalid_content) | error | &lt;%name%&gt; cannot have children |  |
| [`svelte::svelte_meta_invalid_placement`](https://svelte.dev/e/svelte_meta_invalid_placement) | error | `<%name%>` tags cannot be inside elements or blocks |  |
| [`svelte::svelte_meta_invalid_tag`](https://svelte.dev/e/svelte_meta_invalid_tag) | error | Valid `<svelte:...>` tag names are %list% |  |
| [`svelte::svelte_options_deprecated_tag`](https://svelte.dev/e/svelte_options_deprecated_tag) | error | "tag" option is deprecated — use "customElement" instead |  |
| [`svelte::svelte_options_invalid_attribute`](https://svelte.dev/e/svelte_options_invalid_attribute) | error | `<svelte:options>` can only receive static attributes |  |
| [`svelte::svelte_options_invalid_attribute_value`](https://svelte.dev/e/svelte_options_invalid_attribute_value) | error | Value must be %list%, if specified |  |
| [`svelte::svelte_options_invalid_customelement`](https://svelte.dev/e/svelte_options_invalid_customelement) | error | "customElement" must be a string literal defining a valid custom element name or an object of the form { tag?: string; shadow?: "open" \| "none" \| `ShadowRootInit`; props?: { [key: string]: { attribute?: string; reflect?: boolean; type: .. } } } |  |
| [`svelte::svelte_options_invalid_customelement_props`](https://svelte.dev/e/svelte_options_invalid_customelement_props) | error | "props" must be a statically analyzable object literal of the form "{ [key: string]: { attribute?: string; reflect?: boolean; type?: "String" \| "Boolean" \| "Number" \| "Array" \| "Object" }" |  |
| [`svelte::svelte_options_invalid_customelement_shadow`](https://svelte.dev/e/svelte_options_invalid_customelement_shadow) | error | "shadow" must be either "open", "none" or `ShadowRootInit` object. |  |
| [`svelte::svelte_options_invalid_tagname`](https://svelte.dev/e/svelte_options_invalid_tagname) | error | Tag name must be lowercase and hyphenated |  |
| [`svelte::svelte_options_reserved_tagname`](https://svelte.dev/e/svelte_options_reserved_tagname) | error | Tag name is reserved |  |
| [`svelte::svelte_options_unknown_attribute`](https://svelte.dev/e/svelte_options_unknown_attribute) | error | `<svelte:options>` unknown attribute '%name%' |  |
| [`svelte::tag_invalid_name`](https://svelte.dev/e/tag_invalid_name) | error | Expected a valid element or component name. Components must have a valid variable name or dot notation expression |  |
| [`svelte::tag_invalid_placement`](https://svelte.dev/e/tag_invalid_placement) | error | {@%name% ...} tag cannot be %location% |  |
| [`svelte::transition_conflict`](https://svelte.dev/e/transition_conflict) | error | Cannot use `%type%:` alongside existing `%existing%:` directive |  |
| [`svelte::transition_duplicate`](https://svelte.dev/e/transition_duplicate) | error | Cannot use multiple `%type%:` directives on a single element |  |
| [`svelte::typescript_invalid_feature`](https://svelte.dev/e/typescript_invalid_feature) | error | TypeScript language features like %feature% are not natively supported, and their use is generally discouraged. Outside of `<script>` tags, these features are not supported. For use within `<script>` tags, you will need to use a preprocessor to convert it to JavaScript before it gets passed to the Svelte compiler. If you are using `vitePreprocess`, make sure to specifically enable preprocessing script tags (`vitePreprocess({ script: true })`) |  |
| [`svelte::unexpected_eof`](https://svelte.dev/e/unexpected_eof) | error | Unexpected end of input |  |
| [`svelte::unexpected_reserved_word`](https://svelte.dev/e/unexpected_reserved_word) | error | '%word%' is a reserved word in JavaScript and cannot be used here |  |
| [`svelte::unterminated_string_constant`](https://svelte.dev/e/unterminated_string_constant) | error | Unterminated string constant |  |
| [`svelte::void_element_invalid_content`](https://svelte.dev/e/void_element_invalid_content) | error | Void elements cannot have children or closing tags |  |
