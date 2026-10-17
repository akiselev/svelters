//! The scope-related slices of the analysis phase (Svelte 5.57.2 @ f3de3f2), so consumers get a
//! complete binding model without the full `phases/2-analyze` port:
//!
//! - `phases/2-analyze/index.js`: `js()`, and from `analyze_component` the scope creation for
//!   module / instance / template, the synthetic `store_sub` bindings for `$store` references
//!   (with their errors), runes-mode detection, the legacy `$$props` / `$$restProps` bindings and
//!   the `synthetic_stores_legacy_check`;
//! - `phases/2-analyze/visitors/VariableDeclarator.js`: binding kinds for `$state`, `$state.raw`,
//!   `$derived(.by)`, `$props` (prop / bindable_prop / rest_prop, `prop_alias`, `exclude_props`,
//!   rewired `initial`) and its errors, run over the three ASTs with the analysis walker's
//!   `set_scope` semantics.
//!
//! SVELTERS: not ported here (they belong to the full analysis port): warnings
//! (`store_rune_conflict`, custom element warnings), the legacy-mode binding kinds set by other
//! visitors (`export let` props, `$:` dependencies, legacy `state` promotion outside the store
//! check), and every other analysis visitor.

use super::ast_utils::{PatternTarget, extract_identifiers, extract_paths};
use super::js::number_to_string;
use super::support::{determine_slot, is_rune};
use super::walk::{NodeRef, Visitor, Walker, attribute_like_ref, expr_ref, fragment_node_ref};
use super::{
    BindingKind, BindingMetadata, ComponentScopes, CreatedScopes, DeclarationKind, Ident, Initial, NodeKey, ScopeArena,
    ScopeId, create_scopes, get_rune, get_rune_of_call, set_scope, validate_identifier_name,
};
use crate::estree::*;
use crate::svelte::ast::{self, Root, Script};
use crate::svelte::errors::CompileError;
use crate::svelte::scope_errors as e;
use std::collections::HashMap;
use std::sync::OnceLock;

/// `{ ast, scope, scopes, has_await }` of `js()` / the template.
pub type Js = CreatedScopes;

/// The scopes of a component (`analysis.module`, `.instance`, `.template`, `.root`, `.runes`).
#[derive(Debug)]
pub struct ComponentScopeAnalysis<'a> {
    pub arena: ScopeArena<'a>,
    pub module: Js,
    pub instance: Js,
    pub template: Js,
    pub runes: bool,
}

impl<'a> ComponentScopeAnalysis<'a> {
    /// The scope recorded for `node` in any of the three scope maps.
    pub fn scope_for(&self, node: NodeRef<'_>) -> Option<ScopeId> {
        let k = node.key();
        self.template
            .scopes
            .get(&k)
            .or_else(|| self.module.scopes.get(&k))
            .or_else(|| self.instance.scopes.get(&k))
            .copied()
    }
}

fn empty_program() -> &'static Program {
    static EMPTY: OnceLock<Program> = OnceLock::new();
    EMPTY.get_or_init(|| Program {
        start: 0,
        end: 0,
        body: Vec::new(),
        source_type: SourceType::Module,
    })
}

/// `js(script, root, allow_reactive_declarations, parent)`
pub fn js<'a>(
    script: Option<&'a Script>,
    arena: &mut ScopeArena<'a>,
    allow_reactive_declarations: bool,
    parent: Option<ScopeId>,
) -> Result<Js, CompileError> {
    let ast: &'a Program = match script {
        Some(s) => &s.content,
        None => empty_program(),
    };

    create_scopes(arena, NodeRef::Program(ast), allow_reactive_declarations, parent)
}

const RESERVED: &[&str] = &["$$props", "$$restProps", "$$slots"];

/// Scope creation of `analyze_component(root, source, options)`. `runes_option` is
/// `options.runes` (`None`: infer).
pub fn analyze_component_scopes<'a>(
    root: &'a Root,
    runes_option: Option<bool>,
) -> Result<ComponentScopeAnalysis<'a>, CompileError> {
    let mut arena = ScopeArena::new();

    let module = js(root.module.as_ref(), &mut arena, false, None)?;
    let instance = js(root.instance.as_ref(), &mut arena, true, Some(module.scope))?;

    let template = create_scopes(
        &mut arena,
        NodeRef::Fragment(&root.fragment),
        false,
        Some(instance.scope),
    )?;

    let mut synthetic_stores_legacy_check = Vec::new();

    // create synthetic bindings for store subscriptions
    let module_references: Vec<(String, Vec<super::ReferenceId>)> = arena
        .scope(module.scope)
        .references
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    for (name, references) in module_references {
        if !name.starts_with('$') || RESERVED.contains(&name.as_str()) {
            continue;
        }
        let first = || arena.reference_record(references[0]).node.span();
        if name == "$" || name.as_bytes().get(1) == Some(&b'$') {
            return Err(e::global_reference_invalid(first(), &name));
        }

        let store_name = &name[1..];
        let declaration = arena.get(instance.scope, store_name);
        let init = declaration.and_then(|d| arena.binding(d).initial);
        let init_expr = init.and_then(Initial::as_expression);

        // If we're not in legacy mode through the compiler option, assume the user
        // is referencing a rune and not a global store.
        if runes_option == Some(false)
            || !is_rune(&name)
            || (declaration.is_some()
                // const state = $state(0) is valid
                && (get_rune(&arena, init_expr, instance.scope).is_none()
                    // rune-line names received as props are valid too (but we have to protect against $props as store)
                    || (store_name != "props" && get_rune(&arena, init_expr, instance.scope) == Some("$props")))
                // allow `import { derived } from 'svelte/store'` in the same file as `const x = $derived(..)` because one is not a subscription to the other
                && !(name == "$derived"
                    && matches!(init, Some(Initial::ImportDeclaration(i))
                        if matches!(&i.source, LiteralNode::Literal(Literal { value: LiteralValue::String(s), .. }) if s == "svelte/store"))))
        {
            let mut is_nested_store_subscription_node = None;
            'search: for &reference in &references {
                let reference = arena.reference_record(reference);
                for node in reference.path.iter().rev() {
                    let k = node.key();
                    let scope = template
                        .scopes
                        .get(&k)
                        .or_else(|| module.scopes.get(&k))
                        .or_else(|| instance.scopes.get(&k));
                    if let Some(&scope) = scope {
                        let owner = arena.owner(scope, store_name);
                        if let Some(owner) = owner
                            && owner != module.scope
                            && owner != instance.scope
                        {
                            is_nested_store_subscription_node = Some(reference.node.span());
                            break 'search;
                        }
                        break;
                    }
                }
            }

            if let Some(node) = is_nested_store_subscription_node {
                return Err(e::store_invalid_scoped_subscription(node));
            }

            if runes_option != Some(false)
                && declaration.is_none()
                && store_name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
            {
                return Err(e::global_reference_invalid(first(), &name));
            }
            // (`store_rune_conflict` warning not ported)

            if let Some(module_script) = &root.module {
                let program = &module_script.content;
                for &reference in &references {
                    let reference = arena.reference_record(reference);
                    let Some(span) = reference.node.span() else { continue };
                    // if the reference is inside module, error. this is a bit hacky but it works
                    let parent_rune = match reference.path.last() {
                        Some(NodeRef::CallExpression(c)) => get_rune_of_call(&arena, c, module.scope),
                        _ => None,
                    };
                    if span.start > program.start && span.end < program.end && parent_rune.is_none() {
                        return Err(e::store_invalid_subscription(Some(span)));
                    }
                }
            }

            // we push to the array because at this moment in time we can't be sure if we are in legacy
            // mode yet because we are still changing the module scope
            synthetic_stores_legacy_check.push(declaration);

            let binding = arena.declare(
                instance.scope,
                Ident::synthetic(&name),
                BindingKind::StoreSub,
                DeclarationKind::Synthetic,
                None,
            )?;
            arena.set_binding_references(binding, references.clone());
            arena
                .scope_mut(instance.scope)
                .references
                .insert(name.clone(), references);
            arena.scope_mut(module.scope).references.remove(&name);
        }
    }

    let runes = runes_option.unwrap_or_else(|| {
        template.has_await || instance.has_await || arena.scope(module.scope).references.keys().any(is_rune)
    });

    if !runes {
        for declaration in synthetic_stores_legacy_check.into_iter().flatten() {
            // if we are creating a synthetic binding for a let declaration we should also declare
            // the declaration as state in case it's reassigned and we are not in runes mode (the function will
            // not be called if we are not in runes mode, that's why there's no !runes check here)
            let d = arena.binding_mut(declaration);
            if d.kind == BindingKind::Normal && d.declaration_kind == DeclarationKind::Let && d.reassigned {
                d.kind = BindingKind::State;
            }
        }

        arena.declare(
            instance.scope,
            Ident::synthetic("$$props"),
            BindingKind::RestProp,
            DeclarationKind::Synthetic,
            None,
        )?;
        arena.declare(
            instance.scope,
            Ident::synthetic("$$restProps"),
            BindingKind::RestProp,
            DeclarationKind::Synthetic,
            None,
        )?;
    }

    let mut analysis = ComponentScopeAnalysis {
        arena,
        module,
        instance,
        template,
        runes,
    };

    let module_ast = NodeRef::Program(root.module.as_ref().map_or(empty_program(), |s| &s.content));
    let instance_ast = NodeRef::Program(root.instance.as_ref().map_or(empty_program(), |s| &s.content));
    let template_ast = NodeRef::Fragment(&root.fragment);
    for (ast, ast_type) in [
        (module_ast, AstType::Module),
        (instance_ast, AstType::Instance),
        (template_ast, AstType::Template),
    ] {
        let (scope, scopes, component_scopes) = match ast_type {
            AstType::Module => (
                analysis.module.scope,
                &analysis.module.scopes,
                &analysis.module.component_scopes,
            ),
            AstType::Instance => (
                analysis.instance.scope,
                &analysis.instance.scopes,
                &analysis.instance.component_scopes,
            ),
            AstType::Template => (
                analysis.template.scope,
                &analysis.template.scopes,
                &analysis.template.component_scopes,
            ),
        };
        let mut v = DeclaratorKinds {
            arena: &mut analysis.arena,
            scopes,
            component_scopes,
            runes,
            ast_type,
            module_scope: analysis.module.scope,
            instance_scope: analysis.instance.scope,
            error: None,
        };
        Walker::walk(ast, &scope, &mut v);
        if let Some(err) = v.error {
            return Err(err);
        }
    }

    Ok(analysis)
}

/// The scopes of a `.svelte.js` module (`analysis.module` of `analyze_module`).
#[derive(Debug)]
pub struct ModuleScopeAnalysis<'a> {
    pub arena: ScopeArena<'a>,
    pub module: Js,
}

/// Scope creation of `analyze_module(source, options)` (runes mode): the module scope, the
/// `$name` reference checks and the `VariableDeclarator` binding kinds.
pub fn analyze_module_scopes<'a>(ast: &'a Program) -> Result<ModuleScopeAnalysis<'a>, CompileError> {
    let mut arena = ScopeArena::new();
    let module = create_scopes(&mut arena, NodeRef::Program(ast), false, None)?;

    for (name, references) in arena.scope(module.scope).references.iter() {
        if !name.starts_with('$') || RESERVED.contains(&name) {
            continue;
        }
        let first = arena.reference_record(references[0]).node.span();
        if name == "$" || name.as_bytes().get(1) == Some(&b'$') {
            return Err(e::global_reference_invalid(first, name));
        }
        let binding = arena.get(module.scope, &name[1..]);
        if binding.is_some() && !is_rune(name) {
            return Err(e::store_invalid_subscription_module(first));
        }
    }

    let mut v = DeclaratorKinds {
        arena: &mut arena,
        scopes: &module.scopes,
        component_scopes: &module.component_scopes,
        runes: true,
        ast_type: AstType::Module,
        module_scope: module.scope,
        instance_scope: module.scope,
        error: None,
    };
    Walker::walk(NodeRef::Program(ast), &module.scope, &mut v);
    if let Some(err) = v.error {
        return Err(err);
    }
    Ok(ModuleScopeAnalysis { arena, module })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AstType {
    Module,
    Instance,
    Template,
}

/// The analysis walk restricted to the `_` (set_scope) and `VariableDeclarator` visitors (plus
/// the component visitor's slot scopes, `visitors/shared/component.js`).
struct DeclaratorKinds<'s, 'a> {
    arena: &'s mut ScopeArena<'a>,
    scopes: &'s HashMap<NodeKey, ScopeId>,
    component_scopes: &'s HashMap<NodeKey, ComponentScopes>,
    runes: bool,
    ast_type: AstType,
    module_scope: ScopeId,
    instance_scope: ScopeId,
    error: Option<CompileError>,
}

impl<'s, 'a> Visitor<'a> for DeclaratorKinds<'s, 'a> {
    type State = ScopeId;

    fn visit(&mut self, node: NodeRef<'a>, state: &ScopeId, w: &mut Walker<'a>) {
        let scope = set_scope(self.scopes, node, *state);
        match node {
            NodeRef::VariableDeclarator(d) => {
                if let Err(err) = self.variable_declarator(d, scope) {
                    self.error = Some(err);
                    w.stop();
                    return;
                }
                w.next(self, node, &scope);
            }
            NodeRef::Component(_) | NodeRef::SvelteSelf(_) | NodeRef::SvelteComponent(_) => {
                let Some(cs) = self.component_scopes.get(&node.key()).cloned() else {
                    return w.next(self, node, &scope);
                };
                let (attributes, fragment) = match node {
                    NodeRef::SvelteComponent(n) => {
                        w.visit(self, node, expr_ref(&n.expression), &scope);
                        (&n.attributes, &n.fragment)
                    }
                    NodeRef::Component(n) | NodeRef::SvelteSelf(n) => (&n.attributes, &n.fragment),
                    _ => unreachable!(),
                };
                let default_scope = if determine_slot(node).is_some() {
                    scope
                } else {
                    cs.default
                };
                for attribute in attributes {
                    let s = if matches!(attribute, ast::AttributeLike::LetDirective(_)) {
                        default_scope
                    } else {
                        scope
                    };
                    w.visit(self, node, attribute_like_ref(attribute), &s);
                }
                for child in &fragment.nodes {
                    let child = fragment_node_ref(child);
                    let s = match determine_slot(child) {
                        Some(slot) => cs
                            .slots
                            .iter()
                            .find(|(n, _)| n == slot)
                            .map_or(default_scope, |(_, s)| *s),
                        None => default_scope,
                    };
                    w.visit(self, node, child, &s);
                }
            }
            _ => w.next(self, node, &scope),
        }
    }
}

fn literal_to_string(l: &Literal) -> String {
    match &l.value {
        LiteralValue::String(s) => s.clone(),
        LiteralValue::Number(n) => number_to_string(*n),
        LiteralValue::Boolean(b) => b.to_string(),
        LiteralValue::Null => "null".into(),
        LiteralValue::BigInt => l.bigint.clone().unwrap_or_default(),
        LiteralValue::RegExp => l.raw.clone(),
    }
}

impl<'s, 'a> DeclaratorKinds<'s, 'a> {
    /// `ensure_no_module_import_conflict(node, state)` (visitors/shared/utils.js)
    fn ensure_no_module_import_conflict(
        &self,
        node: &'a VariableDeclarator,
        scope: ScopeId,
    ) -> Result<(), CompileError> {
        for id in extract_identifiers(&node.id) {
            if self.ast_type == AstType::Instance
                && scope == self.instance_scope
                && self
                    .arena
                    .get(self.module_scope, &id.name)
                    .is_some_and(|b| self.arena.binding(b).declaration_kind == DeclarationKind::Import)
            {
                return Err(e::declaration_duplicate_module_import(Some(node.id.span())));
            }
        }
        Ok(())
    }

    /// `VariableDeclarator(node, context)`
    fn variable_declarator(&mut self, node: &'a VariableDeclarator, scope: ScopeId) -> Result<(), CompileError> {
        self.ensure_no_module_import_conflict(node, scope)?;
        let arena = &mut *self.arena;

        if self.runes {
            let init = node.init.as_ref();
            let rune = get_rune(arena, init, scope);
            let paths = extract_paths(&node.id);

            let name_of = |t: PatternTarget<'a>| match t {
                PatternTarget::Identifier(i) => Some(i.name.as_str()),
                PatternTarget::MemberExpression(_) => None,
            };

            for path in &paths {
                let binding = name_of(path.node).and_then(|n| arena.get(scope, n));
                validate_identifier_name(arena, binding, None)?;
            }

            // TODO feels like this should happen during scope creation?
            if matches!(
                rune,
                Some("$state" | "$state.raw" | "$derived" | "$derived.by" | "$props")
            ) {
                for path in &paths {
                    let Some(binding) = name_of(path.node).and_then(|n| arena.get(scope, n)) else {
                        continue;
                    };
                    let kind = match rune {
                        Some("$state") => BindingKind::State,
                        Some("$state.raw") => BindingKind::RawState,
                        Some("$derived" | "$derived.by") => BindingKind::Derived,
                        _ if path.is_rest => BindingKind::RestProp,
                        _ => BindingKind::Prop,
                    };
                    arena.binding_mut(binding).kind = kind;
                    if rune == Some("$props")
                        && kind == BindingKind::RestProp
                        && let Pattern::ObjectPattern(o) = &node.id
                    {
                        let mut exclude_props = Vec::new();
                        for property in &o.properties {
                            let ObjectPatternMember::Property(property) = property else {
                                continue;
                            };
                            let key = match &property.key {
                                PropertyKey::Expression(k) => match &**k {
                                    Expression::Identifier(i) => i.name.clone(),
                                    Expression::Literal(l) => literal_to_string(l),
                                    _ => continue,
                                },
                                PropertyKey::Private(_) => continue,
                            };
                            exclude_props.push(key);
                        }
                        arena
                            .binding_mut(binding)
                            .metadata
                            .get_or_insert_with(BindingMetadata::default)
                            .exclude_props = Some(exclude_props);
                    }
                }
            }

            if rune == Some("$props") {
                match &node.id {
                    Pattern::Identifier(id) => {
                        if let Some(binding) = arena.get(scope, &id.name) {
                            let b = arena.binding_mut(binding);
                            b.initial = None; // else would be $props()
                            b.kind = BindingKind::RestProp;
                        }
                    }
                    Pattern::ObjectPattern(o) => {
                        for property in &o.properties {
                            let ObjectPatternMember::Property(property) = property else {
                                continue;
                            };

                            if property.computed {
                                return Err(e::props_invalid_pattern(Some(property.span())));
                            }

                            let key = match &property.key {
                                PropertyKey::Expression(k) => &**k,
                                PropertyKey::Private(_) => return Err(e::props_invalid_pattern(Some(property.span()))),
                            };

                            if let Expression::Identifier(k) = key
                                && k.name.starts_with("$$")
                            {
                                return Err(e::props_illegal_name(Some(property.span())));
                            }

                            let value = match &property.value {
                                Pattern::AssignmentPattern(a) => &*a.left,
                                v => v,
                            };

                            let Pattern::Identifier(value) = value else {
                                return Err(e::props_invalid_pattern(Some(property.span())));
                            };

                            let alias = match key {
                                Expression::Identifier(k) => k.name.clone(),
                                Expression::Literal(l) => literal_to_string(l),
                                _ => String::new(),
                            };

                            let initial = match &property.value {
                                Pattern::AssignmentPattern(a) => Some(&*a.right),
                                _ => None,
                            };

                            let Some(binding) = arena.get(scope, &value.name) else {
                                continue;
                            };
                            let b = arena.binding_mut(binding);
                            b.prop_alias = Some(alias);

                            // rewire initial from $props() to the actual initial value, stripping $bindable() if necessary
                            if let Some(Expression::CallExpression(call)) = initial
                                && let Expression::Identifier(callee) = &*call.callee
                                && callee.name == "$bindable"
                            {
                                b.initial = match call.arguments.first() {
                                    Some(ArrayElement::Expression(e)) => Some(Initial::Expression(e)),
                                    _ => None,
                                };
                                b.kind = BindingKind::BindableProp;
                            } else {
                                b.initial = initial.map(Initial::Expression);
                            }
                        }
                    }
                    _ => return Err(e::props_invalid_identifier(Some(node.span()))),
                }
            }
        } else if let Some(init @ Expression::CallExpression(call)) = &node.init
            && let Expression::Identifier(callee) = &*call.callee
            && matches!(callee.name.as_str(), "$state" | "$derived" | "$props")
            && arena
                .get(scope, &callee.name)
                .is_none_or(|b| arena.binding(b).kind != BindingKind::StoreSub)
        {
            return Err(e::rune_invalid_usage(Some(init.span()), &callee.name));
        }

        Ok(())
    }
}
