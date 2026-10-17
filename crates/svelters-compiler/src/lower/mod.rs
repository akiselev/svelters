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

use crate::diag::Diagnostics;
use crate::estree::{ArrayElement, Expression, Pattern, Span, Spanned};
use crate::semantics::{BackendSemantics, ElementRef, Lifetime, StyleLowering, Unresolved};
use crate::style::{Combinator, ComplexSelector, Compound, StyleSheet};
use crate::svelte::ast::{
    Attribute, AttributeLike, AttributeValue, Element as AstElement, ExpressionTagNode, Fragment as AstFragment, FragmentNode, RenderTag,
    Root, SnippetBlock, TextOrExpressionTag,
};
use ir::*;
use serde_json::Value as Json;
use std::collections::{HashMap, HashSet};

/// The slot proxy: `<sv-slot name="x">` puts its content into the parent element's slot `x`.
pub const SLOT_PROXY: &str = "sv-slot";

pub fn lower<'a>(root: &'a Root, sem: &dyn BackendSemantics, styles: Option<&'a StyleSheet>, diags: &mut Diagnostics) -> Component<'a> {
    let mut snippet_roots = HashMap::new();
    collect_snippets(&root.fragment, &mut snippet_roots);
    let mut l = Lower {
        sem,
        diags,
        next_id: 0,
        styles,
        reported: HashSet::new(),
        single_slots: Default::default(),
        in_block: false,
        matched: HashSet::new(),
        forced_slot: None,
        const_ok: false,
        snippet_roots,
    };
    l.check_style_sheet();
    let root_fragment = l.fragment(&root.fragment, &Parent::Root, &[]);
    if let Some(sheet) = styles {
        for rule in &sheet.rules {
            for sel in &rule.selectors {
                if !l.matched.contains(&sel.span) && sel.parts.iter().all(|(_, c)| c.states.is_empty()) {
                    l.diags.warning("style_unused_selector", sel.span, "this selector matches no element of the component (its declarations are not checked)");
                }
            }
        }
    }
    Component { root: root_fragment, elements: l.next_id }
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
    static_classes: Vec<String>,
    cond_classes: Vec<(String, &'a Expression)>,
    /// The element id when it has a dynamic `class` (its class set is matched at runtime).
    dynamic: Option<usize>,
}

struct Lower<'a, 's, 'd> {
    sem: &'s dyn BackendSemantics,
    diags: &'d mut Diagnostics,
    next_id: usize,
    styles: Option<&'a StyleSheet>,
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
    /// The fragment being lowered may hold `{@const}` (set by the caller, consumed by `fragment`).
    const_ok: bool,
    /// Snippets declared in this component by name: their root elements (None: the name is
    /// declared more than once, so render sites are not checked statically).
    snippet_roots: HashMap<String, Option<Vec<&'a AstElement>>>,
}

fn sp(s: Span) -> (u32, u32) {
    (s.start, s.end)
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
    fn check_style_sheet(&mut self) {
        let Some(sheet) = self.styles else { return };
        for rule in &sheet.rules {
            for sel in &rule.selectors {
                for (_, c) in &sel.parts {
                    if !c.states.is_empty() {
                        self.diags.error(
                            "style_state_unsupported",
                            c.span,
                            format!("state selectors (:{}) need native state tracking, which this compiler does not implement yet", c.states.join(", :")),
                        );
                    }
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
                    out.items.push(Item::If(IfBlock { span: b.span(), slot, test: &b.test, consequent, alternate }));
                }
                FragmentNode::EachBlock(b) => {
                    let context = b.context.as_ref();
                    if let Some(p) = context
                        && !matches!(p, Pattern::Identifier(_))
                    {
                        self.diags.error(
                            "unsupported_feature",
                            sp(p.span()),
                            "destructuring in {#each} is not supported by the native compiler yet; use a plain identifier",
                        );
                    }
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
                    let value = b.value.as_ref().and_then(|p| self.simple_binding(p, "{:then}"));
                    let error = b.error.as_ref().and_then(|p| self.simple_binding(p, "{:catch}"));
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
                        (Pattern::Identifier(id), Some(init)) => out.consts.push(ConstDecl { span: c.span(), name: id.name.clone(), init }),
                        _ => {
                            self.diags.error(
                                "unsupported_feature",
                                sp(d.id.span()),
                                "destructuring in {@const} is not supported by the native compiler yet; declare one {@const} per value",
                            );
                        }
                    }
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
        Some(Element { id, span, kind: er.tag.clone(), present: None, slot, class_expr: None, ops: vec![op], children: Fragment::default() })
    }

    /// `{:then value}` / `{:catch error}`: only plain identifiers.
    fn simple_binding(&mut self, p: &Pattern, what: &str) -> Option<String> {
        match p {
            Pattern::Identifier(id) => Some(id.name.clone()),
            _ => {
                self.diags.error("unsupported_feature", sp(p.span()), format!("destructuring in {what} is not supported by the native compiler yet; use a plain identifier"));
                None
            }
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
        let mut params = Vec::new();
        for p in &b.parameters {
            match p {
                Pattern::Identifier(id) => params.push(id.name.clone()),
                other => {
                    self.diags.error(
                        "unsupported_feature",
                        sp(other.span()),
                        "destructured or defaulted snippet parameters are not supported by the native compiler yet; use plain identifiers",
                    );
                }
            }
        }
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
                        Ok(plan) => slots.push((!plan.is_default).then_some(plan.slot)),
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

        // pass 1: slot request and classes (they don't produce ops)
        let mut requested_slot: Option<String> = None;
        let mut class_expr = None;
        let mut subject = StyleSubject { tag: el.name.clone(), static_classes: Vec::new(), cond_classes: Vec::new(), dynamic: None };
        for a in &el.attributes {
            match a {
                AttributeLike::Attribute(at) if at.name == "slot" || at.name == "data-slot" => match static_text(at) {
                    Some(s) => requested_slot = Some(s),
                    None => {
                        self.diags.error("slot_not_static", sp(at.span()), "`slot` must be a static name");
                    }
                },
                AttributeLike::Attribute(at) if at.name == "class" => match static_text(at) {
                    Some(s) => subject.static_classes.extend(s.split_ascii_whitespace().map(str::to_string)),
                    None if class_prop.is_some() => {}
                    None => {
                        // matched at runtime against the rules' class conditions (runtime-contract.md)
                        class_expr = dynamic_of(at);
                        subject.dynamic = class_expr.is_some().then_some(id);
                    }
                },
                AttributeLike::ClassDirective(d) => match &d.expression {
                    Some(e) => subject.cond_classes.push((d.name.clone(), e)),
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

        // placement
        let (slot, present) = match parent {
            Parent::Root | Parent::Detached => {
                // A toplevel kind is presented when mounted at the root. Other kinds are allowed at a
                // component's top level: a child component's root elements go to the container the
                // parent passes in (placement is then checked at runtime; the compiler cannot see
                // across component boundaries). Mounting a non-toplevel at the app root throws.
                // Detached content (snippets, component children) is placed by its render site.
                let lt = self.sem.lifetime(&er);
                let slot = if matches!(parent, Parent::Detached) { requested_slot.clone() } else { None };
                (slot, if lt.lifetime == Lifetime::Toplevel { lt.present } else { None })
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
                    ((!plan.is_default).then_some(plan.slot), None)
                }
                Err(u) => {
                    u.report(self.diags, tag_span);
                    (None, None)
                }
            },
        };

        // pass 2: ops in source order
        let mut ops: Vec<Op<'a>> = Vec::new();
        let mut explicit_props: Vec<String> = Vec::new();
        let mut bound_props: Vec<String> = Vec::new();
        for a in &el.attributes {
            match a {
                AttributeLike::Attribute(at) if matches!(at.name.as_str(), "slot" | "data-slot") || (at.name == "class" && class_prop.is_none()) => {}
                AttributeLike::Attribute(at) if at.name.starts_with("on") && matches!(at.value, AttributeValue::Expression(_)) => {
                    let AttributeValue::Expression(ExpressionTagNode::ExpressionTag(t)) = &at.value else { unreachable!() };
                    match self.sem.event(&er, &at.name) {
                        Ok(ev) => ops.push(Op::Listen { signal: ev.signal, handler: &t.expression, span: at.span() }),
                        Err(u) => u.report(self.diags, name_span(at)),
                    }
                }
                // Legacy `on:name={handler}` (deprecated in Svelte 5, still compiled in runes mode): a
                // listener like `onname={handler}`; modifiers and bare forwarding are rejected below
                AttributeLike::OnDirective(d) if d.modifiers.is_empty() && d.expression.is_some() => {
                    match self.sem.event(&er, &format!("on{}", d.name)) {
                        Ok(ev) => ops.push(Op::Listen { signal: ev.signal, handler: d.expression.as_ref().unwrap(), span: d.span() }),
                        Err(u) => u.report(self.diags, d.name_span.map_or(sp(d.span()), sp)),
                    }
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
                    ops.push(Op::BindThis { target, span: d.span() });
                }
                AttributeLike::BindDirective(d) => {
                    let Some(target) = &d.expression else { unreachable!("the parser synthesises bind:x shorthands") };
                    match self.sem.plan_binding(&er, &d.name) {
                        Ok(plan) => {
                            bound_props.push(plan.prop.clone());
                            ops.push(Op::Bind { prop: plan.prop.clone(), signal: plan.signal.clone(), arg: plan.arg_operand(), target, span: d.span() });
                        }
                        Err(u) => u.report(self.diags, sp(d.span())),
                    }
                }
                AttributeLike::ClassDirective(d) => {
                    if let (Some(cp), Some(e)) = (&class_prop, &d.expression) {
                        ops.push(Op::Token { prop: cp.clone(), token: d.name.clone(), cond: vec![CondAtom::Expr(e)], span: d.span() });
                    }
                }
                AttributeLike::SpreadAttribute(s) => {
                    // Decision (runtime-contract.md "Spread attributes"): rejected on native elements.
                    self.diags
                        .error(
                            "native_spread_unsupported",
                            sp(s.span()),
                            format!(
                                "spread attributes on native elements cannot be checked against the {} spec of <{}> at compile time, so the native compiler rejects them",
                                self.sem.backend(),
                                el.name
                            ),
                        )
                        .help("set the properties and events explicitly (`label={p.label}`), or spread onto a component (`<Child {...p} />`)");
                }
                other => {
                    let what = match other {
                        AttributeLike::AttachTag(_) => "{@attach}",
                        AttributeLike::OnDirective(_) => "on: directives with modifiers or without a handler (use on<event>={handler})",
                        AttributeLike::UseDirective(_) => "use: actions",
                        AttributeLike::TransitionDirective(_) => "transitions",
                        AttributeLike::AnimateDirective(_) => "animate: directives",
                        AttributeLike::StyleDirective(_) => "style: directives (use <style> rules)",
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

        // style-derived ops
        let mut chain = ancestors.to_vec();
        chain.push(subject);
        self.style_ops(&er, &chain, &explicit_props, &bound_props, &mut ops, tag_span);

        // children: text content or native children (an element's children are not in its
        // parent's <sv-slot>)
        let forced = self.forced_slot.take();
        let children = if let Some(tc) = self.sem.text_content(&er).cloned() {
            self.text_content(el, &tc.js, &mut ops);
            Fragment::default()
        } else {
            self.fragment(&el.fragment, &Parent::Element(er.clone(), id), &chain)
        };
        self.forced_slot = forced;

        Some(Element { id, span: el.span(), kind: er.tag.clone(), present, slot, class_expr, ops, children })
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
        let parts = trim_parts(self.text_parts(el, &el.fragment));
        let span = el.fragment.nodes.first().map(|n| n.span()).unwrap_or(el.span());
        match parts.as_slice() {
            [] => {}
            [TemplatePart::Text(t)] => ops.push(Op::Set { prop: prop.to_string(), value: Json::String(t.clone()), span }),
            _ => ops.push(Op::SetDynamic { prop: prop.to_string(), value: Dynamic::Template(parts), span }),
        }
    }

    fn text_parts(&mut self, el: &AstElement, f: &'a AstFragment) -> Vec<TemplatePart<'a>> {
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
                other => {
                    self.diags
                        .native_error("text_content_children", sp(other.span()), format!("<{}> holds text only on {}; it cannot contain this", el.name, self.sem.backend()))
                        .help("only text, {expressions} and {#if} blocks of those can be folded into its text");
                }
            }
        }
        parts
    }

    /// Resolves the component's style rules for one element (see docs/notes/native-compiler/styling.md):
    /// explicit attributes > rules by (specificity, source order) > native default.
    fn style_ops(&mut self, er: &ElementRef, chain: &[StyleSubject<'a>], explicit: &[String], bound: &[String], ops: &mut Vec<Op<'a>>, _tag_span: (u32, u32)) {
        let Some(sheet) = self.styles else { return };
        struct Cand<'a> {
            spec: (u32, u32),
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
                let Some(cond) = match_selector(sel, chain) else { continue };
                self.matched.insert(sel.span);
                for d in &rule.declarations {
                    let dspan = (d.span.0, d.span.1);
                    match self.sem.style_declaration(er, &d.name, &d.value) {
                        Ok(lowering) => cands.push(Cand {
                            spec: sel.specificity(),
                            order: order * 1000 + cands.len(),
                            cond: cond.clone(),
                            lowering,
                            span: Span::new(d.span.0, d.span.1),
                        }),
                        // A rule that reaches this element only through its own dynamic class set
                        // may never apply to it: a kind mismatch is not an error then.
                        Err(u)
                            if u.code == crate::semantics::codes::STYLE_NOT_APPLICABLE
                                && cond.iter().any(|a| matches!(a, CondAtom::Class { element, .. } if Some(*element) == chain.last().and_then(|s| s.dynamic))) => {}
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
            }
        }
        if cands.is_empty() {
            return;
        }
        // flatten intents into their actions
        let mut flat: Vec<(Cand<'a>, StyleLowering)> = Vec::new();
        for c in cands {
            match &c.lowering {
                StyleLowering::Intent { actions, .. } => {
                    for a in actions.clone() {
                        flat.push((Cand { spec: c.spec, order: c.order, cond: c.cond.clone(), lowering: a.clone(), span: c.span }, a));
                    }
                }
                l => {
                    let l = l.clone();
                    flat.push((c, l));
                }
            }
        }
        // highest precedence first
        flat.sort_by(|a, b| (b.0.spec, b.0.order).cmp(&(a.0.spec, a.0.order)));

        let mut props: Vec<String> = Vec::new();
        let mut tokens: Vec<(String, String)> = Vec::new();
        for (_, l) in &flat {
            match l {
                StyleLowering::SetProp { prop, .. } if !props.contains(prop) => props.push(prop.clone()),
                StyleLowering::AddToken { prop, token } if !tokens.contains(&(prop.clone(), token.clone())) => tokens.push((prop.clone(), token.clone())),
                _ => {}
            }
        }
        for prop in props {
            let mine: Vec<&(Cand<'a>, StyleLowering)> = flat.iter().filter(|(_, l)| matches!(l, StyleLowering::SetProp { prop: p, .. } if *p == prop)).collect();
            let span = mine[0].0.span;
            if bound.contains(&prop) {
                self.diags.native_error("style_conflicts_binding", sp(span), format!("style sets `{prop}`, which is bound with bind: on this element"));
                continue;
            }
            if explicit.contains(&prop) {
                self.diags.native_warning("style_overridden", sp(span), format!("`{prop}` is set explicitly on <{}>, which takes precedence over this style declaration", er.tag));
                continue;
            }
            let mut candidates: Vec<(Cond<'a>, Json)> = Vec::new();
            for (c, l) in mine {
                let StyleLowering::SetProp { value, .. } = l else { unreachable!() };
                candidates.push((c.cond.clone(), value.to_json()));
                if c.cond.is_empty() {
                    break;
                }
            }
            if candidates.len() == 1 && candidates[0].0.is_empty() {
                let (_, v) = candidates.pop().unwrap();
                ops.push(Op::Set { prop, value: v, span });
                continue;
            }
            let fallback = if candidates.last().is_some_and(|c| c.0.is_empty()) {
                Fallback::Default(Json::Null) // unreachable at runtime: the last candidate always holds
            } else {
                match self.sem.property(er, &prop).ok().and_then(|p| p.default.clone()) {
                    Some(d) => Fallback::Default(d),
                    None => Fallback::Restore,
                }
            };
            ops.push(Op::StyleProp { prop, candidates, fallback, span });
        }
        for (prop, token) in tokens {
            let mine: Vec<&(Cand<'a>, StyleLowering)> =
                flat.iter().filter(|(_, l)| matches!(l, StyleLowering::AddToken { prop: p, token: t } if *p == prop && *t == token)).collect();
            let span = mine[0].0.span;
            let cond = if mine.iter().any(|(c, _)| c.cond.is_empty()) {
                Vec::new()
            } else {
                if mine.len() > 1 {
                    self.diags.native_warning("style_token_conditions", sp(span), "several conditional rules add the same native token; only the highest-precedence condition is used");
                }
                mine[0].0.cond.clone()
            };
            ops.push(Op::Token { prop, token, cond, span });
        }
    }


    /// A component call. Children: `{#snippet}` blocks directly inside become props of the same
    /// name; everything else becomes the implicit `children` snippet (Svelte 5 semantics).
    fn component(&mut self, el: &'a AstElement, ancestors: &[StyleSubject<'a>]) -> Option<ComponentCall<'a>> {
        let mut props = Vec::new();
        let mut binds = Vec::new();
        let mut slot = None;
        for a in &el.attributes {
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
                    // Svelte pushes bindings after the other props, so a spread cannot override them
                    binds.push(ComponentProp::Bind { name: d.name.clone(), target });
                }
                AttributeLike::SpreadAttribute(s) => props.push(ComponentProp::Spread(&s.expression)),
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
        let mut frag = self.fragment(&el.fragment, &Parent::Detached, ancestors);
        self.in_block = outer;
        self.forced_slot = forced;
        let snippets = std::mem::take(&mut frag.snippets);
        let consts = std::mem::take(&mut frag.consts);
        if snippets.iter().any(|s| s.name == "children") && !frag.items.is_empty() {
            self.diags.error(
                "snippet_conflict",
                (el.start, el.end),
                "Cannot use explicit children snippet at the same time as implicit children content. Remove either the non-whitespace content or the children snippet block",
            );
        }
        let children = (!frag.items.is_empty()).then_some(frag);
        Some(ComponentCall { span: el.span(), node: el, slot, name: el.name.clone(), props, consts, snippets, children })
    }
}

/// Trims a branch's leading and trailing whitespace and drops empty text parts.
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
    let mut regular: Vec<Tx<'a>> = nodes
        .iter()
        .filter(|n| !matches!(n, FragmentNode::Comment(_)))
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
    let mut out: Vec<Entry<'a>> = Vec::new();
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
