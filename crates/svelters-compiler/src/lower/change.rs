//! W-C4: change classes (presto `compiler-and-js.md` §3.4).
//!
//! Tags each dynamic prop write with its prop's `change` class from the spec, each `class:`
//! toggle (and dynamic class set) with the costliest class of what the component's rules for
//! those classes declare, and each element's `style:` directives with the costliest class of their
//! properties. A class is left unknown (`None`) when any property involved has none. Codegen
//! passes the class to the runtime for a backend with `change_classes`.
//!
//! In a dev build, `layout_write_per_frame` warns about a layout-class write whose value reads a
//! variable that a handler of a per-frame signal (the spec's `passive` signals: wheel, pointer
//! moves) assigns: every such event then relayouts; a `transform` would only repaint.

use super::ir::*;
use crate::diag::Diagnostics;
use crate::estree::*;
use crate::semantics::{ChangeClass, PlatformSemantics};
use crate::svelte::css::ast::{Node, SimpleKind, SimpleSelector, SelectorList};
use crate::svelte::scope::walk::{NodeRef, Visitor, Walker, expr_ref};
use std::collections::{HashMap, HashSet};

/// Tags the writes of `f` (and every fragment inside it); `dev`: also warn.
pub fn tag(f: &mut Fragment, sem: &dyn PlatformSemantics, src: &str, root: &crate::svelte::ast::Root, diags: &mut Diagnostics, dev: bool) {
    let classes = class_changes(sem, src, root);
    let mut per_frame = PerFrame::default();
    if dev {
        per_frame.collect(f, sem);
    }
    let mut cx = Cx { sem, classes, per_frame, diags, dev };
    cx.fragment(f);
}

/// The change class each class of the component's `<style>` can cause: the costliest class of
/// the declarations of every rule whose selector names it (None: a declaration of unknown class).
fn class_changes(sem: &dyn PlatformSemantics, src: &str, root: &crate::svelte::ast::Root) -> HashMap<String, Option<ChangeClass>> {
    let mut out: HashMap<String, Option<ChangeClass>> = HashMap::new();
    let Ok(Some(sheet)) = crate::svelte::css::parse_style(src, root) else { return out };
    fn rules(nodes: &[Node], sem: &dyn PlatformSemantics, out: &mut HashMap<String, Option<ChangeClass>>) {
        for n in nodes {
            match n {
                Node::Rule(r) => {
                    let mut names = Vec::new();
                    class_names(&r.prelude, &mut names);
                    let mut change: Option<Option<ChangeClass>> = None;
                    for d in r.block.children.iter() {
                        if let Node::Declaration(d) = d {
                            let c = sem.style_property(&d.property).and_then(|p| p.change);
                            change = Some(match (change, c) {
                                (None, c) => c,
                                (Some(Some(a)), Some(b)) => Some(a.max(b)),
                                _ => None,
                            });
                        }
                    }
                    if let Some(c) = change {
                        for name in names {
                            let e = out.entry(name).or_insert(c);
                            *e = match (*e, c) {
                                (Some(a), Some(b)) => Some(a.max(b)),
                                _ => None,
                            };
                        }
                    }
                    rules(&r.block.children, sem, out);
                }
                Node::Atrule(a) => {
                    if let Some(b) = &a.block {
                        rules(&b.children, sem, out);
                    }
                }
                Node::Declaration(_) => {}
            }
        }
    }
    rules(&sheet.children, sem, &mut out);
    out
}

fn class_names(list: &SelectorList, out: &mut Vec<String>) {
    for c in &list.children {
        for r in &c.children {
            for s in &r.selectors {
                simple(s, out);
            }
        }
    }
}

fn simple(s: &SimpleSelector, out: &mut Vec<String>) {
    if s.kind == SimpleKind::ClassSelector {
        out.push(s.name.clone());
    }
    if let Some(args) = &s.args {
        class_names(args, out);
    }
}

/// The variables a per-frame handler assigns, with the signal (for the message).
#[derive(Default)]
struct PerFrame {
    assigned: HashMap<String, String>,
}

impl PerFrame {
    fn collect(&mut self, f: &Fragment, sem: &dyn PlatformSemantics) {
        each_element(f, &mut |e| {
            let Ok(el) = sem.resolve_element(&e.kind) else { return };
            for op in &e.ops {
                if let Op::Listen { signal, handler: Handler::Expr(h), .. } = op
                    && sem.event(&el, signal).is_ok_and(|ev| ev.passive)
                {
                    let mut names = HashSet::new();
                    assigned(h, &mut names);
                    for n in names {
                        self.assigned.entry(n).or_insert_with(|| signal.clone());
                    }
                }
            }
        });
    }
}

/// Every element of `f`, depth first (also inside blocks, snippets, components and templates).
pub(super) fn each_element<'f>(f: &'f Fragment<'f>, visit: &mut dyn FnMut(&'f Element<'f>)) {
    for s in &f.snippets {
        each_element(&s.body, visit);
    }
    for item in &f.items {
        match item {
            Item::Element(e) => {
                visit(e);
                each_element(&e.children, visit);
            }
            Item::Template(t) => {
                visit(&t.root);
                each_element(&t.root.children, visit);
            }
            Item::If(b) => {
                each_element(&b.consequent, visit);
                if let Some(a) = &b.alternate {
                    each_element(a, visit);
                }
            }
            Item::Each(b) => {
                each_element(&b.body, visit);
                if let Some(fb) = &b.fallback {
                    each_element(fb, visit);
                }
            }
            Item::Key(b) => each_element(&b.body, visit),
            Item::Await(b) => {
                for fr in [&b.pending, &b.then, &b.catch].into_iter().flatten() {
                    each_element(fr, visit);
                }
            }
            Item::Component(c) => {
                for s in &c.snippets {
                    each_element(&s.body, visit);
                }
                if let Some(ch) = &c.children {
                    each_element(ch, visit);
                }
            }
            Item::Dynamic(d) => each_element(&d.children, visit),
            Item::Render(_) => {}
        }
    }
}

/// The variables an expression (a handler) assigns: `x = ...`, `x++`, `x.y = ...` (`x`).
fn assigned(e: &Expression, out: &mut HashSet<String>) {
    struct A<'o>(&'o mut HashSet<String>);
    impl<'a> Visitor<'a> for A<'_> {
        type State = ();
        fn visit(&mut self, node: NodeRef<'a>, s: &(), w: &mut Walker<'a>) {
            match node {
                NodeRef::AssignmentExpression(a) => {
                    if let Some(n) = pattern_root(&a.left) {
                        self.0.insert(n);
                    }
                }
                NodeRef::UpdateExpression(u) => {
                    if let Some(n) = expr_root(&u.argument) {
                        self.0.insert(n);
                    }
                }
                _ => {}
            }
            w.next(self, node, s);
        }
    }
    Walker::walk(expr_ref(e), &(), &mut A(out));
}

fn pattern_root(p: &Pattern) -> Option<String> {
    match p {
        Pattern::Identifier(id) => Some(id.name.clone()),
        Pattern::MemberExpression(m) => expr_root(&m.object),
        _ => None,
    }
}

fn expr_root(e: &Expression) -> Option<String> {
    match e {
        Expression::Identifier(id) => Some(id.name.clone()),
        Expression::MemberExpression(m) => expr_root(&m.object),
        Expression::ParenthesizedExpression(p) => expr_root(&p.expression),
        _ => None,
    }
}

/// The identifiers an expression reads (not the names of non-computed members).
fn reads(e: &Expression, out: &mut HashSet<String>) {
    struct R<'o>(&'o mut HashSet<String>);
    impl<'a> Visitor<'a> for R<'_> {
        type State = ();
        fn visit(&mut self, node: NodeRef<'a>, s: &(), w: &mut Walker<'a>) {
            match node {
                NodeRef::Identifier(id) => {
                    self.0.insert(id.name.clone());
                }
                NodeRef::MemberExpression(m) if !m.computed => w.visit(self, node, expr_ref(&m.object), s),
                _ => w.next(self, node, s),
            }
        }
    }
    Walker::walk(expr_ref(e), &(), &mut R(out));
}

fn dynamic_reads(d: &Dynamic, out: &mut HashSet<String>) {
    fn parts(ps: &[TemplatePart], out: &mut HashSet<String>) {
        for p in ps {
            match p {
                TemplatePart::Text(_) => {}
                TemplatePart::Expr(e) => reads(e, out),
                TemplatePart::If { test, consequent, alternate } => {
                    reads(test, out);
                    parts(consequent, out);
                    parts(alternate, out);
                }
            }
        }
    }
    match d {
        Dynamic::Expr(e) => reads(e, out),
        Dynamic::Template(ps) => parts(ps, out),
        Dynamic::Member { object, .. } => reads(object, out),
    }
}

struct Cx<'s, 'd> {
    sem: &'s dyn PlatformSemantics,
    classes: HashMap<String, Option<ChangeClass>>,
    per_frame: PerFrame,
    diags: &'d mut Diagnostics,
    dev: bool,
}

/// The costliest of some classes; None when any is unknown (or there are none).
fn costliest(cs: impl IntoIterator<Item = Option<ChangeClass>>) -> Option<ChangeClass> {
    let mut out = None;
    for c in cs {
        let c = c?;
        out = Some(out.map_or(c, |o: ChangeClass| o.max(c)));
    }
    out
}

impl Cx<'_, '_> {
    fn fragment(&mut self, f: &mut Fragment) {
        for s in &mut f.snippets {
            self.fragment(&mut s.body);
        }
        for item in &mut f.items {
            match item {
                Item::Element(e) => self.element(e),
                Item::Template(t) => self.element(&mut t.root),
                Item::If(b) => {
                    self.fragment(&mut b.consequent);
                    if let Some(a) = &mut b.alternate {
                        self.fragment(a);
                    }
                }
                Item::Each(b) => {
                    self.fragment(&mut b.body);
                    if let Some(fb) = &mut b.fallback {
                        self.fragment(fb);
                    }
                }
                Item::Key(b) => self.fragment(&mut b.body),
                Item::Await(b) => {
                    for fr in [&mut b.pending, &mut b.then, &mut b.catch].into_iter().flatten() {
                        self.fragment(fr);
                    }
                }
                Item::Component(c) => {
                    for s in &mut c.snippets {
                        self.fragment(&mut s.body);
                    }
                    if let Some(ch) = &mut c.children {
                        self.fragment(ch);
                    }
                }
                Item::Dynamic(d) => self.fragment(&mut d.children),
                Item::Render(_) => {}
            }
        }
    }

    fn element(&mut self, e: &mut Element) {
        if let Ok(el) = self.sem.resolve_element(&e.kind) {
            for op in &mut e.ops {
                match op {
                    Op::SetDynamic { prop, value, change, span } => {
                        *change = self.sem.property(&el, prop).ok().and_then(|p| p.change);
                        if *change == Some(ChangeClass::Layout) {
                            let mut r = HashSet::new();
                            dynamic_reads(value, &mut r);
                            self.warn(&r, &format!("`{prop}` of <{}>", e.kind), *span);
                        }
                    }
                    Op::Classes { dynamic, toggles, change, span, .. } => {
                        *change = if dynamic.is_some() {
                            // any class of the sheet can come and go
                            costliest(self.classes.values().copied())
                        } else {
                            costliest(toggles.iter().map(|(name, _)| self.classes.get(name).copied().flatten()))
                        };
                        for (name, x) in toggles.iter() {
                            if self.classes.get(name).copied().flatten() == Some(ChangeClass::Layout) {
                                let mut r = HashSet::new();
                                reads(x, &mut r);
                                self.warn(&r, &format!("`class:{name}` of <{}> (its rules set layout properties)", e.kind), *span);
                            }
                        }
                    }
                    Op::InlineStyle { dynamic: None, directives, change, span, .. } if !directives.is_empty() => {
                        *change = costliest(directives.iter().map(|(name, _)| self.sem.style_property(name).and_then(|p| p.change)));
                        for (name, v) in directives.iter() {
                            if let AttrValue::Dynamic(d) = v
                                && self.sem.style_property(name).and_then(|p| p.change) == Some(ChangeClass::Layout)
                            {
                                let mut r = HashSet::new();
                                dynamic_reads(d, &mut r);
                                self.warn(&r, &format!("`style:{name}` of <{}>", e.kind), *span);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        self.fragment(&mut e.children);
    }

    /// `layout_write_per_frame` when `read` holds a variable a per-frame handler assigns.
    fn warn(&mut self, read: &HashSet<String>, what: &str, span: Span) {
        if !self.dev {
            return;
        }
        let mut hit: Vec<(&String, &String)> = self.per_frame.assigned.iter().filter(|(n, _)| read.contains(*n)).collect();
        hit.sort();
        if let Some((name, signal)) = hit.first() {
            self.diags
                .native_warning(
                    "layout_write_per_frame",
                    (span.start, span.end),
                    format!("{what} is a layout property driven by `{name}`, which an `on{signal}` handler sets on every event: each event relayouts"),
                )
                .help("move the element with `style:transform` (translate) instead, which only repaints");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn costliest_is_unknown_when_any_is() {
        assert_eq!(costliest([Some(ChangeClass::Paint), Some(ChangeClass::Layout)]), Some(ChangeClass::Layout));
        assert_eq!(costliest([Some(ChangeClass::Paint), None]), None);
        assert_eq!(costliest([]), None);
    }

    #[test]
    fn handlers_assign_their_roots() {
        let mut comments = Vec::new();
        let p = crate::svelte::parse::acorn::parse("((e) => { x = e.x; pos.y = e.y; n++; })", &mut comments, false, false).unwrap();
        let Some(Statement::ExpressionStatement(s)) = p.body.first() else { panic!() };
        let mut out = HashSet::new();
        assigned(&s.expression, &mut out);
        let mut v: Vec<_> = out.into_iter().collect();
        v.sort();
        assert_eq!(v, ["n", "pos", "x"]);
    }
}
