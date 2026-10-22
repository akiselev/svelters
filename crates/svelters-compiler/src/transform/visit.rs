//! The edit-producing visitor shared by the instance script, the module script, template
//! expressions and `.svelte.js` modules. One method per upstream visitor (client transform), plus
//! the rune validations of the analysis phase that are cheap to do here
//! (`phases/2-analyze/visitors/{CallExpression,ExportNamedDeclaration,shared/utils}.js`).

use super::classes::{self, ClassFields, addr};
use super::{Ctx, T, is_identifier_name, read_text};
use crate::codegen::component::{Mode, js_str};
use crate::codegen::edit::Edits;
use crate::estree::*;
use crate::svelte::scope::walk::{NodeRef, Visitor, Walker, array_element_ref, expr_ref, stmt_ref};
use crate::svelte::scope::{BindingKind, DeclarationKind, Initial, ScopeId, get_rune_of_call, should_proxy};
use std::collections::HashSet;
use std::rc::Rc;

pub(crate) const EACH_ITEM_MSG: &str = "Cannot reassign or bind to each block argument in runes mode. Use the array and index variables instead (e.g. `array[i] = value` instead of `entry = value`, or `bind:value={array[i]}` instead of `bind:value={entry}`)";
const STATE_PLACEMENT: &str = "can only be used as a variable declaration initializer, a class field declaration, or the first assignment to a class field at the top level of the constructor.";

pub(crate) struct Rw<'c, 'r, 'a> {
    pub cx: &'c Ctx<'r, 'a>,
    pub edits: &'c mut Edits,
    pub mode: Mode,
    /// Visiting the instance script (or a template expression).
    pub is_instance: bool,
    /// `state_fields` of the enclosing classes (innermost last).
    pub(crate) classes: Vec<Rc<ClassFields>>,
    pub(crate) in_constructor: bool,
    /// Identifiers whose span is rewritten by an enclosing edit.
    skip: HashSet<usize>,
}

fn is_non_coercive(op: AssignmentOperator) -> bool {
    matches!(op, AssignmentOperator::Assign | AssignmentOperator::OrAssign | AssignmentOperator::AndAssign | AssignmentOperator::NullishAssign)
}

fn is_logical(op: AssignmentOperator) -> bool {
    matches!(op, AssignmentOperator::OrAssign | AssignmentOperator::AndAssign | AssignmentOperator::NullishAssign)
}

fn binary_of(op: AssignmentOperator) -> Option<&'static str> {
    match op {
        AssignmentOperator::Assign => None,
        other => Some(&other.as_str()[..other.as_str().len() - 1]),
    }
}

/// `is_simple_expression(node)` (utils/ast.js).
pub(crate) fn is_simple_expression(e: &Expression) -> bool {
    match e {
        Expression::Literal(_) | Expression::Identifier(_) | Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => true,
        Expression::ConditionalExpression(c) => is_simple_expression(&c.test) && is_simple_expression(&c.consequent) && is_simple_expression(&c.alternate),
        Expression::BinaryExpression(b) => matches!(&b.left, BinaryLeft::Expression(l) if is_simple_expression(l)) && is_simple_expression(&b.right),
        Expression::LogicalExpression(b) => is_simple_expression(&b.left) && is_simple_expression(&b.right),
        _ => false,
    }
}

/// Whether `e` contains an `await` outside nested functions (`is_expression_async`).
pub(crate) fn has_await(e: &Expression) -> bool {
    node_has_await(expr_ref(e))
}

/// `has_await` for any node (a destructuring pattern's defaults and computed keys).
pub(crate) fn node_has_await(n: NodeRef<'_>) -> bool {
    struct A(bool);
    impl<'n> Visitor<'n> for A {
        type State = ();
        fn visit(&mut self, node: NodeRef<'n>, s: &(), w: &mut Walker<'n>) {
            match node {
                NodeRef::AwaitExpression(_) => self.0 = true,
                n if n.is_function() => {}
                _ => w.next(self, node, s),
            }
        }
    }
    let mut a = A(false);
    Walker::walk(n, &(), &mut a);
    a.0
}

fn arg_expr(a: &ArrayElement) -> Option<&Expression> {
    match a {
        ArrayElement::Expression(e) => Some(e),
        ArrayElement::Spread(_) => None,
    }
}

impl<'c, 'r, 'a> Rw<'c, 'r, 'a> {
    pub fn new(cx: &'c Ctx<'r, 'a>, edits: &'c mut Edits, mode: Mode, is_instance: bool) -> Self {
        Rw { cx, edits, mode, is_instance, classes: Vec::new(), in_constructor: false, skip: HashSet::new() }
    }

    fn src(&self, s: Span) -> &'r str {
        &self.cx.src[s.start as usize..s.end as usize]
    }

    /// The transformed text of a node (nested visit with its own edits; no source mapping).
    pub(crate) fn text_ref(&self, node: NodeRef<'_>, span: Span, scope: ScopeId) -> String {
        let mut edits = Edits::new();
        let mut rw = Rw { cx: self.cx, edits: &mut edits, mode: self.mode, is_instance: self.is_instance, classes: self.classes.clone(), in_constructor: self.in_constructor, skip: HashSet::new() };
        Walker::walk(node, &scope, &mut rw);
        edits.apply(self.cx.src, span.start, span.end)
    }

    pub(crate) fn text(&self, e: &Expression, scope: ScopeId) -> String {
        self.text_ref(expr_ref(e), e.span(), scope)
    }

    /// One statement of a `<script>` / module body (top level: exports are handled here).
    pub fn top_level_statement(&mut self, stmt: &Statement, scope: ScopeId, is_instance: bool) {
        let component = self.cx.instance_scope.is_some();
        match stmt {
            Statement::ImportDeclaration(_) => {}
            Statement::ExportNamedDeclaration(e) => {
                if let Some(d) = &e.declaration {
                    if let Statement::VariableDeclaration(v) = d.as_ref() {
                        self.validate_export_declaration(e.span(), v, scope);
                        if is_instance && v.kind == VariableKind::Let {
                            self.cx.error("legacy_export_invalid", e.span(), "Cannot use `export let` in runes mode — use `$props()` instead");
                        }
                    }
                    if is_instance {
                        self.edits.overwrite(e.start, d.span().start, "");
                    }
                    Walker::walk(stmt_ref(d), &scope, self);
                } else {
                    for s in &e.specifiers {
                        if let ModuleExportName::Identifier(i) = &s.exported
                            && i.name == "default"
                            && component
                        {
                            self.cx.error("module_illegal_default_export", e.span(), "A component cannot have a default export");
                        }
                        if !is_instance && e.source.is_none()
                            && let ModuleExportName::Identifier(local) = &s.local
                        {
                            self.validate_export(e.span(), &local.name, scope);
                        }
                    }
                    if is_instance {
                        if e.source.is_some() {
                            self.cx.error("unsupported_feature", e.span(), "re-exports from the instance script are not supported");
                        }
                        self.edits.overwrite(e.start, e.end, "");
                    }
                }
            }
            Statement::ExportDefaultDeclaration(e) if component => {
                self.cx.error("module_illegal_default_export", e.span(), "A component cannot have a default export");
            }
            Statement::ExportAllDeclaration(e) if is_instance => {
                self.cx.error("unsupported_feature", e.span(), "re-exports from the instance script are not supported");
            }
            _ => Walker::walk(stmt_ref(stmt), &scope, self),
        }
    }

    fn validate_export_declaration(&self, span: Span, v: &VariableDeclaration, scope: ScopeId) {
        for decl in &v.declarations {
            for id in crate::svelte::scope::ast_utils::extract_identifiers(&decl.id) {
                self.validate_export(span, &id.name, scope);
            }
        }
    }

    /// `validate_export` / the ExportNamedDeclaration checks.
    fn validate_export(&self, span: Span, name: &str, scope: ScopeId) {
        let Some(b) = self.cx.arena.get(scope, name) else { return };
        let x = self.cx.arena.binding(b);
        match x.kind {
            BindingKind::Derived => self.cx.error("derived_invalid_export", span, "Cannot export derived state from a module. To expose the current derived value, export a function returning its value"),
            BindingKind::State | BindingKind::RawState if x.reassigned => self.cx.error(
                "state_invalid_export",
                span,
                "Cannot export state from a module if it is reassigned. Either export a function returning the state value or only mutate the state value's properties",
            ),
            _ => {}
        }
    }

    // ---------------------------------------------------------------------------------------------
    // VariableDeclaration

    fn var_decl<'n>(&mut self, d: &'n VariableDeclaration, node: NodeRef<'n>, scope: ScopeId, w: &mut Walker<'n>) {
        let mut removed = vec![false; d.declarations.len()];
        for (i, decl) in d.declarations.iter().enumerate() {
            let call = match &decl.init {
                Some(Expression::CallExpression(c)) => Some(c),
                _ => None,
            };
            let rune = call.and_then(|c| get_rune_of_call(self.cx.arena, c, scope));
            let top = self.is_instance && Some(scope) == self.cx.instance_scope;
            match (rune, call) {
                (Some("$props"), Some(c)) => {
                    self.check_spread(c, "$props");
                    if !top {
                        self.cx.error("props_invalid_placement", c.span(), "`$props()` can only be used at the top level of components as a variable declaration initializer");
                    }
                    if self.cx.has_props_rune.replace(true) {
                        self.cx.error("props_duplicate", c.span(), "Cannot use `$props()` more than once");
                    }
                    if !c.arguments.is_empty() {
                        self.cx.error("rune_invalid_arguments", c.span(), "`$props` cannot be called with arguments");
                    }
                    removed[i] = !self.props(decl, scope);
                }
                (Some("$props.id"), Some(c)) => {
                    if !top || !matches!(decl.id, Pattern::Identifier(_)) {
                        self.cx.error("props_id_invalid_placement", c.span(), "`$props.id()` can only be used at the top level of components as a variable declaration initializer");
                    }
                    if self.cx.props_id.get().is_some() {
                        self.cx.error("props_duplicate", c.span(), "Cannot use `$props.id()()` more than once");
                    }
                    if !c.arguments.is_empty() {
                        self.cx.error("rune_invalid_arguments", c.span(), "`$props.id` cannot be called with arguments");
                    }
                    if let Pattern::Identifier(id) = &decl.id {
                        self.cx.props_id.set(self.cx.arena.get(scope, &id.name));
                    }
                    self.edits.overwrite(c.start, c.end, "$.props_id()");
                }
                (Some(r @ ("$state" | "$state.raw")), Some(c)) => self.state_declarator(decl, c, r, scope, w),
                (Some(r @ ("$derived" | "$derived.by")), Some(c)) => self.derived_declarator(decl, c, r, scope, w),
                _ => w.visit(self, node, NodeRef::VariableDeclarator(decl), &scope),
            }
        }
        if removed.iter().all(|r| *r) {
            self.edits.overwrite(d.start, d.end, "");
        } else {
            for (i, decl) in d.declarations.iter().enumerate().filter(|(i, _)| removed[*i]) {
                match d.declarations.get(i + 1) {
                    Some(next) => self.edits.overwrite(decl.start, next.start, ""),
                    None => self.edits.overwrite(d.declarations[i - 1].end, decl.end, ""),
                }
            }
        }
    }

    fn check_spread(&self, c: &CallExpression, rune: &str) {
        if c.arguments.iter().any(|a| matches!(a, ArrayElement::Spread(_))) {
            self.cx.error("rune_invalid_spread", c.span(), format!("`{rune}` cannot be called with a spread argument"));
        }
    }

    fn check_args(&self, c: &CallExpression, rune: &str, ok: bool, what: &str) {
        if !ok {
            self.cx.error("rune_invalid_arguments_length", c.span(), format!("`{rune}` must be called with {what}"));
        }
    }

    fn label(&self, name: &str) -> String {
        js_str(name)
    }

    /// `let x = $state(v)` / `let { a, b } = $state(v)`.
    fn state_declarator<'n>(&mut self, decl: &'n VariableDeclarator, call: &'n CallExpression, rune: &'static str, scope: ScopeId, w: &mut Walker<'n>) {
        self.check_spread(call, rune);
        self.check_args(call, rune, call.arguments.len() <= 1, "zero or one arguments");
        let arg = call.arguments.first().and_then(arg_expr);
        let call_node = NodeRef::CallExpression(call);
        match &decl.id {
            Pattern::Identifier(id) => {
                let b = self.cx.arena.get(scope, &id.name);
                let is_state = b.is_some_and(|b| self.cx.is_state_source(b));
                if self.cx.dev && is_state {
                    self.edits.wrap(call.start, call.end, "$.tag(", format!(", {})", self.label(&id.name)));
                }
                match arg {
                    Some(a) => {
                        let s = a.span();
                        if is_state {
                            let cs = call.callee.span();
                            self.edits.overwrite(cs.start, cs.end, "$.state");
                        } else {
                            self.edits.overwrite(call.start, s.start, "");
                            self.edits.overwrite(s.end, call.end, "");
                        }
                        let proxy = rune == "$state" && should_proxy(self.cx.arena, a, Some(scope));
                        if proxy && self.cx.dev && !is_state {
                            self.edits.wrap(s.start, s.end, "$.tag_proxy(", format!(", {})", self.label(&id.name)));
                        }
                        if proxy {
                            self.edits.wrap(s.start, s.end, "$.proxy(", ")");
                        }
                        if self.keyed_state(scope, is_state || proxy) {
                            let src = self.cx.src;
                            self.key_declaration(&id.name, rune, is_state, s, &src[s.start as usize..s.end as usize]);
                        }
                        w.visit(self, call_node, expr_ref(a), &scope);
                    }
                    None if is_state && self.keyed_state(scope, true) => {
                        self.cx.keyed.borrow_mut().push(format!("{rune}:{}:", id.name));
                        self.edits.overwrite(call.start, call.end, format!("$.state({}({}, () => void 0, () => $.get({})))", self.cx.keep_callee.get(), self.label(&id.name), id.name));
                    }
                    None => self.edits.overwrite(call.start, call.end, if is_state { "$.state(void 0)" } else { "void 0" }),
                }
            }
            pattern => {
                let tmp = self.cx.generate("tmp");
                let value = arg.map_or("void 0".to_string(), |a| self.text(a, scope));
                let (inserts, paths) = self.extract_paths(pattern, tmp.clone(), scope, true);
                let mut parts = vec![format!("{tmp} = {value}")];
                let kind = if matches!(pattern, Pattern::ArrayPattern(_)) { "iterable" } else { "object" };
                for ins in inserts {
                    let call = format!("$.derived(() => {})", ins.value);
                    parts.push(format!("{} = {}", ins.id, if self.cx.dev { format!("$.tag({call}, {})", js_str(&format!("[$state {kind}]"))) } else { call }));
                }
                for p in paths {
                    let super::paths::Target::Id(id) = p.node else { continue };
                    let b = self.cx.arena.get(scope, &id.name);
                    let value = match b.map(|b| self.cx.arena.binding(b).kind) {
                        Some(BindingKind::State | BindingKind::RawState) => self.create_state_declarator(&id.name, p.expression, rune, b.is_some_and(|b| self.cx.is_state_source(b))),
                        _ => p.expression,
                    };
                    parts.push(format!("{} = {value}", id.name));
                }
                self.edits.overwrite(decl.start, decl.end, parts.join(", "));
            }
        }
    }

    /// Whether a `$state` declaration is keyed for HMR state preservation: `--hmr`, top level of the
    /// instance script, and something that can change (a reassigned source or a proxy).
    fn keyed_state(&self, scope: ScopeId, mutable: bool) -> bool {
        let top = match self.cx.instance_scope {
            Some(i) => scope == i,
            None => scope == self.cx.module_scope,
        };
        self.cx.hmr.get() && mutable && top
    }

    /// Wraps the initializer `[s]` as `$n.hmr_keep("x", () => init, () => read)`: a restored value
    /// replaces the initializer (HMR.md 2.4 item 3). The wrap is added last, so it is innermost.
    fn key_declaration(&mut self, name: &str, rune: &str, is_state: bool, s: Span, init: &str) {
        self.cx.keyed.borrow_mut().push(format!("{rune}:{name}:{}", super::normalize_initializer(init)));
        let read = if is_state { format!("$.get({name})") } else { name.to_string() };
        self.edits.wrap(s.start, s.end, format!("{}({}, () => ", self.cx.keep_callee.get(), self.label(name)), format!(", () => {read})"));
    }

    fn create_state_declarator(&self, name: &str, value: String, rune: &str, is_state: bool) -> String {
        let mut v = value;
        if rune == "$state" {
            v = format!("$.proxy({v})");
            if self.cx.dev && !is_state {
                v = format!("$.tag_proxy({v}, {})", self.label(name));
            }
        }
        if is_state {
            v = format!("$.state({v})");
            if self.cx.dev {
                v = format!("$.tag({v}, {})", self.label(name));
            }
        }
        v
    }

    /// `let x = $derived(e)` / `$derived.by(f)` / destructured.
    fn derived_declarator<'n>(&mut self, decl: &'n VariableDeclarator, call: &'n CallExpression, rune: &'static str, scope: ScopeId, w: &mut Walker<'n>) {
        self.check_spread(call, rune);
        self.check_args(call, rune, call.arguments.len() == 1, "exactly one argument");
        let Some(arg) = call.arguments.first().and_then(arg_expr) else { return };
        if rune == "$derived" && has_await(arg) {
            self.cx.error("unsupported_feature", call.span(), "async `$derived` (with `await`) is not supported by the native compiler yet");
            return;
        }
        let call_node = NodeRef::CallExpression(call);
        match &decl.id {
            Pattern::Identifier(id) => {
                if self.cx.dev {
                    self.edits.wrap(call.start, call.end, "$.tag(", format!(", {})", self.label(&id.name)));
                }
                let cs = call.callee.span();
                self.edits.overwrite(cs.start, cs.end, "$.derived");
                if rune == "$derived" {
                    let s = arg.span();
                    self.edits.wrap(s.start, s.end, "() => (", ")");
                }
                w.visit(self, call_node, expr_ref(arg), &scope);
            }
            pattern => {
                let mut parts = Vec::new();
                let value = self.text(arg, scope);
                let rhs = if rune == "$derived" && matches!(arg, Expression::Identifier(_)) {
                    value
                } else {
                    let id = self.cx.generate("$$d");
                    let f = if rune == "$derived" { format!("() => ({value})") } else { value };
                    parts.push(format!("{id} = $.derived({f})"));
                    format!("$.get({id})")
                };
                let (inserts, paths) = self.extract_paths(pattern, rhs, scope, true);
                let kind = if matches!(pattern, Pattern::ArrayPattern(_)) { "iterable" } else { "object" };
                for ins in inserts {
                    let call = format!("$.derived(() => {})", ins.value);
                    parts.push(format!("{} = {}", ins.id, if self.cx.dev { format!("$.tag({call}, {})", js_str(&format!("[$derived {kind}]"))) } else { call }));
                }
                for p in paths {
                    let super::paths::Target::Id(id) = p.node else { continue };
                    let call = format!("$.derived(() => {})", p.expression);
                    parts.push(format!("{} = {}", id.name, if self.cx.dev { format!("$.tag({call}, {})", self.label(&id.name)) } else { call }));
                }
                self.edits.overwrite(decl.start, decl.end, parts.join(", "));
            }
        }
    }

    /// `let { a, b = 1, c = $bindable(), ...rest } = $props()`; false when nothing is left of
    /// the declarator (props read straight from `$$props`).
    fn props(&mut self, decl: &VariableDeclarator, scope: ScopeId) -> bool {
        let mut seen: Vec<String> = ["$$slots", "$$events", "$$legacy"].iter().map(|s| s.to_string()).collect();
        let rest_excludes = |this: &Self, seen: &[String]| {
            let id = this.cx.generate("rest_excludes");
            let names: Vec<String> = seen.iter().map(|s| js_str(s)).collect();
            this.cx.hoisted.borrow_mut().push(format!("var {id} = new Set([{}]);", names.join(", ")));
            id
        };
        let out = match &decl.id {
            Pattern::Identifier(id) => {
                let ex = rest_excludes(self, &seen);
                let dev = if self.cx.dev { format!(", {}", js_str(&id.name)) } else { String::new() };
                format!("{} = $.rest_props($$props, {ex}{dev})", id.name)
            }
            Pattern::ObjectPattern(op) => {
                let mut parts = Vec::new();
                for m in &op.properties {
                    match m {
                        ObjectPatternMember::Property(p) => {
                            let Some(name) = classes::get_name(&p.key) else { continue };
                            seen.push(name.clone());
                            let id = match &p.value {
                                Pattern::AssignmentPattern(ap) => ap.left.as_ref(),
                                v => v,
                            };
                            let Pattern::Identifier(id) = id else { continue };
                            let Some(b) = self.cx.arena.get(scope, &id.name) else { continue };
                            let x = self.cx.arena.binding(b);
                            let initial = match x.initial {
                                Some(Initial::Expression(e)) => Some(e),
                                _ => None,
                            };
                            let mut init_text = initial.map(|e| self.text(e, scope));
                            let mut proxied = false;
                            if let (Some(t), Some(e)) = (&init_text, initial)
                                && x.kind == BindingKind::BindableProp
                                && should_proxy(self.cx.arena, e, Some(scope))
                            {
                                let mut v = format!("$.proxy({t})");
                                if self.cx.dev {
                                    v = format!("$.tag_proxy({v}, {})", js_str(&id.name));
                                }
                                init_text = Some(v);
                                proxied = true;
                            }
                            if self.cx.is_prop_source(b) {
                                parts.push(format!("{} = {}", id.name, self.prop_source(b, &name, init_text, initial, proxied)));
                            }
                        }
                        ObjectPatternMember::RestElement(r) => {
                            let Pattern::Identifier(arg) = r.argument.as_ref() else { continue };
                            let ex = rest_excludes(self, &seen);
                            let dev = if self.cx.dev { format!(", {}", js_str(&arg.name)) } else { String::new() };
                            parts.push(format!("{} = $.rest_props($$props, {ex}{dev})", arg.name));
                        }
                    }
                }
                if parts.is_empty() {
                    return false;
                }
                parts.join(", ")
            }
            _ => return true,
        };
        self.edits.overwrite(decl.start, decl.end, out);
        true
    }

    /// `get_prop_source(binding, state, name, initial)`.
    fn prop_source(&self, b: crate::svelte::scope::BindingId, name: &str, init_text: Option<String>, initial: Option<&Expression>, proxied: bool) -> String {
        let x = self.cx.arena.binding(b);
        let mut flags = super::PROPS_IS_IMMUTABLE | super::PROPS_IS_RUNES;
        if x.kind == BindingKind::BindableProp {
            flags |= super::PROPS_IS_BINDABLE;
        }
        if self.cx.reassigned(b) || x.mutated {
            flags |= super::PROPS_IS_UPDATED;
        }
        let mut arg = String::new();
        if let (Some(t), Some(e)) = (init_text, initial) {
            if !proxied && is_simple_expression(e) {
                arg = format!(", {t}");
            } else {
                match e {
                    Expression::CallExpression(c) if !proxied && c.arguments.is_empty() && matches!(c.callee.as_ref(), Expression::Identifier(_)) => {
                        arg = format!(", {}", self.text(&c.callee, x.scope));
                    }
                    _ => arg = format!(", () => {}", if t.starts_with('{') { format!("({t})") } else { t }),
                }
                flags |= super::PROPS_IS_LAZY_INITIAL;
            }
        }
        format!("$.prop($$props, {}, {flags}{arg})", js_str(name))
    }

    // ---------------------------------------------------------------------------------------------
    // assignments

    fn validate_assignment_target(&self, id: &Identifier, span: Span) {
        let Some(b) = self.cx.arena.resolve(id) else { return };
        let x = self.cx.arena.binding(b);
        if Some(b) == self.cx.props_id.get() {
            self.cx.error("constant_assignment", span, "Cannot assign to $props.id()");
            return;
        }
        if x.kind == BindingKind::Each {
            self.cx.error("each_item_invalid_assignment", span, EACH_ITEM_MSG);
            return;
        }
        if x.kind == BindingKind::Snippet {
            self.cx.error("snippet_parameter_assignment", span, "Cannot reassign or bind to snippet parameter");
            return;
        }
        if x.kind == BindingKind::Template {
            self.cx.error("constant_assignment", span, "Cannot assign to constant");
            return;
        }
        if x.declaration_kind == DeclarationKind::Import || (x.declaration_kind == DeclarationKind::Const && x.kind != BindingKind::Each) {
            let thing = if x.declaration_kind == DeclarationKind::Import { "import" } else { "constant" };
            self.cx.error("constant_assignment", span, format!("Cannot assign to {thing}"));
        }
    }

    /// Text between the operator of `a` and its right-hand side (e.g. ` (` of `x = (a, b)`).
    fn after_operator(&self, a: &AssignmentExpression) -> String {
        let between = &self.cx.src[a.left.span().end as usize..a.right.span().start as usize];
        let op = a.operator.as_str();
        match between.find(op) {
            Some(i) => between[i + op.len()..].trim_start().to_string(),
            None => String::new(),
        }
    }

    fn store_unsub(&self, name: &str) -> Option<String> {
        let scope = self.cx.instance_scope?;
        let store = format!("${name}");
        let b = self.cx.arena.get(scope, &store)?;
        (self.cx.arena.binding(b).kind == BindingKind::StoreSub).then_some(store)
    }

    fn assignment<'n>(&mut self, a: &'n AssignmentExpression, node: NodeRef<'n>, scope: ScopeId, w: &mut Walker<'n>) -> bool {
        match &a.left {
            Pattern::ObjectPattern(_) | Pattern::ArrayPattern(_) | Pattern::RestElement(_) => self.destructuring_assignment(a, node, scope, w),
            Pattern::Identifier(id) => {
                self.validate_assignment_target(id, a.span());
                let t = self.cx.transform_of_id(id);
                let n = &id.name;
                let bin = binary_of(a.operator);
                let after = self.after_operator(a);
                let (prefix, suffix) = match &t {
                    T::Source { proxy } => {
                        let proxy = *proxy && is_non_coercive(a.operator) && should_proxy(self.cx.arena, &a.right, Some(scope));
                        let mut prefix = match bin {
                            None => format!("$.set({n}, "),
                            Some(b) => format!("$.set({n}, $.get({n}) {b} ("),
                        };
                        let mut suffix = format!("{}{})", if bin.is_some() { ")" } else { "" }, if proxy { ", true" } else { "" });
                        if let Some(store) = self.store_unsub(n) {
                            prefix = format!("$.store_unsub({prefix}");
                            suffix = format!("{suffix}, {}, $$stores)", js_str(&store));
                        }
                        (prefix, suffix)
                    }
                    T::PropSource => (
                        match bin {
                            None => format!("{n}("),
                            Some(b) => format!("{n}({n}() {b} ("),
                        },
                        if bin.is_some() { "))" } else { ")" }.to_string(),
                    ),
                    T::Store(s) => (
                        match bin {
                            None => format!("$.store_set({s}, "),
                            Some(b) => format!("$.store_set({s}, {n}() {b} ("),
                        },
                        if bin.is_some() { "))" } else { ")" }.to_string(),
                    ),
                    T::Each | T::PropDirect(_) | T::ReadOnly | T::Thunk | T::None => return false,
                };
                self.edits.overwrite(a.start, a.right.span().start, format!("{prefix}{after}"));
                self.edits.close(a.end, suffix);
                w.visit(self, node, expr_ref(&a.right), &scope);
                true
            }
            Pattern::MemberExpression(m) => self.member_assignment(a, m, node, scope, w),
            _ => false,
        }
    }

    fn current_field(&self, name: &str) -> Option<&classes::Field> {
        self.classes.last().and_then(|c| c.get(name))
    }

    fn private_name(m: &MemberExpression) -> Option<String> {
        match &m.property {
            PropertyKey::Private(PrivateIdentifierNode::PrivateIdentifier(p)) => Some(format!("#{}", p.name)),
            _ => None,
        }
    }

    /// Reads `obj.#field` of a state field: `$.get(obj.#field)` (or `.v` in a constructor).
    fn private_read_text(&self, rune: &str, member_src: &str) -> String {
        if self.in_constructor && matches!(rune, "$state" | "$state.raw") { format!("{member_src}.v") } else { format!("$.get({member_src})") }
    }

    fn member_assignment<'n>(&mut self, a: &'n AssignmentExpression, m: &'n MemberExpression, node: NodeRef<'n>, scope: ScopeId, w: &mut Walker<'n>) -> bool {
        // state field declared in the constructor: `this.x = $state(v)` -> `this.#x = $.state(v)`
        if let Some(f) = self.classes.last().and_then(|c| c.by_decl(addr(a))).cloned()
            && let Expression::CallExpression(call) = a.right.as_ref()
            && a.operator == AssignmentOperator::Assign
        {
            if f.public {
                self.edits.overwrite(m.object.span().end, m.end, format!(".#{}", f.private));
            }
            if self.cx.dev {
                self.edits.wrap(call.start, call.end, "$.tag(", format!(", {})", js_str(&format!("{}.{}", self.class_name(w), f.name))));
            }
            let saved = self.in_constructor;
            self.in_constructor = !f.rune.starts_with("$derived");
            self.state_call(call, f.rune, scope, w);
            self.in_constructor = saved;
            return true;
        }
        if let Some(name) = Self::private_name(m)
            && let Some(f) = self.current_field(&name).cloned()
        {
            let left = self.src(m.span()).to_string();
            let read = self.private_read_text(f.rune, &left);
            let after = self.after_operator(a);
            let proxy = f.rune == "$state" && is_non_coercive(a.operator) && (is_logical(a.operator) || a.operator == AssignmentOperator::Assign) && should_proxy(self.cx.arena, &a.right, Some(scope));
            let close = if proxy { ", true)" } else { ")" };
            let (prefix, suffix) = if is_logical(a.operator) {
                (format!("{read} {} $.set({left}, {after}", binary_of(a.operator).unwrap()), close.to_string())
            } else {
                match binary_of(a.operator) {
                    None => (format!("$.set({left}, {after}"), close.to_string()),
                    Some(b) => (format!("$.set({left}, {read} {b} ({after}"), "))".to_string()),
                }
            };
            self.edits.overwrite(a.start, a.right.span().start, prefix);
            self.edits.close(a.end, suffix);
            w.visit(self, node, expr_ref(&a.right), &scope);
            return true;
        }
        self.mutation(node, Some(m), scope, w)
    }

    /// Mutations through a store (`$store.x = v`) or a bindable prop (`prop.x = v`).
    fn mutation<'n>(&mut self, node: NodeRef<'n>, m: Option<&'n MemberExpression>, scope: ScopeId, w: &mut Walker<'n>) -> bool {
        let Some(m) = m else { return false };
        let mut obj: &Expression = &m.object;
        while let Expression::MemberExpression(inner) = obj {
            obj = &inner.object;
        }
        let Expression::Identifier(root) = obj else { return false };
        let Some(b) = self.cx.arena.resolve(root) else { return false };
        let span = node.span();
        match self.cx.transform_of(b) {
            T::Store(s) => {
                let untracked = format!("$.untrack({})", root.name);
                self.edits.open(span.start, format!("$.store_mutate({s}, "));
                self.edits.overwrite(root.start, root.end, untracked.clone());
                self.edits.close(span.end, format!(", {untracked})"));
                self.skip.insert(addr(root));
                w.next(self, node, &scope);
                true
            }
            T::PropSource if self.cx.arena.binding(b).kind == BindingKind::BindableProp => {
                self.edits.open(span.start, format!("{}(", root.name));
                self.edits.close(span.end, ", true)");
                w.next(self, node, &scope);
                true
            }
            _ => false,
        }
    }

    fn class_name(&self, w: &Walker<'_>) -> String {
        for n in w.path.iter().rev() {
            if let NodeRef::ClassDeclaration(c) | NodeRef::ClassExpression(c) = n {
                return c.id.as_ref().map_or("[class]".to_string(), |i| i.name.clone());
            }
        }
        "[class]".to_string()
    }

    /// `[a, b] = [b, a]` (`visit_assignment_expression` with patterns).
    fn destructuring_assignment<'n>(&mut self, a: &'n AssignmentExpression, _node: NodeRef<'n>, scope: ScopeId, w: &mut Walker<'n>) -> bool {
        let targets_change = crate::svelte::scope::ast_utils::extract_identifiers(&a.left).iter().any(|id| self.cx.transform_of_id(id) != T::None);
        if !targets_change {
            for id in crate::svelte::scope::ast_utils::extract_identifiers(&a.left) {
                self.validate_assignment_target(id, a.span());
            }
            return false;
        }
        let value = self.text(&a.right, scope);
        let should_cache = !is_identifier_name(&value);
        let rhs = if should_cache { "$$value".to_string() } else { value.clone() };
        let (inserts, paths) = self.extract_paths(&a.left, rhs.clone(), scope, false);
        let mut assignments = Vec::new();
        for p in paths {
            match p.node {
                super::paths::Target::Id(id) => {
                    self.validate_assignment_target(id, a.span());
                    let n = &id.name;
                    let v = &p.expression;
                    assignments.push(match self.cx.transform_of_id(id) {
                        T::Source { proxy } => {
                            let set = format!("$.set({n}, {v}{})", if proxy { ", true" } else { "" });
                            match self.store_unsub(n) {
                                Some(store) => format!("$.store_unsub({set}, {}, $$stores)", js_str(&store)),
                                None => set,
                            }
                        }
                        T::PropSource => format!("{n}({v})"),
                        T::Store(s) => format!("$.store_set({s}, {v})"),
                        _ => format!("{n} = {v}"),
                    });
                }
                super::paths::Target::Member(m) => {
                    let field = Self::private_name(m).and_then(|n| self.current_field(&n).cloned());
                    match field {
                        Some(f) => assignments.push(format!("$.set({}, {}{})", self.src(m.span()), p.expression, if f.rune == "$state" { ", true" } else { "" })),
                        None => assignments.push(format!("{} = {}", self.text_ref(NodeRef::MemberExpression(m), m.span(), scope), p.expression)),
                    }
                }
            }
        }
        // SVELTERS: upstream checks `type.endsWith('Statement')`, which drops the value of
        // `return [a, b] = ...`; only an expression statement discards it.
        let standalone = matches!(w.parent(), Some(NodeRef::ExpressionStatement(_)));
        let text = if !inserts.is_empty() || should_cache {
            let mut body = String::new();
            for i in &inserts {
                body.push_str(&format!("var {} = {}; ", i.id, i.value));
            }
            for x in &assignments {
                body.push_str(&format!("{x}; "));
            }
            if !standalone {
                body.push_str(&format!("return {rhs}; "));
            }
            // upstream: `is_expression_async(value) || assignments.some(is_expression_async)`; an
            // assignment awaits when a default value (or a computed key) of the pattern does
            if has_await(&a.right) || node_has_await(crate::svelte::scope::walk::pattern_ref(&a.left)) {
                format!("(await (async ({rhs}) => {{ {body}}})({value}))")
            } else {
                format!("(({rhs}) => {{ {body}}})({value})")
            }
        } else {
            if !standalone {
                assignments.push(rhs);
            }
            format!("({})", assignments.join(", "))
        };
        self.edits.overwrite(a.start, a.end, text);
        true
    }

    fn update<'n>(&mut self, u: &'n UpdateExpression, node: NodeRef<'n>, scope: ScopeId, w: &mut Walker<'n>) -> bool {
        let pre = if u.prefix { "_pre" } else { "" };
        let delta = if u.operator == UpdateOperator::Decrement { ", -1" } else { "" };
        match u.argument.as_ref() {
            Expression::Identifier(id) => {
                self.validate_assignment_target(id, u.span());
                let n = &id.name;
                let text = match self.cx.transform_of_id(id) {
                    T::Source { .. } => format!("$.update{pre}({n}{delta})"),
                    T::PropSource => format!("$.update{pre}_prop({n}{delta})"),
                    T::Store(s) => format!("$.update{pre}_store({s}, {n}(){delta})"),
                    _ => return false,
                };
                self.edits.overwrite(u.start, u.end, text);
                true
            }
            Expression::MemberExpression(m) => {
                if let Some(name) = Self::private_name(m)
                    && self.current_field(&name).is_some()
                {
                    let text = format!("$.update{pre}({}{delta})", self.src(m.span()));
                    self.edits.overwrite(u.start, u.end, text);
                    return true;
                }
                self.mutation(node, Some(m), scope, w)
            }
            _ => false,
        }
    }

    // ---------------------------------------------------------------------------------------------
    // calls

    /// `$state(v)` / `$derived(e)` in a class field or constructor assignment (CallExpression).
    fn state_call<'n>(&mut self, call: &'n CallExpression, rune: &str, scope: ScopeId, w: &mut Walker<'n>) {
        self.check_spread(call, rune);
        let node = NodeRef::CallExpression(call);
        let cs = call.callee.span();
        match rune {
            "$state" | "$state.raw" => {
                self.check_args(call, rune, call.arguments.len() <= 1, "zero or one arguments");
                self.edits.overwrite(cs.start, cs.end, "$.state");
                if let Some(a) = call.arguments.first().and_then(arg_expr) {
                    if rune == "$state" && should_proxy(self.cx.arena, a, Some(scope)) {
                        let s = a.span();
                        self.edits.wrap(s.start, s.end, "$.proxy(", ")");
                    }
                    w.visit(self, node, expr_ref(a), &scope);
                }
            }
            _ => {
                self.check_args(call, rune, call.arguments.len() == 1, "exactly one argument");
                self.edits.overwrite(cs.start, cs.end, "$.derived");
                if let Some(a) = call.arguments.first().and_then(arg_expr) {
                    if rune == "$derived" {
                        let s = a.span();
                        self.edits.wrap(s.start, s.end, "() => (", ")");
                    }
                    w.visit(self, node, expr_ref(a), &scope);
                }
            }
        }
    }

    fn visit_args<'n>(&mut self, c: &'n CallExpression, scope: ScopeId, w: &mut Walker<'n>) {
        let node = NodeRef::CallExpression(c);
        for a in &c.arguments {
            w.visit(self, node, array_element_ref(a), &scope);
        }
    }

    fn call<'n>(&mut self, c: &'n CallExpression, scope: ScopeId, w: &mut Walker<'n>) -> bool {
        let Some(rune) = get_rune_of_call(self.cx.arena, c, scope) else { return false };
        if rune != "$inspect" {
            self.check_spread(c, rune);
        }
        let n = c.arguments.len();
        let cs = c.callee.span();
        let parent = w.parent();
        match rune {
            "$state" | "$state.raw" | "$derived" | "$derived.by" => {
                self.cx.error("state_invalid_placement", c.span(), format!("`{rune}(...)` {STATE_PLACEMENT}"));
                false
            }
            "$props" => {
                self.cx.error("props_invalid_placement", c.span(), "`$props()` can only be used at the top level of components as a variable declaration initializer");
                false
            }
            "$props.id" => {
                self.cx.error("props_id_invalid_placement", c.span(), "`$props.id()` can only be used at the top level of components as a variable declaration initializer");
                false
            }
            "$bindable" => {
                self.cx.error("bindable_invalid_location", c.span(), "`$bindable()` can only be used inside a `$props()` declaration");
                false
            }
            "$host" => {
                self.cx.error("host_invalid_placement", c.span(), "`$host()` can only be used inside custom element component instances (custom elements are not supported by the native compiler)");
                false
            }
            "$effect" | "$effect.pre" => {
                if !matches!(parent, Some(NodeRef::ExpressionStatement(_))) {
                    self.cx.error("effect_invalid_placement", c.span(), "`$effect()` can only be used as an expression statement");
                }
                self.check_args(c, rune, n == 1, "exactly one argument");
                self.edits.overwrite(cs.start, cs.end, if rune == "$effect" { "$.user_effect" } else { "$.user_pre_effect" });
                self.visit_args(c, scope, w);
                true
            }
            "$effect.root" => {
                self.check_args(c, rune, n == 1, "exactly one argument");
                self.edits.overwrite(cs.start, cs.end, "$.effect_root");
                self.visit_args(c, scope, w);
                true
            }
            "$effect.tracking" => {
                if n != 0 {
                    self.cx.error("rune_invalid_arguments", c.span(), "`$effect.tracking` cannot be called with arguments");
                }
                self.edits.overwrite(c.start, c.end, "$.effect_tracking()");
                true
            }
            "$effect.pending" => {
                self.edits.overwrite(c.start, c.end, "$.eager(() => $.pending())");
                true
            }
            "$state.snapshot" => {
                self.check_args(c, rune, n == 1, "exactly one argument");
                self.edits.overwrite(cs.start, cs.end, "$.snapshot");
                self.visit_args(c, scope, w);
                true
            }
            "$state.eager" => {
                self.check_args(c, rune, n == 1, "exactly one argument");
                self.edits.overwrite(cs.start, cs.end, "$.eager");
                if let Some(a) = c.arguments.first().and_then(arg_expr) {
                    let s = a.span();
                    self.edits.wrap(s.start, s.end, "() => (", ")");
                }
                self.visit_args(c, scope, w);
                true
            }
            "$inspect" => {
                self.check_args(c, rune, n >= 1, "one or more arguments");
                self.inspect(c, c, None, scope, w);
                true
            }
            "$inspect().with" => {
                self.check_args(c, rune, n == 1, "exactly one argument");
                let Expression::MemberExpression(m) = c.callee.as_ref() else { return false };
                let Expression::CallExpression(inner) = m.object.as_ref() else { return false };
                self.inspect(c, inner, c.arguments.first().and_then(arg_expr), scope, w);
                true
            }
            "$inspect.trace" => {
                self.cx.error("inspect_trace_invalid_placement", c.span(), "`$inspect.trace(...)` must be the first statement of a function body");
                true
            }
            _ => false,
        }
    }

    /// `$inspect(a, b)` / `$inspect(a).with(fn)`: dev only (`transform_inspect_rune`).
    fn inspect<'n>(&mut self, outer: &'n CallExpression, inner: &'n CallExpression, with: Option<&'n Expression>, scope: ScopeId, w: &mut Walker<'n>) {
        if !self.cx.dev {
            self.edits.overwrite(outer.start, outer.end, "void 0");
            return;
        }
        let (Some(first), Some(last)) = (inner.arguments.first(), inner.arguments.last()) else { return };
        self.edits.overwrite(outer.start, first_span(first).start, "$.inspect(() => [");
        match with {
            None => self.edits.overwrite(first_span(last).end, outer.end, "], (...$$args) => console.log(...$$args), true)"),
            Some(f) => {
                self.edits.overwrite(first_span(last).end, f.span().start, "], (...$$args) => (");
                self.edits.overwrite(f.span().end, outer.end, ")(...$$args))");
            }
        }
        self.visit_args(inner, scope, w);
        if let Some(f) = with {
            w.visit(self, NodeRef::CallExpression(outer), expr_ref(f), &scope);
        }
    }

    fn trace_call<'x>(&self, s: &'x Statement, scope: ScopeId) -> Option<&'x CallExpression> {
        let Statement::ExpressionStatement(es) = s else { return None };
        let Expression::CallExpression(c) = &es.expression else { return None };
        (get_rune_of_call(self.cx.arena, c, scope) == Some("$inspect.trace")).then_some(c)
    }

    /// `$inspect.trace(label?)` as the first statement of a function body.
    fn trace_block<'n>(&mut self, b: &'n BlockStatement, node: NodeRef<'n>, scope: ScopeId, w: &mut Walker<'n>) -> bool {
        let Some(first) = b.body.first() else { return false };
        let Some(c) = self.trace_call(first, scope) else { return false };
        let Some(func) = w.parent().filter(|p| p.is_function()) else { return false };
        if c.arguments.len() > 1 {
            self.cx.error("rune_invalid_arguments_length", c.span(), "`$inspect.trace` must be called with zero or one arguments");
        }
        let fs = first.span();
        if !self.cx.dev {
            self.edits.overwrite(fs.start, fs.end, "");
        } else {
            let is_async = match func {
                NodeRef::FunctionDeclaration(f) | NodeRef::FunctionExpression(f) => f.is_async,
                NodeRef::ArrowFunctionExpression(f) => f.is_async,
                _ => false,
            };
            let label = match c.arguments.first().and_then(arg_expr) {
                Some(a) => format!("() => ({})", self.text(a, scope)),
                None => {
                    let name = self.function_label(w, func).unwrap_or_else(|| "trace".to_string());
                    let (line, col) = line_col(self.cx.src, func.span().start);
                    format!("() => {}", js_str(&format!("{name} ({}:{line}:{col})", self.cx.filename)))
                }
            };
            let (aw, asy) = if is_async { ("await ", "async ") } else { ("", "") };
            self.cx.tracing.set(true);
            self.edits.overwrite(fs.start, fs.end, format!("return {aw}$.trace({label}, {asy}() => {{"));
            self.edits.close(b.end - 1, "});");
        }
        for s in &b.body[1..] {
            w.visit(self, node, stmt_ref(s), &scope);
        }
        true
    }

    /// `get_function_label(path)`.
    fn function_label(&self, w: &Walker<'_>, func: NodeRef<'_>) -> Option<String> {
        if let NodeRef::FunctionDeclaration(f) | NodeRef::FunctionExpression(f) = func
            && let Some(id) = &f.id
        {
            return Some(id.name.clone());
        }
        let parent = w.path.len().checked_sub(2).map(|i| w.path[i])?;
        match parent {
            NodeRef::CallExpression(c) => Some(format!("{}(...)", self.src(c.callee.span()))),
            NodeRef::Property(p) if !p.computed => classes::get_name(&p.key),
            NodeRef::VariableDeclarator(d) => match &d.id {
                Pattern::Identifier(i) => Some(i.name.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    // ---------------------------------------------------------------------------------------------
    // reads

    fn read(&mut self, id: &Identifier, parent: Option<NodeRef<'_>>) {
        if self.skip.contains(&addr(id)) {
            return;
        }
        let t = self.cx.transform_of_id(id);
        if t == T::None || (t == T::Each && self.mode != Mode::Read) {
            return;
        }
        // a `var` may be read before its declaration ran (Svelte's add_state_transformers)
        let is_var = matches!(t, T::Source { .. })
            && self.cx.arena.resolve(id).is_some_and(|b| self.cx.arena.binding(b).declaration_kind == crate::svelte::scope::DeclarationKind::Var);
        let text = if is_var { format!("$.safe_get({})", id.name) } else { read_text(&t, &id.name) };
        // `{ count }` shorthand property -> `{ count: $.get(count) }`
        if let Some(NodeRef::Property(p)) = parent
            && p.shorthand
        {
            self.edits.overwrite(p.start, p.end, format!("{}: {text}", id.name));
            return;
        }
        match t {
            T::PropDirect(m) => self.edits.overwrite(id.start, id.end, m),
            T::PropSource | T::Store(_) | T::Thunk => self.edits.wrap(id.start, id.end, "", "()"),
            _ if is_var => self.edits.wrap(id.start, id.end, "$.safe_get(", ")"),
            _ => self.edits.wrap(id.start, id.end, "$.get(", ")"),
        }
    }

    fn member_read<'n>(&mut self, m: &'n MemberExpression, node: NodeRef<'n>, scope: ScopeId, w: &mut Walker<'n>) -> bool {
        let Some(name) = Self::private_name(m) else { return false };
        let Some(f) = self.current_field(&name).cloned() else { return false };
        if self.in_constructor && matches!(f.rune, "$state" | "$state.raw") {
            self.edits.wrap(m.start, m.end, "", ".v");
        } else {
            self.edits.wrap(m.start, m.end, "$.get(", ")");
        }
        w.visit(self, node, expr_ref(&m.object), &scope);
        true
    }

    // ---------------------------------------------------------------------------------------------
    // classes

    fn class_body<'n>(&mut self, b: &'n ClassBody, node: NodeRef<'n>, scope: ScopeId, w: &mut Walker<'n>) {
        let fields = Rc::new(classes::analyze(b, self.cx, scope));
        self.classes.push(fields.clone());
        let class_name = self.class_name(w);
        let accessors = |key: &str, f: &classes::Field| {
            let member = format!("this.#{}", f.private);
            let proxy = if f.rune == "$state" { ", true" } else { "" };
            format!("get {key}() {{ return $.get({member}); }} set {key}(value) {{ $.set({member}, value{proxy}); }}")
        };
        let mut inserted = String::new();
        for f in fields.fields.iter().filter(|f| f.ctor && f.public) {
            inserted.push_str(&format!(" #{}; {}", f.private, accessors(&super::object_key(&f.name), f)));
        }
        if !inserted.is_empty() {
            self.edits.open(b.start + 1, inserted);
        }
        for el in &b.body {
            let el_ref = match el {
                ClassElement::MethodDefinition(m) => NodeRef::MethodDefinition(m),
                ClassElement::PropertyDefinition(p) => NodeRef::PropertyDefinition(p),
                ClassElement::StaticBlock(s) => NodeRef::StaticBlock(s),
                // type-only member, stripped from the output
                ClassElement::TSIndexSignature(_) => continue,
            };
            let ClassElement::PropertyDefinition(p) = el else {
                w.visit(self, node, el_ref, &scope);
                continue;
            };
            let Some(f) = fields.by_decl(addr(p)).cloned() else {
                // `count;` declared before the constructor's `this.count = $state(..)`: upstream
                // drops it (the inserted accessors replace it; an own field would shadow them)
                if !p.computed
                    && !p.is_static
                    && let Some(name) = classes::get_name(&p.key)
                    && fields.get(&name).is_some_and(|f| f.ctor && f.public)
                {
                    self.edits.overwrite(p.start, p.end, String::new());
                    continue;
                }
                w.visit(self, node, el_ref, &scope);
                continue;
            };
            let Some(Expression::CallExpression(call)) = &p.value else { continue };
            if f.public {
                let ks = key_span(&p.key);
                self.edits.overwrite(ks.start, ks.end, format!("#{}", f.private));
            }
            if self.cx.dev {
                self.edits.wrap(call.start, call.end, "$.tag(", format!(", {})", js_str(&format!("{class_name}.{}", f.name))));
            }
            self.state_call(call, f.rune, scope, w);
            if f.public {
                let key = self.src(key_span(&p.key)).to_string();
                let semi = if self.src(p.span()).trim_end().ends_with(';') { "" } else { ";" };
                self.edits.close(p.end, format!("{semi} {}", accessors(&key, &f)));
            }
        }
        self.classes.pop();
    }

    fn expression_statement<'n>(&mut self, s: &'n ExpressionStatement, scope: ScopeId, w: &mut Walker<'n>) -> bool {
        let Expression::CallExpression(c) = &s.expression else { return false };
        if get_rune_of_call(self.cx.arena, c, scope) != Some("$inspect.trace") {
            return false;
        }
        // reached only when not the first statement of a function body
        let first_of_fn = w.path.len() >= 2 && matches!(w.path[w.path.len() - 1], NodeRef::BlockStatement(b) if b.body.first().is_some_and(|f| f.span() == s.span())) && w.path[w.path.len() - 2].is_function();
        if !first_of_fn {
            self.cx.error("inspect_trace_invalid_placement", c.span(), "`$inspect.trace(...)` must be the first statement of a function body");
        }
        true
    }
}

fn first_span(a: &ArrayElement) -> Span {
    match a {
        ArrayElement::Expression(e) => e.span(),
        ArrayElement::Spread(SpreadElementNode::SpreadElement(s)) => s.span(),
    }
}

fn key_span(k: &PropertyKey) -> Span {
    match k {
        PropertyKey::Private(PrivateIdentifierNode::PrivateIdentifier(p)) => p.span(),
        PropertyKey::Expression(e) => e.span(),
    }
}

fn line_col(src: &str, pos: u32) -> (usize, usize) {
    let before = &src[..pos as usize];
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count());
    (line, col)
}

impl<'n> Visitor<'n> for Rw<'_, '_, '_> {
    type State = ScopeId;
    fn visit(&mut self, node: NodeRef<'n>, state: &ScopeId, w: &mut Walker<'n>) {
        let scope = self.cx.set_scope(node, *state);
        match node {
            NodeRef::VariableDeclaration(d) => return self.var_decl(d, node, scope, w),
            NodeRef::ClassBody(b) => return self.class_body(b, node, scope, w),
            NodeRef::ImportDeclaration(_) | NodeRef::ExportSpecifier(_) => return,
            NodeRef::FunctionDeclaration(_) | NodeRef::FunctionExpression(_) | NodeRef::ArrowFunctionExpression(_) => {
                let saved = self.in_constructor;
                self.in_constructor = matches!(node, NodeRef::FunctionExpression(_)) && matches!(w.parent(), Some(NodeRef::MethodDefinition(m)) if m.kind == MethodKind::Constructor);
                w.next(self, node, &scope);
                self.in_constructor = saved;
                return;
            }
            NodeRef::BlockStatement(b) => {
                if self.trace_block(b, node, scope, w) {
                    return;
                }
            }
            NodeRef::ExpressionStatement(s) => {
                if self.expression_statement(s, scope, w) {
                    return;
                }
            }
            NodeRef::AssignmentExpression(a) => {
                if self.assignment(a, node, scope, w) {
                    return;
                }
            }
            NodeRef::UpdateExpression(u) => {
                if self.update(u, node, scope, w) {
                    return;
                }
            }
            NodeRef::CallExpression(c) => {
                if self.call(c, scope, w) {
                    return;
                }
            }
            NodeRef::MemberExpression(m) => {
                if self.member_read(m, node, scope, w) {
                    return;
                }
            }
            NodeRef::Identifier(id) => {
                let parent = w.parent();
                self.read(id, parent);
                return;
            }
            _ => {}
        }
        w.next(self, node, &scope);
    }
}
