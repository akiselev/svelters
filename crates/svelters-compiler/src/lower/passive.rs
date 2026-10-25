//! W-C5: passive-listener inference (presto `compiler-and-js.md` §3.5).
//!
//! A listener is passive when it cannot cancel its event: the signal is not cancelable, or the
//! handler is a function literal that takes no parameter, or whose event parameter is only ever
//! read as `event.<name>` for a name other than `preventDefault`, `stopPropagation` and
//! `stopImmediatePropagation`, and never written (`event.result = ...` sets a sync result). An
//! event that escapes (passed on, stored, read with a computed key, a rest parameter, a function
//! using `arguments`) and a handler the compiler cannot see (a member, a forwarded prop, a variable
//! that is not a top-level `function` or `const` function of the component's scripts) make the
//! listener non-passive. `|passive` and `|nonpassive` override the inference.

use crate::estree::*;
use crate::lower::ir::Handler;
use crate::semantics::EventSpec;
use crate::svelte::scope::walk::{NodeRef, Visitor, Walker, expr_ref};
use std::collections::HashMap;

/// A handler function declared at the top level of a script, by name.
#[derive(Clone, Copy)]
pub enum Declared<'a> {
    /// `function name(...) {}`.
    Function(&'a Function),
    /// `const name = (...) => ...` (or a function expression).
    Const(&'a Expression),
}

/// The top-level `function` declarations and `const` function bindings of the component's scripts.
pub fn declared<'a>(scripts: [Option<&'a crate::svelte::ast::Script>; 2]) -> HashMap<String, Declared<'a>> {
    let mut out = HashMap::new();
    for s in scripts.into_iter().flatten() {
        for stmt in &s.content.body {
            match stmt {
                Statement::FunctionDeclaration(f) => {
                    if let Some(id) = &f.id {
                        out.insert(id.name.clone(), Declared::Function(f));
                    }
                }
                Statement::VariableDeclaration(d) if d.kind == VariableKind::Const => {
                    for v in &d.declarations {
                        if let (Pattern::Identifier(id), Some(init)) = (&v.id, &v.init)
                            && matches!(unparen(init), Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_))
                        {
                            out.insert(id.name.clone(), Declared::Const(init));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// What a handler can do to its event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Passivity {
    /// It never cancels the event.
    Passive,
    /// It cancels it, or may: the span of the `preventDefault` (or write) found.
    Cancels(Span),
    /// The event escapes, or the handler is not visible.
    Unknown,
}

impl Passivity {
    pub fn is_passive(self) -> bool {
        self == Passivity::Passive
    }
}

/// The members of an event a listener cancels it with.
const CANCEL: &[&str] = &["preventDefault", "stopPropagation", "stopImmediatePropagation"];

/// The passivity of `handler` for the signal of `ev`; `declared`: the scripts' handler functions.
pub fn infer(handler: &Handler, ev: &EventSpec, declared: &HashMap<String, Declared>) -> Passivity {
    if !ev.cancelable {
        return Passivity::Passive;
    }
    match handler {
        Handler::Expr(h) => match unparen(h) {
            Expression::Identifier(id) => match declared.get(&id.name) {
                Some(Declared::Function(f)) => of_parts(&f.params, body_of(f), true),
                Some(Declared::Const(e)) => of_function(unparen(e)),
                None => Passivity::Unknown,
            },
            h => of_function(h),
        },
        Handler::Member { .. } | Handler::Forward { .. } => Passivity::Unknown,
    }
}

fn body_of(f: &Function) -> NodeRef<'_> {
    let BlockStatementNode::BlockStatement(b) = &f.body;
    NodeRef::BlockStatement(b)
}

fn unparen(e: &Expression) -> &Expression {
    match e {
        Expression::ParenthesizedExpression(p) => unparen(&p.expression),
        e => e,
    }
}

fn of_function(h: &Expression) -> Passivity {
    let (params, body, arguments) = match h {
        Expression::ArrowFunctionExpression(f) => {
            let body = match &f.body {
                ArrowBody::Expression(e) => expr_ref(e),
                ArrowBody::Block(BlockStatementNode::BlockStatement(b)) => NodeRef::BlockStatement(b),
            };
            (&f.params, body, false)
        }
        Expression::FunctionExpression(f) => (&f.params, body_of(f), true),
        _ => return Passivity::Unknown,
    };
    of_parts(params, body, arguments)
}

/// A function's passivity from its parameters and body; `arguments`: it has its own `arguments`.
fn of_parts(params: &[Pattern], body: NodeRef<'_>, arguments: bool) -> Passivity {
    // a function's `arguments` reaches the event whatever its parameters
    if arguments && uses_arguments(body) {
        return Passivity::Unknown;
    }
    let Some(first) = params.first() else { return Passivity::Passive };
    let first = match first {
        Pattern::AssignmentPattern(a) => &a.left,
        p => p,
    };
    match first {
        Pattern::Identifier(id) => uses_of(&id.name, body),
        Pattern::ObjectPattern(o) => {
            for m in &o.properties {
                match m {
                    ObjectPatternMember::Property(p) if !p.computed => {
                        if let PropertyKey::Expression(k) = &p.key
                            && let Expression::Identifier(k) = k.as_ref()
                        {
                            if CANCEL.contains(&k.name.as_str()) {
                                return Passivity::Cancels(k.span());
                            }
                            continue;
                        }
                        return Passivity::Unknown;
                    }
                    _ => return Passivity::Unknown,
                }
            }
            Passivity::Passive
        }
        _ => Passivity::Unknown,
    }
}

/// How the body uses the event parameter `name`.
fn uses_of(name: &str, body: NodeRef<'_>) -> Passivity {
    struct Uses<'n> {
        name: &'n str,
        result: Passivity,
    }
    impl<'a> Visitor<'a> for Uses<'_> {
        type State = ();
        fn visit(&mut self, node: NodeRef<'a>, s: &(), w: &mut Walker<'a>) {
            if matches!(self.result, Passivity::Cancels(_)) {
                return;
            }
            match node {
                // `event.x = v`, `event.x++`: a write (a sync result) can cancel
                NodeRef::AssignmentExpression(a) if member_of(pattern_member(&a.left), self.name) => {
                    self.result = Passivity::Cancels(a.span());
                }
                NodeRef::UpdateExpression(u) if member_of(expr_member(&u.argument), self.name) => {
                    self.result = Passivity::Cancels(u.span());
                }
                NodeRef::MemberExpression(m) if is_name(&m.object, self.name) => {
                    match (&m.property, m.computed) {
                        (PropertyKey::Expression(k), false) => {
                            if let Expression::Identifier(k) = k.as_ref()
                                && CANCEL.contains(&k.name.as_str())
                            {
                                self.result = Passivity::Cancels(k.span());
                            }
                        }
                        _ => self.result = Passivity::Unknown,
                    }
                    // the property is not a use; a computed key may use other names
                    if m.computed
                        && let PropertyKey::Expression(k) = &m.property
                    {
                        w.visit(self, node, expr_ref(k), s);
                    }
                }
                // a plain property name (`other.event`) is not a use of the event
                NodeRef::MemberExpression(m) if !m.computed => w.visit(self, node, expr_ref(&m.object), s),
                NodeRef::Identifier(id) if id.name == self.name => self.result = Passivity::Unknown,
                _ => w.next(self, node, s),
            }
        }
    }
    let mut u = Uses { name, result: Passivity::Passive };
    Walker::walk(body, &(), &mut u);
    u.result
}

fn pattern_member(p: &Pattern) -> Option<&MemberExpression> {
    match p {
        Pattern::MemberExpression(m) => Some(m),
        _ => None,
    }
}

fn expr_member(e: &Expression) -> Option<&MemberExpression> {
    match unparen(e) {
        Expression::MemberExpression(m) => Some(m),
        _ => None,
    }
}

fn member_of(m: Option<&MemberExpression>, name: &str) -> bool {
    m.is_some_and(|m| is_name(&m.object, name))
}

fn is_name(e: &Expression, name: &str) -> bool {
    matches!(unparen(e), Expression::Identifier(id) if id.name == name)
}

/// Whether a non-arrow function's body reads `arguments` (outside nested non-arrow functions).
fn uses_arguments(body: NodeRef<'_>) -> bool {
    struct A(bool);
    impl<'a> Visitor<'a> for A {
        type State = ();
        fn visit(&mut self, node: NodeRef<'a>, s: &(), w: &mut Walker<'a>) {
            match node {
                NodeRef::Identifier(id) if id.name == "arguments" => self.0 = true,
                NodeRef::FunctionExpression(_) | NodeRef::FunctionDeclaration(_) => {}
                _ => w.next(self, node, s),
            }
        }
    }
    let mut a = A(false);
    Walker::walk(body, &(), &mut a);
    a.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handler(src: &str) -> Expression {
        let mut comments = Vec::new();
        let program = crate::svelte::parse::acorn::parse(&format!("({src})"), &mut comments, false, false).expect("parses");
        match program.body.into_iter().next() {
            Some(Statement::ExpressionStatement(s)) => s.expression,
            _ => panic!("an expression"),
        }
    }

    fn ev(cancelable: bool) -> EventSpec {
        EventSpec {
            signal: "wheel".into(),
            owner: "Widget".into(),
            args: Vec::new(),
            handler_result: crate::semantics::HandlerResult::BoolStop,
            sync_required: false,
            disconnect_on_drop: true,
            sync: true,
            phases: Vec::new(),
            via_alias: None,
            cancelable,
            passive: true,
            id: None,
        }
    }

    fn of(src: &str) -> Passivity {
        let h = handler(src);
        infer(&Handler::Expr(&h), &ev(true), &HashMap::new())
    }

    #[test]
    fn handlers_that_cannot_cancel_are_passive() {
        assert_eq!(of("() => count++"), Passivity::Passive);
        assert_eq!(of("(e) => { x = e.dx; y = e.dy; }"), Passivity::Passive);
        assert_eq!(of("({ dx, dy }) => move(dx, dy)"), Passivity::Passive);
        assert_eq!(of("function (e) { log(e.target, other.e); }"), Passivity::Passive);
        let h = handler("(e) => e.preventDefault()");
        assert_eq!(infer(&Handler::Expr(&h), &ev(false), &HashMap::new()), Passivity::Passive, "a signal that cannot be canceled");
    }

    #[test]
    fn handlers_that_cancel_are_not() {
        assert!(matches!(of("(e) => e.preventDefault()"), Passivity::Cancels(_)));
        assert!(matches!(of("(e) => { if (x) e.stopPropagation(); }"), Passivity::Cancels(_)));
        assert!(matches!(of("(e) => { e.result = true; }"), Passivity::Cancels(_)));
        assert!(matches!(of("({ preventDefault }) => preventDefault()"), Passivity::Cancels(_)));
    }

    #[test]
    fn escaping_events_are_unknown() {
        assert_eq!(of("(e) => handle(e)"), Passivity::Unknown);
        assert_eq!(of("(e) => e[key]"), Passivity::Unknown);
        assert_eq!(of("(...args) => f(args)"), Passivity::Unknown);
        assert_eq!(of("function () { f(arguments[0]); }"), Passivity::Unknown);
        assert_eq!(of("onwheel"), Passivity::Unknown);
    }
}
