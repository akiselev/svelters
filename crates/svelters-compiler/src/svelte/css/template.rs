//! The template metadata the CSS pruner reads, which upstream's analysis visitors write onto the
//! nodes (Svelte 5.57.2): `analysis.elements`, `metadata.path` of elements, render tags and
//! components (`visitors/RegularElement.js`, `SvelteElement.js`, `RenderTag.js`,
//! `shared/component.js`), `RenderTag` / `Component` `metadata.snippets` and `SnippetBlock`
//! `metadata.sites` (`2-analyze/index.js`), and `analysis.snippets`.
//!
//! Paths are zimmerframe's: every template ancestor from the root `Fragment` down, `Fragment`s
//! included. Nodes are identified by [`NodeKey`].

use crate::estree::Expression;
use crate::svelte::ast::{self, AttributeLike, AttributeValue, ExpressionTagNode, FragmentNode, TextOrExpressionTag};
use crate::svelte::scope::analyze::ComponentScopeAnalysis;
use crate::svelte::scope::walk::{fragment_node_ref, NodeKey, NodeRef};
use crate::svelte::scope::{BindingKind, DeclarationKind, Initial, ScopeId};
use std::collections::HashMap;

pub struct Template<'a> {
    /// `analysis.elements` (RegularElement and SvelteElement), in visit order
    pub elements: Vec<NodeRef<'a>>,
    paths: HashMap<NodeKey, Vec<NodeRef<'a>>>,
    /// RenderTag / Component-like -> `metadata.snippets` (SnippetBlock refs, set order)
    snippets_of: HashMap<NodeKey, Vec<NodeRef<'a>>>,
    /// SnippetBlock -> `metadata.sites`
    sites: HashMap<NodeKey, Vec<NodeRef<'a>>>,
}

/// The template children of a node (zimmerframe key order), template nodes only.
pub fn template_children<'a>(n: NodeRef<'a>) -> Vec<NodeRef<'a>> {
    use NodeRef as N;
    let f = |f: &'a ast::Fragment| NodeRef::Fragment(f);
    match n {
        N::Root(r) => vec![f(&r.fragment)],
        N::Fragment(fr) => fr.nodes.iter().map(fragment_node_ref).collect(),
        N::Component(e) | N::TitleElement(e) | N::SlotElement(e) | N::RegularElement(e) | N::SvelteBody(e) | N::SvelteDocument(e)
        | N::SvelteFragment(e) | N::SvelteBoundary(e) | N::SvelteHead(e) | N::SvelteOptions(e) | N::SvelteSelf(e) | N::SvelteWindow(e) => {
            vec![f(&e.fragment)]
        }
        N::SvelteComponent(e) => vec![f(&e.fragment)],
        N::SvelteElement(e) => vec![f(&e.fragment)],
        N::EachBlock(b) => std::iter::once(f(&b.body)).chain(b.fallback.as_ref().map(f)).collect(),
        N::IfBlock(b) => std::iter::once(f(&b.consequent)).chain(b.alternate.as_ref().map(f)).collect(),
        N::AwaitBlock(b) => [&b.pending, &b.then, &b.catch].into_iter().flatten().map(f).collect(),
        N::KeyBlock(b) => vec![f(&b.fragment)],
        N::SnippetBlock(b) => vec![f(&b.body)],
        _ => Vec::new(),
    }
}

/// `{ name, attributes }` of an element-like node.
pub fn element_parts<'a>(n: NodeRef<'a>) -> Option<(&'a str, &'a [AttributeLike])> {
    use NodeRef as N;
    match n {
        N::Component(e) | N::TitleElement(e) | N::SlotElement(e) | N::RegularElement(e) | N::SvelteBody(e) | N::SvelteDocument(e)
        | N::SvelteFragment(e) | N::SvelteBoundary(e) | N::SvelteHead(e) | N::SvelteOptions(e) | N::SvelteSelf(e) | N::SvelteWindow(e) => {
            Some((&e.name, &e.attributes))
        }
        N::SvelteComponent(e) => Some((&e.name, &e.attributes)),
        N::SvelteElement(e) => Some((&e.name, &e.attributes)),
        _ => None,
    }
}

pub fn fragment_of<'a>(n: NodeRef<'a>) -> Option<&'a ast::Fragment> {
    template_children(n).into_iter().find_map(|c| match c {
        NodeRef::Fragment(f) if element_parts(n).is_some() => Some(f),
        _ => None,
    })
}

pub fn is_element(n: NodeRef) -> bool {
    matches!(n, NodeRef::RegularElement(_) | NodeRef::SvelteElement(_))
}

fn is_component_like(n: NodeRef) -> bool {
    matches!(n, NodeRef::Component(_) | NodeRef::SvelteComponent(_) | NodeRef::SvelteSelf(_))
}

/// `is_resolved_snippet(binding)`
fn is_resolved_snippet(analysis: &ComponentScopeAnalysis, binding: Option<crate::svelte::scope::BindingId>) -> bool {
    let Some(b) = binding else { return true };
    let b = analysis.arena.binding(b);
    b.declaration_kind == DeclarationKind::Import
        || matches!(b.kind, BindingKind::Prop | BindingKind::RestProp | BindingKind::BindableProp)
        || matches!(b.initial, Some(Initial::SnippetBlock(_)))
}

fn snippet_initial<'a>(analysis: &ComponentScopeAnalysis<'a>, binding: Option<crate::svelte::scope::BindingId>) -> Option<&'a ast::SnippetBlock> {
    match analysis.arena.binding(binding?).initial {
        Some(Initial::SnippetBlock(s)) => Some(s),
        _ => None,
    }
}

struct Builder<'s, 'a> {
    analysis: &'s ComponentScopeAnalysis<'a>,
    t: Template<'a>,
    all_snippets: Vec<NodeRef<'a>>,
    /// snippet_renderers: (node, resolved), in visit order
    renderers: Vec<(NodeRef<'a>, bool)>,
}

impl<'s, 'a> Builder<'s, 'a> {
    /// `context.state.scope` at `node`: the scope of the nearest node (itself or an ancestor) the
    /// scope creation recorded one for.
    fn scope_at(&self, node: NodeRef<'a>, path: &[NodeRef<'a>]) -> ScopeId {
        let scopes = &self.analysis.template.scopes;
        std::iter::once(node)
            .chain(path.iter().rev().copied())
            .find_map(|n| scopes.get(&n.key()).copied())
            .unwrap_or(self.analysis.template.scope)
    }

    fn walk(&mut self, node: NodeRef<'a>, path: &mut Vec<NodeRef<'a>>) {
        match node {
            NodeRef::RegularElement(_) | NodeRef::SvelteElement(_) => {
                self.t.paths.insert(node.key(), path.clone());
                self.t.elements.push(node);
            }
            NodeRef::RenderTag(r) => {
                self.t.paths.insert(node.key(), path.clone());
                let mut expr = &r.expression;
                if let Expression::ChainExpression(c) = expr {
                    expr = &c.expression;
                }
                let mut resolved = false;
                let mut snippets = Vec::new();
                if let Expression::CallExpression(call) = expr {
                    if let Expression::Identifier(id) = &*call.callee {
                        let scope = self.scope_at(node, path);
                        let binding = self.analysis.arena.get(scope, &id.name);
                        resolved = is_resolved_snippet(self.analysis, binding);
                        if let Some(s) = snippet_initial(self.analysis, binding) {
                            snippets.push(NodeRef::SnippetBlock(s));
                        }
                    }
                }
                self.t.snippets_of.insert(node.key(), snippets);
                self.renderers.push((node, resolved));
            }
            NodeRef::SnippetBlock(_) => self.all_snippets.push(node),
            _ if is_component_like(node) => {
                self.t.paths.insert(node.key(), path.clone());
                let (_, attributes) = element_parts(node).unwrap();
                let mut resolved = true;
                let mut snippets: Vec<NodeRef<'a>> = Vec::new();
                let scope = self.scope_at(node, path);
                for a in attributes {
                    match a {
                        AttributeLike::SpreadAttribute(_) | AttributeLike::BindDirective(_) => {
                            resolved = false;
                            continue;
                        }
                        AttributeLike::Attribute(attr) => {
                            let Some(expression) = attribute_expression(&attr.value) else { continue };
                            match expression {
                                Expression::Identifier(id) => {
                                    let binding = self.analysis.arena.get(scope, &id.name);
                                    resolved &= is_resolved_snippet(self.analysis, binding);
                                    if let Some(s) = snippet_initial(self.analysis, binding) {
                                        push_unique(&mut snippets, NodeRef::SnippetBlock(s));
                                    }
                                }
                                Expression::Literal(_) => {}
                                _ => resolved = false,
                            }
                        }
                        _ => {}
                    }
                }
                if resolved {
                    for child in &fragment_of(node).unwrap().nodes {
                        if let FragmentNode::SnippetBlock(s) = child {
                            push_unique(&mut snippets, NodeRef::SnippetBlock(s));
                        }
                    }
                }
                self.t.snippets_of.insert(node.key(), snippets);
                self.renderers.push((node, resolved));
            }
            _ => {}
        }
        path.push(node);
        for c in template_children(node) {
            self.walk(c, path);
        }
        path.pop();
    }
}

fn push_unique<'a>(v: &mut Vec<NodeRef<'a>>, n: NodeRef<'a>) {
    if !v.iter().any(|x| x.key() == n.key()) {
        v.push(n);
    }
}

/// `get_attribute_expression` of an expression attribute (`is_expression_attribute`).
fn attribute_expression(v: &AttributeValue) -> Option<&Expression> {
    match v {
        AttributeValue::Expression(ExpressionTagNode::ExpressionTag(t)) => Some(&t.expression),
        AttributeValue::Sequence(s) if s.len() == 1 => match &s[0] {
            TextOrExpressionTag::ExpressionTag(t) => Some(&t.expression),
            _ => None,
        },
        _ => None,
    }
}

impl<'a> Template<'a> {
    pub fn build(root: &'a ast::Root, analysis: &ComponentScopeAnalysis<'a>) -> Template<'a> {
        let mut b = Builder {
            analysis,
            t: Template { elements: Vec::new(), paths: HashMap::new(), snippets_of: HashMap::new(), sites: HashMap::new() },
            all_snippets: Vec::new(),
            renderers: Vec::new(),
        };
        let mut path = Vec::new();
        b.walk(NodeRef::Fragment(&root.fragment), &mut path);
        let Builder { mut t, all_snippets, renderers, .. } = b;
        for (node, resolved) in renderers {
            if !resolved {
                t.snippets_of.insert(node.key(), all_snippets.clone());
            }
            for s in t.snippets_of[&node.key()].clone() {
                let sites = t.sites.entry(s.key()).or_default();
                push_unique(sites, node);
            }
        }
        t
    }

    pub fn path(&self, n: NodeRef<'a>) -> &[NodeRef<'a>] {
        self.paths.get(&n.key()).map_or(&[], |p| p.as_slice())
    }

    pub fn snippets(&self, n: NodeRef<'a>) -> &[NodeRef<'a>] {
        self.snippets_of.get(&n.key()).map_or(&[], |p| p.as_slice())
    }

    pub fn sites(&self, n: NodeRef<'a>) -> &[NodeRef<'a>] {
        self.sites.get(&n.key()).map_or(&[], |p| p.as_slice())
    }
}
