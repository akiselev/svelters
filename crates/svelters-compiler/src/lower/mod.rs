//! Native lowering: Svelte template AST + native style sheet + a backend's semantics -> typed IR.
//!
//! This is where native semantics influence what source is valid: element tags, properties,
//! events, bindings, child slots, text content and style declarations are all resolved against
//! the backend specification, and every mismatch becomes a diagnostic at the source span.
//! Expression *meaning* (state reads/writes) is not decided here; codegen does that with the
//! scope analysis.

pub mod ir;

use crate::diag::Diagnostics;
use crate::estree::{Expression, Pattern, Span, Spanned};
use crate::semantics::{BackendSemantics, ElementRef, Lifetime, StyleLowering};
use crate::style::{Combinator, ComplexSelector, Compound, StyleSheet};
use crate::svelte::ast::{
    Attribute, AttributeLike, AttributeValue, Element as AstElement, Fragment as AstFragment, FragmentNode, Root,
    TextOrExpressionTag,
};
use ir::*;
use serde_json::Value as Json;
use std::collections::HashSet;

pub fn lower<'a>(root: &'a Root, sem: &dyn BackendSemantics, styles: Option<&'a StyleSheet>, diags: &mut Diagnostics) -> Component<'a> {
    let mut l = Lower { sem, diags, next_id: 0, styles, reported: HashSet::new(), single_slots: Default::default(), in_block: false };
    l.check_style_sheet();
    let root_fragment = l.fragment(&root.fragment, &Parent::Root, &[]);
    Component { root: root_fragment, elements: l.next_id }
}

enum Parent {
    Root,
    /// The parent element and its IR id.
    Element(ElementRef, usize),
}

/// What style matching needs to know about an element (and its ancestors in this component).
#[derive(Clone)]
struct StyleSubject<'a> {
    tag: String,
    static_classes: Vec<String>,
    cond_classes: Vec<(String, &'a Expression)>,
}

struct Lower<'a, 's, 'd> {
    sem: &'s dyn BackendSemantics,
    diags: &'d mut Diagnostics,
    next_id: usize,
    styles: Option<&'a StyleSheet>,
    /// (span, code) pairs already reported, so a declaration matching many elements reports once.
    reported: HashSet<(u32, u32, &'static str)>,
    /// Single-child slots already filled by a static element: (parent id, slot) -> byte offset.
    single_slots: std::collections::HashMap<(usize, String), u32>,
    /// Inside an {#if}/{#each} branch: alternatives may share a single-child slot.
    in_block: bool,
}

fn sp(s: Span) -> (u32, u32) {
    (s.start, s.end)
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

    fn fragment(&mut self, f: &'a AstFragment, parent: &Parent, ancestors: &[StyleSubject<'a>]) -> Fragment<'a> {
        let mut out = Fragment::default();
        for node in &f.nodes {
            match node {
                FragmentNode::Text(t) => {
                    if !t.data.trim().is_empty() {
                        let msg = match parent {
                            Parent::Root => "text is not allowed at the top level of a native component".to_string(),
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
                FragmentNode::RegularElement(el) => {
                    if let Some(e) = self.element(el, parent, ancestors) {
                        out.items.push(Item::Element(e));
                    }
                }
                FragmentNode::Component(el) => {
                    if let Some(c) = self.component(el) {
                        out.items.push(Item::Component(c));
                    }
                }
                FragmentNode::IfBlock(b) => {
                    let outer = std::mem::replace(&mut self.in_block, true);
                    let consequent = self.fragment(&b.consequent, parent, ancestors);
                    let alternate = b.alternate.as_ref().map(|a| self.fragment(a, parent, ancestors));
                    self.in_block = outer;
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
                    let outer = std::mem::replace(&mut self.in_block, true);
                    let body = self.fragment(&b.body, parent, ancestors);
                    let fallback = b.fallback.as_ref().map(|f| self.fragment(f, parent, ancestors));
                    self.in_block = outer;
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
                other => {
                    let what = unsupported_node_name(other);
                    self.diags.error("unsupported_feature", sp(other.span()), format!("{what} is not supported by the native compiler yet"));
                }
            }
        }
        out
    }

    /// A block region lives in one (parent, slot): every item inside must target the same slot.
    fn block_slot<const N: usize>(&mut self, span: (u32, u32), frags: [Option<&Fragment<'a>>; N]) -> Slot {
        let mut slots: Vec<&Slot> = Vec::new();
        for f in frags.into_iter().flatten() {
            for item in &f.items {
                slots.push(match item {
                    Item::Element(e) => &e.slot,
                    Item::If(b) => &b.slot,
                    Item::Each(b) => &b.slot,
                    Item::Component(c) => &c.slot,
                });
            }
        }
        let first = slots.first().cloned().cloned().flatten();
        if slots.iter().any(|s| **s != first) {
            self.diags.native_error(
                "block_mixed_slots",
                span,
                "the elements inside one {#if}/{#each} block must all go to the same child slot of their parent",
            );
        }
        first
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

        // pass 1: slot request and classes (they don't produce ops)
        let mut requested_slot: Option<String> = None;
        let mut subject = StyleSubject { tag: el.name.clone(), static_classes: Vec::new(), cond_classes: Vec::new() };
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
                    None => {
                        self.diags
                            .error("dynamic_class_unsupported", sp(at.span()), "a dynamic `class` value cannot be resolved against native styles")
                            .help("use class:name={condition} directives");
                    }
                },
                AttributeLike::ClassDirective(d) => match &d.expression {
                    Some(e) => subject.cond_classes.push((d.name.clone(), e)),
                    None => unreachable!("the parser synthesises class:x shorthands"),
                },
                _ => {}
            }
        }

        // placement
        let (slot, present) = match parent {
            Parent::Root => {
                // A toplevel kind is presented when mounted at the root. Other kinds are allowed at a
                // component's top level: a child component's root elements go to the container the
                // parent passes in (placement is then checked at runtime; the compiler cannot see
                // across component boundaries). Mounting a non-toplevel at the app root throws.
                let lt = self.sem.lifetime(&er);
                (None, if lt.lifetime == Lifetime::Toplevel { lt.present } else { None })
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
                                    format!("<{}>'s `{}` slot takes one child on {}, and it already has one (at byte {})", p.tag, plan.slot, self.sem.backend(), first),
                                )
                                .help("wrap the children in a container such as <box>");
                        } else if !self.in_block {
                            self.single_slots.insert(key, tag_span.0);
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
                AttributeLike::Attribute(at) if matches!(at.name.as_str(), "slot" | "data-slot" | "class") => {}
                AttributeLike::Attribute(at) if at.name.starts_with("on") && matches!(at.value, AttributeValue::Expression(_)) => {
                    let AttributeValue::Expression(crate::svelte::ast::ExpressionTagNode::ExpressionTag(t)) = &at.value else { unreachable!() };
                    match self.sem.event(&er, &at.name) {
                        Ok(ev) => ops.push(Op::Listen { signal: ev.signal, handler: &t.expression, span: at.span() }),
                        Err(u) => u.report(self.diags, name_span(at)),
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
                        AttributeValue::Sequence(parts) => {
                            let tp = parts
                                .iter()
                                .map(|p| match p {
                                    TextOrExpressionTag::Text(t) => TemplatePart::Text(t.data.clone()),
                                    TextOrExpressionTag::ExpressionTag(e) => TemplatePart::Expr(&e.expression),
                                })
                                .collect();
                            ops.push(Op::SetDynamic { prop: prop.js.clone(), value: Dynamic::Template(tp), span: at.span() });
                        }
                        AttributeValue::Expression(crate::svelte::ast::ExpressionTagNode::ExpressionTag(t)) => {
                            ops.push(Op::SetDynamic { prop: prop.js.clone(), value: Dynamic::Expr(&t.expression), span: at.span() });
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
                AttributeLike::ClassDirective(_) => {}
                other => {
                    let what = match other {
                        AttributeLike::SpreadAttribute(_) => "spread attributes ({...props})",
                        AttributeLike::AttachTag(_) => "{@attach}",
                        AttributeLike::OnDirective(_) => "on: directives (use on<event>={handler})",
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

        // children: text content or native children
        let children = if let Some(tc) = self.sem.text_content(&er).cloned() {
            self.text_content(el, &tc.js, &mut ops);
            Fragment::default()
        } else {
            self.fragment(&el.fragment, &Parent::Element(er.clone(), id), &chain)
        };

        Some(Element { id, span: el.span(), kind: er.tag.clone(), present, slot, ops, children })
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
    /// runs collapse to one space; the fragment's leading/trailing whitespace is trimmed).
    fn text_content(&mut self, el: &'a AstElement, prop: &str, ops: &mut Vec<Op<'a>>) {
        let mut parts: Vec<TemplatePart<'a>> = Vec::new();
        for n in &el.fragment.nodes {
            match n {
                FragmentNode::Text(t) => parts.push(TemplatePart::Text(collapse_ws(&t.data))),
                FragmentNode::ExpressionTag(e) => parts.push(TemplatePart::Expr(&e.expression)),
                FragmentNode::Comment(_) => {}
                other => {
                    self.diags
                        .native_error("text_content_children", sp(other.span()), format!("<{}> holds text only on {}; it cannot contain this", el.name, self.sem.backend()));
                }
            }
        }
        if let Some(TemplatePart::Text(t)) = parts.first_mut() {
            *t = t.trim_start().to_string();
        }
        if let Some(TemplatePart::Text(t)) = parts.last_mut() {
            *t = t.trim_end().to_string();
        }
        parts.retain(|p| !matches!(p, TemplatePart::Text(t) if t.is_empty()));
        let span = el.fragment.nodes.first().map(|n| n.span()).unwrap_or(el.span());
        match parts.as_slice() {
            [] => {}
            [TemplatePart::Text(t)] => ops.push(Op::Set { prop: prop.to_string(), value: Json::String(t.clone()), span }),
            _ => ops.push(Op::SetDynamic { prop: prop.to_string(), value: Dynamic::Template(parts), span }),
        }
    }

    /// Resolves the component's style rules for one element (see docs/notes/native-compiler/styling.md):
    /// explicit attributes > rules by (specificity, source order) > native default.
    fn style_ops(&mut self, er: &ElementRef, chain: &[StyleSubject<'a>], explicit: &[String], bound: &[String], ops: &mut Vec<Op<'a>>, tag_span: (u32, u32)) {
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
                        Err(u) => {
                            if self.reported.insert((dspan.0, dspan.1, u.code)) {
                                let mut msg = u.message.clone();
                                if !u.suggestions.is_empty() {
                                    msg.push_str(&format!(" (did you mean {}?)", u.suggestions.join(", ")));
                                }
                                let d = self.diags.native_error(u.code, dspan, format!("{msg} [matched <{}> at byte {}]", er.tag, tag_span.0));
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

    fn component(&mut self, el: &'a AstElement) -> Option<ComponentCall<'a>> {
        let mut props = Vec::new();
        let mut slot = None;
        for a in &el.attributes {
            match a {
                AttributeLike::Attribute(at) if at.name == "slot" => slot = static_text(at),
                AttributeLike::Attribute(at) => match &at.value {
                    AttributeValue::True(_) => props.push(ComponentProp::Static { name: at.name.clone(), value: Json::Bool(true) }),
                    AttributeValue::Expression(crate::svelte::ast::ExpressionTagNode::ExpressionTag(t)) => {
                        props.push(ComponentProp::Dynamic { name: at.name.clone(), value: Dynamic::Expr(&t.expression) })
                    }
                    AttributeValue::Sequence(_) => match static_text(at) {
                        Some(s) => props.push(ComponentProp::Static { name: at.name.clone(), value: Json::String(s) }),
                        None => {
                            let AttributeValue::Sequence(parts) = &at.value else { unreachable!() };
                            let tp = parts
                                .iter()
                                .map(|p| match p {
                                    TextOrExpressionTag::Text(t) => TemplatePart::Text(t.data.clone()),
                                    TextOrExpressionTag::ExpressionTag(e) => TemplatePart::Expr(&e.expression),
                                })
                                .collect();
                            props.push(ComponentProp::Dynamic { name: at.name.clone(), value: Dynamic::Template(tp) });
                        }
                    },
                },
                AttributeLike::BindDirective(d) => {
                    let Some(target) = &d.expression else { unreachable!() };
                    props.push(ComponentProp::Bind { name: d.name.clone(), target });
                }
                other => {
                    self.diags.error("unsupported_feature", sp(other.span()), "this attribute kind is not supported on components by the native compiler yet");
                }
            }
        }
        for n in &el.fragment.nodes {
            if !matches!(n, FragmentNode::Text(t) if t.data.trim().is_empty()) && !matches!(n, FragmentNode::Comment(_)) {
                self.diags.error("unsupported_feature", sp(n.span()), "component children (snippets) are not supported by the native compiler yet");
                break;
            }
        }
        Some(ComponentCall { span: el.span(), slot, name: el.name.clone(), props })
    }
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
        let (_, e) = s.cond_classes.iter().find(|(n, _)| n == class)?;
        cond.push(*e);
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
