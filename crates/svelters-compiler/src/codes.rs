//! The registry of diagnostic codes (CX1): every code the compiler reports, with its stable
//! namespaced id, its usual severity, a one-line summary and the fix-it it offers.
//!
//! - **`code`** is the short code a [`Diagnostic`](crate::diag::Diagnostic) carries
//!   (`bind_partially_observed`). It is what the CLI prints (`error[code]`), what the Vite plugin
//!   and the conformance runner match, and it is unique across the compiler.
//! - **`id`** is the stable, namespaced name tools should key on (`svelters::bind::partially_observed`).
//!   Codes ported from Svelte keep Svelte's code under `svelte::` (`svelte::expected_token`) and are
//!   documented at `https://svelte.dev/e/<code>`; the native compiler's own codes are
//!   `svelters::<area>::<name>`.
//!
//! **Stability.** An id, once released, is never renamed or reused for something else; a code
//! that is no longer reported stays in the table (marked as such). New codes get a row here: the
//! `cx1_codes` test scans the sources for codes without one, and a debug build panics when it
//! reports an unregistered code. The documentation page
//! (`docs/site/src/content/docs/guides/diagnostics.md`) is generated from this table by
//! [`docs_markdown`] (`UPDATE=1 cargo nextest run -p svelters-compiler --test cx1_codes`).

use crate::diag::Severity;
use crate::diag::Severity::{Error, Note, Warning};

/// One registered diagnostic code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodeInfo {
    /// The short code of [`Diagnostic::code`](crate::diag::Diagnostic::code).
    pub code: &'static str,
    /// The stable namespaced id.
    pub id: &'static str,
    /// Where the code comes from: Svelte's compiler (the port keeps its code) or svelters.
    pub origin: Origin,
    /// The usual severity. `--strict-native` turns the observability warnings into errors.
    pub severity: Severity,
    /// One line: what the code means (Svelte's message template for Svelte codes, `%name%` being
    /// a placeholder).
    pub summary: &'static str,
    /// The fix-it offered with the diagnostic, if any (empty: none).
    pub fix: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Svelte,
    Svelters,
}

impl CodeInfo {
    /// Svelte's documentation page of a Svelte code.
    pub fn docs_url(&self) -> Option<String> {
        (self.origin == Origin::Svelte).then(|| format!("https://svelte.dev/e/{}", self.code))
    }
}

const SUGGEST: &str = "\"did you mean\": replace the name with the suggestion";
const ONE_WAY: &str = "set it one way: `bind:x={v}` becomes the attribute the help names";
const REMOVE_MODIFIER: &str = "remove the modifier";

const fn svelte(code: &'static str, id: &'static str, severity: Severity, summary: &'static str) -> CodeInfo {
    CodeInfo { code, id, origin: Origin::Svelte, severity, summary, fix: "" }
}

const fn svelte_fix(code: &'static str, id: &'static str, severity: Severity, summary: &'static str, fix: &'static str) -> CodeInfo {
    CodeInfo { code, id, origin: Origin::Svelte, severity, summary, fix }
}

const fn own(code: &'static str, id: &'static str, severity: Severity, summary: &'static str, fix: &'static str) -> CodeInfo {
    CodeInfo { code, id, origin: Origin::Svelters, severity, summary, fix }
}

/// Every registered code: the native compiler's own codes by area, then Svelte's (alphabetical).
/// The id of a Svelte row is `svelte::<code>`.
static CODES: &[CodeInfo] = &[
    // elements
    own("unknown_element", "svelters::element::unknown", Error, "The tag is not an element of the backend's semantic spec (nor a component in scope).", SUGGEST),
    own("children_not_allowed", "svelters::element::children_not_allowed", Error, "The element takes no child elements on this backend.", ""),
    own("text_not_allowed", "svelters::element::text_not_allowed", Error, "Text or an expression tag where the parent cannot hold text; only elements whose spec has text content (e.g. `<label>`) can.", ""),
    own("text_content_children", "svelters::element::text_content_children", Error, "A text-content element has both text and child elements; it takes one or the other.", ""),
    own("show_invalid", "svelters::element::show_invalid", Error, "A toplevel's `show` is not a static `modal`, `modeless` or `none`.", ""),
    own("bad_attribute_value", "svelters::element::bad_attribute_value", Error, "A static attribute value cannot be converted to the property's type.", ""),
    // properties and methods
    own("unknown_property", "svelters::property::unknown", Error, "The element has no property of that name on this backend.", SUGGEST),
    own("read_only_property", "svelters::property::read_only", Error, "The property cannot be written (or, for `bind:`, is write-only or not readable and writable).", ONE_WAY),
    own("unknown_method", "svelters::method::unknown", Error, "The element has no annotated method of that name.", SUGGEST),
    // events
    own("unknown_event", "svelters::event::unknown", Error, "The element has no event of that name on this backend.", SUGGEST),
    own("listen_phase_unsupported", "svelters::event::phase_unsupported", Error, "The event's signal does not support the requested phase (`capture`, ...).", ""),
    own("duplicate_event", "svelters::event::duplicate", Error, "A bare `on:x` (forwarding) is not the only handler of `x`, or a component gets the prop `onx` twice.", ""),
    own("legacy_forwarding_lowered", "svelters::event::forwarding_lowered", Note, "A bare `on:x` forwards the component's own `onx` callback prop (FW1); once per component.", ""),
    own("sync_handler_arguments", "svelters::event::sync_handler_arguments", Warning, "A handler of a sync signal declares a parameter for a signal argument, but listeners get `(event, ...args)`.", ""),
    own("sync_handler_return", "svelters::event::sync_handler_return", Warning, "The return value of a sync signal's listener is ignored.", ""),
    // bindings (REACTIVITY.md section 8)
    own("bind_no_plan", "svelters::bind::no_plan", Error, "`bind:` on a property with no binding plan: whether its native changes are announced is unknown.", SUGGEST),
    own("bind_not_observable", "svelters::bind::not_observable", Error, "`bind:` on a property whose native changes are never announced (or that never changes).", ONE_WAY),
    own("bind_set_only", "svelters::bind::set_only", Error, "`bind:` on a set-only property: only the app's own writes change it, so there is nothing to bind back.", ONE_WAY),
    own("bind_partially_observed", "svelters::bind::partially_observed", Warning, "`bind:` on a property only some of whose native changes are announced; the bound value can go stale (error under `--strict-native`).", ""),
    own("bind_observation_unverified", "svelters::bind::observation_unverified", Warning, "`bind:` on an `exact` property whose change delivery no generated row has seen (error under `--strict-native`).", ""),
    own("bind_modifier_invalid", "svelters::bind::modifier_invalid", Error, "An unknown, repeated or meaningless `bind:` modifier (only `|nonreactive`, on native element properties).", REMOVE_MODIFIER),
    own("nonreactive_redundant", "svelters::bind::nonreactive_redundant", Warning, "`bind:x|nonreactive` on a property that is set-only or constant, so `|nonreactive` changes nothing (error under `--strict-native`).", ONE_WAY),
    own("observability_differs", "svelters::bind::observability_differs", Warning, "Portability mode: the bound property is observed differently on the other platforms (error under `--strict-native`).", ""),
    own("bind_and_prop", "svelters::bind::and_prop", Error, "A property is both bound with `bind:` and set explicitly on the same element.", ""),
    own("dynamic_element_bind", "svelters::bind::dynamic_element", Error, "`bind:` on `<svelte:element>`: the binding plan needs the element's kind at compile time.", ""),
    own("native_read_not_observable", "svelters::reactivity::native_read_not_observable", Warning, "A reactive context reads a native property whose changes are not announced, so it does not rerun when the property changes (error under `--strict-native`).", ""),
    // slots and child placement
    own("unknown_slot", "svelters::slot::unknown", Error, "The parent element has no slot of that name.", ""),
    own("no_default_slot", "svelters::slot::no_default", Error, "The parent element has no default slot; name one with `slot=\"...\"`.", ""),
    own("slot_rejects_child", "svelters::slot::rejects_child", Error, "The slot does not accept this kind of child.", ""),
    own("slot_cardinality", "svelters::slot::cardinality", Error, "A single-child slot already has a child.", ""),
    own("slot_conflict", "svelters::slot::conflict", Error, "An element or snippet asks for a slot other than the one its `<sv-slot>` names.", ""),
    own("slot_not_applicable", "svelters::slot::not_applicable", Error, "The child attaches to its parent in another way and takes no slot.", ""),
    own("slot_not_static", "svelters::slot::not_static", Error, "`slot` (or `<sv-slot name>`) is not a static name.", ""),
    own("slot_proxy_attribute", "svelters::slot::proxy_attribute", Error, "`<sv-slot>` takes exactly one static `name` attribute.", ""),
    own("slot_proxy_placement", "svelters::slot::proxy_placement", Error, "`<sv-slot>` is not a direct child of a native element, or is nested in another `<sv-slot>`.", ""),
    own("block_mixed_slots", "svelters::slot::mixed_in_block", Error, "The elements of one block (or the roots of one snippet) go to different slots of their parent.", ""),
    own("unknown_child_attr", "svelters::child_attr::unknown", Error, "The slot has no child attribute (`parent:<name>`) of that name.", SUGGEST),
    own("child_attr_bad_value", "svelters::child_attr::bad_value", Error, "A static `parent:<name>` value cannot be converted to the child attribute's type.", ""),
    // styles
    own("style_syntax", "svelters::style::syntax", Error, "A syntax error in `<style>`.", ""),
    own("style_unsupported_selector", "svelters::style::unsupported_selector", Error, "A selector form native styles do not support (`*`, pseudo-elements, ...).", ""),
    own("style_unsupported_combinator", "svelters::style::unsupported_combinator", Error, "A combinator other than descendant (whitespace) and child (`>`).", ""),
    own("style_unsupported_at_rule", "svelters::style::unsupported_at_rule", Error, "A setter property inside an at-rule (`@media`, `@native`); setter properties are written with setProp, not CSS.", ""),
    own("style_unsupported_nesting", "svelters::style::unsupported_nesting", Error, "A setter property in a nested rule.", ""),
    own("style_unsupported_important", "svelters::style::unsupported_important", Error, "`!important` is not supported; native styles apply by specificity and source order.", "remove `!important`"),
    own("style_unsupported_value", "svelters::style::unsupported_value", Error, "A value form native styles do not support (`,` or `/` lists, ...).", ""),
    own("style_unknown_declaration", "svelters::style::unknown_declaration", Error, "A declaration no setter property or CSS property of the backend knows (reserved; not reported yet).", ""),
    own("style_not_applicable", "svelters::style::not_applicable", Error, "The declaration applies to other elements (or to the children of other elements) on this backend.", ""),
    own("style_bad_value", "svelters::style::bad_value", Error, "A declaration's value cannot be converted to the property's type (or uses an unsupported unit).", ""),
    own("style_conflicts_binding", "svelters::style::conflicts_binding", Error, "A style sets a property that is bound with `bind:` on the element.", ""),
    own("style_overridden", "svelters::style::overridden", Warning, "A style sets a property that the element also sets explicitly; the explicit value wins.", ""),
    own("style_unused_selector", "svelters::style::unused_selector", Warning, "The selector matches no element of the component, so its declarations are not checked.", ""),
    // virtualized lists (`@svelters/lists`)
    own("list_children", "svelters::list::children", Error, "A virtualized list has children other than its `row` snippet.", ""),
    own("list_row_missing", "svelters::list::row_missing", Error, "A virtualized list has no `row` snippet.", ""),
    own("list_row_params", "svelters::list::row_params", Error, "The `row` snippet takes more than `(item, index)`.", ""),
    own("list_row_toplevel", "svelters::list::row_toplevel", Error, "A row of a virtualized list is a toplevel element.", ""),
    own("list_prop_invalid", "svelters::list::prop_invalid", Error, "A list property has a value outside its allowed set.", ""),
    own("list_bind_invalid", "svelters::list::bind_invalid", Error, "A list binds something other than `bind:selected` or `bind:this`.", ""),
    // transitions
    own("transition_modifier_invalid", "svelters::transition::modifier_invalid", Error, "A transition modifier other than `local` or `global`.", REMOVE_MODIFIER),
    // language subset
    own("unsupported_feature", "svelters::feature::unsupported", Error, "A Svelte feature the native compiler does not support (legacy syntax, event modifiers, re-exports, async `$derived`, ...); see the language subset.", "event modifiers: the handler rewritten without them, when that is exact"),
    // Svelte's codes (svelte/src/compiler/errors.js and warnings.js, 5.57.2)
    svelte("animation_duplicate", "svelte::animation_duplicate", Error, "An element can only have one 'animate' directive"),
    svelte("animation_invalid_placement", "svelte::animation_invalid_placement", Error, "An element that uses the `animate:` directive must be the only child of a keyed `{#each ...}` block"),
    svelte("animation_missing_key", "svelte::animation_missing_key", Error, "An element that uses the `animate:` directive must be the only child of a keyed `{#each ...}` block. Did you forget to add a key to your each block?"),
    svelte("attribute_duplicate", "svelte::attribute_duplicate", Error, "Attributes need to be unique"),
    svelte("attribute_empty_shorthand", "svelte::attribute_empty_shorthand", Error, "Attribute shorthand cannot be empty"),
    svelte("bind_invalid_target", "svelte::bind_invalid_target", Error, "`bind:%name%` can only be used with %elements%"),
    svelte("bind_invalid_value", "svelte::bind_invalid_value", Error, "Can only bind to state or props"),
    svelte("bindable_invalid_location", "svelte::bindable_invalid_location", Error, "`$bindable()` can only be used inside a `$props()` declaration"),
    svelte("block_duplicate_clause", "svelte::block_duplicate_clause", Error, "%name% cannot appear more than once within a block"),
    svelte("block_invalid_continuation_placement", "svelte::block_invalid_continuation_placement", Error, "{:...} block is invalid at this position (did you forget to close the preceding element or block?)"),
    svelte_fix("block_invalid_elseif", "svelte::block_invalid_elseif", Error, "'elseif' should be 'else if'", "replace `elseif` with `else if`"),
    svelte("block_invalid_placement", "svelte::block_invalid_placement", Error, "{#%name% ...} block cannot be %location%"),
    svelte("block_unclosed", "svelte::block_unclosed", Error, "Block was left open"),
    svelte("block_unexpected_close", "svelte::block_unexpected_close", Error, "Unexpected block closing tag"),
    svelte("const_tag_invalid_expression", "svelte::const_tag_invalid_expression", Error, "{@const ...} must consist of a single variable declaration"),
    svelte("const_tag_invalid_placement", "svelte::const_tag_invalid_placement", Error, "`{@const}` must be the immediate child of `{#snippet}`, `{#if}`, `{:else if}`, `{:else}`, `{#each}`, `{:then}`, `{:catch}`, `<svelte:fragment>`, `<svelte:boundary>` or `<Component>`"),
    svelte("constant_assignment", "svelte::constant_assignment", Error, "Cannot assign to %thing%"),
    svelte("css_empty_declaration", "svelte::css_empty_declaration", Error, "Declaration cannot be empty"),
    svelte("css_expected_identifier", "svelte::css_expected_identifier", Error, "Expected a valid CSS identifier"),
    svelte("css_global_block_invalid_combinator", "svelte::css_global_block_invalid_combinator", Error, "A `:global` selector cannot follow a `%name%` combinator"),
    svelte("css_global_block_invalid_declaration", "svelte::css_global_block_invalid_declaration", Error, "A top-level `:global {...}` block can only contain rules, not declarations"),
    svelte("css_global_block_invalid_list", "svelte::css_global_block_invalid_list", Error, "A `:global` selector cannot be part of a selector list with entries that don't contain `:global`"),
    svelte("css_global_block_invalid_modifier", "svelte::css_global_block_invalid_modifier", Error, "A `:global` selector cannot modify an existing selector"),
    svelte("css_global_block_invalid_modifier_start", "svelte::css_global_block_invalid_modifier_start", Error, "A `:global` selector can only be modified if it is a descendant of other selectors"),
    svelte("css_global_block_invalid_placement", "svelte::css_global_block_invalid_placement", Error, "A `:global` selector cannot be inside a pseudoclass"),
    svelte("css_global_invalid_placement", "svelte::css_global_invalid_placement", Error, "`:global(...)` can be at the start or end of a selector sequence, but not in the middle"),
    svelte("css_global_invalid_selector", "svelte::css_global_invalid_selector", Error, "`:global(...)` must contain exactly one selector"),
    svelte("css_global_invalid_selector_list", "svelte::css_global_invalid_selector_list", Error, "`:global(...)` must not contain type or universal selectors when used in a compound selector"),
    svelte("css_nesting_selector_invalid_placement", "svelte::css_nesting_selector_invalid_placement", Error, "Nesting selectors can only be used inside a rule or as the first selector inside a lone `:global(...)`"),
    svelte("css_selector_invalid", "svelte::css_selector_invalid", Error, "Invalid selector"),
    svelte("css_type_selector_invalid_placement", "svelte::css_type_selector_invalid_placement", Error, "`:global(...)` must not be followed by a type selector"),
    svelte("css_unused_selector", "svelte::css_unused_selector", Warning, "Unused CSS selector \"%name%\""),
    svelte("debug_tag_invalid_arguments", "svelte::debug_tag_invalid_arguments", Error, "{@debug ...} arguments must be identifiers, not arbitrary expressions"),
    svelte("declaration_duplicate", "svelte::declaration_duplicate", Error, "`%name%` has already been declared"),
    svelte("declaration_duplicate_module_import", "svelte::declaration_duplicate_module_import", Error, "Cannot declare a variable with the same name as an import from `<script module>`"),
    svelte("declaration_tag_invalid_type", "svelte::declaration_tag_invalid_type", Error, "Declaration tags must be `let` or `const` declarations"),
    svelte("derived_invalid_export", "svelte::derived_invalid_export", Error, "Cannot export derived state from a module. To expose the current derived value, export a function returning its value"),
    svelte("directive_invalid_value", "svelte::directive_invalid_value", Error, "Directive value must be a JavaScript expression enclosed in curly braces"),
    svelte("directive_missing_name", "svelte::directive_missing_name", Error, "`%type%` name cannot be empty"),
    svelte("dollar_binding_invalid", "svelte::dollar_binding_invalid", Error, "The $ name is reserved, and cannot be used for variables and imports"),
    svelte("dollar_prefix_invalid", "svelte::dollar_prefix_invalid", Error, "The $ prefix is reserved, and cannot be used for variables and imports"),
    svelte("each_item_invalid_assignment", "svelte::each_item_invalid_assignment", Error, "Cannot reassign or bind to each block argument in runes mode. Use the array and index variables instead (e.g. `array[i] = value` instead of `entry = value`, or `bind:value={array[i]}` instead of `bind:value={entry}`)"),
    svelte("effect_invalid_placement", "svelte::effect_invalid_placement", Error, "`$effect()` can only be used as an expression statement"),
    svelte("element_implicitly_closed", "svelte::element_implicitly_closed", Warning, "This element is implicitly closed by the following `%tag%`, which can cause an unexpected DOM structure. Add an explicit `%closing%` to avoid surprises."),
    svelte("element_invalid_closing_tag", "svelte::element_invalid_closing_tag", Error, "`</%name%>` attempted to close an element that was not open"),
    svelte("element_invalid_closing_tag_autoclosed", "svelte::element_invalid_closing_tag_autoclosed", Error, "`</%name%>` attempted to close element that was already automatically closed by `<%reason%>` (cannot nest `<%reason%>` inside `<%name%>`)"),
    svelte("element_unclosed", "svelte::element_unclosed", Error, "`<%name%>` was left open"),
    svelte("expected_attribute_value", "svelte::expected_attribute_value", Error, "Expected attribute value"),
    svelte("expected_block_type", "svelte::expected_block_type", Error, "Expected 'if', 'each', 'await', 'key' or 'snippet'"),
    svelte("expected_identifier", "svelte::expected_identifier", Error, "Expected an identifier"),
    svelte("expected_pattern", "svelte::expected_pattern", Error, "Expected identifier or destructure pattern"),
    svelte("expected_tag", "svelte::expected_tag", Error, "Expected 'html', 'render', 'attach', 'const', or 'debug'"),
    svelte("expected_token", "svelte::expected_token", Error, "Expected token %token%"),
    svelte("expected_whitespace", "svelte::expected_whitespace", Error, "Expected whitespace"),
    svelte("global_reference_invalid", "svelte::global_reference_invalid", Error, "`%name%` is an illegal variable name. To reference a global variable called `%name%`, use `globalThis.%name%`"),
    svelte("host_invalid_placement", "svelte::host_invalid_placement", Error, "`$host()` can only be used inside custom element component instances"),
    svelte("inspect_trace_invalid_placement", "svelte::inspect_trace_invalid_placement", Error, "`$inspect.trace(...)` must be the first statement of a function body"),
    svelte("js_parse_error", "svelte::js_parse_error", Error, "%message%"),
    svelte("legacy_export_invalid", "svelte::legacy_export_invalid", Error, "Cannot use `export let` in runes mode — use `$props()` instead"),
    svelte("module_illegal_default_export", "svelte::module_illegal_default_export", Error, "A component cannot have a default export"),
    svelte("props_duplicate", "svelte::props_duplicate", Error, "Cannot use `%rune%()` more than once"),
    svelte("props_id_invalid_placement", "svelte::props_id_invalid_placement", Error, "`$props.id()` can only be used at the top level of components as a variable declaration initializer"),
    svelte("props_illegal_name", "svelte::props_illegal_name", Error, "Declaring or accessing a prop starting with `$$` is illegal (they are reserved for Svelte internals)"),
    svelte("props_invalid_identifier", "svelte::props_invalid_identifier", Error, "`$props()` can only be used with an object destructuring pattern"),
    svelte("props_invalid_pattern", "svelte::props_invalid_pattern", Error, "`$props()` assignment must not contain nested properties or computed keys"),
    svelte("props_invalid_placement", "svelte::props_invalid_placement", Error, "`$props()` can only be used at the top level of components as a variable declaration initializer"),
    svelte("render_tag_invalid_expression", "svelte::render_tag_invalid_expression", Error, "`{@render ...}` tags can only contain call expressions"),
    svelte("render_tag_invalid_spread_argument", "svelte::render_tag_invalid_spread_argument", Error, "cannot use spread arguments in `{@render ...}` tags"),
    svelte("rune_invalid_arguments", "svelte::rune_invalid_arguments", Error, "`%rune%` cannot be called with arguments"),
    svelte("rune_invalid_arguments_length", "svelte::rune_invalid_arguments_length", Error, "`%rune%` must be called with %args%"),
    svelte("rune_invalid_spread", "svelte::rune_invalid_spread", Error, "`%rune%` cannot be called with a spread argument"),
    svelte("rune_invalid_usage", "svelte::rune_invalid_usage", Error, "Cannot use `%rune%` rune in non-runes mode"),
    svelte("script_duplicate", "svelte::script_duplicate", Error, "A component can have a single top-level `<script>` element and/or a single top-level `<script module>` element"),
    svelte("script_invalid_attribute_value", "svelte::script_invalid_attribute_value", Error, "If the `%name%` attribute is supplied, it must be a boolean attribute"),
    svelte("script_invalid_context", "svelte::script_invalid_context", Error, "If the context attribute is supplied, its value must be \"module\""),
    svelte("script_reserved_attribute", "svelte::script_reserved_attribute", Error, "The `%name%` attribute is reserved and cannot be used"),
    svelte("script_unknown_attribute", "svelte::script_unknown_attribute", Warning, "Unrecognised attribute — should be one of `generics`, `lang` or `module`. If this exists for a preprocessor, ensure that the preprocessor removes it"),
    svelte("snippet_conflict", "svelte::snippet_conflict", Error, "Cannot use explicit children snippet at the same time as implicit children content. Remove either the non-whitespace content or the children snippet block"),
    svelte("snippet_parameter_assignment", "svelte::snippet_parameter_assignment", Error, "Cannot reassign or bind to snippet parameter"),
    svelte("state_field_duplicate", "svelte::state_field_duplicate", Error, "`%name%` has already been declared on this class"),
    svelte("state_invalid_export", "svelte::state_invalid_export", Error, "Cannot export state from a module if it is reassigned. Either export a function returning the state value or only mutate the state value's properties"),
    svelte("state_invalid_placement", "svelte::state_invalid_placement", Error, "`%rune%(...)` can only be used as a variable declaration initializer, a class field declaration, or the first assignment to a class field at the top level of the constructor."),
    svelte("store_invalid_scoped_subscription", "svelte::store_invalid_scoped_subscription", Error, "Cannot subscribe to stores that are not declared at the top level of the component"),
    svelte("store_invalid_subscription", "svelte::store_invalid_subscription", Error, "Cannot reference store value inside `<script module>`"),
    svelte("store_invalid_subscription_module", "svelte::store_invalid_subscription_module", Error, "Cannot reference store value outside a `.svelte` file"),
    svelte("style_duplicate", "svelte::style_duplicate", Error, "A component can have a single top-level `<style>` element"),
    svelte("svelte_component_invalid_this", "svelte::svelte_component_invalid_this", Error, "Invalid component definition — must be an `{expression}`"),
    svelte("svelte_component_missing_this", "svelte::svelte_component_missing_this", Error, "`<svelte:component>` must have a 'this' attribute"),
    svelte("svelte_element_invalid_this", "svelte::svelte_element_invalid_this", Warning, "`this` should be an `{expression}`. Using a string attribute value will cause an error in future versions of Svelte"),
    svelte("svelte_element_missing_this", "svelte::svelte_element_missing_this", Error, "`<svelte:element>` must have a 'this' attribute with a value"),
    svelte("svelte_meta_duplicate", "svelte::svelte_meta_duplicate", Error, "A component can only have one `<%name%>` element"),
    svelte("svelte_meta_invalid_content", "svelte::svelte_meta_invalid_content", Error, "<%name%> cannot have children"),
    svelte("svelte_meta_invalid_placement", "svelte::svelte_meta_invalid_placement", Error, "`<%name%>` tags cannot be inside elements or blocks"),
    svelte("svelte_meta_invalid_tag", "svelte::svelte_meta_invalid_tag", Error, "Valid `<svelte:...>` tag names are %list%"),
    svelte("svelte_options_deprecated_tag", "svelte::svelte_options_deprecated_tag", Error, "\"tag\" option is deprecated — use \"customElement\" instead"),
    svelte("svelte_options_invalid_attribute", "svelte::svelte_options_invalid_attribute", Error, "`<svelte:options>` can only receive static attributes"),
    svelte("svelte_options_invalid_attribute_value", "svelte::svelte_options_invalid_attribute_value", Error, "Value must be %list%, if specified"),
    svelte("svelte_options_invalid_customelement", "svelte::svelte_options_invalid_customelement", Error, "\"customElement\" must be a string literal defining a valid custom element name or an object of the form { tag?: string; shadow?: \"open\" | \"none\" | `ShadowRootInit`; props?: { [key: string]: { attribute?: string; reflect?: boolean; type: .. } } }"),
    svelte("svelte_options_invalid_customelement_props", "svelte::svelte_options_invalid_customelement_props", Error, "\"props\" must be a statically analyzable object literal of the form \"{ [key: string]: { attribute?: string; reflect?: boolean; type?: \"String\" | \"Boolean\" | \"Number\" | \"Array\" | \"Object\" }\""),
    svelte("svelte_options_invalid_customelement_shadow", "svelte::svelte_options_invalid_customelement_shadow", Error, "\"shadow\" must be either \"open\", \"none\" or `ShadowRootInit` object."),
    svelte("svelte_options_invalid_tagname", "svelte::svelte_options_invalid_tagname", Error, "Tag name must be lowercase and hyphenated"),
    svelte("svelte_options_reserved_tagname", "svelte::svelte_options_reserved_tagname", Error, "Tag name is reserved"),
    svelte("svelte_options_unknown_attribute", "svelte::svelte_options_unknown_attribute", Error, "`<svelte:options>` unknown attribute '%name%'"),
    svelte("tag_invalid_name", "svelte::tag_invalid_name", Error, "Expected a valid element or component name. Components must have a valid variable name or dot notation expression"),
    svelte("tag_invalid_placement", "svelte::tag_invalid_placement", Error, "{@%name% ...} tag cannot be %location%"),
    svelte("transition_conflict", "svelte::transition_conflict", Error, "Cannot use `%type%:` alongside existing `%existing%:` directive"),
    svelte("transition_duplicate", "svelte::transition_duplicate", Error, "Cannot use multiple `%type%:` directives on a single element"),
    svelte("typescript_invalid_feature", "svelte::typescript_invalid_feature", Error, "TypeScript language features like %feature% are not natively supported, and their use is generally discouraged. Outside of `<script>` tags, these features are not supported. For use within `<script>` tags, you will need to use a preprocessor to convert it to JavaScript before it gets passed to the Svelte compiler. If you are using `vitePreprocess`, make sure to specifically enable preprocessing script tags (`vitePreprocess({ script: true })`)"),
    svelte("unexpected_eof", "svelte::unexpected_eof", Error, "Unexpected end of input"),
    svelte("unexpected_reserved_word", "svelte::unexpected_reserved_word", Error, "'%word%' is a reserved word in JavaScript and cannot be used here"),
    svelte("unterminated_string_constant", "svelte::unterminated_string_constant", Error, "Unterminated string constant"),
    svelte("void_element_invalid_content", "svelte::void_element_invalid_content", Error, "Void elements cannot have children or closing tags"),
];

/// Every registered code.
pub fn all() -> &'static [CodeInfo] {
    CODES
}

fn index() -> &'static std::collections::HashMap<&'static str, usize> {
    static INDEX: std::sync::OnceLock<std::collections::HashMap<&'static str, usize>> = std::sync::OnceLock::new();
    INDEX.get_or_init(|| {
        let mut m = std::collections::HashMap::new();
        for (i, c) in all().iter().enumerate() {
            m.insert(c.code, i);
            m.insert(c.id, i);
        }
        m
    })
}

/// The registry row of a short code or a namespaced id.
pub fn lookup(code_or_id: &str) -> Option<&'static CodeInfo> {
    index().get(code_or_id).map(|&i| &all()[i])
}

/// The namespaced id of a short code; an unregistered code (a bug, see the module docs) gets
/// `svelters::unregistered::<code>`.
pub fn id_of(code: &str) -> &'static str {
    match lookup(code) {
        Some(c) => c.id,
        None => {
            debug_assert!(
                false,
                "diagnostic code `{code}` is not registered: add a row to crates/svelters-compiler/src/codes.rs (CX1) and regenerate the docs page"
            );
            "svelters::unregistered"
        }
    }
}

/// `svelters-compile --explain <code|id>`: the registry row as text.
pub fn explain(code_or_id: &str) -> Option<String> {
    let c = lookup(code_or_id)?;
    let mut s = format!("{} ({})\n  code: {}\n  severity: {}\n  {}\n", c.id, origin_name(c.origin), c.code, severity_name(c.severity), c.summary);
    if !c.fix.is_empty() {
        s.push_str(&format!("  fix-it: {}\n", c.fix));
    }
    if let Some(u) = c.docs_url() {
        s.push_str(&format!("  docs: {u}\n"));
    }
    Some(s)
}

fn origin_name(o: Origin) -> &'static str {
    match o {
        Origin::Svelte => "Svelte",
        Origin::Svelters => "svelters",
    }
}

fn severity_name(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Note => "note",
    }
}

/// Markdown table cell text: `|` escaped, and `<`/`>` escaped outside code spans (the docs site
/// renders Markdown with HTML enabled).
fn cell(s: &str) -> String {
    let mut out = String::new();
    for (i, part) in s.split('`').enumerate() {
        if i > 0 {
            out.push('`');
        }
        if i % 2 == 0 {
            out.push_str(&part.replace('<', "&lt;").replace('>', "&gt;").replace('|', "\\|"));
        } else {
            out.push_str(&part.replace('|', "\\|"));
        }
    }
    out
}

/// The generated documentation page (`docs/site/src/content/docs/guides/diagnostics.md`).
pub fn docs_markdown() -> String {
    let mut s = String::from(
        "---\ntitle: Compiler diagnostics\ndescription: Every diagnostic code the svelters compiler reports, with its stable namespaced id, severity and fix-it.\n---\n\n\
<!-- Generated from crates/svelters-compiler/src/codes.rs by `UPDATE=1 cargo nextest run -p svelters-compiler --test cx1_codes`. Do not edit. -->\n\n\
Every diagnostic has a short **code** (`bind_partially_observed`, printed as `error[code]` by `svelters-compile`) and a stable \
namespaced **id** (`svelters::bind::partially_observed`) that tools should key on. Ids are never renamed or reused. Codes from \
Svelte's own compiler keep Svelte's code under `svelte::` and are documented on svelte.dev; the native compiler's own codes are \
`svelters::<area>::<name>`.\n\n\
Severities: **error** fails the build; **warning** does not (the observability warnings become errors under `--strict-native`); \
**note** explains how something was lowered and never fails a build.\n\n\
`svelters-compile --check --json <file>` reports diagnostics without generating code, with byte and line/column ranges and \
fix-its as text edits; `svelters-compile --explain <code or id>` prints one row of this page. In Rust, \
`svelters_compiler::check::check` does the same in process, with incremental state per file.\n",
    );
    let mut area = "";
    s.push_str("\n## svelters codes\n");
    for c in all().iter().filter(|c| c.origin == Origin::Svelters) {
        let a = c.id.split("::").nth(1).unwrap_or("");
        if a != area {
            area = a;
            s.push_str(&format!("\n### {a}\n\n| Id | Code | Severity | Meaning | Fix-it |\n| --- | --- | --- | --- | --- |\n"));
        }
        s.push_str(&format!("| `{}` | `{}` | {} | {} | {} |\n", c.id, c.code, severity_name(c.severity), cell(c.summary), cell(c.fix)));
    }
    s.push_str(
        "\n## Svelte codes\n\nThe parser, scope analysis and rune transform are ported from Svelte 5.57.2 and report Svelte's codes \
(with svelters' wording where the native compiler differs). The meaning column is Svelte's message template.\n\n\
| Id | Severity | Meaning | Fix-it |\n| --- | --- | --- | --- |\n",
    );
    let mut svelte: Vec<&CodeInfo> = all().iter().filter(|c| c.origin == Origin::Svelte).collect();
    svelte.sort_by_key(|c| c.code);
    for c in svelte {
        s.push_str(&format!(
            "| [`{}`](https://svelte.dev/e/{}) | {} | {} | {} |\n",
            c.id,
            c.code,
            severity_name(c.severity),
            cell(c.summary),
            cell(c.fix)
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_ids_are_unique_and_well_formed() {
        let mut seen = std::collections::HashSet::new();
        for c in all() {
            assert!(seen.insert(c.code), "duplicate code {}", c.code);
            assert!(seen.insert(c.id), "duplicate id {}", c.id);
            let parts: Vec<&str> = c.id.split("::").collect();
            match c.origin {
                Origin::Svelte => assert_eq!(parts, ["svelte", c.code]),
                Origin::Svelters => {
                    assert_eq!(parts.len(), 3, "{}", c.id);
                    assert_eq!(parts[0], "svelters");
                }
            }
            assert!(c.id.split("::").all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')), "{}", c.id);
            assert!(!c.summary.is_empty());
        }
        assert_eq!(lookup("bind_partially_observed").unwrap().id, "svelters::bind::partially_observed");
        assert_eq!(lookup("svelte::expected_token").unwrap().code, "expected_token");
        assert_eq!(id_of("expected_token"), "svelte::expected_token");
    }
}
