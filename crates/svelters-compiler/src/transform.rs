//! Rune and reactivity transforms as span edits over the user's JavaScript, driven by the ported
//! scope analysis (`svelte::scope`). Implements [`codegen::component::Rewriter`].
//!
//! Supported subset (docs/notes/native-compiler/language-subset.md):
//! - `$state(v)` / `$state.raw(v)` declarations -> `$.state(...)` (objects/arrays proxied as
//!   Svelte does: `$.proxy(v)` when `should_proxy`); reads `$.get(x)`; `x = v` -> `$.set(x, v)`;
//!   compound assignment `x op= v` -> `$.set(x, $.get(x) op (v))`; `x++`/`--x` -> `$.update(x)` /
//!   `$.update_pre(x, -1)`.
//! - `$derived(e)` -> `$.derived(() => e)`, `$derived.by(f)` -> `$.derived(f)`; reads `$.get(d)`.
//! - `let { a, b = 1, c = $bindable(), ...rest } = $props()` -> `$.prop($$props, 'a', flags, fallback)`
//!   getters (reads `a()`, writes `a(v)`), `$.rest_props`.
//! - `$effect`, `$effect.pre`, `$effect.root`, `$effect.tracking`, `$state.snapshot`.
//! - each-block item/index: runtime sources, read with `$.get(...)` (plain values in the key fn).
//!
//! Deliberate simplifications (documented as departures): every `$state` gets a source even when
//! never reassigned (Svelte inlines those); destructuring assignments to state, `$inspect`,
//! `$props.id`, `$host`, stores and module scripts are diagnostics.

use crate::codegen::component::{Mode, Rewriter};
use crate::codegen::edit::Edits;
use crate::codegen::writer::Writer;
use crate::diag::Diagnostics;
use crate::estree::*;
use crate::svelte::ast::Root;
use crate::svelte::scope::analyze::ComponentScopeAnalysis;
use crate::svelte::scope::walk::{NodeRef, Visitor, Walker, expr_ref, stmt_ref};
use crate::svelte::scope::{BindingKind, Initial, ScopeArena, ScopeId, get_rune_of_call, should_proxy};
use std::cell::RefCell;
use std::collections::HashSet;

const PROPS_IS_RUNES: u32 = 1 << 1;
const PROPS_IS_UPDATED: u32 = 1 << 2;
const PROPS_IS_BINDABLE: u32 = 1 << 3;
const PROPS_IS_LAZY_INITIAL: u32 = 1 << 4;

pub struct ScopeRewriter<'a> {
    src: &'a str,
    root: &'a Root,
    an: &'a ComponentScopeAnalysis<'a>,
    script: Edits,
    pub diags: RefCell<Diagnostics>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    /// `$state` / `$state.raw` / `$derived`: a source read with `$.get`.
    Source { proxied: bool, writable: bool },
    /// `$props()` member: a getter/setter function.
    Prop,
    /// each-block item / index: a runtime source, plain in the key function.
    EachSource,
    /// Snippet parameter: a thunk, read `x()`, not assignable.
    Thunk,
    Other,
}

fn classify(arena: &ScopeArena<'_>, id: &Identifier) -> Class {
    let Some(b) = arena.resolve(id) else { return Class::Other };
    let b = arena.binding(b);
    match b.kind {
        BindingKind::State => Class::Source { proxied: true, writable: true },
        BindingKind::RawState => Class::Source { proxied: false, writable: true },
        BindingKind::Derived => Class::Source { proxied: false, writable: false },
        BindingKind::Prop | BindingKind::BindableProp => Class::Prop,
        BindingKind::Each => Class::EachSource,
        BindingKind::Template | BindingKind::Static if matches!(b.initial, Some(Initial::EachBlock(_))) => Class::EachSource,
        // `{@const x}` (a derived) and `{:then v}` / `{:catch e}` (sources): read `$.get(x)`
        BindingKind::Template => Class::Source { proxied: false, writable: false },
        BindingKind::Snippet => Class::Thunk,
        _ => Class::Other,
    }
}

impl<'a> ScopeRewriter<'a> {
    pub fn new(src: &'a str, root: &'a Root, an: &'a ComponentScopeAnalysis<'a>, diags: Diagnostics) -> ScopeRewriter<'a> {
        let mut rw = ScopeRewriter { src, root, an, script: Edits::new(), diags: RefCell::new(diags) };
        if let Some(script) = &root.instance {
            let mut edits = Edits::new();
            let mut v = Rw::new(rw.an, rw.src, &mut edits, Mode::Read, &rw.diags, Some(an.instance.scope));
            for stmt in &script.content.body {
                match stmt {
                    Statement::ImportDeclaration(_) => {}
                    Statement::ExportNamedDeclaration(_) | Statement::ExportDefaultDeclaration(_) | Statement::ExportAllDeclaration(_) => {
                        let s = stmt.span();
                        rw.diags.borrow_mut().error("unsupported_feature", (s.start, s.end), "exports from the instance script are not supported by the native compiler yet");
                    }
                    Statement::VariableDeclaration(d) => v.declaration(d),
                    _ => Walker::walk(stmt_ref(stmt), &(), &mut v),
                }
            }
            rw.script = edits;
        }
        rw
    }

    pub fn into_diagnostics(self) -> Diagnostics {
        self.diags.into_inner()
    }
}

impl Rewriter for ScopeRewriter<'_> {
    fn expr(&self, e: &Expression, mode: Mode, edits: &mut Edits) {
        let mut v = Rw::new(self.an, self.src, edits, mode, &self.diags, Some(self.an.instance.scope));
        Walker::walk(expr_ref(e), &(), &mut v);
    }

    fn is_reactive(&self, e: &Expression) -> bool {
        struct R<'x, 'a> {
            arena: &'x ScopeArena<'a>,
            found: bool,
        }
        impl<'n> Visitor<'n> for R<'_, '_> {
            type State = ();
            fn visit(&mut self, node: NodeRef<'n>, state: &(), w: &mut Walker<'n>) {
                match node {
                    NodeRef::Identifier(id) if classify(self.arena, id) != Class::Other => self.found = true,
                    NodeRef::CallExpression(_) | NodeRef::NewExpression(_) | NodeRef::TaggedTemplateExpression(_) => self.found = true,
                    _ => {}
                }
                if !self.found {
                    w.next(self, node, state);
                }
            }
        }
        let mut r = R { arena: &self.an.arena, found: false };
        Walker::walk(expr_ref(e), &(), &mut r);
        r.found
    }

    fn assign(&self, target: &Expression, value: &str) -> String {
        match target {
            Expression::Identifier(id) => match classify(&self.an.arena, id) {
                // never proxied: bound values are native values or runtime nodes (Svelte does the same)
                Class::Source { writable: true, .. } => format!("$.set({}, {value})", id.name),
                Class::Prop => format!("{}({value})", id.name),
                Class::Source { writable: false, .. } | Class::EachSource | Class::Thunk => {
                    self.diags.borrow_mut().error("bind_invalid_target", (id.start, id.end), format!("cannot bind to `{}`: it is not assignable", id.name));
                    "void 0".to_string()
                }
                Class::Other => format!("{} = {value}", id.name),
            },
            _ => {
                let mut edits = Edits::new();
                self.expr(target, Mode::Read, &mut edits);
                let s = target.span();
                format!("{} = {value}", edits.apply(self.src, s.start, s.end))
            }
        }
    }

    fn script(&self, w: &mut Writer, hoist: bool) {
        let Some(script) = &self.root.instance else { return };
        for stmt in &script.content.body {
            let s = stmt.span();
            match (hoist, stmt) {
                (true, Statement::ImportDeclaration(_)) => {
                    w.copy(self.src, s.start, s.end);
                    w.write("\n");
                }
                (false, Statement::ImportDeclaration(_)) | (true, _) => {}
                (false, Statement::ExportNamedDeclaration(_) | Statement::ExportDefaultDeclaration(_) | Statement::ExportAllDeclaration(_)) => {}
                (false, _) => {
                    w.newline();
                    self.script.emit(w, self.src, s.start, s.end);
                }
            }
        }
    }
}

/// The edit-producing visitor shared by the script and template expressions.
struct Rw<'r, 'a> {
    an: &'r ComponentScopeAnalysis<'a>,
    src: &'r str,
    edits: &'r mut Edits,
    mode: Mode,
    diags: &'r RefCell<Diagnostics>,
    scope: Option<ScopeId>,
    handled: HashSet<usize>,
}

impl<'r, 'a> Rw<'r, 'a> {
    fn new(an: &'r ComponentScopeAnalysis<'a>, src: &'r str, edits: &'r mut Edits, mode: Mode, diags: &'r RefCell<Diagnostics>, scope: Option<ScopeId>) -> Self {
        Rw { an, src, edits, mode, diags, scope, handled: HashSet::new() }
    }

    fn arena(&self) -> &ScopeArena<'a> {
        &self.an.arena
    }

    fn text(&self, s: Span) -> &str {
        &self.src[s.start as usize..s.end as usize]
    }

    fn rune(&self, call: &CallExpression) -> Option<&'static str> {
        get_rune_of_call(self.arena(), call, self.scope?)
    }

    /// A top-level `let/const` in the instance script: rune declarations, then ordinary visiting.
    fn declaration(&mut self, d: &VariableDeclaration) {
        for decl in &d.declarations {
            let Some(Expression::CallExpression(call)) = &decl.init else {
                self.walk_declarator(decl);
                continue;
            };
            match self.rune(call) {
                Some("$state") | Some("$state.raw") => {
                    let raw = self.rune(call) == Some("$state.raw");
                    self.edits.overwrite(call.callee.span().start, call.callee.span().end, "$.state");
                    if let Some(arg) = call.arguments.first() {
                        if let ArrayElement::Expression(a) = arg {
                            if !raw && should_proxy(self.arena(), a, self.scope) {
                                let s = a.span();
                                self.edits.wrap(s.start, s.end, "$.proxy(", ")");
                            }
                            Walker::walk(expr_ref(a), &(), self);
                        }
                    }
                    if !matches!(decl.id, Pattern::Identifier(_)) {
                        let s = decl.id.span();
                        self.diags.borrow_mut().error("unsupported_feature", (s.start, s.end), "destructuring a $state declaration is not supported by the native compiler yet");
                    }
                }
                Some("$derived") => {
                    self.edits.overwrite(call.callee.span().start, call.callee.span().end, "$.derived");
                    if let Some(ArrayElement::Expression(a)) = call.arguments.first() {
                        let s = a.span();
                        self.edits.wrap(s.start, s.end, "() => (", ")");
                        Walker::walk(expr_ref(a), &(), self);
                    }
                }
                Some("$derived.by") => {
                    self.edits.overwrite(call.callee.span().start, call.callee.span().end, "$.derived");
                    for a in &call.arguments {
                        if let ArrayElement::Expression(a) = a {
                            Walker::walk(expr_ref(a), &(), self);
                        }
                    }
                }
                Some("$props") => self.props(decl),
                Some(r) => {
                    let s = call.span();
                    self.diags.borrow_mut().error("unsupported_feature", (s.start, s.end), format!("{r} is not supported by the native compiler yet"));
                }
                None => self.walk_declarator(decl),
            }
        }
    }

    fn walk_declarator(&mut self, decl: &VariableDeclarator) {
        if let Some(init) = &decl.init {
            Walker::walk(expr_ref(init), &(), self);
        }
    }

    /// `let { a, b = 1, c = $bindable(x), ...rest } = $props()`.
    fn props(&mut self, decl: &VariableDeclarator) {
        let full = Span::new(decl.start, decl.end);
        let out = match &decl.id {
            Pattern::Identifier(id) => format!("{} = $$props", id.name),
            Pattern::ObjectPattern(op) => {
                let mut parts = Vec::new();
                let mut seen = Vec::new();
                for m in &op.properties {
                    match m {
                        ObjectPatternMember::Property(p) => {
                            let key = match &p.key {
                                PropertyKey::Expression(k) => match k.as_ref() {
                                    Expression::Identifier(i) => i.name.clone(),
                                    Expression::Literal(Literal { value: LiteralValue::String(s), .. }) => s.clone(),
                                    other => {
                                        let s = other.span();
                                        self.diags.borrow_mut().error("unsupported_feature", (s.start, s.end), "computed $props() keys are not supported");
                                        continue;
                                    }
                                },
                                PropertyKey::Private(_) => continue,
                            };
                            seen.push(key.clone());
                            let (local, default) = match &p.value {
                                Pattern::Identifier(i) => (i, None),
                                Pattern::AssignmentPattern(ap) => match ap.left.as_ref() {
                                    Pattern::Identifier(i) => (i, Some(ap.right.as_ref())),
                                    other => {
                                        let s = other.span();
                                        self.diags.borrow_mut().error("unsupported_feature", (s.start, s.end), "nested destructuring in $props() is not supported");
                                        continue;
                                    }
                                },
                                other => {
                                    let s = other.span();
                                    self.diags.borrow_mut().error("unsupported_feature", (s.start, s.end), "nested destructuring in $props() is not supported");
                                    continue;
                                }
                            };
                            let binding = self.arena().bindings().find(|(_, b)| b.node.is(local)).map(|(id, _)| id);
                            let mut flags = PROPS_IS_RUNES;
                            let mut fallback = None;
                            let mut bindable = false;
                            if let Some(d) = default {
                                let d = match d {
                                    Expression::CallExpression(c) if self.rune(c) == Some("$bindable") => {
                                        bindable = true;
                                        c.arguments.first().and_then(|a| match a {
                                            ArrayElement::Expression(e) => Some(e),
                                            _ => None,
                                        })
                                    }
                                    other => Some(other),
                                };
                                if let Some(d) = d {
                                    let t = self.text(d.span()).to_string();
                                    let simple = matches!(d, Expression::Literal(_) | Expression::Identifier(_));
                                    if simple {
                                        fallback = Some(t);
                                    } else {
                                        flags |= PROPS_IS_LAZY_INITIAL;
                                        fallback = Some(format!("() => ({t})"));
                                    }
                                }
                            }
                            if bindable || binding.is_some_and(|b| self.arena().binding(b).kind == BindingKind::BindableProp) {
                                flags |= PROPS_IS_BINDABLE;
                            }
                            if binding.is_some_and(|b| self.arena().binding(b).updated()) {
                                flags |= PROPS_IS_UPDATED;
                            }
                            let fb = fallback.map(|f| format!(", {f}")).unwrap_or_default();
                            parts.push(format!("{} = $.prop($$props, {}, {flags}{fb})", local.name, crate::codegen::component::js_str(&key)));
                        }
                        ObjectPatternMember::RestElement(r) => match r.argument.as_ref() {
                            Pattern::Identifier(i) => {
                                let keys: Vec<String> = seen.iter().map(|k| crate::codegen::component::js_str(k)).collect();
                                parts.push(format!("{} = $.rest_props($$props, [{}])", i.name, keys.join(", ")));
                            }
                            other => {
                                let s = other.span();
                                self.diags.borrow_mut().error("unsupported_feature", (s.start, s.end), "unsupported rest pattern in $props()");
                            }
                        },
                    }
                }
                parts.join(", ")
            }
            other => {
                let s = other.span();
                self.diags.borrow_mut().error("unsupported_feature", (s.start, s.end), "unsupported $props() pattern");
                return;
            }
        };
        self.edits.overwrite(full.start, full.end, out);
    }

    fn assignment(&mut self, a: &AssignmentExpression) -> bool {
        let Pattern::Identifier(id) = &a.left else {
            if pattern_has_reactive(self.arena(), &a.left) {
                let s = a.left.span();
                self.diags.borrow_mut().error("unsupported_feature", (s.start, s.end), "destructuring assignment to reactive state is not supported by the native compiler yet");
            }
            return false;
        };
        let class = classify(self.arena(), id);
        let op = a.operator;
        let right = a.right.span();
        let bin = match op {
            AssignmentOperator::Assign => None,
            other => Some(&other.as_str()[..other.as_str().len() - 1]),
        };
        match class {
            Class::Source { writable: true, proxied } => {
                let n = &id.name;
                let proxy = proxied && op == AssignmentOperator::Assign && should_proxy(self.arena(), &a.right, self.scope);
                let prefix = match bin {
                    None => format!("$.set({n}, "),
                    Some(b) => format!("$.set({n}, $.get({n}) {b} ("),
                };
                self.edits.overwrite(a.start, right.start, prefix);
                let suffix = format!("{}{})", if bin.is_some() { ")" } else { "" }, if proxy { ", true" } else { "" });
                self.edits.close(a.end, suffix);
            }
            Class::Prop => {
                let n = &id.name;
                let prefix = match bin {
                    None => format!("{n}("),
                    Some(b) => format!("{n}({n}() {b} ("),
                };
                self.edits.overwrite(a.start, right.start, prefix);
                self.edits.close(a.end, if bin.is_some() { "))" } else { ")" });
            }
            Class::Source { writable: false, .. } | Class::EachSource | Class::Thunk => {
                self.diags.borrow_mut().error("constant_assignment", (id.start, id.end), format!("cannot assign to `{}`", id.name));
            }
            Class::Other => return false,
        }
        self.handled.insert(id as *const Identifier as usize);
        true
    }

    fn update(&mut self, u: &UpdateExpression) -> bool {
        let Expression::Identifier(id) = u.argument.as_ref() else { return false };
        let class = classify(self.arena(), id);
        let delta = if u.operator == UpdateOperator::Decrement { ", -1" } else { "" };
        let text = match class {
            Class::Source { writable: true, .. } => format!("$.update{}({}{delta})", if u.prefix { "_pre" } else { "" }, id.name),
            Class::Prop => format!("$.update{}_prop({}{delta})", if u.prefix { "_pre" } else { "" }, id.name),
            Class::Source { writable: false, .. } | Class::EachSource | Class::Thunk => {
                self.diags.borrow_mut().error("constant_assignment", (id.start, id.end), format!("cannot assign to `{}`", id.name));
                return true;
            }
            Class::Other => return false,
        };
        self.edits.overwrite(u.start, u.end, text);
        true
    }

    fn read(&mut self, id: &Identifier, parent: Option<NodeRef<'_>>) {
        if self.handled.contains(&(id as *const Identifier as usize)) {
            return;
        }
        let (open, close) = match classify(self.arena(), id) {
            Class::Source { .. } => ("$.get(", ")"),
            Class::EachSource if self.mode == Mode::Read => ("$.get(", ")"),
            Class::Prop | Class::Thunk => ("", "()"),
            _ => return,
        };
        // `{ count }` shorthand property -> `{ count: $.get(count) }`
        if let Some(NodeRef::Property(p)) = parent
            && p.shorthand
        {
            self.edits.overwrite(p.start, p.end, format!("{0}: {open}{0}{close}", id.name));
            return;
        }
        self.edits.wrap(id.start, id.end, open, close);
    }

    fn call(&mut self, c: &CallExpression) {
        let replacement = match self.rune(c) {
            Some("$effect") => "$.user_effect",
            Some("$effect.pre") => "$.user_pre_effect",
            Some("$effect.root") => "$.effect_root",
            Some("$effect.tracking") => "$.effect_tracking",
            Some("$state.snapshot") => "$.snapshot",
            Some(r @ ("$state" | "$state.raw" | "$derived" | "$derived.by" | "$props" | "$bindable")) => {
                let s = c.span();
                self.diags.borrow_mut().error("state_invalid_placement", (s.start, s.end), format!("`{r}(...)` can only be used as a variable declaration initializer at the top level of the instance script"));
                return;
            }
            Some(r) => {
                let s = c.span();
                self.diags.borrow_mut().error("unsupported_feature", (s.start, s.end), format!("{r} is not supported by the native compiler yet"));
                return;
            }
            None => return,
        };
        let s = c.callee.span();
        self.edits.overwrite(s.start, s.end, replacement);
        self.handled_callee(c);
    }

    fn handled_callee(&mut self, c: &CallExpression) {
        if let Expression::Identifier(id) = c.callee.as_ref() {
            self.handled.insert(id as *const Identifier as usize);
        }
    }
}

impl<'n> Visitor<'n> for Rw<'_, '_> {
    type State = ();
    fn visit(&mut self, node: NodeRef<'n>, state: &(), w: &mut Walker<'n>) {
        match node {
            NodeRef::AssignmentExpression(a) => {
                if self.assignment(a) {
                    Walker::walk(expr_ref(&a.right), state, self);
                    return;
                }
            }
            NodeRef::UpdateExpression(u) => {
                if self.update(u) {
                    return;
                }
            }
            NodeRef::CallExpression(c) => self.call(c),
            NodeRef::Identifier(id) => {
                let parent = w.parent();
                self.read(id, parent);
                return;
            }
            _ => {}
        }
        w.next(self, node, state);
    }
}

fn pattern_has_reactive(arena: &ScopeArena<'_>, p: &Pattern) -> bool {
    crate::svelte::scope::ast_utils::extract_identifiers(p).iter().any(|id| classify(arena, id) != Class::Other)
}
