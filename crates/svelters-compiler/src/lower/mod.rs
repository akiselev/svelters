//! Native lowering: Svelte template AST + native style sheet + a backend's semantics -> typed IR.
//!
//! This is where native semantics influence what source is valid: element tags, properties,
//! events, bindings, child slots, text content and style declarations are all resolved against
//! the backend specification, and every mismatch becomes a diagnostic at the source span.
//! Expression *meaning* (state reads/writes) is not decided here; codegen does that with the
//! scope analysis.
//!
//! Template blocks and their native meaning are in docs/notes/native-compiler/runtime-contract.md
//! ("Template blocks"): snippets are content factories rendered into a region of the render
//! site's (parent, slot); `<sv-slot name>` routes its content (typically `{@render}`) into a named
//! native slot of the parent element without creating a native.

pub mod ir;
mod engine_css;
mod forward;
mod native_attrs;
pub use native_attrs::HELPERS_MODULE;
mod lists;
pub mod native_reads;
pub mod portability;
pub mod template_await;

use crate::diag::Diagnostics;
use crate::estree::{ArrayElement, Expression, Pattern, Span, Spanned};
use crate::semantics::{BindLowering, ElementRef, Lifetime, PlatformSemantics, StyleLowering, Unresolved, codes, plan_bind};
use crate::style::{Combinator, ComplexSelector, Compound, StyleSheet, parse_value, split_declarations};
use crate::svelte::ast::{
    Attribute, AttributeLike, AttributeValue, Element as AstElement, ExpressionTagNode, Fragment as AstFragment, FragmentNode, RenderTag,
    Root, SnippetBlock, TextOrExpressionTag,
};
use ir::*;
use forward::OnSite;
use native_attrs::{animate_host, attr_value, ScriptFacts, CHILD_ATTR_PREFIX};
use serde_json::Value as Json;
use std::collections::{HashMap, HashSet};

/// The slot proxy: `<sv-slot name="x">` puts its content into the parent element's slot `x`.
pub const SLOT_PROXY: &str = "sv-slot";

/// `css_scope`: the class the X6 sheets of this component are scoped with (the Vite plugin's
/// Svelte-compiled CSS); every native element of the component carries it.
pub fn lower<'a>(root: &'a Root, sem: &dyn PlatformSemantics, styles: Option<&'a StyleSheet>, css_scope: Option<&str>, diags: &mut Diagnostics) -> Component<'a> {
    // D9 on egui (CS5e): a backend running the CSS engine gets `<style>` as CSS at run time; its
    // rules are not matched or lowered here (inline `style=` setters still are).
    let styles = if sem.style_engine_in_backend() { None } else { styles };
    // CS5g: their values are still checked here, by the engine itself
    if sem.style_engine_in_backend() {
        engine_css::check(root, sem, diags);
    }
    let mut snippet_roots = HashMap::new();
    collect_snippets(&root.fragment, &mut snippet_roots);
    let mut l = Lower {
        sem,
        diags,
        next_id: 0,
        styles,
        css_scope: css_scope.map(str::to_string),
        bad_selectors: HashSet::new(),
        reported: HashSet::new(),
        single_slots: Default::default(),
        in_block: false,
        matched: HashSet::new(),
        forced_slot: None,
        text_prop: None,
        const_ok: false,
        snippet_roots,
        script: ScriptFacts::collect([root.instance.as_ref(), root.module.as_ref()]),
        list_views: lists::list_view_names(root),
        kinds: false,
        animate_hosts: HashSet::new(),
        forwarding_noted: false,
    };
    l.check_style_sheet();
    let root_fragment = l.fragment(&root.fragment, &Parent::Root, &[]);
    if let Some(sheet) = styles {
        for rule in sheet.rules.iter().filter(|r| r.declarations.iter().any(|d| sem.is_setter_property(&d.name))) {
            for sel in &rule.selectors {
                if !l.matched.contains(&sel.span) && !l.bad_selectors.contains(&sel.span) {
                    l.diags.warning("style_unused_selector", sel.span, "this selector matches no element of the component (its declarations are not checked)");
                }
            }
        }
    }
    Component { root: root_fragment, elements: l.next_id, kinds: l.kinds }
}

enum Parent {
    Root,
    /// The parent element and its IR id.
    Element(ElementRef, usize),
    /// Snippet bodies and component children: placed wherever they are rendered (checked at
    /// local render sites, see `Lower::render`).
    Detached,
}

/// What style matching needs to know about an element (and its ancestors in this component).
#[derive(Clone)]
struct StyleSubject<'a> {
    tag: String,
    /// A static `id`.
    id: Option<String>,
    /// The element id when it has a dynamic `id` (`#id` rules are matched at runtime against it).
    dynamic_id: Option<usize>,
    static_classes: Vec<String>,
    cond_classes: Vec<(String, &'a Expression)>,
    /// The element id when it has a dynamic `class` (its class set is matched at runtime).
    dynamic: Option<usize>,
}

struct Lower<'a, 's, 'd> {
    sem: &'s dyn PlatformSemantics,
    diags: &'d mut Diagnostics,
    next_id: usize,
    styles: Option<&'a StyleSheet>,
    /// The CSS scope class of the component's X6 sheets (added to every native element).
    css_scope: Option<String>,
    /// Selectors of setter rules that setter rules cannot use (reported once).
    bad_selectors: HashSet<(u32, u32)>,
    /// (span, code) pairs already reported, so a declaration matching many elements reports once.
    reported: HashSet<(u32, u32, &'static str)>,
    /// Single-child slots already filled by a static element: (parent id, slot) -> its tag.
    single_slots: std::collections::HashMap<(usize, String), String>,
    /// Inside an {#if}/{#each}/... branch: alternatives may share a single-child slot.
    in_block: bool,
    /// Selector spans that matched at least one element (for `style_unused_selector`).
    matched: HashSet<(u32, u32)>,
    /// Inside `<sv-slot name>`: the slot its content goes to.
    forced_slot: Option<String>,
    /// The text prop being folded (for the mixed-children diagnostic).
    text_prop: Option<String>,
    /// The fragment being lowered may hold `{@const}` (set by the caller, consumed by `fragment`).
    const_ok: bool,
    /// Snippets declared in this component by name: their root elements (None: the name is
    /// declared more than once, so render sites are not checked statically).
    snippet_roots: HashMap<String, Option<Vec<&'a AstElement>>>,
    /// Imports of the scripts (renderer `listen`).
    script: ScriptFacts,
    /// Local names of `ListView` imported from `@svelters/lists` (virtualized lists, `lists.rs`).
    list_views: HashSet<String>,
    /// Some element resolves attributes at run time (the module imports `$$kinds`).
    kinds: bool,
    /// TR1: span starts of the elements that may carry `animate:` (the only element of a keyed each).
    animate_hosts: HashSet<u32>,
    /// FW1: `legacy_forwarding_lowered` was reported (once per component).
    forwarding_noted: bool,
}

fn sp(s: Span) -> (u32, u32) {
    (s.start, s.end)
}

/// A snippet parameter / `{:then}` / `{:catch}` binding.
fn param(p: &Pattern) -> Param<'_> {
    match p {
        Pattern::Identifier(id) => Param::Id(id.name.clone()),
        other => Param::Pattern(other),
    }
}

/// Pre-pass: every `{#snippet}` of the component with its root elements (looking through blocks),
/// so render sites can be checked before or after the declaration (snippets are hoisted).
fn collect_snippets<'a>(f: &'a AstFragment, out: &mut HashMap<String, Option<Vec<&'a AstElement>>>) {
    for n in &f.nodes {
        for child in child_fragments(n) {
            collect_snippets(child, out);
        }
        if let FragmentNode::SnippetBlock(b) = n {
            let mut roots = Vec::new();
            root_elements(&b.body, &mut roots);
            match out.get_mut(&b.expression.name) {
                None => {
                    out.insert(b.expression.name.clone(), Some(roots));
                }
                Some(e) => *e = None,
            }
        }
    }
}

fn root_elements<'a>(f: &'a AstFragment, out: &mut Vec<&'a AstElement>) {
    for n in &f.nodes {
        match n {
            FragmentNode::RegularElement(el) if el.name != SLOT_PROXY => out.push(el),
            FragmentNode::IfBlock(_) | FragmentNode::EachBlock(_) | FragmentNode::KeyBlock(_) | FragmentNode::AwaitBlock(_) => {
                for c in child_fragments(n) {
                    root_elements(c, out);
                }
            }
            _ => {}
        }
    }
}

fn child_fragments(n: &FragmentNode) -> Vec<&AstFragment> {
    use FragmentNode::*;
    match n {
        RegularElement(e) | Component(e) => vec![&e.fragment],
        IfBlock(b) => [Some(&b.consequent), b.alternate.as_ref()].into_iter().flatten().collect(),
        EachBlock(b) => [Some(&b.body), b.fallback.as_ref()].into_iter().flatten().collect(),
        KeyBlock(b) => vec![&b.fragment],
        AwaitBlock(b) => [b.pending.as_ref(), b.then.as_ref(), b.catch.as_ref()].into_iter().flatten().collect(),
        SnippetBlock(b) => vec![&b.body],
        _ => vec![],
    }
}

fn dynamic_of(at: &Attribute) -> Option<Dynamic<'_>> {
    match &at.value {
        AttributeValue::True(_) => None,
        AttributeValue::Expression(ExpressionTagNode::ExpressionTag(t)) => Some(Dynamic::Expr(&t.expression)),
        AttributeValue::Sequence(parts) => Some(Dynamic::Template(
            parts
                .iter()
                .map(|p| match p {
                    TextOrExpressionTag::Text(t) => TemplatePart::Text(t.data.clone()),
                    TextOrExpressionTag::ExpressionTag(e) => TemplatePart::Expr(&e.expression),
                })
                .collect(),
        )),
    }
}

impl<'a> Lower<'a, '_, '_> {
    /// Setter rules take one compound of type, `.class` and `#id` (as on the stock path): rules
    /// with setter declarations and a combinator or a `:state` are reported, those selectors unused.
    fn check_style_sheet(&mut self) {
        let Some(sheet) = self.styles else { return };
        for rule in &sheet.rules {
            let names: Vec<&str> = rule.declarations.iter().filter(|d| self.sem.is_setter_property(&d.name)).map(|d| d.name.as_str()).collect();
            if names.is_empty() {
                continue;
            }
            let props = names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ");
            for sel in &rule.selectors {
                let why = if sel.parts.len() > 1 {
                    Some(("style_unsupported_combinator", "combinators are not supported"))
                } else if sel.parts.iter().any(|(_, c)| !c.states.is_empty()) {
                    Some(("style_state_unsupported", "states are CSS-only"))
                } else {
                    None
                };
                if let Some((code, why)) = why {
                    self.bad_selectors.insert(sel.span);
                    self.diags
                        .error(code, sel.span, format!("setter properties ({props}) only take type, class and id selectors: {why}"))
                        .help("move the setter properties to a rule with a single compound selector (`box.toolbar`, `#main`); keep CSS properties where they are");
                }
            }
        }
    }

    /// A block branch: alternatives may share single-child slots; `{@const}` allowed per Svelte.
    fn branch(&mut self, f: &'a AstFragment, parent: &Parent, ancestors: &[StyleSubject<'a>], const_ok: bool) -> Fragment<'a> {
        let outer = std::mem::replace(&mut self.in_block, true);
        self.const_ok = const_ok;
        let out = self.fragment(f, parent, ancestors);
        self.in_block = outer;
        out
    }

    fn fragment(&mut self, f: &'a AstFragment, parent: &Parent, ancestors: &[StyleSubject<'a>]) -> Fragment<'a> {
        let const_ok = std::mem::take(&mut self.const_ok);
        let mut out = Fragment::default();
        // A backend with text nodes (spec `text_node`): text runs become text elements.
        let text_node = self.sem.text_node();
        let entries = match &text_node {
            Some(_) => {
                let parent_tag = match parent {
                    Parent::Root | Parent::Detached => None,
                    Parent::Element(p, _) => Some(p.tag.as_str()),
                };
                text_runs(&f.nodes, parent_tag)
            }
            None => f.nodes.iter().map(Entry::Node).collect(),
        };
        for entry in entries {
            let node = match entry {
                Entry::Node(n) => n,
                Entry::Text(parts, span) => {
                    let (er, prop) = text_node.clone().expect("text runs only with a text_node spec");
                    if let Some(e) = self.text_element(&er, &prop, parts, span, parent) {
                        out.items.push(Item::Element(e));
                    }
                    continue;
                }
            };
            match node {
                FragmentNode::Text(t) => {
                    if !t.data.trim().is_empty() {
                        let msg = match parent {
                            Parent::Root => "text is not allowed at the top level of a native component".to_string(),
                            Parent::Detached => "text is not allowed here: snippet and component-children content goes among native children".to_string(),
                            Parent::Element(p, _) => format!("<{}> cannot contain text on {}", p.tag, self.sem.backend()),
                        };
                        self.diags
                            .native_error("text_not_allowed", (t.start, t.end), msg)
                            .help("put text in a <label> (or an element whose spec has text content)");
                    }
                }
                FragmentNode::Comment(_) => {}
                FragmentNode::ExpressionTag(e) => {
                    self.diags.native_error(
                        "text_not_allowed",
                        (e.start, e.end),
                        "an expression tag here would be text, which only text-content elements (e.g. <label>) can hold",
                    );
                }
                FragmentNode::RegularElement(el) if el.name == SLOT_PROXY => self.slot_proxy(el, parent, ancestors, &mut out),
                FragmentNode::RegularElement(el) => {
                    if let Some(e) = self.element(el, parent, ancestors) {
                        out.items.push(Item::Element(e));
                    }
                }
                FragmentNode::Component(el) => {
                    if let Some(c) = self.component(el, ancestors) {
                        out.items.push(Item::Component(c));
                    }
                }
                // `<svelte:boundary onerror failed>`: called like a component whose props are its
                // attributes and snippets (`failed`, `pending`, `children`), see runtime `boundary`
                FragmentNode::SvelteComponent(n) => {
                    if let Some(c) = self.svelte_component(n, ancestors) {
                        out.items.push(Item::Component(c));
                    }
                }
                FragmentNode::SvelteElement(n) => {
                    if let Some(d) = self.dynamic_element(n, parent, ancestors) {
                        out.items.push(Item::Dynamic(d));
                    }
                }
                FragmentNode::SvelteBoundary(el) => {
                    if let Some(mut c) = self.component(el, ancestors) {
                        c.name = "$n.boundary".to_string();
                        out.items.push(Item::Component(c));
                    }
                }
                FragmentNode::IfBlock(b) => {
                    let consequent = self.branch(&b.consequent, parent, ancestors, true);
                    let alternate = b.alternate.as_ref().map(|a| self.branch(a, parent, ancestors, true));
                    let slot = self.block_slot((b.start, b.end), [Some(&consequent), alternate.as_ref()]);
                    out.items.push(Item::If(IfBlock { span: b.span(), elseif: b.elseif, slot, test: &b.test, consequent, alternate }));
                }
                FragmentNode::EachBlock(b) => {
                    // TR1: `animate:` is allowed on the only element of a keyed each (Svelte's
                    // animation_invalid_placement / animation_missing_key)
                    if let Some(e) = animate_host(&b.body) {
                        self.animate_hosts.insert(e.start);
                        if b.key.is_none() {
                            self.missing_key_animate(e);
                        }
                    }
                    let context = b.context.as_ref();
                    let body = self.branch(&b.body, parent, ancestors, true);
                    let fallback = b.fallback.as_ref().map(|f| self.branch(f, parent, ancestors, true));
                    let slot = self.block_slot((b.start, b.end), [Some(&body), fallback.as_ref()]);
                    out.items.push(Item::Each(EachBlock {
                        span: b.span(),
                        slot,
                        collection: &b.expression,
                        context,
                        index: b.index.clone(),
                        key: b.key.as_ref(),
                        body,
                        fallback,
                    }));
                }
                FragmentNode::KeyBlock(b) => {
                    let body = self.branch(&b.fragment, parent, ancestors, false);
                    let slot = self.block_slot((b.start, b.end), [Some(&body)]);
                    out.items.push(Item::Key(KeyBlock { span: b.span(), slot, key: &b.expression, body }));
                }
                FragmentNode::AwaitBlock(b) => {
                    let value = b.value.as_ref().map(param);
                    let error = b.error.as_ref().map(param);
                    let pending = b.pending.as_ref().map(|f| self.branch(f, parent, ancestors, false));
                    let then = b.then.as_ref().map(|f| self.branch(f, parent, ancestors, true));
                    let catch = b.catch.as_ref().map(|f| self.branch(f, parent, ancestors, true));
                    let slot = self.block_slot((b.start, b.end), [pending.as_ref(), then.as_ref(), catch.as_ref()]);
                    out.items.push(Item::Await(AwaitBlock { span: b.span(), slot, expression: &b.expression, value, error, pending, then, catch }));
                }
                FragmentNode::SnippetBlock(b) => {
                    let s = self.snippet(b, ancestors);
                    out.snippets.push(s);
                }
                FragmentNode::RenderTag(t) => {
                    if let Some(r) = self.render(t, parent) {
                        out.items.push(Item::Render(r));
                    }
                }
                FragmentNode::ConstTag(c) => {
                    if !const_ok {
                        self.diags.error(
                            "const_tag_invalid_placement",
                            (c.start, c.end),
                            "`{@const}` must be the immediate child of `{#snippet}`, `{#if}`, `{:else if}`, `{:else}`, `{#each}`, `{:then}`, `{:catch}`, `<svelte:fragment>`, `<svelte:boundary>` or `<Component>`",
                        );
                        continue;
                    }
                    let d = &c.declaration.declarations[0];
                    match (&d.id, &d.init) {
                        (Pattern::Identifier(id), Some(init)) => out.consts.push(ConstDecl { span: c.span(), name: id.name.clone(), init: Some(init), pattern: None, declaration: None }),
                        (pattern, Some(init)) => out.consts.push(ConstDecl { span: c.span(), name: String::new(), init: Some(init), pattern: Some(pattern), declaration: None }),
                        // the parser requires an initializer
                        (_, None) => {}
                    }
                }
                // `{let x = $state(0)}` / `{const y = ...}`: declared where the fragment's
                // `{@const}`s are, transformed like a script declaration
                FragmentNode::DebugTag(d) => out.debugs.push(d),
                FragmentNode::DeclarationTag(d) => {
                    out.consts.push(ConstDecl { span: d.span(), name: String::new(), init: None, pattern: None, declaration: Some(&d.declaration) })
                }
                other => {
                    let what = unsupported_node_name(other);
                    self.diags.error("unsupported_feature", sp(other.span()), format!("{what} is not supported by the native compiler yet"));
                }
            }
        }
        out
    }

    /// A text run as an element of the backend's text kind (spec `text_node`).
    fn text_element(&mut self, er: &ElementRef, prop: &str, parts: Vec<TemplatePart<'a>>, span: Span, parent: &Parent) -> Option<Element<'a>> {
        let slot = match parent {
            Parent::Root | Parent::Detached => None,
            Parent::Element(p, _) => match self.sem.place_child(p, er, None) {
                Ok(plan) => (!plan.is_default).then_some(plan.slot),
                Err(u) => {
                    u.report(self.diags, sp(span));
                    return None;
                }
            },
        };
        let id = self.next_id;
        self.next_id += 1;
        let op = if parts.iter().all(|p| matches!(p, TemplatePart::Text(_))) {
            let text: String = parts.iter().map(|p| if let TemplatePart::Text(t) = p { t.as_str() } else { "" }).collect();
            Op::Set { prop: prop.to_string(), value: Json::String(text), span }
        } else {
            Op::SetDynamic { prop: prop.to_string(), value: Dynamic::Template(parts), span }
        };
        Some(Element { id, span, kind: er.tag.clone(), present: None, toplevel: false, show: None, attached: false, slot, class_expr: None, id_expr: None, ops: vec![op], children: Fragment::default() })
    }

    /// The modifiers of a native element's `bind:` (REACTIVITY.md section 8.2): `Some(nonreactive)`,
    /// or `None` after reporting `bind_modifier_invalid` for an unknown or repeated modifier (stock
    /// Svelte ignores them silently; the native compiler must not).
    fn native_bind_modifiers(&mut self, d: &crate::svelte::ast::Directive) -> Option<bool> {
        let allowed = self.sem.bind_modifiers();
        let mut nonreactive = false;
        let mut ok = true;
        for m in &d.modifiers {
            if m == "nonreactive" && allowed.contains(&"nonreactive") && !nonreactive {
                nonreactive = true;
                continue;
            }
            ok = false;
            let msg = if m == "nonreactive" && nonreactive {
                format!("`bind:{}`: `|nonreactive` is given twice", d.name)
            } else if allowed.is_empty() {
                format!("`bind:{}|{m}`: bindings take no modifiers on {}", d.name, self.sem.backend())
            } else {
                format!("`bind:{}|{m}`: unknown binding modifier `{m}` (native bindings take {})", d.name, allowed.iter().map(|a| format!("`|{a}`")).collect::<Vec<_>>().join(", "))
            };
            self.diags.native_error(codes::BIND_MODIFIER_INVALID, sp(d.span()), msg);
        }
        ok.then_some(nonreactive)
    }

    /// `bind:this` and component bindings take no modifiers: `|nonreactive` has no meaning there.
    fn no_bind_modifiers(&mut self, d: &crate::svelte::ast::Directive, what: &str) {
        if let Some(m) = d.modifiers.first() {
            let why = if m == "nonreactive" { "has no meaning" } else { "is not a binding modifier" };
            self.diags
                .native_error(codes::BIND_MODIFIER_INVALID, sp(d.span()), format!("`bind:{}|{}`: `|{m}` {why} on {what}", d.name, d.modifiers.join("|")))
                .help("`|nonreactive` applies to bindings of native element properties only");
        }
    }

    /// A block region lives in one (parent, slot): every item inside must target the same slot.
    fn block_slot<const N: usize>(&mut self, span: (u32, u32), frags: [Option<&Fragment<'a>>; N]) -> Slot {
        let mut slots: Vec<&Slot> = Vec::new();
        for f in frags.into_iter().flatten() {
            for item in &f.items {
                slots.push(item.slot());
            }
        }
        let Some(first) = slots.first().cloned().cloned() else { return self.forced_slot.clone() };
        if slots.iter().any(|s| **s != first) {
            self.diags.native_error(
                "block_mixed_slots",
                span,
                "the elements inside one {#if}/{#each}/{#key}/{#await} block must all go to the same child slot of their parent",
            );
        }
        first
    }

    /// `<sv-slot name="x">...</sv-slot>`: no native; the content goes to the parent's slot `x`.
    fn slot_proxy(&mut self, el: &'a AstElement, parent: &Parent, ancestors: &[StyleSubject<'a>], out: &mut Fragment<'a>) {
        let span = (el.start, el.end);
        if !matches!(parent, Parent::Element(..)) {
            self.diags.error("slot_proxy_placement", sp(el.name_span), "<sv-slot> must be a direct child of a native element (it names one of that element's slots)");
            return;
        }
        if self.forced_slot.is_some() {
            self.diags.error("slot_proxy_placement", sp(el.name_span), "<sv-slot> cannot be nested in another <sv-slot>");
            return;
        }
        let mut name = None;
        for a in &el.attributes {
            match a {
                AttributeLike::Attribute(at) if at.name == "name" => match static_text(at) {
                    Some(s) => name = Some(s),
                    None => {
                        self.diags.error("slot_not_static", sp(at.span()), "<sv-slot name> must be a static slot name");
                    }
                },
                other => {
                    self.diags.error("slot_proxy_attribute", sp(other.span()), "<sv-slot> takes only a static `name`");
                }
            }
        }
        let Some(name) = name else {
            self.diags.error("slot_proxy_attribute", span, "<sv-slot> needs a static `name` (the parent's slot)");
            return;
        };
        if let Parent::Element(p, _) = parent {
            self.check_slot_exists(p, Some(&name), sp(el.name_span));
        }
        let outer = self.forced_slot.replace(name);
        let frag = self.fragment(&el.fragment, parent, ancestors);
        self.forced_slot = outer;
        out.snippets.extend(frag.snippets);
        out.items.extend(frag.items);
    }

    /// Whether `parent` has slot `name` (None: a default slot), whatever the child: content whose
    /// kinds are unknown at compile time (snippet props) is checked by the host at insertion.
    fn check_slot_exists(&mut self, parent: &ElementRef, name: Option<&str>, span: (u32, u32)) {
        use crate::semantics::codes;
        if let Err(u) = self.sem.place_child(parent, parent, name)
            && [codes::UNKNOWN_SLOT, codes::CHILDREN_NOT_ALLOWED, codes::NO_DEFAULT_SLOT].contains(&u.code)
        {
            u.report(self.diags, span);
        }
    }

    fn snippet(&mut self, b: &'a SnippetBlock, ancestors: &[StyleSubject<'a>]) -> Snippet<'a> {
        let params = b.parameters.iter().map(param).collect();
        let forced = self.forced_slot.take();
        let body = self.branch(&b.body, &Parent::Detached, ancestors, true);
        self.forced_slot = forced;
        Snippet { span: b.span(), name: b.expression.name.clone(), params, body }
    }

    /// `{@render f(args)}`. At a native parent, a snippet declared in this component is checked
    /// against the parent's slots: each of its root elements must be placeable in the slot the
    /// render site targets, and they must all go to the same slot (the region's).
    fn render(&mut self, t: &'a RenderTag, parent: &Parent) -> Option<RenderCall<'a>> {
        let span = (t.start, t.end);
        let (call, optional) = match &t.expression {
            Expression::CallExpression(c) => (c, c.optional),
            Expression::ChainExpression(ch) => match ch.expression.as_ref() {
                Expression::CallExpression(c) => (c, true),
                _ => {
                    self.diags.error("render_tag_invalid_expression", span, "`{@render ...}` tags can only contain call expressions");
                    return None;
                }
            },
            _ => {
                self.diags.error("render_tag_invalid_expression", span, "`{@render ...}` tags can only contain call expressions");
                return None;
            }
        };
        let mut args = Vec::new();
        for a in &call.arguments {
            match a {
                ArrayElement::Expression(e) => args.push(e),
                ArrayElement::Spread(_) => {
                    self.diags.error("render_tag_invalid_spread_argument", span,"cannot use spread arguments in `{@render ...}` tags");
                }
            }
        }
        let forced = self.forced_slot.clone();
        let mut slot = forced.clone();
        if let Parent::Element(p, _) = parent {
            let local = match call.callee.as_ref() {
                Expression::Identifier(id) => self.snippet_roots.get(&id.name).cloned().flatten().map(|r| (id.name.clone(), r)),
                _ => None,
            };
            if let Some((name, roots)) = local {
                let mut slots: Vec<Slot> = Vec::new();
                for el in roots {
                    let Ok(er) = self.sem.resolve_element(&el.name) else { continue }; // reported at the element
                    let own = el.attributes.iter().find_map(|a| match a {
                        AttributeLike::Attribute(at) if at.name == "slot" || at.name == "data-slot" => static_text(at),
                        _ => None,
                    });
                    if let (Some(o), Some(f)) = (&own, &forced)
                        && o != f
                    {
                        self.diags.error("slot_conflict", span, format!("snippet `{name}` puts <{}> in slot `{o}`, but this render site is in <sv-slot name=\"{f}\">", el.name));
                        continue;
                    }
                    let req = own.or(forced.clone());
                    match self.sem.place_child(p, &er, req.as_deref()) {
                        Ok(plan) => {
                            self.check_snippet_child_attrs(p, &plan.slot, el, &name, span);
                            slots.push((!plan.is_default).then_some(plan.slot))
                        }
                        // an unknown <sv-slot> name was reported at the <sv-slot>
                        Err(u) if u.code == crate::semantics::codes::UNKNOWN_SLOT && forced.is_some() => {}
                        Err(u) => {
                            let u = Unresolved {
                                message: format!("{{@render {name}()}} places <{}> (a root element of snippet `{name}`) in <{}>: {}", el.name, p.tag, u.message),
                                ..u
                            };
                            u.report(self.diags, span);
                        }
                    }
                }
                if let Some(first) = slots.first().cloned() {
                    if slots.iter().any(|s| *s != first) {
                        self.diags.native_error(
                            "block_mixed_slots",
                            span,
                            format!("the root elements of snippet `{name}` go to different child slots of <{}>; a rendered snippet fills one slot", p.tag),
                        );
                    }
                    slot = first;
                }
            } else if forced.is_none() {
                // content unknown here (a prop, an import): at least the parent needs a default slot
                self.check_slot_exists(p, None, span);
            }
        }
        Some(RenderCall { span: t.span(), slot, callee: &call.callee, args, optional })
    }

    fn element(&mut self, el: &'a AstElement, parent: &Parent, ancestors: &[StyleSubject<'a>]) -> Option<Element<'a>> {
        let span = (el.start, el.end);
        let tag_span = sp(el.name_span);
        let er = match self.sem.resolve_element(&el.name) {
            Ok(r) => r,
            Err(u) => {
                u.report(self.diags, if tag_span.1 > tag_span.0 { tag_span } else { span });
                return None;
            }
        };
        let id = self.next_id;
        self.next_id += 1;
        // A backend whose kinds have a `class` prop (the DOM conformance backend) also gets
        // `class` / `class:` as that prop; GTK and Qt have none, so classes only drive styles.
        let class_prop = self.sem.writable_property(&er, "class").ok().map(|p| p.js.clone());
        // GTK / Qt: `class`, `id`, `style` and `style:` drive the X6 styling and setter rules
        let native_style = class_prop.is_none();
        let parent_er = match parent {
            Parent::Element(p, _) => Some(p.clone()),
            _ => None,
        };

        // pass 1: slot request and classes (they don't produce ops)
        let mut requested_slot: Option<String> = None;
        let mut class_expr = None;
        let mut id_expr = None;
        let mut subject = StyleSubject { tag: el.name.clone(), id: None, dynamic_id: None, static_classes: Vec::new(), cond_classes: Vec::new(), dynamic: None };
        let mut has_class = false;
        for a in &el.attributes {
            match a {
                AttributeLike::Attribute(at) if at.name == "slot" || at.name == "data-slot" => match static_text(at) {
                    Some(s) => requested_slot = Some(s),
                    None => {
                        self.diags.error("slot_not_static", sp(at.span()), "`slot` must be a static name");
                    }
                },
                AttributeLike::Attribute(at) if native_style && at.name == "id" => {
                    subject.id = static_text(at);
                    if subject.id.is_none() {
                        // matched at runtime against the rules' `#id` conditions, like a dynamic class
                        id_expr = dynamic_of(at);
                        subject.dynamic_id = id_expr.is_some().then_some(id);
                    }
                }
                AttributeLike::Attribute(at) if at.name == "class" => match static_text(at) {
                    Some(s) => {
                        has_class = true;
                        subject.static_classes.extend(s.split_ascii_whitespace().map(str::to_string))
                    }
                    None if class_prop.is_some() => {}
                    None => {
                        // matched at runtime against the rules' class conditions (runtime-contract.md)
                        has_class = true;
                        class_expr = dynamic_of(at);
                        subject.dynamic = class_expr.is_some().then_some(id);
                    }
                },
                AttributeLike::ClassDirective(d) => match &d.expression {
                    Some(e) => {
                        has_class = true;
                        subject.cond_classes.push((d.name.clone(), e))
                    }
                    None => unreachable!("the parser synthesises class:x shorthands"),
                },
                _ => {}
            }
        }
        if let Some(f) = &self.forced_slot {
            match &requested_slot {
                Some(r) if r != f => {
                    self.diags.error("slot_conflict", tag_span, format!("<{}> asks for slot `{r}` inside <sv-slot name=\"{f}\">", el.name));
                }
                _ => requested_slot = Some(f.clone()),
            }
        }

        // placement (`attr_target`: where `parent:` attributes go, when known here)
        let mut attr_target: Option<(ElementRef, String)> = None;
        let lt = self.sem.lifetime(&er);
        let toplevel = lt.lifetime == Lifetime::Toplevel;
        let mut attached = false;
        let (slot, present) = match parent {
            Parent::Root | Parent::Detached => {
                // A toplevel kind is shown when mounted at the root (its `show` policy). Other kinds
                // are allowed at a component's top level: a child component's root elements go to the
                // container the parent passes in (placement is then checked at runtime; the compiler
                // cannot see across component boundaries). Mounting a non-toplevel at the app root
                // throws. Detached content (snippets, component children) is placed by its render site.
                let slot = if matches!(parent, Parent::Detached) && !toplevel { requested_slot.clone() } else { None };
                (slot, if toplevel { lt.present.clone() } else { None })
            }
            // X7: a toplevel declared inside an element is transient for the enclosing toplevel
            // (backend-abi.md "Toplevels in the tree"), never a child of the element's slots.
            Parent::Element(..) if toplevel => (None, lt.present.clone()),
            Parent::Element(p, _) if self.sem.attaches_to(&er).is_some_and(|t| self.sem.is_a(p, t)) && self.sem.place_child(p, &er, requested_slot.as_deref()).is_err() => {
                // an event controller, a context menu: inserted into the element itself, no slot
                attached = true;
                if requested_slot.is_some() {
                    self.diags.native_error("slot_not_applicable", tag_span, format!("<{}> attaches to <{}> and takes no slot", el.name, p.tag));
                }
                (None, None)
            }
            Parent::Element(p, pid) => match self.sem.place_child(p, &er, requested_slot.as_deref()) {
                Ok(plan) => {
                    if !plan.many {
                        // a single-child slot: a second statically placed element is an error
                        let key = (*pid, plan.slot.clone());
                        if let Some(first) = self.single_slots.get(&key) {
                            self.diags
                                .native_error(
                                    "slot_cardinality",
                                    tag_span,
                                    format!("<{}>'s `{}` slot takes one child on {}, and it already has <{}>", p.tag, plan.slot, self.sem.backend(), first),
                                )
                                .help("wrap the children in a container such as <box>");
                        } else if !self.in_block {
                            self.single_slots.insert(key, el.name.clone());
                        }
                    }
                    attr_target = Some((p.clone(), plan.slot.clone()));
                    // an explicitly named slot is passed by name even when it is the default one, as the
                    // renderer does (`data-slot="center"` -> `insert(.., "center")`)
                    ((!plan.is_default || plan.routing == crate::semantics::Routing::Explicit).then_some(plan.slot), None)
                }
                Err(u) => {
                    u.report(self.diags, tag_span);
                    (None, None)
                }
            },
        };

        // A spread whose keys are only known at run time: every attribute goes into one object in
        // source order, applied by `$n.spread` (runtime-contract.md "Spread attributes"); explicit
        // attributes are still resolved and typed here.
        let runtime_attrs = el.attributes.iter().any(|a| match a {
            AttributeLike::SpreadAttribute(s) => !self.is_static_spread(&s.expression),
            // a backend whose `style` is a prop (the DOM conformance backend): `style:` directives
            // merge into it at run time, as Svelte's `[STYLE]` attribute object
            AttributeLike::StyleDirective(_) => !native_style,
            _ => false,
        });
        let mut entries: Vec<SpreadEntry<'a>> = Vec::new();
        let mut spread_at: Option<usize> = None;
        let mut spread_span: Option<Span> = None;

        // pass 2: ops in source order
        let mut ops: Vec<Op<'a>> = Vec::new();
        let mut explicit_props: Vec<String> = Vec::new();
        // TR1: the element's `in` / `out` transitions so far, and whether it has `animate:`
        let mut transitions: (Option<&'a crate::svelte::ast::TransitionDirective>, Option<&'a crate::svelte::ast::TransitionDirective>) = (None, None);
        let mut animate_seen = false;
        let mut bound_props: Vec<String> = Vec::new();
        // inline style: static CSS text, a dynamic `style=`, CSS `style:` directives, setters
        let mut inline_css = String::new();
        let mut inline_dynamic: Option<Dynamic<'a>> = None;
        let mut inline_dirs: Vec<(String, AttrValue<'a>)> = Vec::new();
        let mut inline_setters: Vec<(StyleLowering, Span)> = Vec::new();
        let mut style_span: Option<Span> = None;
        let mut show: Option<String> = None;
        self.check_duplicate_events(&el.attributes, OnSite::Element(&el.name));
        for a in &el.attributes {
            match a {
                // X7: how a toplevel is shown once it (and the enclosing toplevel) is in place
                AttributeLike::Attribute(at) if toplevel && at.name == "show" => match static_text(at).as_deref() {
                    Some(m @ ("modal" | "modeless" | "none")) => show = Some(m.to_string()),
                    _ => {
                        self.diags
                            .native_error("show_invalid", name_span(at), format!("<{}> `show` must be a static \"modal\", \"modeless\" or \"none\"", el.name))
                            .help("`none` leaves opening the toplevel to the app (`bind:this` + call('present'))");
                    }
                },
                AttributeLike::Attribute(at) if matches!(at.name.as_str(), "slot" | "data-slot") => {}
                // X6 styling attributes of an element whose attributes are applied at run time
                AttributeLike::Attribute(at) if runtime_attrs && native_style && matches!(at.name.as_str(), "class" | "id" | "style") => {
                    spread_at.get_or_insert(ops.len());
                    entries.push(SpreadEntry::Attr { key: at.name.clone(), value: attr_value(at) });
                }
                AttributeLike::Attribute(at) if at.name == "class" && class_prop.is_none() => {}
                AttributeLike::Attribute(at) if native_style && at.name == "id" => {
                    let value = match (static_text(at), dynamic_of(at)) {
                        (Some(t), _) => AttrValue::Static(Json::String(t)),
                        (None, Some(d)) => AttrValue::Dynamic(d),
                        (None, None) => AttrValue::Static(Json::String(String::new())),
                    };
                    ops.push(Op::WidgetName { value, span: at.span() });
                }
                AttributeLike::Attribute(at) if native_style && at.name == "style" => {
                    style_span.get_or_insert(at.span());
                    match static_text(at) {
                        Some(text) => inline_css = self.inline_style(&er, parent_er.as_ref(), &text, sp(at.span()), &mut inline_setters),
                        None => inline_dynamic = dynamic_of(at),
                    }
                }
                AttributeLike::StyleDirective(d) if runtime_attrs => {
                    if let Some(value) = self.directive_value(d) {
                        spread_at.get_or_insert(ops.len());
                        entries.push(SpreadEntry::Style { name: d.name.clone(), value });
                    }
                }
                AttributeLike::StyleDirective(d) if native_style => {
                    style_span.get_or_insert(d.span());
                    self.style_directive(&er, parent_er.as_ref(), d, &mut ops, &mut inline_dirs, &mut inline_setters);
                }
                // TR1: the transition events are the runtime's own (dispatched by the node's
                // `dispatchEvent`), on every element, in no backend spec
                AttributeLike::Attribute(at) if matches!(at.name.as_str(), "onintrostart" | "onintroend" | "onoutrostart" | "onoutroend") && matches!(at.value, AttributeValue::Expression(_)) => {
                    let AttributeValue::Expression(ExpressionTagNode::ExpressionTag(t)) = &at.value else { unreachable!() };
                    ops.push(Op::Listen { signal: at.name[2..].to_string(), handler: Handler::Expr(&t.expression), flags: 0, sync: false, span: at.span() });
                }
                AttributeLike::Attribute(at) if at.name.starts_with("on") && matches!(at.value, AttributeValue::Expression(_)) => {
                    let AttributeValue::Expression(ExpressionTagNode::ExpressionTag(t)) = &at.value else { unreachable!() };
                    let op = self.listen_op(&er, &at.name, Handler::Expr(&t.expression), at.span(), name_span(at));
                    if runtime_attrs {
                        if op.is_some() {
                            spread_at.get_or_insert(ops.len());
                            entries.push(SpreadEntry::Event { key: at.name.clone(), handler: &t.expression });
                        }
                    } else {
                        ops.extend(op);
                    }
                }
                AttributeLike::Attribute(at) if at.name.starts_with(CHILD_ATTR_PREFIX) => {
                    let name = &at.name[CHILD_ATTR_PREFIX.len()..];
                    ops.extend(self.child_attr_op(attr_target.as_ref(), name, at));
                }
                // FW1: a bare `on:name` forwards the component's own callback prop, the listener
                // `onname={$$props.onname}` (ARCH-EMBED §8.7)
                AttributeLike::OnDirective(d) if d.modifiers.is_empty() && d.expression.is_none() => {
                    let ns = d.name_span.map_or(sp(d.span()), sp);
                    let key = format!("on{}", d.name);
                    self.note_forwarding(d, OnSite::Element(&el.name));
                    let op = self.listen_op(&er, &key, Handler::Forward { key: key.clone() }, d.span(), ns);
                    if runtime_attrs {
                        if op.is_some() {
                            spread_at.get_or_insert(ops.len());
                            entries.push(SpreadEntry::Forward { key });
                        }
                    } else {
                        ops.extend(op);
                    }
                }
                AttributeLike::OnDirective(d) if !d.modifiers.is_empty() => self.on_modifiers(d, OnSite::Element(&el.name)),
                // Legacy `on:name={handler}` (deprecated in Svelte 5, still compiled in runes mode): a
                // listener like `onname={handler}`
                AttributeLike::OnDirective(d) if d.modifiers.is_empty() && d.expression.is_some() => {
                    let ns = d.name_span.map_or(sp(d.span()), sp);
                    let key = format!("on{}", d.name);
                    let op = self.listen_op(&er, &key, Handler::Expr(d.expression.as_ref().unwrap()), d.span(), ns);
                    if runtime_attrs {
                        if op.is_some() {
                            spread_at.get_or_insert(ops.len());
                            entries.push(SpreadEntry::Event { key, handler: d.expression.as_ref().unwrap() });
                        }
                    } else {
                        ops.extend(op);
                    }
                }
                AttributeLike::AttachTag(t) => ops.extend(self.attach_op(&er, t)),
                AttributeLike::UseDirective(d) => ops.push(Op::Action { name: d.name.clone(), arg: d.expression.as_ref(), span: d.span() }),
                AttributeLike::TransitionDirective(d) => {
                    if let Some(op) = self.transition_op(d, &mut transitions) {
                        ops.push(op);
                    }
                }
                AttributeLike::AnimateDirective(d) => {
                    if animate_seen {
                        self.diags.error("animation_duplicate", sp(d.span()), "An element can only have one 'animate' directive");
                    } else if !self.animate_hosts.contains(&el.start) {
                        self.diags.error("animation_invalid_placement", sp(d.span()), "An element that uses the `animate:` directive must be the only child of a keyed `{#each ...}` block");
                    } else {
                        ops.push(Op::Animate { name: d.name.clone(), arg: d.expression.as_ref(), span: d.span() });
                    }
                    animate_seen = true;
                }
                AttributeLike::Attribute(at) => {
                    let prop = match self.sem.writable_property(&er, &at.name) {
                        Ok(p) => p.clone(),
                        Err(u) => {
                            u.report(self.diags, name_span(at));
                            continue;
                        }
                    };
                    explicit_props.push(prop.js.clone());
                    if runtime_attrs {
                        // typed here, applied (in source order with the spread) by `$n.spread`
                        let value = match (static_text(at), &at.value) {
                            (_, AttributeValue::True(_)) => self.sem.coerce_static(&prop, None).map(|v| AttrValue::Static(v.to_json())),
                            (Some(text), _) => self.sem.coerce_static(&prop, Some(&text)).map(|v| AttrValue::Static(v.to_json())),
                            (None, _) => Ok(AttrValue::Dynamic(dynamic_of(at).expect("not a bare attribute"))),
                        };
                        match value {
                            Ok(value) => {
                                spread_at.get_or_insert(ops.len());
                                entries.push(SpreadEntry::Attr { key: prop.js.clone(), value });
                            }
                            Err(msg) => {
                                self.diags.native_error("bad_attribute_value", sp(at.span()), msg);
                            }
                        }
                        continue;
                    }
                    match &at.value {
                        AttributeValue::True(_) => self.push_static(&mut ops, &prop, None, at),
                        AttributeValue::Sequence(parts) if parts.iter().all(|p| matches!(p, TextOrExpressionTag::Text(_))) => {
                            let text: String = parts.iter().map(|p| if let TextOrExpressionTag::Text(t) = p { t.data.as_str() } else { "" }).collect();
                            self.push_static(&mut ops, &prop, Some(&text), at);
                        }
                        _ => {
                            let value = dynamic_of(at).expect("not a bare attribute");
                            ops.push(Op::SetDynamic { prop: prop.js.clone(), value, span: at.span() });
                        }
                    }
                    // a one-way prop the native side also edits: mirror native edits (no echo)
                    if let Ok(plan) = self.sem.plan_binding(&er, &prop.js) {
                        ops.push(Op::Track { prop: plan.prop.clone(), signal: plan.signal.clone(), arg: plan.arg_operand() });
                    }
                }
                AttributeLike::BindDirective(d) if d.name == "this" => {
                    let Some(target) = &d.expression else { unreachable!("the parser synthesises bind:x shorthands") };
                    self.no_bind_modifiers(d, "bind:this");
                    ops.push(Op::BindThis { target, span: d.span() });
                }
                AttributeLike::BindDirective(d) => {
                    let Some(target) = &d.expression else { unreachable!("the parser synthesises bind:x shorthands") };
                    let Some(nonreactive) = self.native_bind_modifiers(d) else { continue };
                    match plan_bind(self.sem, &er, &d.name, nonreactive) {
                        Ok(plan) => {
                            for w in &plan.warnings {
                                w.report_warning(self.diags, sp(d.span()));
                            }
                            bound_props.push(plan.prop.clone());
                            let (prop, span) = (plan.prop, d.span());
                            ops.push(match plan.lowering {
                                BindLowering::Prop { fallback } => Op::BindProp { prop, fallback, target, span },
                                BindLowering::Readonly => Op::BindReadonly { prop, target, span },
                                BindLowering::Once { readonly } => Op::BindOnce { prop, readonly, target, span },
                                BindLowering::Signal { signal, arg } => Op::Bind { prop, signal, arg, target, span },
                            });
                        }
                        Err(u) => u.report(self.diags, sp(d.span())),
                    }
                }
                AttributeLike::ClassDirective(d) if runtime_attrs => {
                    if let Some(e) = &d.expression {
                        spread_at.get_or_insert(ops.len());
                        entries.push(SpreadEntry::Class { name: d.name.clone(), expr: e });
                    }
                }
                AttributeLike::ClassDirective(d) => {
                    if let (Some(cp), Some(e)) = (&class_prop, &d.expression) {
                        ops.push(Op::Token { prop: cp.clone(), token: d.name.clone(), cond: vec![CondAtom::Expr(e)], span: d.span() });
                    }
                }
                AttributeLike::SpreadAttribute(s) if runtime_attrs => {
                    spread_at.get_or_insert(ops.len());
                    spread_span.get_or_insert(s.span());
                    entries.push(SpreadEntry::Spread(&s.expression));
                }
                AttributeLike::SpreadAttribute(s) => {
                    // static keys (an object literal or a top-level const one): one op per key
                    self.static_spread(&er, &s.expression, s.span(), attr_target.as_ref(), &mut ops, &mut explicit_props);
                }
                other => {
                    let what = match other {
                        AttributeLike::AttachTag(_) => "{@attach}",
                        AttributeLike::UseDirective(_) => "use: actions",
                        AttributeLike::TransitionDirective(_) => "transitions",
                        AttributeLike::AnimateDirective(_) => "animate: directives",
                        AttributeLike::StyleDirective(_) => "style: directives on this backend",
                        AttributeLike::LetDirective(_) => "let: directives",
                        _ => "this attribute",
                    };
                    self.diags.error("unsupported_feature", sp(other.span()), format!("{what} are not supported by the native compiler yet"));
                }
            }
        }
        for p in bound_props.iter().filter(|p| explicit_props.contains(p)) {
            self.diags.error("bind_and_prop", span, format!("`{p}` is both bound and set explicitly"));
        }
        if runtime_attrs {
            self.kinds = true;
            let css = if native_style { self.css_scope.clone() } else { None };
            let at = spread_at.unwrap_or(ops.len()).min(ops.len());
            ops.insert(at, Op::Spread { entries, css, span: spread_span.unwrap_or(el.span()) });
        }

        // Bindings write their value after the element's other attributes (as Svelte's own bindings,
        // and the retired renderer, did): a bound value must not be written before the props that
        // constrain it (`<scale bind:value lower upper>`: GTK clamps to the adjustment's range).
        // Actions go with them, in source order ("actions need to run after attribute updates in
        // order with bindings/events", Svelte's UseDirective visitor).
        let (binds, rest): (Vec<_>, Vec<_>) =
            ops.into_iter().partition(|o| matches!(o, Op::Bind { .. } | Op::BindProp { .. } | Op::BindReadonly { .. } | Op::BindOnce { .. } | Op::Action { .. } | Op::Transition { .. } | Op::Animate { .. }));
        ops = rest;
        ops.extend(binds);

        // the X6 styling: classes (with the CSS scope), inline style (`$n.spread` applies them
        // when the attributes are applied at run time)
        if native_style && !runtime_attrs {
            if has_class || self.css_scope.is_some() {
                let mut base = subject.static_classes.clone();
                if let Some(sc) = &self.css_scope {
                    base.push(sc.clone());
                }
                let toggles = subject.cond_classes.clone();
                ops.push(Op::Classes { base: base.join(" "), dynamic: subject.dynamic, toggles, span: el.span() });
            }
            if !inline_css.is_empty() || inline_dynamic.is_some() || !inline_dirs.is_empty() {
                let span = style_span.unwrap_or(el.span());
                ops.push(Op::InlineStyle { base: inline_css, dynamic: inline_dynamic, directives: inline_dirs, span });
            }
        }

        // style-derived ops
        let explicit_attrs: Vec<String> = ops.iter().filter_map(|o| if let Op::ChildAttr { name, .. } = o { Some(name.clone()) } else { None }).collect();
        let dynamic_keys: Vec<String> =
            ops.iter().filter_map(|o| if let Op::StyleSetter { target, child_attr, .. } = o { Some(key(*child_attr, target)) } else { None }).collect();
        let mut chain = ancestors.to_vec();
        chain.push(subject);
        // A dynamic `style={...}` is applied at runtime: it must know which props are explicit
        // (they win over it) and write rule-set props through the rule guard (it wins over rules).
        let guard = ops.iter().any(|o| matches!(o, Op::InlineStyle { dynamic: Some(_), .. }));
        if guard {
            let mut props = explicit_props.clone();
            props.extend(bound_props.iter().cloned());
            ops.push(Op::Explicit { props, attrs: explicit_attrs.clone() });
        }
        let explicit = Explicit { props: &explicit_props, attrs: &explicit_attrs, bound: &bound_props, guard };
        self.style_ops(&er, parent_er.as_ref(), &chain, &explicit, inline_setters, &dynamic_keys, &mut ops);

        // children: text content or native children (an element's children are not in its
        // parent's <sv-slot>)
        let forced = self.forced_slot.take();
        // A kind with a text prop folds text children into it; only element children (no text)
        // stay native children; text mixed with elements is an error (text_content).
        let text_prop = self.sem.text_content(&er).cloned().filter(|_| !only_element_children(&el.fragment));
        let children = if let Some(tc) = text_prop {
            self.text_content(el, &tc.js, &mut ops);
            // attached children (a context menu, an event controller) do not mix with the text:
            // they are not the element's content
            let mut f = Fragment::default();
            for n in &el.fragment.nodes {
                if let FragmentNode::RegularElement(c) = n
                    && self.attaches(&c.name)
                    && let Some(e) = self.element(c, &Parent::Element(er.clone(), id), &chain)
                {
                    f.items.push(Item::Element(e));
                }
            }
            f
        } else {
            self.fragment(&el.fragment, &Parent::Element(er.clone(), id), &chain)
        };
        self.forced_slot = forced;

        Some(Element { id, span: el.span(), kind: er.tag.clone(), present, toplevel, show, attached, slot, class_expr, id_expr, ops, children })
    }

    /// Whether `tag` is a kind that attaches to its parent element (spec `attaches_to`).
    fn attaches(&self, tag: &str) -> bool {
        self.sem.resolve_element(tag).ok().is_some_and(|r| self.sem.attaches_to(&r).is_some())
    }

    fn push_static(&mut self, ops: &mut Vec<Op<'a>>, prop: &crate::semantics::PropertySpec, text: Option<&str>, at: &Attribute) {
        match self.sem.coerce_static(prop, text) {
            Ok(v) => ops.push(Op::Set { prop: prop.js.clone(), value: v.to_json(), span: at.span() }),
            Err(msg) => {
                self.diags.native_error("bad_attribute_value", sp(at.span()), msg);
            }
        }
    }

    /// Text children of a text-content element fold into one prop (Svelte's whitespace rules:
    /// runs collapse to one space; the fragment's leading/trailing whitespace is trimmed, as is
    /// each block branch's). `{#if}` blocks holding only text/expressions become conditional parts.
    fn text_content(&mut self, el: &'a AstElement, prop: &str, ops: &mut Vec<Op<'a>>) {
        self.text_prop = Some(prop.to_string());
        let parts = trim_parts(self.text_parts((&el.name, &el.fragment), &el.fragment));
        self.text_prop = None;
        let span = el.fragment.nodes.first().map(|n| n.span()).unwrap_or(el.span());
        match parts.as_slice() {
            [] => {}
            [TemplatePart::Text(t)] => ops.push(Op::Set { prop: prop.to_string(), value: Json::String(t.clone()), span }),
            _ => ops.push(Op::SetDynamic { prop: prop.to_string(), value: Dynamic::Template(parts), span }),
        }
    }

    fn text_parts(&mut self, el: (&str, &AstFragment), f: &'a AstFragment) -> Vec<TemplatePart<'a>> {
        let mut parts = Vec::new();
        for n in &f.nodes {
            match n {
                FragmentNode::Text(t) => parts.push(TemplatePart::Text(collapse_ws(&t.data))),
                FragmentNode::ExpressionTag(e) => parts.push(TemplatePart::Expr(&e.expression)),
                FragmentNode::Comment(_) => {}
                FragmentNode::IfBlock(b) => {
                    let consequent = trim_parts(self.text_parts(el, &b.consequent));
                    let alternate = b.alternate.as_ref().map(|a| trim_parts(self.text_parts(el, a))).unwrap_or_default();
                    parts.push(TemplatePart::If { test: &b.test, consequent, alternate });
                }
                // lowered as the element's attached children (element(), text-content branch)
                FragmentNode::RegularElement(c) if std::ptr::eq(f, el.1) && self.attaches(&c.name) => {}
                other => {
                    let prop = self.text_prop.clone().unwrap_or_default();
                    let element = matches!(
                        other,
                        FragmentNode::RegularElement(_) | FragmentNode::Component(_) | FragmentNode::SvelteComponent(_) | FragmentNode::SvelteElement(_) | FragmentNode::SvelteSelf(_) | FragmentNode::RenderTag(_)
                    );
                    let backend = self.sem.backend();
                    let msg = if element {
                        format!("<{}> mixes text and elements: on {backend} its text children set `{prop}`, so it cannot also hold child elements", el.0)
                    } else {
                        format!("<{}> holds text only on {backend} (its text children set `{prop}`); it cannot contain this", el.0)
                    };
                    self.diags
                        .native_error("text_content_children", sp(other.span()), msg)
                        .help(format!("give it either text (text, {{expressions}} and {{#if}} blocks of those, folded into `{prop}`) or child elements, not both; the text can also go in the `{prop}` attribute"));
                }
            }
        }
        parts
    }

    /// `style="..."` (static) on a native element: CSS declarations are returned as the inline CSS
    /// text (the X6 translator's at runtime), setter ones are lowered into `setters`.
    fn inline_style(&mut self, er: &ElementRef, parent: Option<&ElementRef>, text: &str, span: (u32, u32), setters: &mut Vec<(StyleLowering, Span)>) -> String {
        let mut css = Vec::new();
        for (name, value) in split_declarations(text) {
            if !self.sem.is_setter_property(&name) {
                css.push(format!("{name}: {value}"));
                continue;
            }
            match parse_value(&value) {
                Ok(tokens) => match self.sem.style_declaration(er, parent, &name, &tokens) {
                    Ok(Some(l)) => setters.push((l, Span::new(span.0, span.1))),
                    Ok(None) => {}
                    Err(u) => u.report(self.diags, span),
                },
                Err(msg) => {
                    self.diags.native_error(crate::semantics::codes::STYLE_BAD_VALUE, span, format!("`{name}`: {msg}"));
                }
            }
        }
        css.join("; ")
    }

    /// `style:name=...` on a native element: a setter property becomes a setter op (static values
    /// are checked now, dynamic ones parsed at runtime), a CSS one joins the inline style.
    fn style_directive(
        &mut self,
        er: &ElementRef,
        parent: Option<&ElementRef>,
        d: &'a crate::svelte::ast::StyleDirective,
        ops: &mut Vec<Op<'a>>,
        dirs: &mut Vec<(String, AttrValue<'a>)>,
        setters: &mut Vec<(StyleLowering, Span)>,
    ) {
        let span = sp(d.span());
        if !d.modifiers.is_empty() {
            self.diags.error("unsupported_feature", span, format!("`style:{}|{}`: style directive modifiers are not supported natively", d.name, d.modifiers.join("|")));
            return;
        }
        let value: Option<AttrValue<'a>> = match &d.value {
            AttributeValue::True(_) => None,
            AttributeValue::Expression(ExpressionTagNode::ExpressionTag(t)) => Some(AttrValue::Dynamic(Dynamic::Expr(&t.expression))),
            AttributeValue::Sequence(parts) if parts.iter().all(|p| matches!(p, TextOrExpressionTag::Text(_))) => {
                Some(AttrValue::Static(Json::String(parts.iter().map(|p| if let TextOrExpressionTag::Text(t) = p { t.data.as_str() } else { "" }).collect())))
            }
            AttributeValue::Sequence(parts) => Some(AttrValue::Dynamic(Dynamic::Template(
                parts
                    .iter()
                    .map(|p| match p {
                        TextOrExpressionTag::Text(t) => TemplatePart::Text(t.data.clone()),
                        TextOrExpressionTag::ExpressionTag(e) => TemplatePart::Expr(&e.expression),
                    })
                    .collect(),
            ))),
        };
        let Some(value) = value else {
            self.diags.error("unsupported_feature", span, format!("the `style:{0}` shorthand is not supported natively; write `style:{0}={{{0}}}`", d.name));
            return;
        };
        let Some(p) = self.sem.style_property(&d.name).filter(|p| p.via != crate::semantics::Via::Css) else {
            dirs.push((d.name.clone(), value));
            return;
        };
        match value {
            AttrValue::Static(Json::String(text)) => match parse_value(&text) {
                Ok(tokens) => match self.sem.style_declaration(er, parent, &d.name, &tokens) {
                    Ok(Some(l)) => setters.push((l, Span::new(span.0, span.1))),
                    Ok(None) => {}
                    Err(u) => u.report(self.diags, span),
                },
                Err(msg) => {
                    self.diags.native_error(crate::semantics::codes::STYLE_BAD_VALUE, span, format!("`{}`: {msg}", d.name));
                }
            },
            AttrValue::Static(_) => unreachable!("static style directive values are text"),
            AttrValue::Dynamic(dv) => {
                let child_attr = p.via == crate::semantics::Via::ChildAttr;
                let (ok, owner) = if child_attr { (parent.is_some_and(|x| p.tags.contains(&x.tag)), parent.map_or("no native parent".to_string(), |x| format!("<{}>", x.tag))) } else { (p.tags.contains(&er.tag), format!("<{}>", er.tag)) };
                if !ok {
                    let tags = p.tags.iter().map(|t| format!("<{t}>")).collect::<Vec<_>>().join(", ");
                    self.diags.native_error(
                        crate::semantics::codes::STYLE_NOT_APPLICABLE,
                        span,
                        format!("`{}` applies to {} on {}, not {}", d.name, if child_attr { format!("the children of {tags}") } else { tags }, self.sem.backend(), owner),
                    );
                    return;
                }
                let target = if child_attr {
                    p.attr.clone().unwrap_or_default()
                } else {
                    match self.sem.writable_property(er, p.prop.as_deref().unwrap_or_default()) {
                        Ok(pr) => pr.js.clone(),
                        Err(u) => {
                            u.report(self.diags, span);
                            return;
                        }
                    }
                };
                ops.push(Op::StyleSetter {
                    name: d.name.clone(),
                    target,
                    child_attr,
                    backend: self.sem.backend().to_string(),
                    value: dv,
                    fallback: (Vec::new(), Fallback::Default(Json::Null)),
                    span: Span::new(span.0, span.1),
                });
            }
        }
    }

    /// Resolves the setter properties for one element (docs/notes/native-compiler/styling.md):
    /// explicit attributes > `style:` / `style=` > rules by (specificity, source order) > native default.
    fn style_ops(
        &mut self,
        er: &ElementRef,
        parent: Option<&ElementRef>,
        chain: &[StyleSubject<'a>],
        explicit: &Explicit,
        inline: Vec<(StyleLowering, Span)>,
        dynamic_keys: &[String],
        ops: &mut Vec<Op<'a>>,
    ) {
        // `style:` with a dynamic value and static `style=` setters: above every rule
        let mut taken: Vec<String> = Vec::new();
        for k in dynamic_keys {
            let (attr, name) = (k.starts_with("a:"), &k[2..]);
            if (!attr && explicit.props.iter().any(|p| p == name)) || (attr && explicit.attrs.iter().any(|p| p == name)) {
                let span = ops
                    .iter()
                    .find_map(|o| match o {
                        Op::StyleSetter { target, child_attr, span, .. } if *child_attr == attr && target == name => Some(sp(*span)),
                        _ => None,
                    })
                    .unwrap_or_default();
                self.diags.native_warning("style_overridden", span, format!("`{name}` is set explicitly on <{}>, which takes precedence over its style: directive", er.tag));
                ops.retain(|o| !matches!(o, Op::StyleSetter { target, child_attr, .. } if *child_attr == attr && target == name));
            }
            taken.push(k.clone());
        }
        let dynamic: Vec<String> = taken.clone();
        // a nullish `style:<setter>` falls back to: the static inline value, else the rules, else
        // the native default (or the captured value)
        let mut setter_fallbacks: Vec<(String, Vec<(Cond<'a>, Json)>)> = Vec::new();
        for (l, span) in inline.into_iter().rev() {
            let (is_attr, name) = match &l {
                StyleLowering::SetProp { prop, .. } => (false, prop.clone()),
                StyleLowering::ChildAttr { attr, .. } => (true, attr.clone()),
            };
            let k = key(is_attr, &name);
            if dynamic.contains(&k) && !setter_fallbacks.iter().any(|(x, _)| *x == k) {
                let v = match &l {
                    StyleLowering::SetProp { value, .. } | StyleLowering::ChildAttr { value, .. } => value.to_json(),
                };
                setter_fallbacks.push((k.clone(), vec![(Vec::new(), v)]));
            }
            if taken.contains(&k) {
                continue;
            }
            taken.push(k);
            if (!is_attr && explicit.props.contains(&name)) || (is_attr && explicit.attrs.contains(&name)) {
                self.diags.native_warning("style_overridden", sp(span), format!("`{name}` is set explicitly on <{}>, which takes precedence over its inline style", er.tag));
                continue;
            }
            if !is_attr && explicit.bound.contains(&name) {
                self.diags.native_error("style_conflicts_binding", sp(span), format!("style sets `{name}`, which is bound with bind: on this element"));
                continue;
            }
            match l {
                StyleLowering::SetProp { prop, value } => ops.push(Op::Set { prop, value: value.to_json(), span }),
                StyleLowering::ChildAttr { attr, value } => ops.push(Op::ChildAttr { name: attr, value: AttrValue::Static(value.to_json()), span }),
            }
        }
        let Some(sheet) = self.styles else {
            self.setter_fallbacks(er, setter_fallbacks, &dynamic, ops);
            return;
        };
        struct Cand<'a> {
            spec: (u32, u32, u32),
            order: usize,
            cond: Cond<'a>,
            lowering: StyleLowering,
            span: Span,
        }
        let mut cands: Vec<Cand<'a>> = Vec::new();
        let mut order = 0;
        for rule in &sheet.rules {
            for sel in &rule.selectors {
                order += 1;
                if self.bad_selectors.contains(&sel.span) {
                    continue;
                }
                let Some(cond) = match_selector(sel, chain) else { continue };
                let mut any = false;
                for d in rule.declarations.iter().filter(|d| self.sem.is_setter_property(&d.name)) {
                    any = true;
                    let dspan = (d.span.0, d.span.1);
                    match self.sem.style_declaration(er, parent, &d.name, &d.value) {
                        Ok(Some(lowering)) => cands.push(Cand { spec: sel.specificity(), order: order * 1000 + cands.len(), cond: cond.clone(), lowering, span: Span::new(d.span.0, d.span.1) }),
                        Ok(None) => {}
                        // A rule that reaches this element only through its own dynamic class set
                        // may never apply to it: a kind mismatch is not an error then.
                        Err(u)
                            if u.code == crate::semantics::codes::STYLE_NOT_APPLICABLE
                                && cond.iter().any(|a| match a {
                                    CondAtom::Class { element, .. } => Some(*element) == chain.last().and_then(|s| s.dynamic),
                                    CondAtom::Id { element, .. } => Some(*element) == chain.last().and_then(|s| s.dynamic_id),
                                    CondAtom::Expr(_) => false,
                                }) => {}
                        Err(u) => {
                            if self.reported.insert((dspan.0, dspan.1, u.code)) {
                                let mut msg = u.message.clone();
                                if !u.suggestions.is_empty() {
                                    msg.push_str(&format!(" (did you mean {}?)", u.suggestions.join(", ")));
                                }
                                let d = self.diags.native_error(u.code, dspan, format!("{msg} (in a rule matching <{}>)", er.tag));
                                if let Some(h) = u.help_text() {
                                    d.help(h);
                                }
                            }
                        }
                    }
                }
                if any {
                    self.matched.insert(sel.span);
                }
            }
        }
        if cands.is_empty() {
            self.setter_fallbacks(er, setter_fallbacks, &dynamic, ops);
            return;
        }
        // highest precedence first
        cands.sort_by(|a, b| (b.spec, b.order).cmp(&(a.spec, a.order)));
        let key_of = |l: &StyleLowering| match l {
            StyleLowering::SetProp { prop, .. } => key(false, prop),
            StyleLowering::ChildAttr { attr, .. } => key(true, attr),
        };
        let mut keys: Vec<String> = Vec::new();
        for c in &cands {
            let k = key_of(&c.lowering);
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
        for k in keys {
            let mine: Vec<&Cand<'a>> = cands.iter().filter(|c| key_of(&c.lowering) == k).collect();
            let span = mine[0].span;
            let (is_attr, name) = (k.starts_with("a:"), k[2..].to_string());
            if dynamic.contains(&k) && !setter_fallbacks.iter().any(|(x, _)| *x == k) {
                let mut cs: Vec<(Cond<'a>, Json)> = Vec::new();
                for c in &mine {
                    let v = match &c.lowering {
                        StyleLowering::SetProp { value, .. } | StyleLowering::ChildAttr { value, .. } => value.to_json(),
                    };
                    cs.push((c.cond.clone(), v));
                    if c.cond.is_empty() {
                        break;
                    }
                }
                setter_fallbacks.push((k.clone(), cs));
            }
            if taken.contains(&k) {
                continue;
            }
            if !is_attr && explicit.bound.contains(&name) {
                self.diags.native_error("style_conflicts_binding", sp(span), format!("style sets `{name}`, which is bound with bind: on this element"));
                continue;
            }
            if (!is_attr && explicit.props.contains(&name)) || (is_attr && explicit.attrs.contains(&name)) {
                self.diags.native_warning("style_overridden", sp(span), format!("`{name}` is set explicitly on <{}>, which takes precedence over this style declaration", er.tag));
                continue;
            }
            let mut candidates: Vec<(Cond<'a>, Json)> = Vec::new();
            for c in &mine {
                let v = match &c.lowering {
                    StyleLowering::SetProp { value, .. } | StyleLowering::ChildAttr { value, .. } => value.to_json(),
                };
                candidates.push((c.cond.clone(), v));
                if c.cond.is_empty() {
                    break;
                }
            }
            if candidates.len() == 1 && candidates[0].0.is_empty() && !explicit.guard {
                let (_, v) = candidates.pop().unwrap();
                if is_attr {
                    ops.push(Op::ChildAttr { name, value: AttrValue::Static(v), span });
                } else {
                    ops.push(Op::Set { prop: name, value: v, span });
                }
                continue;
            }
            let fallback = if candidates.last().is_some_and(|c| c.0.is_empty()) || is_attr {
                // unreachable at runtime when the last candidate always holds; a child attribute
                // falls back to null (the parent's default)
                Fallback::Default(Json::Null)
            } else {
                match self.sem.property(er, &name).ok().and_then(|p| p.default.clone()) {
                    Some(d) => Fallback::Default(d),
                    None => Fallback::Restore,
                }
            };
            ops.push(Op::StyleProp { prop: name, child_attr: is_attr, candidates, fallback, guarded: explicit.guard, span });
        }
        self.setter_fallbacks(er, setter_fallbacks, &dynamic, ops);
    }

    /// Fills in the fallback of each `style:<setter>` op (`dynamic`: their keys): `found` holds the
    /// static inline value or the rule candidates of a key; then the native default, or the value
    /// captured before the first write when the spec has none (a child attribute: null).
    fn setter_fallbacks(&self, er: &ElementRef, found: Vec<(String, Vec<(Cond<'a>, Json)>)>, dynamic: &[String], ops: &mut [Op<'a>]) {
        for k in dynamic {
            let (is_attr, name) = (k.starts_with("a:"), &k[2..]);
            let candidates = found.iter().find(|(x, _)| x == k).map(|(_, c)| c.clone()).unwrap_or_default();
            let fallback = if candidates.last().is_some_and(|c| c.0.is_empty()) || is_attr {
                Fallback::Default(Json::Null)
            } else {
                match self.sem.property(er, name).ok().and_then(|p| p.default.clone()) {
                    Some(d) => Fallback::Default(d),
                    None => Fallback::Restore,
                }
            };
            for o in ops.iter_mut() {
                if let Op::StyleSetter { target, child_attr, fallback: f, .. } = o
                    && *child_attr == is_attr
                    && target == name
                {
                    *f = (candidates.clone(), fallback.clone());
                }
            }
        }
    }

    /// A component call. Children: `{#snippet}` blocks directly inside become props of the same
    /// name; everything else becomes the implicit `children` snippet (Svelte 5 semantics).
    fn component(&mut self, el: &'a AstElement, ancestors: &[StyleSubject<'a>]) -> Option<ComponentCall<'a>> {
        let parts = TagParts { name: &el.name, name_span: el.name_span, span: el.span(), attributes: &el.attributes, fragment: &el.fragment };
        self.component_parts(parts, Some(el), None, ancestors)
    }

    /// `<svelte:component this={expr}>`: a dynamic component call (Svelte's `$.component`).
    fn svelte_component(&mut self, n: &'a crate::svelte::ast::SvelteComponent, ancestors: &[StyleSubject<'a>]) -> Option<ComponentCall<'a>> {
        let parts = TagParts { name: &n.name, name_span: n.name_span, span: n.span(), attributes: &n.attributes, fragment: &n.fragment };
        self.component_parts(parts, None, Some(&n.expression), ancestors)
    }

    fn component_parts(&mut self, el: TagParts<'a>, node: Option<&'a AstElement>, this: Option<&'a Expression>, ancestors: &[StyleSubject<'a>]) -> Option<ComponentCall<'a>> {
        if node.is_some() && self.list_views.contains(el.name) {
            self.check_list_view(&el);
        }
        let mut props = Vec::new();
        let mut binds = Vec::new();
        let mut slot = None;
        self.check_duplicate_events(el.attributes, OnSite::Component(el.name));
        for a in el.attributes {
            match a {
                AttributeLike::Attribute(at) if at.name == "slot" => slot = static_text(at),
                AttributeLike::Attribute(at) => match &at.value {
                    AttributeValue::True(_) => props.push(ComponentProp::Static { name: at.name.clone(), value: Json::Bool(true) }),
                    AttributeValue::Sequence(_) if static_text(at).is_some() => {
                        props.push(ComponentProp::Static { name: at.name.clone(), value: Json::String(static_text(at).unwrap()) })
                    }
                    _ => props.push(ComponentProp::Dynamic { name: at.name.clone(), value: dynamic_of(at).expect("not a bare attribute") }),
                },
                AttributeLike::BindDirective(d) => {
                    let Some(target) = &d.expression else { unreachable!() };
                    self.no_bind_modifiers(d, "a component binding");
                    // Svelte pushes bindings after the other props, so a spread cannot override them
                    binds.push(ComponentProp::Bind { name: d.name.clone(), target });
                }
                AttributeLike::SpreadAttribute(s) => props.push(ComponentProp::Spread(&s.expression)),
                AttributeLike::AttachTag(t) => props.push(ComponentProp::Attach(&t.expression)),
                // FW1: `on:name={h}` is the prop `onname={h}`; a bare `on:name` forwards this
                // component's own callback prop to the child (ARCH-EMBED §8.7)
                AttributeLike::OnDirective(d) if !d.modifiers.is_empty() => self.on_modifiers(d, OnSite::Component(el.name)),
                AttributeLike::OnDirective(d) => match &d.expression {
                    Some(e) => props.push(ComponentProp::Dynamic { name: format!("on{}", d.name), value: Dynamic::Expr(e) }),
                    None => {
                        self.note_forwarding(d, OnSite::Component(el.name));
                        props.push(ComponentProp::Forward { name: format!("on{}", d.name) });
                    }
                },
                other => {
                    self.diags.error("unsupported_feature", sp(other.span()), "this attribute kind is not supported on components by the native compiler yet");
                }
            }
        }
        props.extend(binds);
        if let Some(f) = &self.forced_slot {
            match &slot {
                Some(s) if s != f => {
                    self.diags.error("slot_conflict", sp(el.name_span), format!("<{}> asks for slot `{s}` inside <sv-slot name=\"{f}\">", el.name));
                }
                _ => slot = Some(f.clone()),
            }
        }
        let forced = self.forced_slot.take();
        let outer = std::mem::replace(&mut self.in_block, true);
        self.const_ok = true;
        let mut frag = self.fragment(el.fragment, &Parent::Detached, ancestors);
        self.in_block = outer;
        self.forced_slot = forced;
        let snippets = std::mem::take(&mut frag.snippets);
        let consts = std::mem::take(&mut frag.consts);
        if snippets.iter().any(|s| s.name == "children") && !frag.items.is_empty() {
            self.diags.error(
                "snippet_conflict",
                (el.span.start, el.span.end),
                "Cannot use explicit children snippet at the same time as implicit children content. Remove either the non-whitespace content or the children snippet block",
            );
        }
        let children = (!frag.items.is_empty()).then_some(frag);
        Some(ComponentCall { span: el.span, node, this, slot, name: el.name.to_string(), props, consts, snippets, children })
    }
}

/// The parts of a component-like tag (`<Comp>`, `<svelte:component>`, `<svelte:boundary>`).
struct TagParts<'a> {
    name: &'a str,
    name_span: Span,
    span: Span,
    attributes: &'a [AttributeLike],
    fragment: &'a AstFragment,
}

/// Trims a branch's leading and trailing whitespace and drops empty text parts.
/// Whether a fragment has child nodes and no text at all (whitespace and comments aside; block
/// bodies count, element children do not): such children of a text-prop kind stay native children.
fn only_element_children(f: &AstFragment) -> bool {
    f.nodes.iter().any(|n| !matches!(n, FragmentNode::Comment(_)) && !matches!(n, FragmentNode::Text(t) if t.data.trim().is_empty())) && !has_text(f)
}

/// Text or `{expressions}` in a fragment or the bodies of its blocks (not inside elements).
fn has_text(f: &AstFragment) -> bool {
    f.nodes.iter().any(|n| match n {
        FragmentNode::Text(t) => !t.data.trim().is_empty(),
        FragmentNode::ExpressionTag(_) | FragmentNode::HtmlTag(_) => true,
        FragmentNode::IfBlock(b) => has_text(&b.consequent) || b.alternate.as_ref().is_some_and(has_text),
        FragmentNode::EachBlock(b) => has_text(&b.body) || b.fallback.as_ref().is_some_and(has_text),
        FragmentNode::KeyBlock(b) => has_text(&b.fragment),
        FragmentNode::AwaitBlock(b) => [&b.pending, &b.then, &b.catch].into_iter().flatten().any(has_text),
        _ => false,
    })
}

fn trim_parts(mut parts: Vec<TemplatePart<'_>>) -> Vec<TemplatePart<'_>> {
    if let Some(TemplatePart::Text(t)) = parts.first_mut() {
        *t = t.trim_start().to_string();
    }
    if let Some(TemplatePart::Text(t)) = parts.last_mut() {
        *t = t.trim_end().to_string();
    }
    parts.retain(|p| !matches!(p, TemplatePart::Text(t) if t.is_empty()));
    parts
}

/// Matches a complex selector right-to-left against `chain` (ancestors first, the element last).
/// Returns the class conditions under which it matches, or None.
fn match_selector<'a>(sel: &ComplexSelector, chain: &[StyleSubject<'a>]) -> Option<Cond<'a>> {
    let mut cond = Vec::new();
    let parts = &sel.parts;
    let (_, subject) = parts.last()?;
    let mut pos = chain.len() - 1;
    match_compound(subject, &chain[pos], &mut cond)?;
    // walk the remaining compounds leftwards; the combinator stored with a part links it to the previous one
    let mut i = parts.len() - 1;
    while i > 0 {
        let comb = parts[i].0;
        let target = &parts[i - 1].1;
        match comb {
            Combinator::Child => {
                if pos == 0 {
                    return None;
                }
                pos -= 1;
                match_compound(target, &chain[pos], &mut cond)?;
            }
            _ => {
                // descendant: nearest ancestor that matches (unconditional matches preferred)
                let mut found = None;
                for p in (0..pos).rev() {
                    let mut c = Vec::new();
                    if match_compound(target, &chain[p], &mut c).is_some() {
                        if c.is_empty() {
                            found = Some((p, c));
                            break;
                        }
                        found.get_or_insert((p, c));
                    }
                }
                let (p, c) = found?;
                pos = p;
                cond.extend(c);
            }
        }
        i -= 1;
    }
    Some(cond)
}

fn match_compound<'a>(c: &Compound, s: &StyleSubject<'a>, cond: &mut Cond<'a>) -> Option<()> {
    if !c.states.is_empty() {
        return None;
    }
    if let Some(id) = &c.id
        && s.id.as_ref() != Some(id)
    {
        cond.push(CondAtom::Id { element: s.dynamic_id?, id: id.clone() });
    }
    if let Some(t) = &c.element
        && *t != s.tag
    {
        return None;
    }
    for class in &c.classes {
        if s.static_classes.contains(class) {
            continue;
        }
        // a class: directive decides its class (it overrides the class attribute, as in Svelte)
        match s.cond_classes.iter().find(|(n, _)| n == class) {
            Some((_, e)) => cond.push(CondAtom::Expr(e)),
            None => cond.push(CondAtom::Class { element: s.dynamic?, class: class.clone() }),
        }
    }
    Some(())
}

/// What explicit attributes an element has (they win over style).
struct Explicit<'x> {
    props: &'x [String],
    attrs: &'x [String],
    bound: &'x [String],
    /// The element has a dynamic `style={...}`: rule-set props go through `$n.rule_set`.
    guard: bool,
}

/// Key of a style target: `p:<prop>` or `a:<child attribute>`.
fn key(child_attr: bool, name: &str) -> String {
    format!("{}:{name}", if child_attr { "a" } else { "p" })
}

fn static_text(at: &Attribute) -> Option<String> {
    match &at.value {
        AttributeValue::Sequence(parts) => {
            let mut s = String::new();
            for p in parts {
                match p {
                    TextOrExpressionTag::Text(t) => s.push_str(&t.data),
                    TextOrExpressionTag::ExpressionTag(_) => return None,
                }
            }
            Some(s)
        }
        _ => None,
    }
}

fn name_span(at: &Attribute) -> (u32, u32) {
    at.name_span.map_or((at.start, at.end), sp)
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut ws = false;
    for c in s.chars() {
        if matches!(c, ' ' | '\t' | '\r' | '\n') {
            if !ws {
                out.push(' ');
            }
            ws = true;
        } else {
            out.push(c);
            ws = false;
        }
    }
    out
}

fn unsupported_node_name(n: &FragmentNode) -> &'static str {
    use FragmentNode::*;
    match n {
        HtmlTag(_) => "{@html}",
        ConstTag(_) => "{@const}",
        DeclarationTag(_) => "declaration tags",
        DebugTag(_) => "{@debug}",
        RenderTag(_) => "{@render}",
        AttachTag(_) => "{@attach}",
        AwaitBlock(_) => "{#await}",
        KeyBlock(_) => "{#key}",
        SnippetBlock(_) => "{#snippet}",
        TitleElement(_) | SvelteHead(_) | SvelteBody(_) | SvelteDocument(_) | SvelteWindow(_) => "this DOM-only element",
        SlotElement(_) => "<slot> (Svelte 4 slots)",
        SvelteComponent(_) | SvelteElement(_) | SvelteSelf(_) | SvelteFragment(_) | SvelteBoundary(_) => "this svelte: element",
        SvelteOptionsRaw(_) => "<svelte:options> here",
        _ => "this node",
    }
}

/// A fragment entry when the backend has text nodes: a node, or a run of text/expression tags.
enum Entry<'a> {
    Node(&'a FragmentNode),
    Text(Vec<TemplatePart<'a>>, Span),
}

fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}

/// Svelte's `clean_nodes` whitespace rules (phases/3-transform/utils.js, `preserveWhitespace`
/// off), then consecutive text/expression tags grouped into runs (one text node each).
fn text_runs<'a>(nodes: &'a [FragmentNode], parent_tag: Option<&str>) -> Vec<Entry<'a>> {
    enum Tx<'a> {
        Text(String, Span),
        Expr(&'a Expression, Span),
        Node(&'a FragmentNode),
    }
    // Svelte's clean_nodes hoists these out of the fragment before trimming whitespace, so the
    // text around them is adjacent (`A\n{#snippet s()}..{/snippet}\nB` is `A B`)
    let hoisted = |n: &FragmentNode| {
        matches!(
            n,
            FragmentNode::ConstTag(_)
                | FragmentNode::DeclarationTag(_)
                | FragmentNode::DebugTag(_)
                | FragmentNode::SvelteBody(_)
                | FragmentNode::SvelteWindow(_)
                | FragmentNode::SvelteDocument(_)
                | FragmentNode::SvelteHead(_)
                | FragmentNode::TitleElement(_)
                | FragmentNode::SnippetBlock(_)
        )
    };
    let mut out: Vec<Entry<'a>> = nodes.iter().filter(|n| hoisted(n)).map(Entry::Node).collect();
    let mut regular: Vec<Tx<'a>> = nodes
        .iter()
        .filter(|n| !matches!(n, FragmentNode::Comment(_)) && !hoisted(n))
        .map(|n| match n {
            FragmentNode::Text(t) => Tx::Text(t.data.clone(), Span::new(t.start, t.end)),
            FragmentNode::ExpressionTag(e) => Tx::Expr(&e.expression, Span::new(e.start, e.end)),
            other => Tx::Node(other),
        })
        .collect();
    while matches!(regular.first(), Some(Tx::Text(t, _)) if t.chars().all(is_ws)) {
        regular.remove(0);
    }
    if let Some(Tx::Text(t, _)) = regular.first_mut() {
        *t = t.trim_start_matches(is_ws).to_string();
    }
    while matches!(regular.last(), Some(Tx::Text(t, _)) if t.chars().all(is_ws)) {
        regular.pop();
    }
    if let Some(Tx::Text(t, _)) = regular.last_mut() {
        *t = t.trim_end_matches(is_ws).to_string();
    }
    let can_remove_entirely = matches!(parent_tag, Some("select" | "tr" | "table" | "tbody" | "thead" | "tfoot" | "colgroup" | "datalist"));
    let mut cleaned: Vec<Tx<'a>> = Vec::new();
    for i in 0..regular.len() {
        let prev_is_expr = i > 0 && matches!(regular[i - 1], Tx::Expr(..));
        let prev_text_ends_ws = i > 0 && matches!(&regular[i - 1], Tx::Text(t, _) if t.ends_with(is_ws));
        let next_is_expr = matches!(regular.get(i + 1), Some(Tx::Expr(..)));
        if let Tx::Text(t, _) = &mut regular[i] {
            if !prev_is_expr && t.starts_with(is_ws) {
                *t = format!("{}{}", if prev_text_ends_ws { "" } else { " " }, t.trim_start_matches(is_ws));
            }
            if !next_is_expr && t.ends_with(is_ws) {
                *t = format!("{} ", t.trim_end_matches(is_ws));
            }
        }
        match &regular[i] {
            Tx::Text(t, s) => {
                if !t.is_empty() && (t != " " || !can_remove_entirely) {
                    cleaned.push(Tx::Text(t.clone(), *s));
                }
            }
            Tx::Expr(e, s) => cleaned.push(Tx::Expr(e, *s)),
            Tx::Node(n) => cleaned.push(Tx::Node(n)),
        }
    }
    for tx in cleaned {
        let (part, s) = match tx {
            Tx::Node(n) => {
                out.push(Entry::Node(n));
                continue;
            }
            Tx::Text(t, s) => (TemplatePart::Text(t), s),
            Tx::Expr(e, s) => (TemplatePart::Expr(e), s),
        };
        if let Some(Entry::Text(parts, span)) = out.last_mut() {
            parts.push(part);
            span.end = s.end;
        } else {
            out.push(Entry::Text(vec![part], s));
        }
    }
    out
}
