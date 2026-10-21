//! Rune and reactivity transforms as span edits over the user's JavaScript, driven by the ported
//! scope analysis (`svelte::scope`). Implements [`codegen::component::Rewriter`] for components
//! and [`transform_module`] for `.svelte.js` modules.
//!
//! The visitors follow Svelte 5.57.2's client transform (`phases/3-transform/client/visitors`:
//! VariableDeclaration, ClassBody, AssignmentExpression, UpdateExpression, Identifier,
//! CallExpression, MemberExpression, Program's store/prop transforms; `transform-client.js`
//! for store setup, component exports and `client_module`) and emit the same runtime calls, but
//! as edits over the source text instead of an AST reprint (see `codegen/edit.rs`). The supported
//! subset and the departures are listed in docs/notes/native-compiler/language-subset.md and
//! deviations.md ("Rune transform").
//!
//! - [`visit`]: the edit-producing visitor (reads, writes, rune calls, declarations, validation).
//! - [`classes`]: class state fields (`phases/2-analyze/visitors/ClassBody.js` + the ClassBody
//!   transform).
//! - [`paths`]: `extract_paths` for destructuring, producing JS text.

mod classes;
mod paths;
mod visit;

use crate::codegen::component::{Mode, Rewriter, js_str};
use crate::codegen::edit::Edits;
use crate::codegen::writer::Writer;
use crate::diag::Diagnostics;
use crate::estree::*;
use crate::svelte::ast::Root;
use crate::svelte::scope::analyze::ComponentScopeAnalysis;
use crate::svelte::scope::walk::{NodeKey, NodeRef, Visitor, Walker, expr_ref};
use crate::svelte::scope::{BindingId, BindingKind, DeclarationKind, Initial, ScopeArena, ScopeId};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use visit::Rw;

pub(crate) const PROPS_IS_IMMUTABLE: u32 = 1;
pub(crate) const PROPS_IS_RUNES: u32 = 1 << 1;
pub(crate) const PROPS_IS_UPDATED: u32 = 1 << 2;
pub(crate) const PROPS_IS_BINDABLE: u32 = 1 << 3;
pub(crate) const PROPS_IS_LAZY_INITIAL: u32 = 1 << 4;

/// How references to a binding are rewritten (`state.transform[name]` upstream).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum T {
    /// Plain JS: no rewrite.
    None,
    /// A source (`$state` that is reassigned, `$derived`): `$.get` / `$.set` / `$.update`.
    /// `proxy`: assignments proxy the value (`$state`, not `$state.raw`/`$derived`).
    Source { proxy: bool },
    /// `$.prop(...)` getter/setter function.
    PropSource,
    /// A prop read straight from `$$props` (never written, no fallback).
    PropDirect(String),
    /// `$store` auto-subscription: read `$store()`, writes through the store (the field is the
    /// JS text of the store expression).
    Store(String),
    /// each-block item / index: a runtime source of the native each block (plain in the key fn).
    Each,
    /// `{@const x}` (a derived) and `{:then v}` / `{:catch e}` (sources): read `$.get(x)`, not
    /// assignable.
    ReadOnly,
    /// Snippet parameter: a thunk, read `x()`, not assignable.
    Thunk,
}

/// Everything the visitors share: the analysis, options, diagnostics and generated names.
pub(crate) struct Ctx<'r, 'a> {
    pub src: &'r str,
    pub arena: &'r ScopeArena<'a>,
    pub maps: Vec<&'r HashMap<NodeKey, ScopeId>>,
    pub dev: bool,
    pub filename: String,
    #[allow(dead_code)]
    pub module_scope: ScopeId,
    /// `None` for `.svelte.js` modules.
    pub instance_scope: Option<ScopeId>,
    /// Bindings treated as reassigned (exported with `export { x }`, as upstream's ExportSpecifier
    /// visitor marks them).
    pub forced: HashSet<BindingId>,
    pub diags: RefCell<Diagnostics>,
    taken: RefCell<HashSet<String>>,
    /// Module-level statements (`var rest_excludes = new Set(...)`).
    pub hoisted: RefCell<Vec<String>>,
    pub props_id: Cell<Option<BindingId>>,
    pub has_props_rune: Cell<bool>,
    /// A `$inspect.trace` was emitted (dev): the module imports the core's tracing flag
    /// (Svelte's `analysis.tracing` -> `import 'svelte/internal/flags/tracing'`).
    pub tracing: Cell<bool>,
}

impl<'r, 'a> Ctx<'r, 'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(src: &'r str, arena: &'r ScopeArena<'a>, maps: Vec<&'r HashMap<NodeKey, ScopeId>>, module_scope: ScopeId, instance_scope: Option<ScopeId>, dev: bool, filename: &str, diags: Diagnostics) -> Self {
        let mut taken: HashSet<String> = arena.root.conflicts.iter().cloned().collect();
        for i in 0..arena.scope_count() {
            let s = arena.scope(ScopeId(i as u32));
            taken.extend(s.declarations.keys().map(str::to_string));
            taken.extend(s.references.keys().map(str::to_string));
        }
        Ctx {
            src,
            arena,
            maps,
            dev,
            filename: filename.to_string(),
            module_scope,
            instance_scope,
            forced: HashSet::new(),
            diags: RefCell::new(diags),
            taken: RefCell::new(taken),
            hoisted: RefCell::new(Vec::new()),
            props_id: Cell::new(None),
            has_props_rune: Cell::new(false),
            tracing: Cell::new(false),
        }
    }

    pub fn error(&self, code: &'static str, span: Span, message: impl Into<String>) {
        self.diags.borrow_mut().error(code, (span.start, span.end), message);
    }

    pub fn set_scope(&self, node: NodeRef<'_>, current: ScopeId) -> ScopeId {
        let k = node.key();
        self.maps.iter().find_map(|m| m.get(&k).copied()).unwrap_or(current)
    }

    /// `scope.generate(preferred)` (names are unique per compilation, which is stricter).
    pub fn generate(&self, preferred: &str) -> String {
        let base: String = preferred.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '$' { c } else { '_' }).collect();
        let mut taken = self.taken.borrow_mut();
        let mut name = base.clone();
        let mut n = 1;
        while taken.contains(&name) {
            name = format!("{base}_{n}");
            n += 1;
        }
        taken.insert(name.clone());
        name
    }

    pub fn reassigned(&self, b: BindingId) -> bool {
        self.arena.binding(b).reassigned || self.forced.contains(&b)
    }

    /// `is_state_source` in runes mode (immutable, no accessors).
    pub fn is_state_source(&self, b: BindingId) -> bool {
        matches!(self.arena.binding(b).kind, BindingKind::State | BindingKind::RawState) && self.reassigned(b)
    }

    /// `is_prop_source` in runes mode.
    pub fn is_prop_source(&self, b: BindingId) -> bool {
        let x = self.arena.binding(b);
        matches!(x.kind, BindingKind::Prop | BindingKind::BindableProp) && (self.reassigned(b) || x.initial.is_some() || x.updated())
    }

    pub fn transform_of(&self, b: BindingId) -> T {
        let x = self.arena.binding(b);
        match x.kind {
            BindingKind::State if self.is_state_source(b) => T::Source { proxy: true },
            BindingKind::RawState if self.is_state_source(b) => T::Source { proxy: false },
            BindingKind::Derived => T::Source { proxy: false },
            BindingKind::Prop | BindingKind::BindableProp if self.is_prop_source(b) => T::PropSource,
            BindingKind::Prop | BindingKind::BindableProp => {
                let key = x.prop_alias.clone().unwrap_or_else(|| x.node.name().to_string());
                T::PropDirect(member_key("$$props", &key))
            }
            BindingKind::StoreSub => T::Store(self.store_text(&x.node.name()[1..])),
            BindingKind::Each => T::Each,
            BindingKind::Template | BindingKind::Static if matches!(x.initial, Some(Initial::EachBlock(_))) => T::Each,
            BindingKind::Template => T::ReadOnly,
            BindingKind::Snippet => T::Thunk,
            _ => T::None,
        }
    }

    pub fn transform_of_id(&self, id: &Identifier) -> T {
        self.arena.resolve(id).map_or(T::None, |b| self.transform_of(b))
    }

    /// `build_getter(b.id(name))` for a name resolved in the instance scope (store expressions).
    pub fn store_text(&self, name: &str) -> String {
        let Some(b) = self.instance_scope.and_then(|s| self.arena.get(s, name)) else { return name.to_string() };
        read_text(&self.transform_of(b), name)
    }
}

/// The read of `name` under transform `t`, as text.
pub(crate) fn read_text(t: &T, name: &str) -> String {
    match t {
        T::Source { .. } | T::Each | T::ReadOnly => format!("$.get({name})"),
        T::PropSource | T::Store(_) | T::Thunk => format!("{name}()"),
        T::PropDirect(m) => m.clone(),
        T::None => name.to_string(),
    }
}

pub(crate) fn is_identifier_name(s: &str) -> bool {
    let mut cs = s.chars();
    cs.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$') && cs.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// `obj.key` or `obj["key"]`.
pub(crate) fn member_key(obj: &str, key: &str) -> String {
    if is_identifier_name(key) { format!("{obj}.{key}") } else { format!("{obj}[{}]", js_str(key)) }
}

/// `b.key(name)`: an identifier, or a string literal.
pub(crate) fn object_key(key: &str) -> String {
    if is_identifier_name(key) { key.to_string() } else { js_str(key) }
}

/// The instance script transform plus template expression rewriting for one component.
pub struct ScopeRewriter<'a> {
    cx: Ctx<'a, 'a>,
    root: &'a Root,
    /// Edits over the instance script / module script source ranges.
    script: Edits,
    module: Edits,
    exports: Vec<(String, String)>,
    has_stores: bool,
    /// TypeScript to remove from the emitted text (`svelte::ts`; empty for JavaScript).
    ts_strip: crate::svelte::ts::TsStrip,
    component_scopes: &'a HashMap<NodeKey, crate::svelte::scope::ComponentScopes>,
}

impl<'a> ScopeRewriter<'a> {
    /// Removes the TypeScript found by `svelte::ts::strip` from everything this rewriter emits.
    pub fn strip_typescript(&mut self, strip: crate::svelte::ts::TsStrip) {
        strip.apply(&mut self.script, None);
        strip.apply(&mut self.module, None);
        self.ts_strip = strip;
    }

    pub fn new(src: &'a str, root: &'a Root, an: &'a ComponentScopeAnalysis<'a>, diags: Diagnostics) -> ScopeRewriter<'a> {
        ScopeRewriter::with_options(src, root, an, diags, false, "Component.svelte")
    }

    pub fn with_options(src: &'a str, root: &'a Root, an: &'a ComponentScopeAnalysis<'a>, diags: Diagnostics, dev: bool, filename: &str) -> ScopeRewriter<'a> {
        let maps = vec![&an.template.scopes, &an.module.scopes, &an.instance.scopes];
        let mut cx = Ctx::new(src, &an.arena, maps, an.module.scope, Some(an.instance.scope), dev, filename, diags);

        // exports (ExportNamedDeclaration / ExportSpecifier analysis, instance in runes mode)
        let mut exports = Vec::new();
        if let Some(script) = &root.instance {
            for stmt in &script.content.body {
                let Statement::ExportNamedDeclaration(e) = stmt else { continue };
                if let Some(d) = &e.declaration {
                    match d.as_ref() {
                        Statement::FunctionDeclaration(f) => {
                            if let Some(i) = &f.id {
                                exports.push((i.name.clone(), i.name.clone()));
                            }
                        }
                        Statement::ClassDeclaration(c) => {
                            if let Some(i) = &c.id {
                                exports.push((i.name.clone(), i.name.clone()));
                            }
                        }
                        Statement::VariableDeclaration(v) if v.kind == VariableKind::Const => {
                            for decl in &v.declarations {
                                for id in crate::svelte::scope::ast_utils::extract_identifiers(&decl.id) {
                                    exports.push((id.name.clone(), id.name.clone()));
                                }
                            }
                        }
                        _ => {}
                    }
                } else if e.source.is_none() {
                    for s in &e.specifiers {
                        let local = export_name(&s.local);
                        let exported = export_name(&s.exported);
                        if let Some(b) = cx.arena.get(an.instance.scope, &local) {
                            cx.forced.insert(b);
                        }
                        exports.push((local, exported));
                    }
                }
            }
        }

        let mut rw = ScopeRewriter { cx, root, script: Edits::new(), module: Edits::new(), exports, has_stores: false, ts_strip: Default::default(), component_scopes: &an.template.component_scopes };
        let has = rw.store_subs().next().is_some();
        rw.has_stores = has;

        if let Some(script) = &root.module {
            let mut edits = Edits::new();
            {
                let mut v = Rw::new(&rw.cx, &mut edits, Mode::Read, false);
                for stmt in &script.content.body {
                    v.top_level_statement(stmt, an.module.scope, false);
                }
            }
            rw.module = edits;
        }
        if let Some(script) = &root.instance {
            let mut edits = Edits::new();
            {
                let mut v = Rw::new(&rw.cx, &mut edits, Mode::Read, true);
                for stmt in &script.content.body {
                    v.top_level_statement(stmt, an.instance.scope, true);
                }
            }
            rw.script = edits;
        }
        rw
    }

    pub fn into_diagnostics(self) -> Diagnostics {
        self.cx.diags.into_inner()
    }

    /// The component function's name: `preferred`, renamed when the component declares or
    /// references that name (Svelte's `module.scope.generate(component_name)`).
    pub fn component_name(&self, preferred: &str) -> String {
        self.cx.generate(preferred)
    }

    fn store_subs(&self) -> impl Iterator<Item = (&str, BindingId)> + '_ {
        let scope = self.cx.instance_scope.expect("component");
        self.cx.arena.scope(scope).declarations.iter().filter(|(_, b)| self.cx.arena.binding(**b).kind == BindingKind::StoreSub).map(|(n, b)| (n, *b))
    }

    /// `component_returned_object` (non-dev shape).
    fn exports_object(&self) -> Option<String> {
        if self.exports.is_empty() {
            return None;
        }
        let scope = self.cx.instance_scope.expect("component");
        let mut parts = Vec::new();
        for (name, alias) in &self.exports {
            let binding = self.cx.arena.get(scope, name);
            let t = binding.map_or(T::None, |b| self.cx.transform_of(b));
            let expr = read_text(&t, name);
            let key = object_key(alias);
            let getter = format!("get {key}() {{ return {expr}; }}");
            let b = binding.map(|b| self.cx.arena.binding(b));
            if t == T::None {
                if b.is_some_and(|b| matches!(b.declaration_kind, DeclarationKind::Let | DeclarationKind::Var)) {
                    parts.push(format!("{getter}, set {key}($$value) {{ {name} = $$value; }}"));
                } else if self.cx.dev {
                    parts.push(getter);
                } else if key == *name {
                    parts.push(name.clone());
                } else {
                    parts.push(format!("{key}: {name}"));
                }
                continue;
            }
            match b.map(|b| b.kind) {
                Some(BindingKind::Prop | BindingKind::BindableProp) => parts.push(format!("{getter}, set {key}($$value) {{ {name}($$value); }}")),
                Some(BindingKind::State) => parts.push(format!("{getter}, set {key}($$value) {{ $.set({name}, $.proxy($$value)); }}")),
                Some(BindingKind::RawState) => parts.push(format!("{getter}, set {key}($$value) {{ $.set({name}, $$value); }}")),
                _ => parts.push(getter),
            }
        }
        Some(format!("{{ {} }}", parts.join(", ")))
    }
}

fn export_name(n: &ModuleExportName) -> String {
    match n {
        ModuleExportName::Identifier(i) => i.name.clone(),
        ModuleExportName::Literal(l) => match &l.value {
            LiteralValue::String(s) => s.clone(),
            _ => l.raw.clone(),
        },
    }
}

impl ScopeRewriter<'_> {
    /// Svelte's `can_hoist_snippet` (2-analyze/visitors/SnippetBlock.js): the snippet references
    /// nothing declared inside the component function (other than hoistable snippets).
    fn can_hoist_snippet(&self, scope: ScopeId, visited: &mut HashSet<BindingId>) -> bool {
        let arena = self.cx.arena;
        let depth = arena.scope(scope).function_depth;
        for name in arena.scope(scope).references.keys() {
            let Some(b) = arena.get(scope, name) else { continue };
            let binding = arena.binding(b);
            let d = arena.scope(binding.scope).function_depth;
            if d == 0 || d >= depth {
                continue;
            }
            if let Some(Initial::SnippetBlock(sb)) = binding.initial {
                if !visited.insert(b) {
                    continue;
                }
                if let Some(s) = self.snippet_scope(sb)
                    && self.can_hoist_snippet(s, visited)
                {
                    continue;
                }
            }
            return false;
        }
        true
    }

    fn snippet_scope(&self, sb: &crate::svelte::ast::SnippetBlock) -> Option<ScopeId> {
        let key = NodeRef::SnippetBlock(sb).key();
        self.cx.maps.iter().find_map(|m| m.get(&key).copied())
    }
}

impl Rewriter for ScopeRewriter<'_> {
    fn dynamic_component(&self, node: &crate::svelte::ast::Element) -> Option<String> {
        let cs = self.component_scopes.get(&NodeRef::Component(node).key())?;
        let scope = self.cx.arena.scope(cs.default).parent?;
        let (first, rest) = node.name.split_once('.').map_or((node.name.as_str(), None), |(a, b)| (a, Some(b)));
        let b = self.cx.arena.get(scope, first)?;
        if self.cx.arena.binding(b).kind == BindingKind::Normal && rest.is_none() {
            return None;
        }
        let read = read_text(&self.cx.transform_of(b), first);
        Some(match rest {
            Some(r) => format!("{read}.{r}"),
            None => read,
        })
    }

    fn hoist_snippet(&self, start: u32) -> bool {
        let Some(sb) = self.cx.arena.bindings().find_map(|(_, b)| match b.initial {
            Some(Initial::SnippetBlock(sb)) if sb.start == start => Some(sb),
            _ => None,
        }) else {
            return false;
        };
        self.snippet_scope(sb).is_some_and(|s| self.can_hoist_snippet(s, &mut HashSet::new()))
    }

    fn expr(&self, e: &Expression, mode: Mode, edits: &mut Edits) {
        self.ts_strip.apply(edits, Some(e.span()));
        let mut v = Rw::new(&self.cx, edits, mode, true);
        Walker::walk(expr_ref(e), &self.cx.instance_scope.expect("component"), &mut v);
    }

    fn is_reactive(&self, e: &Expression) -> bool {
        struct R<'x, 'r, 'a> {
            cx: &'x Ctx<'r, 'a>,
            found: bool,
        }
        impl<'n> Visitor<'n> for R<'_, '_, '_> {
            type State = ();
            fn visit(&mut self, node: NodeRef<'n>, state: &(), w: &mut Walker<'n>) {
                match node {
                    NodeRef::Identifier(id) => {
                        if let Some(b) = self.cx.arena.resolve(id) {
                            let x = self.cx.arena.binding(b);
                            // state that is never reassigned is a proxy (or a constant): reads of
                            // its properties are reactive; imports may be proxies or sources too
                            // like upstream's `has_state`: anything but a function or a known
                            // constant may read state (proxies, class state fields, imports)
                            let stateful = match x.kind {
                                BindingKind::Normal => {
                                    x.declaration_kind == DeclarationKind::Import
                                        || !(x.is_function()
                                            || matches!(x.declaration_kind, DeclarationKind::Function)
                                            || (!x.reassigned && matches!(x.initial, Some(Initial::Expression(Expression::Literal(_))))))
                                }
                                BindingKind::Static => false,
                                _ => true,
                            };
                            if stateful {
                                self.found = true;
                            }
                        }
                    }
                    NodeRef::CallExpression(_) | NodeRef::NewExpression(_) | NodeRef::TaggedTemplateExpression(_) => self.found = true,
                    _ => {}
                }
                if !self.found {
                    w.next(self, node, state);
                }
            }
        }
        let mut r = R { cx: &self.cx, found: false };
        Walker::walk(expr_ref(e), &(), &mut r);
        r.found
    }

    fn uses_store(&self, e: &Expression) -> bool {
        struct S<'x, 'r, 'a> {
            cx: &'x Ctx<'r, 'a>,
            found: bool,
        }
        impl<'n> Visitor<'n> for S<'_, '_, '_> {
            type State = ();
            fn visit(&mut self, node: NodeRef<'n>, state: &(), w: &mut Walker<'n>) {
                if let NodeRef::Identifier(id) = node
                    && self.cx.arena.resolve(id).is_some_and(|b| self.cx.arena.binding(b).kind == BindingKind::StoreSub)
                {
                    self.found = true;
                }
                if !self.found {
                    w.next(self, node, state);
                }
            }
        }
        let mut s = S { cx: &self.cx, found: false };
        Walker::walk(expr_ref(e), &(), &mut s);
        s.found
    }

    fn has_call(&self, e: &Expression) -> bool {
        // upstream: a CallExpression whose callee is not pure (`is_pure`: literals, globals and
        // members of globals, calls of those with pure arguments) or any call when the expression
        // has dependencies; a tagged template with an impure tag; any spread element
        struct H<'x, 'r, 'a> {
            cx: &'x Ctx<'r, 'a>,
            impure: bool,
            calls: bool,
            deps: bool,
        }
        impl H<'_, '_, '_> {
            fn is_pure(&self, e: &Expression) -> bool {
                match e {
                    Expression::Literal(_) => true,
                    Expression::CallExpression(c) => {
                        self.is_pure(&c.callee) && c.arguments.iter().all(|a| matches!(a, ArrayElement::Expression(x) if self.is_pure(x)))
                    }
                    Expression::Identifier(_) | Expression::MemberExpression(_) => {
                        let mut left = e;
                        while let Expression::MemberExpression(m) = left {
                            left = &m.object;
                        }
                        match left {
                            // `$effect.tracking()` is never pure; other globals are assumed to be
                            Expression::Identifier(id) => self.cx.arena.resolve(id).is_none() && !(id.name == "$effect"),
                            other => self.is_pure(other),
                        }
                    }
                    _ => false,
                }
            }
        }
        impl<'n> Visitor<'n> for H<'_, '_, '_> {
            type State = ();
            fn visit(&mut self, node: NodeRef<'n>, state: &(), w: &mut Walker<'n>) {
                match node {
                    n if n.is_function() => return,
                    NodeRef::Identifier(id) => {
                        if self.cx.arena.resolve(id).is_some() {
                            self.deps = true;
                        }
                    }
                    NodeRef::CallExpression(c) => {
                        self.calls = true;
                        if !self.is_pure(&c.callee) {
                            self.impure = true;
                        }
                    }
                    NodeRef::TaggedTemplateExpression(t) => {
                        if !self.is_pure(&t.tag) {
                            self.impure = true;
                        }
                    }
                    NodeRef::SpreadElement(_) => self.impure = true,
                    _ => {}
                }
                w.next(self, node, state);
            }
        }
        let mut h = H { cx: &self.cx, impure: false, calls: false, deps: false };
        Walker::walk(expr_ref(e), &(), &mut h);
        h.impure || (h.calls && h.deps)
    }

    fn tracing(&self) -> bool {
        self.cx.tracing.get()
    }

    fn identifier(&self, id: &Identifier) -> String {
        read_text(&self.cx.transform_of_id(id), &id.name)
    }

    fn declaration(&self, d: &VariableDeclaration) -> String {
        let mut edits = Edits::new();
        self.ts_strip.apply(&mut edits, Some(d.span()));
        // the declaration's bindings live in the scope of its fragment: start there (the binding
        // whose node is the first declared identifier)
        let first = d.declarations.first().and_then(|x| crate::svelte::scope::ast_utils::extract_identifiers(&x.id).first().map(|i| i.span()));
        let scope = first
            .and_then(|span| self.cx.arena.bindings().find(|(_, b)| b.node.span() == Some(span)).map(|(_, b)| b.scope))
            .unwrap_or(self.cx.instance_scope.expect("component"));
        let mut v = visit::Rw::new(&self.cx, &mut edits, Mode::Read, false);
        Walker::walk(NodeRef::VariableDeclaration(d), &scope, &mut v);
        let s = d.span();
        edits.apply(self.cx.src, s.start, s.end).trim_end().trim_end_matches(';').to_string()
    }

    fn directive_name(&self, name: &str) -> String {
        // `parse_directive_name` + visit: the first segment is an identifier read as usual. The
        // scope analysis records directive names as synthetic references, so it is resolved from
        // the instance scope (a name bound only in a template scope reads as written).
        let (first, rest) = name.split_once('.').map_or((name, None), |(a, b)| (a, Some(b)));
        let read = match self.cx.instance_scope.and_then(|s| self.cx.arena.get(s, first)) {
            Some(b) => read_text(&self.cx.transform_of(b), first),
            None => first.to_string(),
        };
        match rest {
            Some(r) => format!("{read}.{r}"),
            None => read,
        }
    }

    fn destructure(&self, pattern: &Pattern, initial: &str, derived_arrays: bool) -> crate::codegen::component::Destructured {
        let mut edits = Edits::new();
        let rw = visit::Rw::new(&self.cx, &mut edits, Mode::Read, true);
        let (inserts, paths) = rw.extract_paths(pattern, initial.to_string(), self.cx.instance_scope.expect("component"), derived_arrays);
        crate::codegen::component::Destructured {
            arrays: inserts.into_iter().map(|i| (i.id, i.value)).collect(),
            leaves: paths
                .into_iter()
                .filter_map(|p| match p.node {
                    paths::Target::Id(id) => Some((id.name.clone(), p.expression, p.has_default_value)),
                    paths::Target::Member(_) => None,
                })
                .collect(),
        }
    }

    fn assign(&self, target: &Expression, value: &str) -> String {
        match target {
            Expression::Identifier(id) => {
                let b = self.cx.arena.resolve(id);
                match self.cx.transform_of_id(id) {
                    // never proxied: bound values are native values or runtime nodes
                    T::Source { .. } if b.is_some_and(|b| self.cx.arena.binding(b).kind == BindingKind::Derived && !self.cx.arena.binding(b).updated()) => {
                        self.cx.error("bind_invalid_value", id.span(), "Can only bind to state or props");
                        "void 0".to_string()
                    }
                    T::Source { .. } => format!("$.set({}, {value})", id.name),
                    T::PropSource => format!("{}({value})", id.name),
                    T::Store(s) => format!("$.store_set({s}, {value})"),
                    T::Each => {
                        self.cx.error("each_item_invalid_assignment", id.span(), visit::EACH_ITEM_MSG);
                        "void 0".to_string()
                    }
                    T::PropDirect(_) => {
                        self.cx.error("bind_invalid_value", id.span(), "Can only bind to state or props");
                        "void 0".to_string()
                    }
                    T::ReadOnly | T::Thunk => {
                        self.cx.error("bind_invalid_target", id.span(), format!("cannot bind to `{}`: it is not assignable", id.name));
                        "void 0".to_string()
                    }
                    T::None => format!("{} = {value}", id.name),
                }
            }
            _ => {
                let mut edits = Edits::new();
                self.expr(target, Mode::Read, &mut edits);
                let s = target.span();
                let text = edits.apply(self.cx.src, s.start, s.end);
                // mutation through a store or a bindable prop (`bind:x={$store.field}`)
                let mut root = target;
                while let Expression::MemberExpression(m) = root {
                    root = &m.object;
                }
                if let Expression::Identifier(id) = root
                    && let Some(b) = self.cx.arena.resolve(id)
                {
                    let t = self.cx.transform_of(b);
                    let root_text = read_text(&t, &id.name);
                    match t {
                        T::Store(store) if text.starts_with(&root_text) => {
                            let untracked = format!("$.untrack({})", id.name);
                            return format!("$.store_mutate({store}, {untracked}{} = {value}, {untracked})", &text[root_text.len()..]);
                        }
                        T::PropSource if self.cx.arena.binding(b).kind == BindingKind::BindableProp => return format!("{}({text} = {value}, true)", id.name),
                        _ => {}
                    }
                }
                format!("{text} = {value}")
            }
        }
    }

    fn script(&self, w: &mut Writer, hoist: bool) {
        if hoist {
            for (script, edits) in [(&self.root.module, &self.module), (&self.root.instance, &self.script)] {
                let Some(script) = script else { continue };
                for stmt in &script.content.body {
                    if let Statement::ImportDeclaration(_) = stmt {
                        let s = stmt.span();
                        // edits on imports are TypeScript removals (`import { type A, B }`)
                        edits.emit(w, self.cx.src, s.start, s.end);
                        w.write("\n");
                    }
                }
            }
            if let Some(script) = &self.root.module {
                for stmt in &script.content.body {
                    if !matches!(stmt, Statement::ImportDeclaration(_)) {
                        let s = stmt.span();
                        self.module.emit(w, self.cx.src, s.start, s.end);
                        w.write("\n");
                    }
                }
            }
            for h in self.cx.hoisted.borrow().iter() {
                w.write(h);
                w.write("\n");
            }
            return;
        }
        // store subscriptions (`store_setup`, then `store_init`)
        for (name, _) in self.store_subs() {
            let store = self.cx.store_text(&name[1..]);
            let get = format!("$.store_get({store}, {}, $$stores)", js_str(name));
            w.newline();
            if self.cx.dev {
                w.write(&format!("const {name} = () => ($.validate_store({store}, {}), {get});", js_str(&name[1..])));
            } else {
                w.write(&format!("const {name} = () => {get};"));
            }
        }
        if self.has_stores {
            w.newline();
            w.write("const [$$stores, $$cleanup] = $.setup_stores();");
        }
        let Some(script) = &self.root.instance else { return };
        for stmt in &script.content.body {
            if matches!(stmt, Statement::ImportDeclaration(_)) {
                continue;
            }
            let s = stmt.span();
            let text = self.script.apply(self.cx.src, s.start, s.end);
            if text.trim().is_empty() {
                continue;
            }
            w.newline();
            self.script.emit(w, self.cx.src, s.start, s.end);
        }
        if let Some(obj) = self.exports_object() {
            w.newline();
            w.write(&format!("var $$exports = {obj};"));
        }
    }

    fn epilogue(&self, w: &mut Writer) {
        let exports = !self.exports.is_empty();
        match (exports, self.has_stores) {
            (false, false) => w.write("$.pop();"),
            (true, false) => w.write("return $.pop($$exports);"),
            (false, true) => {
                w.write("$.pop();");
                w.newline();
                w.write("$$cleanup();");
            }
            (true, true) => {
                w.write("var $$pop = $.pop($$exports);");
                w.newline();
                w.write("$$cleanup();");
                w.newline();
                w.write("return $$pop;");
            }
        }
    }
}

/// `client_module`: transforms a `.svelte.js` module (already analysed) into JS text + map.
pub fn transform_module(src: &str, program: &Program, an: &crate::svelte::scope::analyze::ModuleScopeAnalysis<'_>, dev: bool, filename: &str, diags: Diagnostics) -> (String, String, Diagnostics) {
    let cx = Ctx::new(src, &an.arena, vec![&an.module.scopes], an.module.scope, None, dev, filename, diags);
    let mut edits = Edits::new();
    {
        let mut v = Rw::new(&cx, &mut edits, Mode::Read, false);
        for stmt in &program.body {
            v.top_level_statement(stmt, an.module.scope, false);
        }
    }
    let mut w = Writer::new(src);
    w.write(&format!("import * as $ from '{}';\n", crate::target::RuntimeImports::default().core));
    edits.emit(&mut w, src, 0, src.len() as u32);
    for h in cx.hoisted.borrow().iter() {
        w.write("\n");
        w.write(h);
    }
    if !src.ends_with('\n') {
        w.write("\n");
    }
    let base = filename.rsplit('/').next().unwrap_or(filename);
    let (code, map) = w.finish(filename, src, base);
    (code, map, cx.diags.into_inner())
}
