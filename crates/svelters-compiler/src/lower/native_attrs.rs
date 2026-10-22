//! The X2/X3 parts of the renderer contract on native elements: child attributes
//! (`parent:<name>`), listener phases (`on<event>capture`, `{@attach listen(...)}`), sync
//! handlers (`(event, ...args)` listeners), plus `{@attach}` and spreads of static objects.
//! Semantics: docs/notes/native-compiler/runtime-contract.md ("Child attributes", "Events").

use super::*;
use crate::estree::{ArrowBody, LiteralValue, ObjectExpression, ObjectMember, PropertyKey, PropertyKind, Statement};
use crate::semantics::{codes, EventSpec, LISTEN_AFTER, LISTEN_CAPTURE};
use crate::svelte::ast::{AttachTag, Script};

/// `parent:<name>` attributes (the renderer's CHILD_ATTR_PREFIX).
pub const CHILD_ATTR_PREFIX: &str = "parent:";

/// What the instance/module scripts tell lowering: local names of `listen` imported from
/// `@svelters/native-runtime`.
#[derive(Default)]
pub(super) struct ScriptFacts {
    pub listen: HashSet<String>,
}

impl ScriptFacts {
    pub fn collect(scripts: [Option<&Script>; 2]) -> ScriptFacts {
        let mut f = ScriptFacts::default();
        for s in scripts.into_iter().flatten() {
            for stmt in &s.content.body {
                match stmt {
                    Statement::ImportDeclaration(d) => {
                        let crate::estree::LiteralNode::Literal(src) = &d.source;
                        if !matches!(&src.value, LiteralValue::String(v) if v == HELPERS_MODULE) {
                            continue;
                        }
                        for sp in &d.specifiers {
                            if let crate::estree::ImportSpecifierKind::ImportSpecifier(is) = sp {
                                let imported = match &is.imported {
                                    crate::estree::ModuleExportName::Identifier(i) => i.name.clone(),
                                    crate::estree::ModuleExportName::Literal(l) => match &l.value {
                                        LiteralValue::String(s) => s.clone(),
                                        _ => continue,
                                    },
                                };
                                if imported == "listen" {
                                    f.listen.insert(is.local.name.clone());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        f
    }
}

/// The module whose `listen` helper (and `NativeEvent`, `StaleHandleError`, `LISTEN_*`) components import.
pub const HELPERS_MODULE: &str = "@svelters/native-runtime";

/// Static keys of a spread object literal (no computed keys, accessors or spreads).
fn object_keys(o: &ObjectExpression) -> Option<Vec<(String, &Expression)>> {
    let mut out = Vec::new();
    for m in &o.properties {
        let ObjectMember::Property(p) = m else { return None };
        if p.computed || p.kind != PropertyKind::Init {
            return None;
        }
        let key = match &p.key {
            PropertyKey::Expression(e) => match e.as_ref() {
                Expression::Identifier(i) => i.name.clone(),
                Expression::Literal(l) => match &l.value {
                    LiteralValue::String(s) => s.clone(),
                    _ => return None,
                },
                _ => return None,
            },
            PropertyKey::Private(_) => return None,
        };
        out.push((key, &p.value));
    }
    Some(out)
}

fn bool_literal(e: &Expression) -> Option<bool> {
    match e {
        Expression::Literal(l) => match l.value {
            LiteralValue::Boolean(b) => Some(b),
            _ => None,
        },
        _ => None,
    }
}

impl<'a> Lower<'a, '_, '_> {
    /// `on<event>={handler}` and `on<event>capture={handler}` (Svelte's capture suffix): the
    /// event resolved against the spec, its phase checked, sync handler shape checked.
    pub(super) fn listen_op(&mut self, er: &ElementRef, attr: &str, handler: Handler<'a>, span: Span, name_span: (u32, u32)) -> Option<Op<'a>> {
        let (ev, flags) = match self.sem.event(er, attr) {
            Ok(ev) => (ev, 0),
            Err(u) => {
                // Svelte's is_capture_event: `...capture`, except the pointer-capture events
                let base = attr
                    .strip_suffix("capture")
                    .filter(|b| !b.is_empty() && b != &"on" && !matches!(attr, "ongotpointercapture" | "onlostpointercapture"));
                match base.map(|b| self.sem.event(er, b)) {
                    Some(Ok(ev)) => (ev, LISTEN_CAPTURE),
                    // `on<x>capture` with an unknown `x`: report `x`
                    Some(Err(ub)) => {
                        ub.report(self.diags, name_span);
                        return None;
                    }
                    None => {
                        u.report(self.diags, name_span);
                        return None;
                    }
                }
            }
        };
        if !self.check_phases(er, &ev, flags, name_span) {
            return None;
        }
        if let Handler::Expr(h) = &handler {
            self.check_sync_handler(&ev, h);
        }
        Some(Op::Listen { signal: ev.signal, handler, flags, sync: ev.sync, span })
    }

    /// Every requested phase must be one the backend accepts for the signal (spec `phases`).
    fn check_phases(&mut self, er: &ElementRef, ev: &EventSpec, flags: u32, span: (u32, u32)) -> bool {
        for (bit, name) in [(LISTEN_AFTER, "after"), (LISTEN_CAPTURE, "capture")] {
            if flags & bit != 0 && !ev.phases.iter().any(|p| p == name) {
                let has = if ev.phases.is_empty() { "no listen phases".to_string() } else { format!("only {}", ev.phases.join(", ")) };
                let why = match (self.sem.backend(), name) {
                    ("gtk", "capture") => "; on GTK only event controllers have a capture phase",
                    ("qt", "capture") => "; on Qt only the synthetic key-press has a capture phase",
                    ("qt", "after") => "; on Qt event-filter signals (key-press, close-request) have no after phase",
                    _ => "",
                };
                self.diags.native_error(
                    codes::LISTEN_PHASE_UNSUPPORTED,
                    span,
                    format!("`{}` of <{}> has {has} on {}, not `{name}`{why}", ev.signal, er.tag, self.sem.backend()),
                );
                return false;
            }
        }
        true
    }

    /// Cheap checks of a handler for a sync signal (`(event, ...args)`, result from the event).
    fn check_sync_handler(&mut self, ev: &EventSpec, h: &'a Expression) {
        if !ev.sync {
            return;
        }
        let (params, returns_bool) = match h {
            Expression::ArrowFunctionExpression(f) => {
                let ret = match &f.body {
                    ArrowBody::Expression(e) => bool_literal(e).is_some(),
                    ArrowBody::Block(crate::estree::BlockStatementNode::BlockStatement(b)) => returns_bool(&b.body),
                };
                (&f.params, ret)
            }
            Expression::FunctionExpression(f) => {
                let crate::estree::BlockStatementNode::BlockStatement(b) = &f.body;
                (&f.params, returns_bool(&b.body))
            }
            _ => return,
        };
        if let (Some(Pattern::Identifier(p)), Some((arg, _))) = (params.first(), ev.args.first())
            && p.name == *arg
        {
            self.diags
                .native_warning(
                    "sync_handler_arguments",
                    sp(p.span()),
                    format!("`{}` is a sync signal: its listeners get (event, ...args), so `{}` receives the event object, not the `{arg}` argument", ev.signal, p.name),
                )
                .help(format!("write ({}event, {}) => ...", "", p.name));
        }
        if returns_bool {
            self.diags
                .native_warning(
                    "sync_handler_return",
                    sp(h.span()),
                    format!("the return value of a `{}` listener is ignored (as in Svelte)", ev.signal),
                )
                .help("call e.preventDefault() (result true) or set e.result on the event, the first argument");
        }
    }

    /// `parent:<name>` on a native element. `target`: the parent element and the slot this
    /// element was placed in (None at a component root or in detached content, where the parent
    /// is unknown: the value is passed through unchecked, as the renderer does).
    pub(super) fn child_attr_op(&mut self, target: Option<&(ElementRef, String)>, name: &str, at: &'a Attribute) -> Option<Op<'a>> {
        let span = at.span();
        let static_text = match &at.value {
            AttributeValue::True(_) => Some(None),
            AttributeValue::Sequence(parts) if parts.iter().all(|p| matches!(p, TextOrExpressionTag::Text(_))) => {
                Some(Some(parts.iter().map(|p| if let TextOrExpressionTag::Text(t) = p { t.data.as_str() } else { "" }).collect::<String>()))
            }
            _ => None,
        };
        let Some((parent, slot)) = target else {
            let value = match static_text {
                Some(None) => AttrValue::Static(Json::String(String::new())),
                Some(Some(t)) => AttrValue::Static(Json::String(t)),
                None => AttrValue::Dynamic(dynamic_of(at).expect("not a bare attribute")),
            };
            return Some(Op::ChildAttr { name: name.to_string(), value, span });
        };
        let plan = match self.sem.child_attr(parent, slot, name) {
            Ok(p) => p,
            Err(u) => {
                u.report(self.diags, name_span(at));
                return None;
            }
        };
        let value = match static_text {
            Some(t) => match self.sem.coerce_child_attr(&plan, t.as_deref()) {
                Ok(v) => AttrValue::Static(v.to_json()),
                Err(msg) => {
                    self.diags.native_error(codes::CHILD_ATTR_BAD_VALUE, sp(span), msg);
                    return None;
                }
            },
            None => AttrValue::Dynamic(dynamic_of(at).expect("not a bare attribute")),
        };
        Some(Op::ChildAttr { name: plan.name, value, span })
    }

    /// At a render site in `parent`'s slot `slot`: checks the `parent:` attributes of a snippet
    /// root element (lowered unchecked inside the snippet body).
    pub(super) fn check_snippet_child_attrs(&mut self, parent: &ElementRef, slot: &str, el: &AstElement, snippet: &str, span: (u32, u32)) {
        for a in &el.attributes {
            let AttributeLike::Attribute(at) = a else { continue };
            let Some(name) = at.name.strip_prefix(CHILD_ATTR_PREFIX) else { continue };
            let res = self.sem.child_attr(parent, slot, name).and_then(|plan| match static_text(at) {
                Some(t) => self.sem.coerce_child_attr(&plan, Some(&t)).map(|_| ()).map_err(|m| Unresolved::new(codes::CHILD_ATTR_BAD_VALUE, m)),
                None => Ok(()),
            });
            if let Err(u) = res {
                let u = Unresolved {
                    message: format!("{{@render {snippet}()}} places <{}> (a root element of snippet `{snippet}`) in <{}>: {}", el.name, parent.tag, u.message),
                    ..u
                };
                u.report(self.diags, span);
            }
        }
    }

    /// `{...spread}` on a native element, when its keys are known at compile time (an object
    /// literal): each key is lowered as the attribute of
    /// that name (`on...` events, `parent:` child attributes, props). Returns false when the keys
    /// are not static (`is_static_spread`; such a spread goes through `$n.spread`).
    pub(super) fn static_spread(
        &mut self,
        er: &ElementRef,
        expr: &'a Expression,
        span: Span,
        target: Option<&(ElementRef, String)>,
        ops: &mut Vec<Op<'a>>,
        explicit: &mut Vec<String>,
    ) -> bool {
        let entries = match expr {
            Expression::ObjectExpression(o) => match object_keys(o) {
                Some(k) => k,
                None => return false,
            },
            _ => return false,
        };
        let s = sp(span);
        for (key, v) in entries {
            if key.starts_with("on") && key.len() > 2 {
                if let Some(op) = self.listen_op(er, &key, Handler::Expr(v), span, s) {
                    ops.push(op);
                }
                continue;
            }
            let value = Dynamic::Expr(v);
            if let Some(name) = key.strip_prefix(CHILD_ATTR_PREFIX) {
                if let Some((p, slot)) = target
                    && let Err(u) = self.sem.child_attr(p, slot, name)
                {
                    u.report(self.diags, s);
                    continue;
                }
                ops.push(Op::ChildAttr { name: name.to_string(), value: AttrValue::Dynamic(value), span });
                continue;
            }
            let prop = match self.sem.writable_property(er, &key) {
                Ok(p) => p.clone(),
                Err(u) => {
                    u.report(self.diags, s);
                    continue;
                }
            };
            explicit.push(prop.js.clone());
            ops.push(Op::SetDynamic { prop: prop.js.clone(), value, span });
            if let Ok(plan) = self.sem.plan_binding(er, &prop.js) {
                ops.push(Op::Track { prop: plan.prop.clone(), signal: plan.signal.clone(), arg: plan.arg_operand() });
            }
        }
        true
    }

    /// `{@attach expr}`. `listen(type, handler, { after, capture })` imported from
    /// @svelters/native-runtime, with a static type and static flags, is resolved and phase-checked at
    /// compile time; any other attachment runs as Svelte's `$.attach` with the runtime node.
    pub(super) fn attach_op(&mut self, er: &ElementRef, t: &'a AttachTag) -> Option<Op<'a>> {
        let span = t.span();
        if let Expression::CallExpression(c) = &t.expression
            && let Expression::Identifier(callee) = c.callee.as_ref()
            && self.script.listen.contains(&callee.name)
        {
            let args: Vec<&'a Expression> =
                c.arguments.iter().filter_map(|a| if let ArrayElement::Expression(e) = a { Some(e) } else { None }).collect();
            let ty = match args.first() {
                Some(Expression::Literal(l)) => match &l.value {
                    LiteralValue::String(s) => Some(s.clone()),
                    _ => None,
                },
                _ => None,
            };
            let flags = match args.get(2) {
                None => Some(0),
                Some(Expression::ObjectExpression(o)) => object_keys(o).and_then(|keys| {
                    let mut f = 0;
                    for (k, v) in keys {
                        let bit = match k.as_str() {
                            "after" => LISTEN_AFTER,
                            "capture" => LISTEN_CAPTURE,
                            _ => continue,
                        };
                        if bool_literal(v)? {
                            f |= bit;
                        }
                    }
                    Some(f)
                }),
                Some(_) => None,
            };
            if let (Some(ty), Some(flags), Some(handler), true) = (ty, flags, args.get(1), c.arguments.len() == args.len()) {
                let ev = match self.sem.event(er, &ty) {
                    Ok(ev) => ev,
                    Err(u) => {
                        u.report(self.diags, sp(span));
                        return None;
                    }
                };
                if !self.check_phases(er, &ev, flags, sp(span)) {
                    return None;
                }
                self.check_sync_handler(&ev, handler);
                return Some(Op::ListenAttach { signal: ev.signal, handler, flags, span });
            }
        }
        Some(Op::Attach { expr: &t.expression, span })
    }
}

/// An attribute's value as written: `true` for a bare attribute, the text of a static one, else
/// the expression or template (no spec coercion: the kind is not known, or it is a styling attribute).
pub(super) fn attr_value(at: &Attribute) -> AttrValue<'_> {
    match (&at.value, static_text(at)) {
        (AttributeValue::True(_), _) => AttrValue::Static(Json::Bool(true)),
        (_, Some(t)) => AttrValue::Static(Json::String(t)),
        _ => AttrValue::Dynamic(dynamic_of(at).expect("not a bare attribute")),
    }
}

impl<'a> Lower<'a, '_, '_> {
    /// Whether `{...expr}` has keys known at compile time (`static_spread` lowers it key by key).
    pub(super) fn is_static_spread(&self, expr: &Expression) -> bool {
        // `class` / `style` (and `slot`) need the runtime path: they merge with the element's own
        let ok = |o: &ObjectExpression| object_keys(o).is_some_and(|k| !k.iter().any(|(k, _)| matches!(k.as_str(), "slot" | "data-slot" | "class" | "style")));
        // an object by name may gain keys at run time (`Object.defineProperty`), so only a literal
        matches!(expr, Expression::ObjectExpression(o) if ok(o))
    }

    /// The value of a `style:` directive (the shorthand `style:x` is rejected natively).
    pub(super) fn directive_value(&mut self, d: &'a crate::svelte::ast::StyleDirective) -> Option<AttrValue<'a>> {
        let span = sp(d.span());
        if !d.modifiers.is_empty() {
            self.diags.error("unsupported_feature", span, format!("`style:{}|{}`: style directive modifiers are not supported natively", d.name, d.modifiers.join("|")));
            return None;
        }
        match &d.value {
            AttributeValue::True(_) => {
                self.diags.error("unsupported_feature", span, format!("the `style:{0}` shorthand is not supported natively; write `style:{0}={{{0}}}`", d.name));
                None
            }
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
        }
    }

    /// `<svelte:element this={tag}>` (runtime-contract.md "Spread attributes"): the kind is only
    /// known at run time, so every attribute goes through `$n.spread` and the children are placed
    /// (and checked by the host) at run time.
    pub(super) fn dynamic_element(&mut self, n: &'a crate::svelte::ast::SvelteElement, parent: &Parent, ancestors: &[StyleSubject<'a>]) -> Option<DynamicElement<'a>> {
        let id = self.next_id;
        self.next_id += 1;
        self.kinds = true;
        let mut slot = None;
        let mut entries: Vec<SpreadEntry<'a>> = Vec::new();
        let mut ops: Vec<Op<'a>> = Vec::new();
        let mut binds: Vec<Op<'a>> = Vec::new();
        let mut subject = StyleSubject { tag: n.name.clone(), id: None, dynamic_id: None, static_classes: Vec::new(), cond_classes: Vec::new(), dynamic: None };
        for a in &n.attributes {
            match a {
                AttributeLike::Attribute(at) if matches!(at.name.as_str(), "slot" | "data-slot") => match static_text(at) {
                    Some(s) => slot = Some(s),
                    None => {
                        self.diags.error("slot_not_static", sp(at.span()), "`slot` must be a static name");
                    }
                },
                AttributeLike::Attribute(at) if at.name.starts_with(CHILD_ATTR_PREFIX) => {
                    ops.extend(self.child_attr_op(None, &at.name[CHILD_ATTR_PREFIX.len()..], at));
                }
                AttributeLike::Attribute(at) if at.name.starts_with("on") && matches!(at.value, AttributeValue::Expression(_)) => {
                    let AttributeValue::Expression(ExpressionTagNode::ExpressionTag(t)) = &at.value else { unreachable!() };
                    entries.push(SpreadEntry::Event { key: at.name.clone(), handler: &t.expression });
                }
                AttributeLike::Attribute(at) => {
                    if at.name == "class"
                        && let Some(s) = static_text(at)
                    {
                        subject.static_classes.extend(s.split_ascii_whitespace().map(str::to_string));
                    }
                    entries.push(SpreadEntry::Attr { key: at.name.clone(), value: attr_value(at) });
                }
                AttributeLike::SpreadAttribute(s) => entries.push(SpreadEntry::Spread(&s.expression)),
                AttributeLike::ClassDirective(d) => {
                    if let Some(e) = &d.expression {
                        subject.cond_classes.push((d.name.clone(), e));
                        entries.push(SpreadEntry::Class { name: d.name.clone(), expr: e });
                    }
                }
                AttributeLike::StyleDirective(d) => {
                    if let Some(value) = self.directive_value(d) {
                        entries.push(SpreadEntry::Style { name: d.name.clone(), value });
                    }
                }
                AttributeLike::OnDirective(d) if d.modifiers.is_empty() && d.expression.is_some() => {
                    entries.push(SpreadEntry::Event { key: format!("on{}", d.name), handler: d.expression.as_ref().unwrap() });
                }
                AttributeLike::BindDirective(d) if d.name == "this" => {
                    let Some(target) = &d.expression else { unreachable!("the parser synthesises bind:x shorthands") };
                    self.no_bind_modifiers(d, "bind:this");
                    binds.push(Op::BindThis { target, span: d.span() });
                }
                AttributeLike::BindDirective(d) => {
                    self.diags
                        .native_error(
                            "dynamic_element_bind",
                            sp(d.span()),
                            format!("bind:{} on <svelte:element> needs the element's kind at compile time to find its binding plan", d.name),
                        )
                        .help("use a static element, or set the property and listen to its event");
                }
                AttributeLike::AttachTag(t) => ops.push(Op::Attach { expr: &t.expression, span: t.span() }),
                AttributeLike::UseDirective(d) => binds.push(Op::Action { name: d.name.clone(), arg: d.expression.as_ref(), span: d.span() }),
                other => {
                    let what = match other {
                        AttributeLike::OnDirective(_) => "on: directives with modifiers or without a handler (use on<event>={handler})",
                        AttributeLike::TransitionDirective(_) => "transitions",
                        AttributeLike::AnimateDirective(_) => "animate: directives",
                        AttributeLike::LetDirective(_) => "let: directives",
                        _ => "this attribute",
                    };
                    self.diags.error("unsupported_feature", sp(other.span()), format!("{what} are not supported by the native compiler yet"));
                }
            }
        }
        if let Some(f) = &self.forced_slot {
            match &slot {
                Some(r) if r != f => {
                    self.diags.error("slot_conflict", sp(n.name_span), format!("<svelte:element> asks for slot `{r}` inside <sv-slot name=\"{f}\">"));
                }
                _ => slot = Some(f.clone()),
            }
        }
        let css = if self.sem.text_node().is_none() { self.css_scope.clone() } else { None };
        ops.insert(0, Op::Spread { entries, css, span: n.span() });
        ops.extend(binds);

        // children: text goes to the kind's text-content prop (looked up at run time) on a backend
        // without text nodes; elements go into its slots, placed by the host
        let forced = self.forced_slot.take();
        let mut chain = ancestors.to_vec();
        chain.push(subject);
        let children = if self.sem.text_node().is_none() && has_text(&n.fragment) {
            self.text_prop = Some("its text-content property".to_string());
            let parts = trim_parts(self.text_parts(("svelte:element", &n.fragment), &n.fragment));
            self.text_prop = None;
            let span = n.fragment.nodes.first().map(|x| x.span()).unwrap_or(n.span());
            match parts.as_slice() {
                [] => {}
                [TemplatePart::Text(t)] => ops.push(Op::Text { value: AttrValue::Static(Json::String(t.clone())), span }),
                _ => ops.push(Op::Text { value: AttrValue::Dynamic(Dynamic::Template(parts)), span }),
            }
            Fragment::default()
        } else {
            let outer = std::mem::replace(&mut self.in_block, true);
            let f = self.fragment(&n.fragment, &Parent::Detached, &chain);
            self.in_block = outer;
            f
        };
        self.forced_slot = forced;
        // placement in `parent` is checked by the host at insertion (the kind is not known here)
        let _ = parent;
        Some(DynamicElement { id, span: n.span(), tag: &n.tag, slot, ops, children })
    }
}

fn returns_bool(body: &[Statement]) -> bool {
    body.iter().any(|s| matches!(s, Statement::ReturnStatement(r) if r.argument.as_ref().is_some_and(|a| bool_literal(a).is_some())))
}
