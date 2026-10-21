//! `native_read_not_observable` (milestone R2, REACTIVITY.md section 8.4), best effort: a reactive
//! read `obj.prop` of a non-GUI wrapper whose property is `none` (a snapshot: nothing re-runs) or
//! `partial` (misses some causes) on this platform.
//!
//! Static typing is deliberately narrow: `obj` is a top-level `const obj = new C(...)` of the instance
//! script with `C` imported from a `svelters:<tk>/<ns>` module the platform knows
//! ([`PlatformSemantics::native_type`]). A read counts when it is in a template expression (outside
//! event handlers and other functions) or in the body of `$derived(...)`, `$derived.by(...)`,
//! `$effect(...)` or `$effect.pre(...)`. Method calls, writes, and reads inside `untrack(...)` are
//! not reads. Unknown receivers, unknown props and props without a fact are silent. Not handled:
//! shadowing of `obj` by a nested binding of the same name, `bind:this` targets (element nodes read
//! natively through methods, which are snapshots by design) and callback parameters.

use crate::diag::Diagnostics;
use crate::estree::{ArrayElement, Expression, ImportSpecifierKind, LiteralNode, LiteralValue, ModuleExportName, Pattern, PropertyKey, Spanned, Statement, VariableKind};
use crate::semantics::{PlatformSemantics, TypeRef, codes, spec::ObsClass};
use crate::svelte::ast::Root;
use crate::svelte::scope::walk::{NodeRef, array_element_ref, expr_ref, fragment_node_ref};
use std::collections::HashMap;

pub fn check(root: &Root, sem: &dyn PlatformSemantics, diags: &mut Diagnostics) {
    let Some(script) = &root.instance else { return };
    // local import name -> (module, export)
    let mut imports: HashMap<String, (String, String)> = HashMap::new();
    let mut typed: HashMap<String, TypeRef> = HashMap::new();
    for stmt in &script.content.body {
        match stmt {
            Statement::ImportDeclaration(d) => {
                let LiteralNode::Literal(src) = &d.source;
                let LiteralValue::String(module) = &src.value else { continue };
                if !module.starts_with("svelters:") {
                    continue;
                }
                for sp in &d.specifiers {
                    if let ImportSpecifierKind::ImportSpecifier(is) = sp {
                        let imported = match &is.imported {
                            ModuleExportName::Identifier(i) => i.name.clone(),
                            ModuleExportName::Literal(l) => match &l.value {
                                LiteralValue::String(s) => s.clone(),
                                _ => continue,
                            },
                        };
                        imports.insert(is.local.name.clone(), (module.clone(), imported));
                    }
                }
            }
            Statement::VariableDeclaration(v) if v.kind == VariableKind::Const => {
                for d in &v.declarations {
                    if let (Pattern::Identifier(id), Some(Expression::NewExpression(n))) = (&d.id, &d.init)
                        && let Expression::Identifier(c) = n.callee.as_ref()
                        && let Some((m, e)) = imports.get(&c.name)
                        && let Some(t) = sem.native_type(m, e)
                    {
                        typed.insert(id.name.clone(), t);
                    }
                }
            }
            _ => {}
        }
    }
    if typed.is_empty() {
        return;
    }
    let mut w = Reads { sem, typed: &typed, diags };
    for n in &root.fragment.nodes {
        w.walk(fragment_node_ref(n), true);
    }
    w.walk(NodeRef::Program(&script.content), false);
}

struct Reads<'s, 'd> {
    sem: &'s dyn PlatformSemantics,
    typed: &'s HashMap<String, TypeRef>,
    diags: &'d mut Diagnostics,
}

fn rune(callee: &Expression) -> bool {
    match callee {
        Expression::Identifier(i) => matches!(i.name.as_str(), "$derived" | "$effect"),
        Expression::MemberExpression(m) if !m.computed => {
            let obj = matches!(m.object.as_ref(), Expression::Identifier(i) if matches!(i.name.as_str(), "$derived" | "$effect"));
            let prop = matches!(&m.property, PropertyKey::Expression(e) if matches!(e.as_ref(), Expression::Identifier(p) if matches!(p.name.as_str(), "by" | "pre")));
            obj && prop
        }
        _ => false,
    }
}

impl Reads<'_, '_> {
    fn walk(&mut self, node: NodeRef<'_>, reactive: bool) {
        match node {
            // a function is not run by the render: handlers, callbacks
            NodeRef::ArrowFunctionExpression(_) | NodeRef::FunctionExpression(_) | NodeRef::FunctionDeclaration(_) => {
                for c in node.children() {
                    self.walk(c, false);
                }
            }
            NodeRef::CallExpression(c) => {
                if matches!(c.callee.as_ref(), Expression::Identifier(i) if i.name == "untrack") {
                    return;
                }
                if rune(&c.callee) {
                    for a in &c.arguments {
                        match a {
                            ArrayElement::Expression(Expression::ArrowFunctionExpression(f)) => {
                                for ch in NodeRef::ArrowFunctionExpression(f).children() {
                                    self.walk(ch, true);
                                }
                            }
                            ArrayElement::Expression(Expression::FunctionExpression(f)) => {
                                for ch in NodeRef::FunctionExpression(f).children() {
                                    self.walk(ch, true);
                                }
                            }
                            other => self.walk(array_element_ref(other), true),
                        }
                    }
                    return;
                }
                // `obj.method(...)`: a call, not a property read; its receiver and args still count
                match c.callee.as_ref() {
                    Expression::MemberExpression(m) => self.walk(expr_ref(&m.object), reactive),
                    other => self.walk(expr_ref(other), reactive),
                }
                for a in &c.arguments {
                    self.walk(array_element_ref(a), reactive);
                }
            }
            NodeRef::AssignmentExpression(a) => {
                // the target is written, not read (a compound assignment reads it too, but writes win)
                if !matches!(&a.left, Pattern::MemberExpression(_)) {
                    for c in node.children() {
                        self.walk(c, reactive);
                    }
                } else {
                    self.walk(expr_ref(&a.right), reactive);
                }
            }
            NodeRef::UpdateExpression(_) => {}
            NodeRef::MemberExpression(m) => {
                if reactive
                    && !m.computed
                    && let Expression::Identifier(o) = m.object.as_ref()
                    && let Some(ty) = self.typed.get(&o.name)
                    && let PropertyKey::Expression(p) = &m.property
                    && let Expression::Identifier(p) = p.as_ref()
                {
                    self.report(&o.name, &p.name, ty, (m.span().start, m.span().end));
                }
                for c in node.children() {
                    self.walk(c, reactive);
                }
            }
            _ => {
                for c in node.children() {
                    self.walk(c, reactive);
                }
            }
        }
    }

    fn report(&mut self, obj: &str, prop: &str, ty: &TypeRef, span: (u32, u32)) {
        let Some(tp) = self.sem.type_property(ty, prop) else { return };
        let Some(o) = tp.observability else { return };
        let platform = self.sem.backend();
        let class = ty.key.rsplit(':').next().unwrap_or(&ty.key);
        let msg = match o.class {
            ObsClass::None => format!(
                "`{obj}.{prop}` reads `{}` of {class}, whose native changes are not announced on {platform}: the read is a snapshot and nothing re-runs when it changes",
                tp.native
            ),
            ObsClass::Partial => format!(
                "`{obj}.{prop}` reads `{}` of {class}, which is only partially observed on {platform}: {} are not announced",
                tp.native,
                o.missed_text()
            ),
            _ => return,
        };
        self.diags.native_warning(codes::NATIVE_READ_NOT_OBSERVABLE, span, msg).help("read it where it is needed (an event handler), or mark the snapshot with `untrack(() => ...)`");
    }
}
