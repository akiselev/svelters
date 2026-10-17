//! The X2/X3 parts of the renderer contract on native elements: child attributes
//! (`parent:<name>`), listener phases (`on<event>capture`, `{@attach listen(...)}`), sync
//! handlers (`(event, ...args)` listeners), plus `{@attach}` and spreads of static objects.
//! Semantics: docs/notes/native-compiler/runtime-contract.md ("Child attributes", "Events").

use super::*;
use crate::estree::{ArrowBody, LiteralValue, ObjectExpression, ObjectMember, PropertyKey, PropertyKind, Statement, VariableKind};
use crate::semantics::{codes, EventSpec, LISTEN_AFTER, LISTEN_CAPTURE};
use crate::svelte::ast::{AttachTag, Script};

/// `parent:<name>` attributes (the renderer's CHILD_ATTR_PREFIX).
pub const CHILD_ATTR_PREFIX: &str = "parent:";

/// What the instance/module scripts tell lowering: local names of `listen` imported from
/// `@svelters/renderer`, and top-level `const x = { ... }` object literals (spreadable).
#[derive(Default)]
pub(super) struct ScriptFacts<'a> {
    pub listen: HashSet<String>,
    pub objects: HashMap<String, &'a ObjectExpression>,
}

impl<'a> ScriptFacts<'a> {
    pub fn collect(scripts: [Option<&'a Script>; 2]) -> ScriptFacts<'a> {
        let mut f = ScriptFacts::default();
        for s in scripts.into_iter().flatten() {
            for stmt in &s.content.body {
                match stmt {
                    Statement::ImportDeclaration(d) => {
                        let crate::estree::LiteralNode::Literal(src) = &d.source;
                        if !matches!(&src.value, LiteralValue::String(v) if v == RENDERER_MODULE) {
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
                    Statement::VariableDeclaration(v) if v.kind == VariableKind::Const => {
                        for d in &v.declarations {
                            if let (Pattern::Identifier(id), Some(Expression::ObjectExpression(o))) = (&d.id, &d.init) {
                                f.objects.insert(id.name.clone(), o);
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

/// The renderer module whose `listen` helper compiled components get from the native runtime.
pub const RENDERER_MODULE: &str = "@svelters/renderer";

/// A value taken from a spread object.
pub(super) enum SpreadValue<'a> {
    /// `{...{ key: value }}`: the property's own expression.
    Expr(&'a Expression),
    /// `{...obj}` where `obj` is a top-level `const` object literal: `obj[key]`, read when used.
    Member { object: &'a Expression, key: String },
}

/// Static keys of a spread: an object literal, or a top-level `const` object literal by name.
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
    /// literal, or a top-level `const` object literal): each key is lowered as the attribute of
    /// that name (`on...` events, `parent:` child attributes, props). Returns false when the keys
    /// are not static (the caller reports `native_spread_unsupported`).
    pub(super) fn static_spread(
        &mut self,
        er: &ElementRef,
        expr: &'a Expression,
        span: Span,
        target: Option<&(ElementRef, String)>,
        ops: &mut Vec<Op<'a>>,
        explicit: &mut Vec<String>,
    ) -> bool {
        let entries: Vec<(String, SpreadValue<'a>)> = match expr {
            Expression::ObjectExpression(o) => match object_keys(o) {
                Some(k) => k.into_iter().map(|(k, v)| (k, SpreadValue::Expr(v))).collect(),
                None => return false,
            },
            Expression::Identifier(id) => match self.script.objects.get(&id.name).and_then(|o| object_keys(o)) {
                Some(k) => k.into_iter().map(|(k, _)| (k.clone(), SpreadValue::Member { object: expr, key: k })).collect(),
                None => return false,
            },
            _ => return false,
        };
        let s = sp(span);
        for (key, v) in entries {
            if key.starts_with("on") && key.len() > 2 {
                let handler = match v {
                    SpreadValue::Expr(e) => Handler::Expr(e),
                    SpreadValue::Member { object, key } => Handler::Member { object, key },
                };
                if let Some(op) = self.listen_op(er, &key, handler, span, s) {
                    ops.push(op);
                }
                continue;
            }
            let value = match v {
                SpreadValue::Expr(e) => Dynamic::Expr(e),
                SpreadValue::Member { object, key } => Dynamic::Member { object, key },
            };
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
            if matches!(key.as_str(), "slot" | "data-slot" | "class" | "style") {
                self.diags.error("native_spread_unsupported", s, format!("`{key}` cannot come from a spread on a native element; write it as an attribute"));
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
    /// @svelters/renderer, with a static type and static flags, is resolved and phase-checked at
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

fn returns_bool(body: &[Statement]) -> bool {
    body.iter().any(|s| matches!(s, Statement::ReturnStatement(r) if r.argument.as_ref().is_some_and(|a| bool_literal(a).is_some())))
}
