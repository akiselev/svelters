//! Port of `svelte/src/compiler/phases/scope.js` (Svelte 5.57.2 @ f3de3f2). Owner: agent E.
//!
//! Supporting ports live in `scope/`: `walk` (zimmerframe), `is_reference` (is-reference),
//! `ast_utils` (utils/ast.js), `support` (utils.js, nodes.js, utils/slot.js), `js` (the JS
//! operator semantics `Evaluation` borrows from the engine) and `analyze` (the scope-related
//! slices of `phases/2-analyze`: component scope creation, store subscriptions, rune binding kinds).
//!
//! # Rust design
//!
//! - **Node identity.** The AST has no node ids and is immutable after parsing, so a node is
//!   identified by [`NodeKey`] (its address inside the borrowed AST plus its `type`). Everything
//!   here borrows the AST for `'a`, so keys stay valid. Upstream `Map<Node, Scope>` becomes
//!   `HashMap<NodeKey, ScopeId>`; `node === other` becomes a key / `ptr::eq` comparison.
//! - **Arenas.** All scopes, bindings and references of one component (one upstream
//!   `ScopeRoot`) live in a [`ScopeArena`]: `Scope` → [`ScopeId`], `Binding` → [`BindingId`],
//!   each `{ node, path }` reference record → [`ReferenceId`]. Upstream methods on `Scope`
//!   (`declare`, `get`, `reference`, `evaluate`, ...) are arena methods taking the `ScopeId`.
//! - **Synthetic identifiers.** Upstream builds `b.id(name)` nodes that are not in the AST
//!   (`let:x` without expression, `{#each ... , i}` index, store subscriptions, directive
//!   names); [`Ident::Synthetic`] carries their name (and span when upstream gives one).
//! - **Errors.** Upstream throws; here `create_scopes` stops the walk at the first error and
//!   returns it (`scope_errors` has the constructors).
//! - **Resolution.** [`ScopeArena::resolve`] answers, for any AST `Identifier` that was recorded
//!   as a reference, which binding it resolves to (`None`: a global).
//!
//! SVELTERS deviations:
//! - `node.metadata` is not stored on the AST: the per-node metadata scope creation writes
//!   (`Component.metadata.scopes`, `EachBlock.metadata.index` / `.declarations`) is returned in
//!   [`CreatedScopes`]; `Fragment.metadata.transparent` (set by the parser) is recomputed from the
//!   parent: fragments of element-likes are transparent unless they directly contain a
//!   `DeclarationTag` (`phases/1-parse/state/element.js` + `Parser.pop()`).
//! - `Binding.blocker` and `Scope.tracing` (set during analysis/transform) are not ported.
//! - `Evaluation` of a cyclic expression returns the in-progress evaluation's initial state
//!   (`is_known`, `value: undefined`, empty value set) instead of the live partially-filled object.
//! - the `importKind === 'type'` exception of `validate_identifier_name` is dropped (the AST has
//!   no TS nodes).

pub mod analyze;
pub mod ast_utils;
pub mod is_reference;
pub mod js;
pub mod support;
pub mod walk;

#[cfg(test)]
mod tests;

use crate::estree::*;
use crate::svelte::ast;
use crate::svelte::errors::CompileError;
use crate::svelte::scope_errors as e;
use ast_utils::{
    PatternTarget, extract_identifiers, extract_identifiers_from_destructuring, object, unwrap_expression_pattern,
    unwrap_pattern,
};
use is_reference::is_reference;
use js::{JsValue, Sym, ValueSet};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use support::{as_rune, determine_slot, is_reserved, is_rune};
use walk::{NodeRef, Visitor, Walker, attribute_like_ref, expr_ref, fragment_node_ref, pattern_ref};

pub use walk::NodeKey;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScopeId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BindingId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ReferenceId(pub u32);

/// `BindingKind` (types/index.d.ts).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BindingKind {
    /// A variable that is not in any way special
    Normal,
    /// A normal prop (possibly reassigned or mutated)
    Prop,
    /// A prop one can `bind:` to (possibly reassigned or mutated)
    BindableProp,
    /// A rest prop
    RestProp,
    /// A `$state` variable
    State,
    /// A `$state.raw` variable
    RawState,
    /// A `$derived` variable
    Derived,
    /// An each block parameter
    Each,
    /// A snippet parameter
    Snippet,
    /// A `$store` value
    StoreSub,
    /// A `$:` declaration
    LegacyReactive,
    /// A binding declared in the template, e.g. in an `await` block or `const` tag
    Template,
    /// A binding whose value is known to be static (i.e. each index)
    Static,
}

impl BindingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BindingKind::Normal => "normal",
            BindingKind::Prop => "prop",
            BindingKind::BindableProp => "bindable_prop",
            BindingKind::RestProp => "rest_prop",
            BindingKind::State => "state",
            BindingKind::RawState => "raw_state",
            BindingKind::Derived => "derived",
            BindingKind::Each => "each",
            BindingKind::Snippet => "snippet",
            BindingKind::StoreSub => "store_sub",
            BindingKind::LegacyReactive => "legacy_reactive",
            BindingKind::Template => "template",
            BindingKind::Static => "static",
        }
    }
}

/// `DeclarationKind` (types/index.d.ts).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeclarationKind {
    Var,
    Let,
    Const,
    Using,
    AwaitUsing,
    Function,
    Import,
    Param,
    RestParam,
    Synthetic,
}

impl DeclarationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DeclarationKind::Var => "var",
            DeclarationKind::Let => "let",
            DeclarationKind::Const => "const",
            DeclarationKind::Using => "using",
            DeclarationKind::AwaitUsing => "await using",
            DeclarationKind::Function => "function",
            DeclarationKind::Import => "import",
            DeclarationKind::Param => "param",
            DeclarationKind::RestParam => "rest_param",
            DeclarationKind::Synthetic => "synthetic",
        }
    }
}

impl From<VariableKind> for DeclarationKind {
    fn from(k: VariableKind) -> Self {
        match k {
            VariableKind::Var => DeclarationKind::Var,
            VariableKind::Let => DeclarationKind::Let,
            VariableKind::Const => DeclarationKind::Const,
            VariableKind::Using => DeclarationKind::Using,
            VariableKind::AwaitUsing => DeclarationKind::AwaitUsing,
        }
    }
}

/// An Identifier: one from the AST, or one upstream synthesises with `b.id(name)`.
#[derive(Clone, Debug)]
pub enum Ident<'a> {
    Node(&'a Identifier),
    Synthetic { name: String, span: Option<Span> },
}

impl<'a> Ident<'a> {
    pub fn synthetic(name: &str) -> Ident<'a> {
        Ident::Synthetic {
            name: name.to_string(),
            span: None,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Ident::Node(i) => &i.name,
            Ident::Synthetic { name, .. } => name,
        }
    }

    pub fn span(&self) -> Option<Span> {
        match self {
            Ident::Node(i) => Some(i.span()),
            Ident::Synthetic { span, .. } => *span,
        }
    }

    pub fn node(&self) -> Option<&'a Identifier> {
        match self {
            Ident::Node(i) => Some(i),
            Ident::Synthetic { .. } => None,
        }
    }

    /// `this === node`
    pub fn is(&self, node: &Identifier) -> bool {
        matches!(self, Ident::Node(i) if std::ptr::eq(*i, node))
    }
}

/// What a binding was initialized with (`Binding['initial']`).
#[derive(Clone, Copy, Debug)]
pub enum Initial<'a> {
    Expression(&'a Expression),
    FunctionDeclaration(&'a Function),
    ClassDeclaration(&'a Class),
    ImportDeclaration(&'a ImportDeclaration),
    EachBlock(&'a ast::EachBlock),
    SnippetBlock(&'a ast::SnippetBlock),
}

impl<'a> Initial<'a> {
    pub fn node(self) -> NodeRef<'a> {
        match self {
            Initial::Expression(e) => expr_ref(e),
            Initial::FunctionDeclaration(f) => NodeRef::FunctionDeclaration(f),
            Initial::ClassDeclaration(c) => NodeRef::ClassDeclaration(c),
            Initial::ImportDeclaration(i) => NodeRef::ImportDeclaration(i),
            Initial::EachBlock(b) => NodeRef::EachBlock(b),
            Initial::SnippetBlock(b) => NodeRef::SnippetBlock(b),
        }
    }

    /// `initial.type`
    pub fn type_name(self) -> &'static str {
        self.node().type_name()
    }

    pub fn as_expression(self) -> Option<&'a Expression> {
        match self {
            Initial::Expression(e) => Some(e),
            _ => None,
        }
    }
}

/// A `{ node, path }` reference record.
#[derive(Clone, Debug)]
pub struct Reference<'a> {
    pub node: Ident<'a>,
    pub path: Vec<NodeRef<'a>>,
}

/// `{ value, scope }` entry of `Binding.assignments`.
#[derive(Clone, Copy, Debug)]
pub struct Assignment<'a> {
    pub value: Initial<'a>,
    pub scope: ScopeId,
}

/// `Binding.metadata`
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BindingMetadata {
    pub inside_rest: Option<bool>,
    pub is_template_declaration: Option<bool>,
    pub exclude_props: Option<Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct Binding<'a> {
    pub scope: ScopeId,
    pub node: Ident<'a>,
    pub kind: BindingKind,
    pub declaration_kind: DeclarationKind,
    /// What the value was initialized with.
    /// For destructured props such as `let { foo = 'bar' } = $props()` this is `'bar'` and not `$props()`
    pub initial: Option<Initial<'a>>,
    pub references: Vec<ReferenceId>,
    /// (Re)assignments of this binding. Includes declarations such as `function x() {}`.
    pub assignments: Vec<Assignment<'a>>,
    /// For `legacy_reactive`: its reactive dependencies
    pub legacy_dependencies: Vec<BindingId>,
    /// Bindings that should be invalidated when this binding is invalidated
    pub legacy_indirect_bindings: Vec<BindingId>,
    /// Legacy props: the `class` in `{ export klass as class}`. $props(): The `class` in { class: klass } = $props()
    pub prop_alias: Option<String>,
    /// Additional metadata, varies per binding type
    pub metadata: Option<BindingMetadata>,
    pub mutated: bool,
    pub reassigned: bool,
}

impl<'a> Binding<'a> {
    fn new(
        scope: ScopeId,
        node: Ident<'a>,
        kind: BindingKind,
        declaration_kind: DeclarationKind,
        initial: Option<Initial<'a>>,
    ) -> Binding<'a> {
        let mut assignments = Vec::new();
        if let Some(initial) = initial {
            assignments.push(Assignment { value: initial, scope });
        }
        Binding {
            scope,
            node,
            kind,
            declaration_kind,
            initial,
            references: Vec::new(),
            assignments,
            legacy_dependencies: Vec::new(),
            legacy_indirect_bindings: Vec::new(),
            prop_alias: None,
            metadata: None,
            mutated: false,
            reassigned: false,
        }
    }

    pub fn updated(&self) -> bool {
        self.mutated || self.reassigned
    }

    pub fn is_function(&self) -> bool {
        if self.updated() {
            // even if it's reassigned to another function,
            // we can't use it directly as e.g. an event handler
            return false;
        }

        matches!(
            self.initial,
            Some(
                Initial::Expression(Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_))
                    | Initial::FunctionDeclaration(_)
            )
        )
    }
}

/// An insertion-ordered string map (JS `Map` semantics: `set` on an existing key keeps its position).
#[derive(Clone, Debug)]
pub struct OrderedMap<V> {
    entries: Vec<(String, V)>,
    index: HashMap<String, usize>,
}

impl<V> Default for OrderedMap<V> {
    fn default() -> Self {
        OrderedMap {
            entries: Vec::new(),
            index: HashMap::new(),
        }
    }
}

impl<V> OrderedMap<V> {
    pub fn get(&self, k: &str) -> Option<&V> {
        self.index.get(k).map(|&i| &self.entries[i].1)
    }
    pub fn get_mut(&mut self, k: &str) -> Option<&mut V> {
        self.index.get(k).map(|&i| &mut self.entries[i].1)
    }
    pub fn contains_key(&self, k: &str) -> bool {
        self.index.contains_key(k)
    }
    pub fn insert(&mut self, k: String, v: V) {
        match self.index.get(&k) {
            Some(&i) => self.entries[i].1 = v,
            None => {
                self.index.insert(k.clone(), self.entries.len());
                self.entries.push((k, v));
            }
        }
    }
    pub fn remove(&mut self, k: &str) -> Option<V> {
        let i = self.index.remove(k)?;
        let (_, v) = self.entries.remove(i);
        for idx in self.index.values_mut() {
            if *idx > i {
                *idx -= 1;
            }
        }
        Some(v)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(k, _)| k.as_str())
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct Scope {
    /// The immediate parent scope
    pub parent: Option<ScopeId>,
    /// Whether or not `var` declarations are contained by this scope
    porous: bool,
    /// A map of every identifier declared by this scope, and all the
    /// identifiers that reference it
    pub declarations: OrderedMap<BindingId>,
    /// A map of declarators (VariableDeclarator | LetDirective) to the bindings they declare
    pub declarators: HashMap<NodeKey, Vec<BindingId>>,
    /// A set of all the names referenced with this scope
    /// — useful for generating unique names
    pub references: OrderedMap<Vec<ReferenceId>>,
    /// The scope depth allows us to determine if a state variable is referenced in its own scope,
    /// which is usually an error. Block statements do not increase this value
    pub function_depth: u32,
}

impl Scope {
    pub fn is_porous(&self) -> bool {
        self.porous
    }
}

#[derive(Clone, Debug, Default)]
pub struct ScopeRoot {
    pub conflicts: HashSet<String>,
    /// Tracks the next suffix counter per name to avoid O(n) rescanning in generate/unique.
    name_counters: HashMap<String, u32>,
}

/// `preferred_name.replace(/[^a-zA-Z0-9_$]/g, '_')` (per UTF-16 code unit, like JS).
fn sanitize_name(name: &str) -> String {
    let mut s = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
            s.push(c);
        } else {
            for _ in 0..c.len_utf16() {
                s.push('_');
            }
        }
    }
    s
}

impl ScopeRoot {
    pub fn next_counter(&self, name: &str) -> u32 {
        self.name_counters.get(name).copied().unwrap_or(0)
    }

    pub fn set_counter(&mut self, name: &str, value: u32) {
        self.name_counters.insert(name.to_string(), value);
    }

    /// `unique(preferred_name)`: returns the name of the `b.id(final_name)` upstream returns.
    pub fn unique(&mut self, preferred_name: &str) -> String {
        let preferred_name = sanitize_name(preferred_name);
        let mut n = self.name_counters.get(&preferred_name).copied().unwrap_or(0);
        let mut final_name;

        if n == 0 {
            final_name = preferred_name.clone();
            n = 1;
        } else {
            final_name = format!("{preferred_name}_{n}");
            n += 1;
        }

        while self.conflicts.contains(&final_name) {
            final_name = format!("{preferred_name}_{n}");
            n += 1;
        }

        self.name_counters.insert(preferred_name, n);
        self.conflicts.insert(final_name.clone());
        final_name
    }
}

/// All scopes, bindings and references sharing one [`ScopeRoot`] (one component or module).
#[derive(Debug, Default)]
pub struct ScopeArena<'a> {
    pub root: ScopeRoot,
    scopes: Vec<Scope>,
    bindings: Vec<Binding<'a>>,
    references: Vec<Reference<'a>>,
    /// AST Identifier address -> the binding its reference resolved to
    resolved: HashMap<usize, BindingId>,
    /// Track which expressions are currently being evaluated — this allows
    /// us to prevent cyclical evaluations without passing the map around
    current_evaluations: RefCell<HashSet<NodeKey>>,
}

impl<'a> ScopeArena<'a> {
    pub fn new() -> ScopeArena<'a> {
        ScopeArena::default()
    }

    pub fn scope(&self, id: ScopeId) -> &Scope {
        &self.scopes[id.0 as usize]
    }

    pub fn scope_mut(&mut self, id: ScopeId) -> &mut Scope {
        &mut self.scopes[id.0 as usize]
    }

    pub fn binding(&self, id: BindingId) -> &Binding<'a> {
        &self.bindings[id.0 as usize]
    }

    pub fn binding_mut(&mut self, id: BindingId) -> &mut Binding<'a> {
        &mut self.bindings[id.0 as usize]
    }

    pub fn reference_record(&self, id: ReferenceId) -> &Reference<'a> {
        &self.references[id.0 as usize]
    }

    pub fn scope_count(&self) -> usize {
        self.scopes.len()
    }

    pub fn bindings(&self) -> impl Iterator<Item = (BindingId, &Binding<'a>)> {
        self.bindings.iter().enumerate().map(|(i, b)| (BindingId(i as u32), b))
    }

    /// The binding an AST Identifier recorded as a reference resolved to (`None`: a global, or
    /// not a reference).
    pub fn resolve(&self, node: &Identifier) -> Option<BindingId> {
        self.resolved.get(&(node as *const Identifier as usize)).copied()
    }

    /// `new Scope(root, parent, porous)`
    pub fn new_scope(&mut self, parent: Option<ScopeId>, porous: bool) -> ScopeId {
        let function_depth = match parent {
            Some(p) => self.scope(p).function_depth + if porous { 0 } else { 1 },
            None => 0,
        };
        self.scopes.push(Scope {
            parent,
            porous,
            declarations: OrderedMap::default(),
            declarators: HashMap::new(),
            references: OrderedMap::default(),
            function_depth,
        });
        ScopeId(self.scopes.len() as u32 - 1)
    }

    /// `scope.declare(node, kind, declaration_kind, initial)`
    pub fn declare(
        &mut self,
        scope: ScopeId,
        node: Ident<'a>,
        kind: BindingKind,
        declaration_kind: DeclarationKind,
        initial: Option<Initial<'a>>,
    ) -> Result<BindingId, CompileError> {
        let s = self.scope(scope);
        if let Some(parent) = s.parent {
            if declaration_kind == DeclarationKind::Var && s.porous {
                return self.declare(parent, node, kind, declaration_kind, None);
            }

            if declaration_kind == DeclarationKind::Import {
                return self.declare(parent, node, kind, declaration_kind, initial);
            }
        }

        if let Some(&existing) = s.declarations.get(node.name())
            && self.binding(existing).declaration_kind != DeclarationKind::Var
            && declaration_kind != DeclarationKind::Var
        {
            // This also errors on function types, but that's arguably a good thing
            // declaring function twice is also caught by acorn in the parse phase
            return Err(e::declaration_duplicate(node.span(), node.name()));
        }

        let name = node.name().to_string();
        self.bindings
            .push(Binding::new(scope, node, kind, declaration_kind, initial));
        let binding = BindingId(self.bindings.len() as u32 - 1);

        validate_identifier_name(self, Some(binding), Some(self.scope(scope).function_depth))?;

        self.scope_mut(scope).declarations.insert(name.clone(), binding);
        self.root.conflicts.insert(name);
        Ok(binding)
    }

    /// `scope.child(porous)`
    pub fn child(&mut self, scope: ScopeId, porous: bool) -> ScopeId {
        self.new_scope(Some(scope), porous)
    }

    /// `scope.generate(preferred_name)`
    pub fn generate(&mut self, scope: ScopeId, preferred_name: &str) -> String {
        let s = self.scope(scope);
        if s.porous {
            let parent = s.parent.expect("porous scope has a parent");
            return self.generate(parent, preferred_name);
        }

        let mut preferred_name = sanitize_name(preferred_name);
        if preferred_name.as_bytes().first().is_some_and(u8::is_ascii_digit) {
            preferred_name.replace_range(0..1, "_");
        }

        // Use cached counter to skip names already known to be taken (avoids O(n²) scanning)
        let mut n = self.root.next_counter(&preferred_name);
        let mut name;

        if n == 0 {
            name = preferred_name.clone();
            n = 1;
        } else {
            name = format!("{preferred_name}_{n}");
            n += 1;
        }

        while self.scope(scope).references.contains_key(&name)
            || self.scope(scope).declarations.contains_key(&name)
            || self.root.conflicts.contains(&name)
            || is_reserved(&name)
        {
            name = format!("{preferred_name}_{n}");
            n += 1;
        }

        self.root.set_counter(&preferred_name, n);
        self.scope_mut(scope).references.insert(name.clone(), Vec::new());
        self.root.conflicts.insert(name.clone());
        name
    }

    /// `scope.get(name)`
    pub fn get(&self, scope: ScopeId, name: &str) -> Option<BindingId> {
        let mut s = Some(scope);
        while let Some(id) = s {
            let sc = self.scope(id);
            if let Some(&b) = sc.declarations.get(name) {
                return Some(b);
            }
            s = sc.parent;
        }
        None
    }

    /// `scope.get_bindings(node)` for a VariableDeclarator or LetDirective.
    pub fn get_bindings(&self, scope: ScopeId, node: NodeRef<'_>) -> &[BindingId] {
        match self.scope(scope).declarators.get(&node.key()) {
            Some(b) => b,
            None => panic!("No binding found for declarator"),
        }
    }

    /// `scope.owner(name)`
    pub fn owner(&self, scope: ScopeId, name: &str) -> Option<ScopeId> {
        let sc = self.scope(scope);
        if sc.declarations.contains_key(name) {
            Some(scope)
        } else {
            sc.parent.and_then(|p| self.owner(p, name))
        }
    }

    /// `scope.reference(node, path)`
    pub fn reference(&mut self, scope: ScopeId, node: Ident<'a>, path: Vec<NodeRef<'a>>) -> ReferenceId {
        let name = node.name().to_string();
        let ast_node = node.node();
        self.references.push(Reference { node, path });
        let r = ReferenceId(self.references.len() as u32 - 1);

        let mut s = scope;
        loop {
            let sc = self.scope_mut(s);
            match sc.references.get_mut(&name) {
                Some(list) => list.push(r),
                None => sc.references.insert(name.clone(), vec![r]),
            }

            if let Some(&binding) = sc.declarations.get(&name) {
                self.bindings[binding.0 as usize].references.push(r);
                if let Some(id) = ast_node {
                    self.resolved.insert(id as *const Identifier as usize, binding);
                }
                break;
            } else if let Some(parent) = sc.parent {
                s = parent;
            } else {
                // no binding was found, and this is the top level scope,
                // which means this is a global
                self.root.conflicts.insert(name);
                break;
            }
        }
        r
    }

    /// Re-points the given references to `binding` (used when analysis turns references to a
    /// global `$name` into a store subscription binding).
    pub(crate) fn set_binding_references(&mut self, binding: BindingId, references: Vec<ReferenceId>) {
        for &r in &references {
            if let Some(id) = self.references[r.0 as usize].node.node() {
                self.resolved.insert(id as *const Identifier as usize, binding);
            }
        }
        self.binding_mut(binding).references = references;
    }

    /// Does partial evaluation to find an exact value or at least the rough type of the expression.
    /// Only call this once scope has been fully generated in a first pass,
    /// else this evaluates on incomplete data and may yield wrong results.
    pub fn evaluate(&self, scope: ScopeId, expression: &'a Expression) -> Evaluation {
        let mut values = ValueSet::default();
        self.evaluate_into(scope, Some(Initial::Expression(expression)), &mut values)
    }

    /// `scope.evaluate(expression, values)`; `None` is a node type `Evaluation` has no case for
    /// (PrivateIdentifier, SpreadElement).
    fn evaluate_into(&self, scope: ScopeId, expression: Option<Initial<'a>>, values: &mut ValueSet) -> Evaluation {
        if let Some(e) = expression
            && self.current_evaluations.borrow().contains(&e.node().key())
        {
            return Evaluation::in_progress();
        }

        Evaluation::new(self, scope, expression, values)
    }
}

/// Result of [`ScopeArena::evaluate`].
#[derive(Clone, Debug)]
pub struct Evaluation {
    pub values: ValueSet,
    /// True if there is exactly one possible value
    pub is_known: bool,
    /// True if the possible values contains `UNKNOWN`
    pub has_unknown: bool,
    /// True if the value is known to not be null/undefined
    pub is_defined: bool,
    /// True if the value is known to be a string
    pub is_string: bool,
    /// True if the value is known to be a number
    pub is_number: bool,
    /// True if the value is known to be a primitive
    pub is_primitive: bool,
    /// True if the value is known to be a function
    pub is_function: bool,
    pub value: JsValue,
}

const UNKNOWN: JsValue = JsValue::Symbol(Sym::Unknown);
const NUMBER: JsValue = JsValue::Symbol(Sym::Number);
const STRING: JsValue = JsValue::Symbol(Sym::String);
const FUNCTION: JsValue = JsValue::Symbol(Sym::Function);

impl Evaluation {
    fn in_progress() -> Evaluation {
        Evaluation {
            values: ValueSet::default(),
            is_known: true,
            has_unknown: false,
            is_defined: true,
            is_string: true,
            is_number: true,
            is_primitive: true,
            is_function: true,
            value: JsValue::Undefined,
        }
    }

    fn new<'a>(
        arena: &ScopeArena<'a>,
        scope: ScopeId,
        expression: Option<Initial<'a>>,
        values: &mut ValueSet,
    ) -> Evaluation {
        let key = expression.map(|e| e.node().key());
        if let Some(k) = key {
            arena.current_evaluations.borrow_mut().insert(k);
        }

        let mut this = Evaluation::in_progress();
        let ev = |e: &'a Expression| arena.evaluate_into(scope, Some(Initial::Expression(e)), &mut ValueSet::default());

        match expression {
            Some(Initial::Expression(expression)) => match expression {
                Expression::Literal(l) => values.add(match &l.value {
                    LiteralValue::String(s) => JsValue::String(s.clone()),
                    LiteralValue::Number(n) => JsValue::Number(*n),
                    LiteralValue::Boolean(b) => JsValue::Bool(*b),
                    LiteralValue::Null => JsValue::Null,
                    LiteralValue::RegExp => {
                        JsValue::RegExp(std::rc::Rc::new(l.regex.clone().unwrap_or(RegExpLiteral {
                            pattern: String::new(),
                            flags: String::new(),
                        })))
                    }
                    LiteralValue::BigInt => JsValue::BigInt(l.bigint.clone().unwrap_or_default()),
                }),

                Expression::Identifier(id) => 'identifier: {
                    if let Some(binding) = arena.get(scope, &id.name) {
                        let binding = arena.binding(binding);
                        if let Some(Initial::Expression(init @ Expression::CallExpression(_))) = binding.initial
                            && get_rune(arena, Some(init), scope) == Some("$props.id")
                        {
                            values.add(STRING);
                            break 'identifier;
                        }

                        let is_prop = matches!(
                            binding.kind,
                            BindingKind::Prop | BindingKind::RestProp | BindingKind::BindableProp
                        );

                        if let Some(Initial::EachBlock(each)) = binding.initial
                            && each.index.as_deref() == Some(id.name.as_str())
                        {
                            values.add(NUMBER);
                            break 'identifier;
                        }

                        if let Some(Initial::SnippetBlock(_)) = binding.initial {
                            this.is_defined = true;
                            this.is_known = false;
                            values.add(UNKNOWN);
                            break 'identifier;
                        }

                        if !binding.updated() && binding.initial.is_some() && !is_prop {
                            arena.evaluate_into(binding.scope, binding.initial, values);
                            break 'identifier;
                        }
                    } else if id.name == "undefined" {
                        values.add(JsValue::Undefined);
                        break 'identifier;
                    }

                    // TODO glean what we can from reassignments
                    // TODO one day, expose props and imports somehow

                    values.add(UNKNOWN);
                }

                Expression::BinaryExpression(b) => 'binary: {
                    // `left` cannot be `PrivateIdentifier` unless operator is `in`
                    let a = match &b.left {
                        BinaryLeft::Expression(l) => ev(l),
                        BinaryLeft::Private(_) => arena.evaluate_into(scope, None, &mut ValueSet::default()),
                    };
                    let bv = ev(&b.right);

                    if a.is_known && bv.is_known {
                        values.add(js::binary(b.operator, &a.value, &bv.value));
                        break 'binary;
                    }

                    use BinaryOperator as B;
                    match b.operator {
                        B::NotEq
                        | B::StrictNotEq
                        | B::Lt
                        | B::LtEq
                        | B::Gt
                        | B::GtEq
                        | B::Eq
                        | B::StrictEq
                        | B::In
                        | B::Instanceof => {
                            values.add(JsValue::Bool(true));
                            values.add(JsValue::Bool(false));
                        }

                        B::Rem
                        | B::BitAnd
                        | B::Mul
                        | B::Exp
                        | B::Sub
                        | B::Div
                        | B::Shl
                        | B::Shr
                        | B::UShr
                        | B::BitXor
                        | B::BitOr => values.add(NUMBER),

                        B::Add => {
                            if a.is_string || bv.is_string {
                                values.add(STRING);
                            } else if a.is_number && bv.is_number {
                                values.add(NUMBER);
                            } else {
                                values.add(STRING);
                                values.add(NUMBER);
                            }
                        }
                    }
                }

                Expression::ConditionalExpression(c) => {
                    let test = ev(&c.test);
                    let consequent = ev(&c.consequent);
                    let alternate = ev(&c.alternate);

                    if test.is_known {
                        let branch = if js::to_boolean(&test.value) {
                            consequent
                        } else {
                            alternate
                        };
                        for value in branch.values.iter() {
                            values.add(value.clone());
                        }
                    } else {
                        for value in consequent.values.iter() {
                            values.add(value.clone());
                        }

                        for value in alternate.values.iter() {
                            values.add(value.clone());
                        }
                    }
                }

                Expression::LogicalExpression(l) => 'logical: {
                    let a = ev(&l.left);
                    let b = ev(&l.right);

                    if a.is_known {
                        if b.is_known {
                            values.add(js::logical(l.operator, &a.value, &b.value));
                            break 'logical;
                        }

                        if (l.operator == LogicalOperator::And && !js::to_boolean(&a.value))
                            || (l.operator == LogicalOperator::Or && js::to_boolean(&a.value))
                            || (l.operator == LogicalOperator::Nullish
                                && !matches!(a.value, JsValue::Undefined | JsValue::Null))
                        {
                            values.add(a.value.clone());
                        } else {
                            for value in b.values.iter() {
                                values.add(value.clone());
                            }
                        }

                        break 'logical;
                    }

                    for value in a.values.iter() {
                        values.add(value.clone());
                    }

                    for value in b.values.iter() {
                        values.add(value.clone());
                    }
                }

                Expression::UnaryExpression(u) => {
                    let argument = ev(&u.argument);

                    if argument.is_known {
                        values.add(js::unary(u.operator, &argument.value));
                    } else {
                        use UnaryOperator as U;
                        match u.operator {
                            U::Not | U::Delete => {
                                values.add(JsValue::Bool(false));
                                values.add(JsValue::Bool(true));
                            }

                            U::Plus | U::Minus | U::BitNot => values.add(NUMBER),

                            U::Typeof => values.add(STRING),

                            U::Void => values.add(JsValue::Undefined),
                        }
                    }
                }

                Expression::CallExpression(call) => 'call: {
                    if let Some(keypath) = get_global_keypath(arena, &call.callee, scope) {
                        if is_rune(&keypath) {
                            let arg = call.arguments.first();
                            let arg_expr = match arg {
                                Some(ArrayElement::Expression(e)) => Some(Some(Initial::Expression(e))),
                                Some(ArrayElement::Spread(_)) => Some(None),
                                None => None,
                            };

                            match keypath.as_str() {
                                "$state" | "$state.raw" | "$derived" => match arg_expr {
                                    Some(arg) => {
                                        arena.evaluate_into(scope, arg, values);
                                    }
                                    None => values.add(JsValue::Undefined),
                                },

                                "$props.id" => values.add(STRING),

                                "$effect.tracking" => {
                                    values.add(JsValue::Bool(false));
                                    values.add(JsValue::Bool(true));
                                }

                                "$derived.by" => {
                                    if let Some(ArrayElement::Expression(Expression::ArrowFunctionExpression(f))) = arg
                                        && let ArrowBody::Expression(body) = &f.body
                                    {
                                        arena.evaluate_into(scope, Some(Initial::Expression(body)), values);
                                        break 'call;
                                    }

                                    values.add(UNKNOWN);
                                }

                                _ => values.add(UNKNOWN),
                            }

                            break 'call;
                        }

                        if let Some(ty) = js::global_type(&keypath)
                            && call.arguments.iter().all(|arg| !matches!(arg, ArrayElement::Spread(_)))
                        {
                            let evaluated: Vec<Evaluation> = call
                                .arguments
                                .iter()
                                .map(|arg| match arg {
                                    ArrayElement::Expression(e) => ev(e),
                                    ArrayElement::Spread(_) => unreachable!(),
                                })
                                .collect();

                            if js::global_has_fn(&keypath) && evaluated.iter().all(|e| e.is_known) {
                                let args: Vec<JsValue> = evaluated.into_iter().map(|e| e.value).collect();
                                values.add(js::call_global(&keypath, &args).unwrap_or(UNKNOWN));
                            } else {
                                values.add(JsValue::Symbol(ty));
                            }

                            break 'call;
                        }
                    }

                    values.add(UNKNOWN);
                }

                Expression::TemplateLiteral(t) => {
                    let mut result = t.quasis[0].value.cooked.clone().unwrap_or_default();

                    for (i, expr) in t.expressions.iter().enumerate() {
                        let e = ev(expr);

                        if e.is_known {
                            let cooked = t.quasis[i + 1].value.cooked.clone().unwrap_or_default();
                            match js::binary(BinaryOperator::Add, &e.value, &JsValue::String(cooked)) {
                                JsValue::String(s) => result.push_str(&s),
                                _ => unreachable!("string concatenation"),
                            }
                        } else {
                            values.add(STRING);
                            break;
                        }
                    }

                    values.add(JsValue::String(result));
                }

                Expression::MemberExpression(_) => 'member: {
                    if let Some(keypath) = get_global_keypath(arena, expression, scope)
                        && let Some(c) = js::global_constant(&keypath)
                    {
                        values.add(JsValue::Number(c));
                        break 'member;
                    }

                    values.add(UNKNOWN);
                }

                Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => values.add(FUNCTION),

                _ => values.add(UNKNOWN),
            },

            Some(Initial::FunctionDeclaration(_)) => values.add(FUNCTION),

            _ => values.add(UNKNOWN),
        }

        for value in values.iter() {
            this.value = value.clone(); // saves having special logic for `size === 1`

            if !matches!(value, JsValue::Symbol(Sym::String) | JsValue::String(_)) {
                this.is_string = false;
            }

            if !matches!(value, JsValue::Symbol(Sym::Number) | JsValue::Number(_)) {
                this.is_number = false;
            }

            if !matches!(value, JsValue::Symbol(Sym::Function)) {
                this.is_function = false;
            }

            if matches!(
                value,
                JsValue::Undefined | JsValue::Null | JsValue::Symbol(Sym::Unknown)
            ) {
                this.is_defined = false;
            }

            if matches!(value, JsValue::Symbol(Sym::Unknown)) {
                this.has_unknown = true;
                this.is_primitive = false;
            }
        }

        if values.len() > 1 || this.value.is_symbol() {
            this.is_known = false;
        }

        this.values = values.clone();

        if let Some(k) = key {
            arena.current_evaluations.borrow_mut().remove(&k);
        }

        this
    }
}

// ---------------------------------------------------------------------------------------------
// create_scopes

/// `node.metadata.scopes` of a Component / SvelteComponent / SvelteSelf.
#[derive(Clone, Debug)]
pub struct ComponentScopes {
    pub default: ScopeId,
    /// `slot="name"` scopes, in creation order
    pub slots: Vec<(String, ScopeId)>,
}

/// What `create_scopes` returns (`{ has_await, scope, scopes }`) plus the node metadata it writes.
#[derive(Clone, Debug)]
pub struct CreatedScopes {
    pub has_await: bool,
    pub scope: ScopeId,
    /// A map of node->associated scope. A node appearing in this map does not necessarily mean that it created a scope
    pub scopes: HashMap<NodeKey, ScopeId>,
    /// `Component.metadata.scopes`
    pub component_scopes: HashMap<NodeKey, ComponentScopes>,
    /// `EachBlock.metadata.index` (the generated `$$index` name)
    pub each_block_index: HashMap<NodeKey, String>,
}

#[derive(Clone, Copy)]
enum UpdateTarget<'a> {
    Pattern(&'a Pattern),
    Expression(&'a Expression),
}

struct CreateScopes<'s, 'a> {
    arena: &'s mut ScopeArena<'a>,
    top: ScopeId,
    allow_reactive_declarations: bool,
    scopes: HashMap<NodeKey, ScopeId>,
    component_scopes: HashMap<NodeKey, ComponentScopes>,
    each_block_index: HashMap<NodeKey, String>,
    references: Vec<(ScopeId, &'a Identifier, Vec<NodeRef<'a>>)>,
    updates: Vec<(ScopeId, UpdateTarget<'a>, Initial<'a>)>,
    /// An array of reactive declarations, i.e. the `a` in `$: a = b * 2`
    possible_implicit_declarations: Vec<&'a Identifier>,
    has_await: bool,
    error: Option<CompileError>,
}

impl<'s, 'a> CreateScopes<'s, 'a> {
    fn fail(&mut self, w: &mut Walker<'a>, err: CompileError) {
        if self.error.is_none() {
            self.error = Some(err);
        }
        w.stop();
    }

    fn declare(
        &mut self,
        w: &mut Walker<'a>,
        scope: ScopeId,
        node: Ident<'a>,
        kind: BindingKind,
        declaration_kind: DeclarationKind,
        initial: Option<Initial<'a>>,
    ) -> Option<BindingId> {
        if self.error.is_some() {
            return None;
        }
        match self.arena.declare(scope, node, kind, declaration_kind, initial) {
            Ok(b) => Some(b),
            Err(err) => {
                self.fail(w, err);
                None
            }
        }
    }

    fn add_params(&mut self, w: &mut Walker<'a>, scope: ScopeId, params: &'a [Pattern]) {
        for param in params {
            for node in extract_identifiers(param) {
                let kind = if matches!(param, Pattern::RestElement(_)) {
                    DeclarationKind::RestParam
                } else {
                    DeclarationKind::Param
                };
                self.declare(w, scope, Ident::Node(node), BindingKind::Normal, kind, None);
            }
        }
    }

    fn create_block_scope(&mut self, node: NodeRef<'a>, state: &ScopeId, w: &mut Walker<'a>) {
        let scope = self.arena.child(*state, true);
        self.scopes.insert(node.key(), scope);

        w.next(self, node, &scope);
    }

    fn svelte_fragment(&mut self, node: NodeRef<'a>, state: &ScopeId, w: &mut Walker<'a>) {
        let scope = self.arena.child(*state, false);
        self.scopes.insert(node.key(), scope);
        w.next(self, node, &scope);
    }

    fn component(&mut self, node: NodeRef<'a>, state: &ScopeId, w: &mut Walker<'a>) {
        let default = self.arena.child(*state, false);
        self.component_scopes.insert(
            node.key(),
            ComponentScopes {
                default,
                slots: Vec::new(),
            },
        );

        let (attributes, fragment) = match node {
            NodeRef::SvelteComponent(n) => {
                w.visit(self, node, expr_ref(&n.expression), state);
                (&n.attributes, &n.fragment)
            }
            NodeRef::Component(n) | NodeRef::SvelteSelf(n) => (&n.attributes, &n.fragment),
            _ => unreachable!(),
        };

        let default_state = if determine_slot(node).is_some() {
            *state
        } else {
            default
        };

        for attribute in attributes {
            if let ast::AttributeLike::LetDirective(_) = attribute {
                w.visit(self, node, attribute_like_ref(attribute), &default_state);
            } else {
                w.visit(self, node, attribute_like_ref(attribute), state);
            }
        }

        for child in &fragment.nodes {
            let child = fragment_node_ref(child);
            let mut child_state = default_state;

            if let Some(slot_name) = determine_slot(child) {
                let slot_scope = self.arena.child(*state, false);
                let slots = &mut self.component_scopes.get_mut(&node.key()).unwrap().slots;
                match slots.iter_mut().find(|(n, _)| n == slot_name) {
                    Some(entry) => entry.1 = slot_scope,
                    None => slots.push((slot_name.to_string(), slot_scope)),
                }

                child_state = slot_scope;
            }

            w.visit(self, node, child, &child_state);
        }
    }

    fn svelte_directive(
        &mut self,
        node: NodeRef<'a>,
        d: (&'a str, &'a Option<Expression>),
        state: &ScopeId,
        w: &mut Walker<'a>,
    ) {
        let name = d.0.split('.').next().unwrap_or_default();
        self.arena.reference(*state, Ident::synthetic(name), w.path.clone());

        if let Some(expression) = d.1 {
            w.visit(self, node, expr_ref(expression), state);
        }
    }
}

impl<'s, 'a> Visitor<'a> for CreateScopes<'s, 'a> {
    type State = ScopeId;

    fn visit(&mut self, node: NodeRef<'a>, state: &ScopeId, w: &mut Walker<'a>) {
        let state = *state;
        match node {
            NodeRef::AwaitExpression(_) => {
                // this doesn't _really_ belong here, but it allows us to
                // automatically opt into runes mode on encountering
                // blocking awaits, without doing an additional walk
                // before the analysis occurs
                // TODO remove this in Svelte 7.0 or whenever we get rid of legacy support
                self.has_await = self.has_await || w.path.iter().all(|n| !n.is_function());

                w.next(self, node, &state);
            }

            // references
            NodeRef::Identifier(id) => {
                let parent = w.parent();
                if parent.is_some() && is_reference(node, parent) {
                    self.references.push((state, id, w.path.clone()));
                }
            }

            NodeRef::LabeledStatement(n) => {
                if w.path.len() > 1 || !self.allow_reactive_declarations {
                    return w.next(self, node, &state);
                }
                if n.label.name != "$" {
                    return w.next(self, node, &state);
                }

                // create a scope for the $: block
                let scope = self.arena.child(self.top, false);
                self.scopes.insert(node.key(), scope);

                if let Statement::ExpressionStatement(s) = &*n.body
                    && let Expression::AssignmentExpression(a) = &s.expression
                {
                    for id in extract_identifiers(&a.left) {
                        if !id.name.starts_with('$') {
                            self.possible_implicit_declarations.push(id);
                        }
                    }
                }

                w.next(self, node, &scope);
            }

            NodeRef::SvelteFragment(_)
            | NodeRef::SlotElement(_)
            | NodeRef::SvelteElement(_)
            | NodeRef::RegularElement(_) => self.svelte_fragment(node, &state, w),

            NodeRef::LetDirective(d) => {
                let scope = state;

                let mut bindings = Vec::new();
                let parent = w.parent();
                let path = |n: NodeRef<'a>| -> Vec<NodeRef<'a>> { parent.into_iter().chain([n]).collect() };

                if let Some(expression) = &d.expression {
                    let mut ids = Vec::new();
                    extract_identifiers_from_destructuring(expression, &mut ids);
                    for id in ids {
                        let Some(binding) = self.declare(
                            w,
                            scope,
                            Ident::Node(id),
                            BindingKind::Template,
                            DeclarationKind::Const,
                            None,
                        ) else {
                            return;
                        };
                        self.arena.reference(scope, Ident::Node(id), path(node));
                        bindings.push(binding);
                    }
                } else {
                    let id = Ident::Synthetic {
                        name: d.name.clone(),
                        span: Some(d.span()),
                    };
                    let Some(binding) = self.declare(
                        w,
                        scope,
                        id.clone(),
                        BindingKind::Template,
                        DeclarationKind::Const,
                        None,
                    ) else {
                        return;
                    };
                    self.arena.reference(scope, id, path(node));
                    bindings.push(binding);
                }

                self.arena.scope_mut(scope).declarators.insert(node.key(), bindings);
            }

            NodeRef::Component(n) => {
                let name = n.name.split('.').next().unwrap_or_default();
                self.arena.reference(state, Ident::synthetic(name), w.path.clone());
                self.component(node, &state, w);
            }
            NodeRef::SvelteSelf(_) | NodeRef::SvelteComponent(_) => self.component(node, &state, w),

            // updates
            NodeRef::AssignmentExpression(n) => {
                self.updates
                    .push((state, UpdateTarget::Pattern(&n.left), Initial::Expression(&n.right)));
                w.next(self, node, &state);
            }

            NodeRef::UpdateExpression(n) => {
                let expression = &*n.argument;
                self.updates.push((
                    state,
                    UpdateTarget::Expression(expression),
                    Initial::Expression(expression),
                ));
                w.next(self, node, &state);
            }

            NodeRef::ImportDeclaration(n) => {
                for specifier in &n.specifiers {
                    let local = match specifier {
                        ImportSpecifierKind::ImportSpecifier(s) => &s.local,
                        ImportSpecifierKind::ImportDefaultSpecifier(s) => &s.local,
                        ImportSpecifierKind::ImportNamespaceSpecifier(s) => &s.local,
                    };
                    self.declare(
                        w,
                        state,
                        Ident::Node(local),
                        BindingKind::Normal,
                        DeclarationKind::Import,
                        Some(Initial::ImportDeclaration(n)),
                    );
                }
            }

            NodeRef::FunctionExpression(n) => {
                let scope = self.arena.child(state, true);
                self.scopes.insert(node.key(), scope);

                if let Some(id) = &n.id {
                    self.declare(
                        w,
                        scope,
                        Ident::Node(id),
                        BindingKind::Normal,
                        DeclarationKind::Function,
                        None,
                    );
                }

                self.add_params(w, scope, &n.params);
                w.next(self, node, &scope);
            }

            NodeRef::FunctionDeclaration(n) => {
                if let Some(id) = &n.id {
                    self.declare(
                        w,
                        state,
                        Ident::Node(id),
                        BindingKind::Normal,
                        DeclarationKind::Function,
                        Some(Initial::FunctionDeclaration(n)),
                    );
                }

                let scope = self.arena.child(state, true);
                self.scopes.insert(node.key(), scope);

                self.add_params(w, scope, &n.params);
                w.next(self, node, &scope);
            }

            NodeRef::ArrowFunctionExpression(n) => {
                let scope = self.arena.child(state, true);
                self.scopes.insert(node.key(), scope);

                self.add_params(w, scope, &n.params);
                w.next(self, node, &scope);
            }

            NodeRef::ForStatement(_)
            | NodeRef::ForInStatement(_)
            | NodeRef::ForOfStatement(_)
            | NodeRef::SwitchStatement(_) => self.create_block_scope(node, &state, w),

            NodeRef::BlockStatement(_) => {
                if w.parent().is_some_and(|p| p.is_function()) {
                    // The scopes created for the function nodes above handle the function identifier and
                    // parameters, but the block statement itself holds the non-porous function scope
                    let scope = self.arena.child(state, false);
                    self.scopes.insert(node.key(), scope);
                    w.next(self, node, &scope);
                } else {
                    self.create_block_scope(node, &state, w);
                }
            }

            NodeRef::ClassDeclaration(n) => {
                if let Some(id) = &n.id {
                    self.declare(
                        w,
                        state,
                        Ident::Node(id),
                        BindingKind::Normal,
                        DeclarationKind::Let,
                        Some(Initial::ClassDeclaration(n)),
                    );
                }
                w.next(self, node, &state);
            }

            NodeRef::VariableDeclaration(n) => {
                let is_parent_const_tag = matches!(w.parent(), Some(NodeRef::ConstTag(_)));
                for declarator in &n.declarations {
                    let mut bindings = Vec::new();

                    for id in extract_identifiers(&declarator.id) {
                        let Some(binding) = self.declare(
                            w,
                            state,
                            Ident::Node(id),
                            if is_parent_const_tag {
                                BindingKind::Template
                            } else {
                                BindingKind::Normal
                            },
                            n.kind.into(),
                            declarator.init.as_ref().map(Initial::Expression),
                        ) else {
                            return;
                        };
                        self.arena.binding_mut(binding).metadata = Some(BindingMetadata {
                            is_template_declaration: Some(true),
                            ..Default::default()
                        });
                        bindings.push(binding);
                    }

                    self.arena
                        .scope_mut(state)
                        .declarators
                        .insert(NodeRef::VariableDeclarator(declarator).key(), bindings);
                }

                w.next(self, node, &state);
            }

            NodeRef::CatchClause(n) => {
                if let Some(param) = &n.param {
                    let scope = self.arena.child(state, true);
                    self.scopes.insert(node.key(), scope);

                    for id in extract_identifiers(param) {
                        self.declare(
                            w,
                            scope,
                            Ident::Node(id),
                            BindingKind::Normal,
                            DeclarationKind::Let,
                            None,
                        );
                    }

                    w.next(self, node, &scope);
                } else {
                    w.next(self, node, &state);
                }
            }

            NodeRef::EachBlock(n) => {
                w.visit(self, node, expr_ref(&n.expression), &state);
                if let Some(fallback) = &n.fallback {
                    w.visit(self, node, NodeRef::Fragment(fallback), &state);
                }

                // context and children are a new scope
                let scope = self.arena.child(state, false);
                self.scopes.insert(node.key(), scope);

                if let Some(context) = &n.context {
                    // declarations
                    for id in extract_identifiers(context) {
                        let Some(binding) = self.declare(
                            w,
                            scope,
                            Ident::Node(id),
                            BindingKind::Each,
                            DeclarationKind::Const,
                            None,
                        ) else {
                            return;
                        };

                        let mut finder = RestFinder {
                            id,
                            inside_rest: false,
                            is_rest_id: false,
                        };
                        Walker::walk(pattern_ref(context), &(), &mut finder);

                        self.arena.binding_mut(binding).metadata = Some(BindingMetadata {
                            inside_rest: Some(finder.is_rest_id),
                            ..Default::default()
                        });
                    }

                    // Visit to pick up references from default initializers
                    w.visit(self, node, pattern_ref(context), &scope);
                }

                if let Some(index) = &n.index {
                    let is_keyed = n.key.as_ref().is_some_and(|key| match key {
                        Expression::Identifier(k) => k.name != *index,
                        _ => true,
                    });
                    self.declare(
                        w,
                        scope,
                        Ident::synthetic(index),
                        if is_keyed {
                            BindingKind::Template
                        } else {
                            BindingKind::Static
                        },
                        DeclarationKind::Const,
                        Some(Initial::EachBlock(n)),
                    );
                }
                if let Some(key) = &n.key {
                    w.visit(self, node, expr_ref(key), &scope);
                }

                // children
                for child in &n.body.nodes {
                    w.visit(self, node, fragment_node_ref(child), &scope);
                }

                let index = self.arena.root.unique("$$index");
                self.each_block_index.insert(node.key(), index);
            }

            NodeRef::AwaitBlock(n) => {
                w.visit(self, node, expr_ref(&n.expression), &state);

                if let Some(pending) = &n.pending {
                    w.visit(self, node, NodeRef::Fragment(pending), &state);
                }

                if let Some(then) = &n.then {
                    w.visit(self, node, NodeRef::Fragment(then), &state);
                    if let Some(value) = &n.value {
                        let Some(&then_scope) = self.scopes.get(&NodeRef::Fragment(then).key()) else {
                            return;
                        };
                        let value_scope = self.arena.child(state, false);
                        self.scopes.insert(pattern_ref(value).key(), value_scope);
                        w.visit(self, node, pattern_ref(value), &value_scope);
                        for id in extract_identifiers(value) {
                            self.declare(
                                w,
                                then_scope,
                                Ident::Node(id),
                                BindingKind::Template,
                                DeclarationKind::Const,
                                None,
                            );
                            self.declare(
                                w,
                                value_scope,
                                Ident::Node(id),
                                BindingKind::Normal,
                                DeclarationKind::Const,
                                None,
                            );
                        }
                    }
                }

                if let Some(catch) = &n.catch {
                    w.visit(self, node, NodeRef::Fragment(catch), &state);
                    if let Some(error) = &n.error {
                        let Some(&catch_scope) = self.scopes.get(&NodeRef::Fragment(catch).key()) else {
                            return;
                        };
                        let error_scope = self.arena.child(state, false);
                        self.scopes.insert(pattern_ref(error).key(), error_scope);
                        w.visit(self, node, pattern_ref(error), &error_scope);
                        for id in extract_identifiers(error) {
                            self.declare(
                                w,
                                catch_scope,
                                Ident::Node(id),
                                BindingKind::Template,
                                DeclarationKind::Const,
                                None,
                            );
                            self.declare(
                                w,
                                error_scope,
                                Ident::Node(id),
                                BindingKind::Normal,
                                DeclarationKind::Const,
                                None,
                            );
                        }
                    }
                }
            }

            NodeRef::SnippetBlock(n) => {
                let scope = state;

                self.declare(
                    w,
                    scope,
                    Ident::Node(&n.expression),
                    BindingKind::Normal,
                    DeclarationKind::Function,
                    Some(Initial::SnippetBlock(n)),
                );

                let child_scope = self.arena.child(state, false);
                self.scopes.insert(node.key(), child_scope);

                for param in &n.parameters {
                    for id in extract_identifiers(param) {
                        self.declare(
                            w,
                            child_scope,
                            Ident::Node(id),
                            BindingKind::Snippet,
                            DeclarationKind::Let,
                            None,
                        );
                    }
                }

                w.next(self, node, &child_scope);

                // the parameters and what the body declares end up in the same function
                let Some(&body_scope) = self.scopes.get(&NodeRef::Fragment(&n.body).key()) else {
                    return;
                };
                let duplicate = self
                    .arena
                    .scope(body_scope)
                    .declarations
                    .iter()
                    .find(|(name, _)| self.arena.scope(child_scope).declarations.contains_key(name))
                    .map(|(name, &binding)| e::declaration_duplicate(self.arena.binding(binding).node.span(), name));
                if let Some(err) = duplicate {
                    self.fail(w, err);
                }
            }

            NodeRef::Fragment(f) => {
                // SVELTERS: `node.metadata.transparent` recomputed (see module docs)
                let parent_is_element = matches!(
                    w.parent(),
                    Some(
                        NodeRef::Component(_)
                            | NodeRef::TitleElement(_)
                            | NodeRef::SlotElement(_)
                            | NodeRef::RegularElement(_)
                            | NodeRef::SvelteBody(_)
                            | NodeRef::SvelteComponent(_)
                            | NodeRef::SvelteDocument(_)
                            | NodeRef::SvelteElement(_)
                            | NodeRef::SvelteFragment(_)
                            | NodeRef::SvelteBoundary(_)
                            | NodeRef::SvelteHead(_)
                            | NodeRef::SvelteOptions(_)
                            | NodeRef::SvelteSelf(_)
                            | NodeRef::SvelteWindow(_)
                    )
                );
                let transparent = parent_is_element
                    && !f
                        .nodes
                        .iter()
                        .any(|n| matches!(n, ast::FragmentNode::DeclarationTag(_)));

                let scope = self.arena.child(state, transparent);
                self.scopes.insert(node.key(), scope);
                w.next(self, node, &scope);
            }

            NodeRef::BindDirective(d) => {
                if let Some(expression) = &d.expression
                    && !matches!(expression, Expression::SequenceExpression(_))
                {
                    self.updates.push((
                        state,
                        UpdateTarget::Expression(expression),
                        Initial::Expression(expression),
                    ));
                }

                w.next(self, node, &state);
            }

            NodeRef::AnimateDirective(d) | NodeRef::UseDirective(d) => {
                self.svelte_directive(node, (&d.name, &d.expression), &state, w)
            }
            NodeRef::TransitionDirective(d) => self.svelte_directive(node, (&d.name, &d.expression), &state, w),

            // using it's own function instead of `SvelteDirective` because
            // StyleDirective doesn't have expressions and are generally already
            // handled by `Identifier`. This is the special case for the shorthand
            // eg <button style:height /> where the variable has the same name of
            // the css property
            NodeRef::StyleDirective(d) => {
                if let ast::AttributeValue::True(true) = d.value {
                    let mut path = w.path.clone();
                    path.push(node);
                    self.arena.reference(state, Ident::synthetic(&d.name), path);
                }
                w.next(self, node, &state);
            }

            // TODO others
            _ => w.next(self, node, &state),
        }
    }
}

/// The inner `walk(node.context, null, { Identifier, RestElement })` of the EachBlock visitor.
struct RestFinder<'a> {
    id: &'a Identifier,
    inside_rest: bool,
    is_rest_id: bool,
}

impl<'a> Visitor<'a> for RestFinder<'a> {
    type State = ();

    fn visit(&mut self, node: NodeRef<'a>, state: &(), w: &mut Walker<'a>) {
        match node {
            NodeRef::Identifier(n) => {
                if self.inside_rest && std::ptr::eq(n, self.id) {
                    self.is_rest_id = true;
                }
            }
            NodeRef::RestElement(_) => {
                let prev = self.inside_rest;
                self.inside_rest = true;
                w.next(self, node, state);
                self.inside_rest = prev;
            }
            _ => w.next(self, node, state),
        }
    }
}

/// `create_scopes(ast, root, allow_reactive_declarations, parent)`; the root is the arena's.
pub fn create_scopes<'a>(
    arena: &mut ScopeArena<'a>,
    ast: NodeRef<'a>,
    allow_reactive_declarations: bool,
    parent: Option<ScopeId>,
) -> Result<CreatedScopes, CompileError> {
    let scope = arena.new_scope(parent, false);
    let mut scopes = HashMap::new();
    scopes.insert(ast.key(), scope);

    let mut v = CreateScopes {
        arena,
        top: scope,
        allow_reactive_declarations,
        scopes,
        component_scopes: HashMap::new(),
        each_block_index: HashMap::new(),
        references: Vec::new(),
        updates: Vec::new(),
        possible_implicit_declarations: Vec::new(),
        has_await: false,
        error: None,
    };

    Walker::walk(ast, &scope, &mut v);

    if let Some(err) = v.error {
        return Err(err);
    }

    let CreateScopes {
        arena,
        scopes,
        component_scopes,
        each_block_index,
        references,
        updates,
        possible_implicit_declarations,
        has_await,
        ..
    } = v;

    for id in possible_implicit_declarations {
        let binding = arena.get(scope, &id.name);
        if binding.is_some() {
            continue; // TODO can also be legacy_reactive if declared outside of reactive statement
        }

        arena.declare(
            scope,
            Ident::Node(id),
            BindingKind::LegacyReactive,
            DeclarationKind::Let,
            None,
        )?;
    }

    // we do this after the fact, so that we don't need to worry
    // about encountering references before their declarations
    for (scope, node, path) in references {
        arena.reference(scope, Ident::Node(node), path);
    }

    for (scope, node, value) in updates {
        let targets = match node {
            UpdateTarget::Pattern(p) => {
                let mut nodes = Vec::new();
                unwrap_pattern(p, &mut nodes);
                nodes
            }
            UpdateTarget::Expression(e) => unwrap_expression_pattern(e),
        };

        for expression in targets {
            let Some(left) = object(expression) else { continue };
            let Some(binding) = arena.get(scope, &left.name) else {
                continue;
            };

            if !arena.binding(binding).node.is(left) {
                let b = arena.binding_mut(binding);
                if matches!(expression, PatternTarget::Identifier(i) if std::ptr::eq(i, left)) {
                    b.reassigned = true;
                    b.assignments.push(Assignment { value, scope });
                } else {
                    b.mutated = true;
                }
            }
        }
    }

    Ok(CreatedScopes {
        has_await,
        scope,
        scopes,
        component_scopes,
        each_block_index,
    })
}

/// `set_scope(node, { next, state })`: the scope a visitor should continue with when entering
/// `node`, given the current one.
pub fn set_scope(scopes: &HashMap<NodeKey, ScopeId>, node: NodeRef<'_>, current: ScopeId) -> ScopeId {
    scopes.get(&node.key()).copied().unwrap_or(current)
}

/// Returns the name of the rune if the given expression is a `CallExpression` using a rune.
pub fn get_rune(arena: &ScopeArena<'_>, node: Option<&Expression>, scope: ScopeId) -> Option<&'static str> {
    let Some(Expression::CallExpression(call)) = node else {
        return None;
    };
    get_rune_of_call(arena, call, scope)
}

/// [`get_rune`] for a node already known to be a `CallExpression`.
pub fn get_rune_of_call(arena: &ScopeArena<'_>, call: &CallExpression, scope: ScopeId) -> Option<&'static str> {
    let keypath = get_global_keypath(arena, &call.callee, scope)?;

    as_rune(&keypath)
}

/// Returns the name of the rune if the given expression is a `CallExpression` using a rune.
fn get_global_keypath(arena: &ScopeArena<'_>, node: &Expression, scope: ScopeId) -> Option<String> {
    let mut n = node;

    let mut joined = String::new();

    while let Expression::MemberExpression(m) = n {
        if m.computed {
            return None;
        }
        let PropertyKey::Expression(p) = &m.property else {
            return None;
        };
        let Expression::Identifier(p) = &**p else { return None };
        joined = format!(".{}{joined}", p.name);
        n = &m.object;
    }

    if let Expression::CallExpression(c) = n
        && let Expression::Identifier(_) = &*c.callee
    {
        joined = format!("(){joined}");
        n = &c.callee;
    }

    let Expression::Identifier(n) = n else { return None };

    let binding = arena.get(scope, &n.name);
    if binding.is_some() {
        return None; // rune name, but references a variable or store
    }

    Some(format!("{}{joined}", n.name))
}

/// `should_proxy(node, scope)`
pub fn should_proxy(arena: &ScopeArena<'_>, node: &Expression, scope: Option<ScopeId>) -> bool {
    if matches!(
        node,
        Expression::Literal(_)
            | Expression::TemplateLiteral(_)
            | Expression::ArrowFunctionExpression(_)
            | Expression::FunctionExpression(_)
            | Expression::UnaryExpression(_)
            | Expression::BinaryExpression(_)
    ) || matches!(node, Expression::Identifier(i) if i.name == "undefined")
    {
        return false;
    }

    if let (Expression::Identifier(i), Some(scope)) = (node, scope)
        && let Some(binding) = arena.get(scope, &i.name)
    {
        let binding = arena.binding(binding);
        // Let's see if the reference is something that can be proxied
        if !binding.reassigned
            && let Some(Initial::Expression(initial)) = binding.initial
        {
            return should_proxy(arena, initial, None);
        }
    }

    true
}

/// Checks if the name is valid, which it is when it's not starting with (or is) a dollar sign or if it's a function parameter.
/// The second argument is the depth of the scope, which is there for backwards compatibility reasons: In Svelte 4, you
/// were allowed to define `$`-prefixed variables anywhere below the top level of components. Once legacy mode is gone, this
/// argument can be removed / the call sites adjusted accordingly.
pub fn validate_identifier_name(
    arena: &ScopeArena<'_>,
    binding: Option<BindingId>,
    function_depth: Option<u32>,
) -> Result<(), CompileError> {
    let Some(binding) = binding else { return Ok(()) };
    let binding = arena.binding(binding);

    let declaration_kind = binding.declaration_kind;

    if declaration_kind != DeclarationKind::Synthetic
        && declaration_kind != DeclarationKind::Param
        && declaration_kind != DeclarationKind::RestParam
        && function_depth.is_none_or(|d| d <= 1)
    {
        let node = &binding.node;

        if node.name() == "$" {
            return Err(e::dollar_binding_invalid(node.span()));
        } else if node.name().starts_with('$') {
            return Err(e::dollar_prefix_invalid(node.span()));
        }
    }

    Ok(())
}
