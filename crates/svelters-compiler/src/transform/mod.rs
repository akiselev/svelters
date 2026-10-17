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
}

impl<'a> ScopeRewriter<'a> {
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

        let mut rw = ScopeRewriter { cx, root, script: Edits::new(), module: Edits::new(), exports, has_stores: false };
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

impl Rewriter for ScopeRewriter<'_> {
    fn expr(&self, e: &Expression, mode: Mode, edits: &mut Edits) {
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
            for script in [&self.root.module, &self.root.instance].into_iter().flatten() {
                for stmt in &script.content.body {
                    if let Statement::ImportDeclaration(_) = stmt {
                        let s = stmt.span();
                        w.copy(self.cx.src, s.start, s.end);
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
    w.write("import * as $ from 'svelte/internal/client';\n");
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
