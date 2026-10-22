//! Port of `phases/2-analyze/css/css-prune.js` (Svelte 5.57.2): marks the selectors that may
//! match an element of the template (`used`), the relative selectors and elements that get the
//! scoping class (`scoped`).
//!
//! SVELTERS: upstream builds modified copies of relative selectors (`truncate`, the implicit `&`
//! of nested rules, `:has(...)` rewriting) that share the original's `metadata` object. Here a
//! [`Rel`] view carries the original's id (`None` for the synthetic `&` / `*` selectors, whose
//! metadata upstream writes to module-level objects nobody reads) plus the overridden combinator
//! and selector list.

use super::ast::*;
use super::template::{element_parts, fragment_of, is_element, template_children, Template};
use super::utils::{get_parent_rules, is_outer_global, is_unscoped_pseudo_class};
use super::{analyze::mark_all_complex, Css};
use crate::estree::{ArrayElement, Expression, LiteralValue, LogicalOperator, ObjectMember, PropertyKey};
use crate::svelte::ast::{AttributeLike, AttributeValue, ExpressionTagNode, FragmentNode, TextOrExpressionTag};
use crate::svelte::parse::utils::whitespace::is_whitespace;
use crate::svelte::scope::js::number_to_string;
use crate::svelte::scope::walk::{fragment_node_ref, NodeKey, NodeRef};
use std::collections::{HashMap, HashSet};

const NODE_PROBABLY_EXISTS: u8 = 0;
const NODE_DEFINITELY_EXISTS: u8 = 1;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dir {
    Forward,
    Backward,
}

/// HTML attributes whose enumerated values are case-insensitive per the HTML spec.
const CASE_INSENSITIVE_ATTRIBUTES: &[&str] = &[
    "accept-charset", "autocapitalize", "autocomplete", "behavior", "charset", "crossorigin", "decoding", "dir", "direction",
    "draggable", "enctype", "enterkeyhint", "fetchpriority", "formenctype", "formmethod", "formtarget", "hidden", "http-equiv",
    "inputmode", "kind", "loading", "method", "preload", "referrerpolicy", "rel", "rev", "role", "rules", "scope", "shape",
    "spellcheck", "target", "translate", "type", "valign", "wrap",
];

fn is_ws(c: char) -> bool {
    is_whitespace(c as u32)
}

/// A relative selector as the matcher sees it.
#[derive(Clone)]
struct Rel<'p> {
    id: Option<usize>,
    combinator: Option<&'p str>,
    selectors: Vec<&'p SimpleSelector>,
}

impl<'p> Rel<'p> {
    fn of(r: &'p RelativeSelector) -> Rel<'p> {
        Rel { id: Some(r.id), combinator: r.combinator.as_ref().map(|c| c.name.as_str()), selectors: r.selectors.iter().collect() }
    }
}

/// The synthetic `&` and `*` selectors (`nesting_selector`, `any_selector`).
pub struct Synthetic {
    nesting: SimpleSelector,
    any: SimpleSelector,
}

impl Synthetic {
    pub fn new() -> Synthetic {
        Synthetic {
            nesting: SimpleSelector::new(SimpleKind::NestingSelector, 0, 0, "&".into()),
            any: SimpleSelector::new(SimpleKind::TypeSelector, 0, 0, "*".into()),
        }
    }
}

impl Default for Synthetic {
    fn default() -> Self {
        Self::new()
    }
}

/// An insertion-ordered `Map<node, NodeExistsValue>`.
#[derive(Default)]
struct NodeMap<'a> {
    order: Vec<NodeRef<'a>>,
    values: HashMap<NodeKey, u8>,
}

impl<'a> NodeMap<'a> {
    fn set(&mut self, n: NodeRef<'a>, v: u8) {
        if self.values.insert(n.key(), v).is_none() {
            self.order.push(n);
        }
    }
    fn get(&self, n: NodeRef<'a>) -> Option<u8> {
        self.values.get(&n.key()).copied()
    }
    fn len(&self) -> usize {
        self.order.len()
    }
    fn keys(&self) -> Vec<NodeRef<'a>> {
        self.order.clone()
    }
}

fn has_definite_elements(m: &NodeMap) -> bool {
    m.len() > 0 && m.values.values().any(|v| *v == NODE_DEFINITELY_EXISTS)
}

fn add_to_map<'a>(from: &NodeMap<'a>, to: &mut NodeMap<'a>) {
    for n in &from.order {
        let exist = from.values[&n.key()];
        let v = match to.get(*n) {
            None => exist,
            Some(e2) => exist.max(e2),
        };
        to.set(*n, v);
    }
}

fn is_block(n: NodeRef) -> bool {
    matches!(n, NodeRef::IfBlock(_) | NodeRef::EachBlock(_) | NodeRef::AwaitBlock(_) | NodeRef::KeyBlock(_) | NodeRef::SlotElement(_))
}

pub struct Pruner<'c, 'p, 'a> {
    css: &'c Css<'p>,
    t: &'c Template<'a>,
    synth: &'p Synthetic,
    /// `element.metadata.scoped`
    pub scoped: HashSet<NodeKey>,
}

/// `prune(stylesheet, elements)`; returns the keys of the elements that got `metadata.scoped`.
pub fn prune<'p, 'a>(css: &Css<'p>, t: &Template<'a>, synth: &'p Synthetic) -> HashSet<NodeKey> {
    let mut p = Pruner { css, t, synth, scoped: HashSet::new() };
    for n in &css.sheet.children {
        p.visit(n);
    }
    p.scoped
}

impl<'c, 'p, 'a> Pruner<'c, 'p, 'a> {
    fn meta_rel_global(&self, r: &Rel) -> bool {
        r.id.is_some_and(|i| self.css.meta.rel[i].is_global.get())
    }
    fn meta_rel_global_like(&self, r: &Rel) -> bool {
        r.id.is_some_and(|i| self.css.meta.rel[i].is_global_like.get())
    }
    fn set_scoped(&self, r: &Rel) {
        if let Some(i) = r.id {
            self.css.meta.rel[i].scoped.set(true);
        }
    }
    fn set_used(&self, c: &ComplexSelector) {
        self.css.meta.complex[c.id].used.set(true);
    }
    fn rule(&self, id: usize) -> &'p Rule {
        self.css.rules[id]
    }
    fn parent_rule(&self, id: usize) -> Option<usize> {
        self.css.meta.rules[id].parent_rule.get()
    }

    fn visit(&mut self, n: &'p Node) {
        match n {
            Node::Rule(r) => {
                for c in &r.prelude.children {
                    self.complex(c);
                }
                if !self.css.meta.rules[r.id].is_global_block.get() {
                    for c in &r.block.children {
                        self.visit(c);
                    }
                }
            }
            Node::Atrule(a) => {
                if let Some(b) = &a.block {
                    for c in &b.children {
                        self.visit(c);
                    }
                }
            }
            Node::Declaration(_) => {}
        }
    }

    fn complex(&mut self, node: &'p ComplexSelector) {
        let selectors = self.get_relative_selectors(node);
        let rule = self.css.meta.complex[node.id].rule.get().expect("complex selector of a rule");

        // Global and ICSS export rules do not depend on an element in this component
        if self.every_is_global(&selectors, 0, selectors.len(), rule) {
            self.set_used(node);
        }

        for element in self.t.elements.clone() {
            // Elements rendered through <svelte:head> are not style-scopable.
            let in_head = self.t.path(element).iter().any(|a| matches!(a, NodeRef::SvelteHead(_)));
            if !in_head && self.apply_selector(&selectors, rule, element, Dir::Backward, 0, selectors.len()) {
                self.set_used(node);
            }
        }
    }

    /// Retrieves the relative selectors (minus the trailing globals) from a complex selector,
    /// adding the implicit `&` of a nested rule.
    fn get_relative_selectors(&self, node: &'p ComplexSelector) -> Vec<Rel<'p>> {
        let mut selectors = self.truncate(node);
        let has_parent = self.css.meta.complex[node.id].rule.get().is_some_and(|r| self.parent_rule(r).is_some());
        if has_parent && !selectors.is_empty() {
            // nesting could be inside pseudo classes like :is, :has or :where
            let has_explicit_nesting_selector = selectors.iter().any(|s| s.selectors.iter().any(|x| has_nesting(x)));
            if !has_explicit_nesting_selector {
                if selectors[0].combinator.is_none() {
                    selectors[0].combinator = Some(" ");
                }
                selectors.insert(0, Rel { id: None, combinator: None, selectors: vec![&self.synth.nesting] });
            }
        }
        selectors
    }

    /// Discard trailing `:global(...)` selectors, these are unused for scoping purposes
    fn truncate(&self, node: &'p ComplexSelector) -> Vec<Rel<'p>> {
        let meta = &self.css.meta;
        let i = node.children.iter().rposition(|c| {
            let m = &meta.rel[c.id];
            let first = c.selectors.first();
            // not after a :global selector
            !m.is_global_like.get()
                && !first.is_some_and(|f| f.is_pseudo_class("global") && f.args.is_none())
                // not a :global(...) without a :has/is/where(...) modifier that is scoped
                && !m.is_global.get()
        });
        let end = i.map_or(0, |i| i + 1);
        node.children[..end]
            .iter()
            .map(|child| {
                // In case of `:root.y:has(...)`, `y` is unscoped, but everything in `:has(...)` should be scoped (if not global).
                let root = child.selectors.iter().any(|s| s.is_pseudo_class("root"));
                let mut r = Rel::of(child);
                if root && !meta.rel[child.id].is_global_like.get() {
                    r.selectors.retain(|s| s.is_pseudo_class("has"));
                }
                r
            })
            .collect()
    }

    fn apply_selector(&mut self, rels: &[Rel<'p>], rule: usize, element: NodeRef<'a>, dir: Dir, from: usize, to: usize) -> bool {
        if from >= to {
            return false;
        }
        let selector_index = if dir == Dir::Forward { from } else { to - 1 };
        let rel = &rels[selector_index];
        let (rest_from, rest_to) = if dir == Dir::Forward { (from + 1, to) } else { (from, to - 1) };

        let matched = self.relative_selector_might_apply_to_node(rel, rule, element, dir)
            && self.apply_combinator(rel, rels, rest_from, rest_to, rule, element, dir);

        if matched {
            if !is_outer_global(&rel.selectors) {
                self.set_scoped(rel);
            }
            self.scoped.insert(element.key());
        }
        matched
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_combinator(&mut self, rel: &Rel<'p>, rels: &[Rel<'p>], from: usize, to: usize, rule: usize, node: NodeRef<'a>, dir: Dir) -> bool {
        let combinator = if dir == Dir::Forward {
            if from < to {
                rels[from].combinator
            } else {
                None
            }
        } else {
            rel.combinator
        };
        let Some(combinator) = combinator else { return true };

        match combinator {
            " " | ">" => {
                let is_adjacent = combinator == ">";
                let parents = if dir == Dir::Forward {
                    self.get_descendant_elements(node, is_adjacent)
                } else {
                    self.get_ancestor_elements(node, is_adjacent, &mut HashSet::new())
                };
                let mut parent_matched = false;
                for parent in &parents {
                    if self.apply_selector(rels, rule, *parent, dir, from, to) {
                        parent_matched = true;
                    }
                }
                parent_matched
                    || (dir == Dir::Backward && (!is_adjacent || parents.is_empty()) && self.every_is_global(rels, from, to, rule))
            }
            "+" | "~" => {
                let siblings = self.get_possible_element_siblings(node, dir, combinator == "+", &mut HashSet::new());
                let mut sibling_matched = false;
                for possible_sibling in siblings.keys() {
                    if matches!(possible_sibling, NodeRef::RenderTag(_) | NodeRef::SlotElement(_) | NodeRef::Component(_)) {
                        // `{@render foo()}<p>foo</p>` with `:global(.x) + p` is a match
                        if to - from == 1 && self.meta_rel_global(&rels[from]) {
                            sibling_matched = true;
                        }
                    } else if self.apply_selector(rels, rule, possible_sibling, dir, from, to) {
                        sibling_matched = true;
                    }
                }
                sibling_matched
                    || (dir == Dir::Backward && self.get_element_parent(node).is_none() && self.every_is_global(rels, from, to, rule))
            }
            // TODO other combinators
            _ => true,
        }
    }

    fn every_is_global(&self, rels: &[Rel<'p>], from: usize, to: usize, rule: usize) -> bool {
        (from..to).all(|i| self.is_global(&rels[i], Some(rule)))
    }

    /// `is_global(selector, rule)` of css-prune.js: a `:global(...)` or unscopeable selector, or an
    /// `:is(...)` / `:where(...)` that contains a global selector.
    fn is_global(&self, selector: &Rel<'p>, rule: Option<usize>) -> bool {
        if self.meta_rel_global(selector) || self.meta_rel_global_like(selector) {
            return true;
        }
        let mut explicitly_global = false;
        for s in &selector.selectors {
            let mut selector_list: Option<&'p SelectorList> = None;
            let mut can_be_global = false;
            let mut owner = rule;
            if s.kind == SimpleKind::PseudoClassSelector {
                if (s.name == "is" || s.name == "where") && s.args.is_some() {
                    selector_list = s.args.as_ref();
                } else {
                    can_be_global = is_unscoped_pseudo_class(s);
                }
            }
            if s.kind == SimpleKind::NestingSelector {
                owner = rule.and_then(|r| self.parent_rule(r));
                selector_list = owner.map(|o| &self.rule(o).prelude);
            }
            let has_global_selectors = selector_list
                .is_some_and(|l| l.children.iter().any(|c| c.children.iter().all(|r| self.is_global(&Rel::of(r), owner))));
            explicitly_global |= has_global_selectors;
            if !has_global_selectors && !can_be_global {
                return false;
            }
        }
        explicitly_global || selector.selectors.is_empty()
    }

    /// Ensure that `element` satisfies each simple selector in `relative_selector`
    fn relative_selector_might_apply_to_node(&mut self, rel: &Rel<'p>, rule: usize, element: NodeRef<'a>, dir: Dir) -> bool {
        let mut include_self: Option<bool> = None;

        for selector in rel.selectors.clone() {
            if selector.is_pseudo_class("has") && selector.args.is_some() {
                if include_self.is_none() {
                    // If this is a :has inside a global selector, we gotta include the element itself, too,
                    // because the global selector might be for an element that's outside the component,
                    // e.g. :root:has(.scoped), :global(.foo):has(.scoped), or :root { &:has(.scoped) {} }
                    let rules = get_parent_rules(&self.css.meta, Some(rule));
                    let a = rules.iter().any(|r| {
                        self.rule(*r).prelude.children.iter().any(|c| c.children.iter().any(|s| self.is_global(&Rel::of(s), Some(*r))))
                    });
                    let last = self.rule(*rules.last().unwrap());
                    let b = last.prelude.children.iter().any(|c| {
                        c.children.iter().any(|r| {
                            r.selectors.iter().any(|s| {
                                s.kind == SimpleKind::PseudoClassSelector && (s.name == "root" || (s.name == "global" && s.args.is_some()))
                            })
                        })
                    });
                    include_self = Some(a || b);
                }

                // :has(...) is special in that it means "look downwards in the CSS tree". Since our matching algorithm goes
                // upwards and back-to-front, we need to first check the selectors inside :has(...), then check the rest of the
                // selector in a way that is similar to ancestor matching. In a sense, we're treating `.x:has(.y)` as `.x .y`.
                let mut matched = false;
                for complex in &selector.args.as_ref().unwrap().children {
                    let truncated = self.truncate(complex);
                    let Some(first) = truncated.first() else {
                        // if it was just a :global(...)
                        self.set_used(complex);
                        matched = true;
                        continue;
                    };
                    let rest = &truncated[1..];

                    if include_self == Some(true) {
                        let mut first_self = first.clone();
                        first_self.combinator = None;
                        let mut v = vec![first_self];
                        v.extend(rest.iter().cloned());
                        if self.apply_selector(&v, rule, element, Dir::Forward, 0, v.len()) {
                            self.set_used(complex);
                            matched = true;
                        }
                    }

                    let mut first_excl = first.clone();
                    if first_excl.combinator.is_none() {
                        first_excl.combinator = Some(" ");
                    }
                    let mut v = vec![Rel { id: None, combinator: None, selectors: vec![&self.synth.any] }, first_excl];
                    v.extend(rest.iter().cloned());
                    if self.apply_selector(&v, rule, element, Dir::Forward, 0, v.len()) {
                        self.set_used(complex);
                        matched = true;
                    }
                }
                if !matched {
                    return false;
                }
                continue;
            }

            if matches!(selector.kind, SimpleKind::Percentage | SimpleKind::Nth) {
                continue;
            }

            let name = if selector.name.contains('\\') { unescape(&selector.name) } else { selector.name.clone() };

            match selector.kind {
                SimpleKind::PseudoClassSelector => {
                    if name == "host" || name == "root" {
                        return false;
                    }
                    if name == "global" && selector.args.is_some() && rel.selectors.len() == 1 {
                        let complex = &selector.args.as_ref().unwrap().children[0];
                        let v: Vec<Rel> = complex.children.iter().map(Rel::of).collect();
                        return self.apply_selector(&v, rule, element, Dir::Backward, 0, v.len());
                    }
                    // We came across a :global, everything beyond it is global and therefore a potential match
                    if name == "global" && selector.args.is_none() {
                        return true;
                    }
                    // :not(...) contents should stay unscoped. Scoping them would achieve the opposite of what we want,
                    // because they are then _more_ likely to bleed out of the component. The exception is complex selectors
                    // with descendants, in which case we scope them all.
                    if name == "not" && selector.args.is_some() {
                        for complex in &selector.args.as_ref().unwrap().children {
                            mark_all_complex(self.css, complex);
                            let relative = self.truncate(complex);
                            if complex.children.len() > 1 {
                                // foo:not(bar foo) means that bar is an ancestor of foo (side note: ending with foo is the only way the selector make sense).
                                // We can't fully check if that actually matches with our current algorithm, so we just assume it does.
                                for s in &relative {
                                    self.set_scoped(s);
                                }
                                let mut el = Some(element);
                                while let Some(e) = el {
                                    self.scoped.insert(e.key());
                                    el = self.get_element_parent(e);
                                }
                            }
                        }
                        continue;
                    }
                    if (name == "is" || name == "where") && selector.args.is_some() {
                        let mut matched = false;
                        for complex in &selector.args.as_ref().unwrap().children {
                            let relative = self.truncate(complex);
                            if relative.is_empty() {
                                self.set_used(complex);
                                matched = true;
                            } else if self.apply_selector(&relative, rule, element, Dir::Backward, 0, relative.len()) {
                                self.set_used(complex);
                                matched = true;
                            } else if complex.children.len() > 1 {
                                // foo :is(bar baz) can also mean that bar is an ancestor of foo, and baz a descendant.
                                // We can't fully check if that actually matches with our current algorithm, so we just assume it does.
                                self.set_used(complex);
                                matched = true;
                                for s in &relative {
                                    self.set_scoped(s);
                                }
                            }
                        }
                        if !matched {
                            return false;
                        }
                    }
                }
                SimpleKind::PseudoElementSelector => {}
                SimpleKind::AttributeSelector => {
                    let (ename, _) = element_parts(element).unwrap();
                    let ename = ename.to_lowercase();
                    let sname = selector.name.to_lowercase();
                    let whitelisted = matches!((ename.as_str(), sname.as_str()), ("details", "open") | ("dialog", "open"));
                    let flags = selector.flags.as_deref();
                    let ci = flags.is_some_and(|f| f.contains('i'))
                        || (!flags.is_some_and(|f| f.contains('s')) && CASE_INSENSITIVE_ATTRIBUTES.contains(&sname.as_str()));
                    let expected = selector.value.as_deref().map(unquote);
                    if !whitelisted && !attribute_matches(element, &selector.name, expected.as_deref(), selector.matcher.as_deref(), ci) {
                        return false;
                    }
                }
                SimpleKind::ClassSelector => {
                    if !attribute_matches(element, "class", Some(&name), Some("~="), false) {
                        return false;
                    }
                }
                SimpleKind::IdSelector => {
                    if !attribute_matches(element, "id", Some(&name), Some("="), false) {
                        return false;
                    }
                }
                SimpleKind::TypeSelector => {
                    let (ename, _) = element_parts(element).unwrap();
                    if ename.to_lowercase() != name.to_lowercase() && name != "*" && !matches!(element, NodeRef::SvelteElement(_)) {
                        return false;
                    }
                }
                SimpleKind::NestingSelector => {
                    let mut matched = false;
                    let parent = self.parent_rule(rule).expect("& inside a nested rule");
                    for complex in &self.rule(parent).prelude.children {
                        let rs = self.get_relative_selectors(complex);
                        if self.apply_selector(&rs, parent, element, dir, 0, rs.len())
                            || complex.children.iter().all(|s| self.is_global(&Rel::of(s), Some(parent)))
                        {
                            self.set_used(complex);
                            matched = true;
                        }
                    }
                    if !matched {
                        return false;
                    }
                }
                SimpleKind::Percentage | SimpleKind::Nth => {}
            }
        }
        // possible match
        true
    }

    fn get_element_parent(&self, node: NodeRef<'a>) -> Option<NodeRef<'a>> {
        self.t.path(node).iter().rev().find(|n| is_element(**n)).copied()
    }

    fn get_ancestor_elements(&self, node: NodeRef<'a>, adjacent_only: bool, seen: &mut HashSet<NodeKey>) -> Vec<NodeRef<'a>> {
        let mut ancestors = Vec::new();
        let path = self.t.path(node);
        let mut i = path.len();
        while i > 0 {
            i -= 1;
            let parent = path[i];
            if let NodeRef::SnippetBlock(_) = parent {
                if seen.insert(parent.key()) {
                    for site in self.t.sites(parent) {
                        ancestors.extend(self.get_ancestor_elements(*site, adjacent_only, seen));
                    }
                }
                break;
            }
            if is_element(parent) {
                // Special handling for <option> inside <select>: elements inside <option> should
                // also be considered descendants of <selectedcontent>, which clones the selected option's content
                if let NodeRef::RegularElement(p) = parent {
                    if p.name == "option" {
                        let is_direct_child = ancestors.is_empty();
                        let select_element = path[..i].iter().rev().find(|n| matches!(n, NodeRef::RegularElement(e) if e.name == "select"));
                        if let Some(select) = select_element {
                            if !adjacent_only || is_direct_child {
                                let selectedcontent = find_selectedcontent(*select);
                                if adjacent_only && is_direct_child {
                                    if let Some(sc) = selectedcontent {
                                        return vec![sc, parent];
                                    }
                                } else if let Some(sc) = selectedcontent {
                                    ancestors.push(sc);
                                }
                            }
                        }
                    }
                }
                ancestors.push(parent);
                if adjacent_only {
                    break;
                }
            }
        }
        ancestors
    }

    fn get_descendant_elements(&self, node: NodeRef<'a>, adjacent_only: bool) -> Vec<NodeRef<'a>> {
        let mut descendants = Vec::new();
        let mut seen = HashSet::new();
        let start = if matches!(node, NodeRef::RenderTag(_)) { Some(node) } else { fragment_of(node).map(NodeRef::Fragment) };
        if let Some(start) = start {
            self.walk_children(start, adjacent_only, &mut seen, &mut descendants);
        }

        // Special handling for <selectedcontent>: it clones the content of the selected <option>,
        // so descendants of <option> elements in the same <select> should also be considered descendants
        if let NodeRef::RegularElement(e) = node {
            if e.name == "selectedcontent" {
                let path = self.t.path(node);
                if let Some(select) = path.iter().rev().find(|n| matches!(n, NodeRef::RegularElement(e) if e.name == "select")) {
                    self.walk_options(*select, false, adjacent_only, &mut seen, &mut descendants);
                }
            }
        }
        descendants
    }

    fn walk_options(&self, n: NodeRef<'a>, inside_option: bool, adjacent_only: bool, seen: &mut HashSet<NodeKey>, out: &mut Vec<NodeRef<'a>>) {
        if matches!(n, NodeRef::RegularElement(e) if e.name == "option") {
            for c in template_children(n) {
                self.walk_options(c, true, adjacent_only, seen, out);
            }
        } else if inside_option {
            self.walk_children(n, adjacent_only, seen, out);
        } else {
            for c in template_children(n) {
                self.walk_options(c, false, adjacent_only, seen, out);
            }
        }
    }

    fn walk_children(&self, n: NodeRef<'a>, adjacent_only: bool, seen: &mut HashSet<NodeKey>, out: &mut Vec<NodeRef<'a>>) {
        if is_element(n) {
            out.push(n);
            if !adjacent_only {
                for c in template_children(n) {
                    self.walk_children(c, adjacent_only, seen, out);
                }
            }
        } else if let NodeRef::RenderTag(_) = n {
            for snippet in self.t.snippets(n) {
                if !seen.insert(snippet.key()) {
                    continue;
                }
                if let NodeRef::SnippetBlock(s) = snippet {
                    self.walk_children(NodeRef::Fragment(&s.body), adjacent_only, seen, out);
                }
            }
        } else {
            for c in template_children(n) {
                self.walk_children(c, adjacent_only, seen, out);
            }
        }
    }

    fn get_possible_element_siblings(&self, node: NodeRef<'a>, dir: Dir, adjacent_only: bool, seen: &mut HashSet<NodeKey>) -> NodeMap<'a> {
        let mut result = NodeMap::default();
        let path = self.t.path(node);
        let mut current = node;
        let mut i = path.len() as isize;

        loop {
            // while (i--)
            if i == 0 {
                break;
            }
            i -= 1;
            // const fragment = path[i--]
            let NodeRef::Fragment(fragment) = path[i as usize] else { break };
            i -= 1;
            let idx = fragment.nodes.iter().position(|n| fragment_node_ref(n).key() == current.key()).map_or(-1, |p| p as isize);
            let mut j = idx + if dir == Dir::Forward { 1 } else { -1 };

            while j >= 0 && (j as usize) < fragment.nodes.len() {
                let n = fragment_node_ref(&fragment.nodes[j as usize]);
                if let NodeRef::RegularElement(e) = n {
                    let has_slot_attribute =
                        e.attributes.iter().any(|a| matches!(a, AttributeLike::Attribute(a) if a.name.to_lowercase() == "slot"));
                    if !has_slot_attribute {
                        result.set(n, NODE_DEFINITELY_EXISTS);
                        if adjacent_only {
                            return result;
                        }
                    }
                    // Special case: slots, render tags and svelte:element tags could resolve to no siblings,
                    // so we want to continue until we find a definite sibling even with the adjacent-only combinator
                } else if is_block(n) || matches!(n, NodeRef::Component(_)) {
                    if matches!(n, NodeRef::SlotElement(_) | NodeRef::Component(_)) {
                        result.set(n, NODE_PROBABLY_EXISTS);
                    }
                    let possible_last_child = self.get_possible_nested_siblings(n, dir, adjacent_only, &mut HashSet::new());
                    add_to_map(&possible_last_child, &mut result);
                    if adjacent_only && !matches!(n, NodeRef::Component(_)) && has_definite_elements(&possible_last_child) {
                        return result;
                    }
                } else if let NodeRef::SvelteElement(_) = n {
                    result.set(n, NODE_PROBABLY_EXISTS);
                } else if let NodeRef::RenderTag(_) = n {
                    result.set(n, NODE_PROBABLY_EXISTS);
                    for snippet in self.t.snippets(n) {
                        add_to_map(&self.get_possible_nested_siblings(*snippet, dir, adjacent_only, &mut HashSet::new()), &mut result);
                    }
                }
                j += if dir == Dir::Forward { 1 } else { -1 };
            }

            if i < 0 {
                break;
            }
            current = path[i as usize];

            if matches!(current, NodeRef::Component(_) | NodeRef::SvelteComponent(_) | NodeRef::SvelteSelf(_)) {
                continue;
            }

            if let NodeRef::SnippetBlock(_) = current {
                if !seen.insert(current.key()) {
                    break;
                }
                let sites = self.t.sites(current);
                for site in sites {
                    let siblings = self.get_possible_element_siblings(*site, dir, adjacent_only, seen);
                    add_to_map(&siblings, &mut result);
                    if adjacent_only && sites.len() == 1 && has_definite_elements(&siblings) {
                        return result;
                    }
                }
            }

            if !is_block(current) {
                break;
            }

            if let NodeRef::EachBlock(b) = current {
                if std::ptr::eq(fragment, &b.body) {
                    // `{#each ...}<a /><b />{/each}` — `<b>` can be previous sibling of `<a />`
                    add_to_map(&self.get_possible_nested_siblings(current, dir, adjacent_only, &mut HashSet::new()), &mut result);
                }
            }
        }
        result
    }

    fn get_possible_nested_siblings(&self, node: NodeRef<'a>, dir: Dir, adjacent_only: bool, seen: &mut HashSet<NodeKey>) -> NodeMap<'a> {
        let mut fragments: Vec<Option<&'a crate::svelte::ast::Fragment>> = Vec::new();
        match node {
            NodeRef::EachBlock(b) => fragments.extend([Some(&b.body), b.fallback.as_ref()]),
            NodeRef::IfBlock(b) => fragments.extend([Some(&b.consequent), b.alternate.as_ref()]),
            NodeRef::AwaitBlock(b) => fragments.extend([b.pending.as_ref(), b.then.as_ref(), b.catch.as_ref()]),
            NodeRef::KeyBlock(b) => fragments.push(Some(&b.fragment)),
            NodeRef::SlotElement(e) => fragments.push(Some(&e.fragment)),
            NodeRef::SnippetBlock(b) => {
                if !seen.insert(node.key()) {
                    return NodeMap::default();
                }
                fragments.push(Some(&b.body));
            }
            NodeRef::Component(e) => {
                fragments.push(Some(&e.fragment));
                for s in self.t.snippets(node) {
                    if let NodeRef::SnippetBlock(s) = s {
                        fragments.push(Some(&s.body));
                    }
                }
            }
            _ => {}
        }

        let mut result = NodeMap::default();
        let mut exhaustive = !matches!(node, NodeRef::SlotElement(_) | NodeRef::SnippetBlock(_));
        for fragment in fragments {
            let Some(fragment) = fragment else {
                exhaustive = false;
                continue;
            };
            let map = self.loop_child(&fragment.nodes, dir, adjacent_only, seen);
            exhaustive &= has_definite_elements(&map);
            add_to_map(&map, &mut result);
        }
        if !exhaustive {
            for v in result.values.values_mut() {
                *v = NODE_PROBABLY_EXISTS;
            }
        }
        result
    }

    fn loop_child(&self, children: &'a [FragmentNode], dir: Dir, adjacent_only: bool, seen: &mut HashSet<NodeKey>) -> NodeMap<'a> {
        let mut result = NodeMap::default();
        let n = children.len() as isize;
        let mut i: isize = if dir == Dir::Forward { 0 } else { n - 1 };
        while i >= 0 && i < n {
            let child = fragment_node_ref(&children[i as usize]);
            match child {
                NodeRef::RegularElement(_) => {
                    result.set(child, NODE_DEFINITELY_EXISTS);
                    if adjacent_only {
                        break;
                    }
                }
                NodeRef::SvelteElement(_) => result.set(child, NODE_PROBABLY_EXISTS),
                NodeRef::RenderTag(_) => {
                    for snippet in self.t.snippets(child) {
                        add_to_map(&self.get_possible_nested_siblings(*snippet, dir, adjacent_only, seen), &mut result);
                    }
                }
                _ if is_block(child) => {
                    let child_result = self.get_possible_nested_siblings(child, dir, adjacent_only, seen);
                    add_to_map(&child_result, &mut result);
                    if adjacent_only && has_definite_elements(&child_result) {
                        break;
                    }
                }
                _ => {}
            }
            i += if dir == Dir::Forward { 1 } else { -1 };
        }
        result
    }
}

fn has_nesting(s: &SimpleSelector) -> bool {
    s.kind == SimpleKind::NestingSelector
        || s.args.as_ref().is_some_and(|a| a.children.iter().any(|c| c.children.iter().any(|r| r.selectors.iter().any(has_nesting))))
}

/// The first `<selectedcontent>` inside `select` (preorder).
fn find_selectedcontent(n: NodeRef) -> Option<NodeRef> {
    if matches!(n, NodeRef::RegularElement(e) if e.name == "selectedcontent") {
        return Some(n);
    }
    template_children(n).into_iter().find_map(find_selectedcontent)
}

/// `name.replace(/\\(.)/g, '$1')`
fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.peek() {
                Some(&n) if !matches!(n, '\n' | '\r' | '\u{2028}' | '\u{2029}') => {
                    out.push(n);
                    it.next();
                }
                _ => out.push(c),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn unquote(s: &str) -> String {
    let first = s.chars().next();
    let last = s.chars().last();
    if (first.is_some() && first == last && first == Some('\'')) || first == Some('"') {
        let inner: Vec<char> = s.chars().collect();
        if inner.len() < 2 {
            return String::new();
        }
        return inner[1..inner.len() - 1].iter().collect();
    }
    s.to_string()
}

fn test_attribute(operator: Option<&str>, expected: &str, case_insensitive: bool, value: &str) -> bool {
    let (expected, value) = if case_insensitive { (expected.to_lowercase(), value.to_lowercase()) } else { (expected.to_string(), value.to_string()) };
    match operator {
        Some("=") => value == expected,
        Some("~=") => value.split(is_ws).any(|p| p == expected),
        Some("|=") => format!("{value}-").starts_with(&format!("{expected}-")),
        Some("^=") => value.starts_with(&expected),
        Some("$=") => value.ends_with(&expected),
        Some("*=") => value.contains(&expected),
        // upstream throws "this shouldn't happen"
        _ => false,
    }
}

/// A value of `gather_possible_values`' set (`UNKNOWN`, strings, and the falsy non-strings).
#[derive(Clone, PartialEq)]
enum PV {
    Unknown,
    S(String),
    False,
    NaN,
    Zero,
}

impl PV {
    fn falsy(&self) -> bool {
        matches!(self, PV::False | PV::NaN | PV::Zero) || matches!(self, PV::S(s) if s.is_empty())
    }
    fn to_js_string(&self) -> String {
        match self {
            PV::S(s) => s.clone(),
            PV::False => "false".into(),
            PV::NaN => "NaN".into(),
            PV::Zero => "0".into(),
            PV::Unknown => "[object Object]".into(),
        }
    }
}

#[derive(Default)]
struct PVSet(Vec<PV>);

impl PVSet {
    fn add(&mut self, v: PV) {
        if !self.0.contains(&v) {
            self.0.push(v);
        }
    }
    fn has_unknown(&self) -> bool {
        self.0.contains(&PV::Unknown)
    }
}

fn literal_string(l: &crate::estree::Literal) -> String {
    match &l.value {
        LiteralValue::String(s) => s.clone(),
        LiteralValue::Number(n) => number_to_string(*n),
        LiteralValue::Boolean(b) => b.to_string(),
        LiteralValue::Null => "null".into(),
        LiteralValue::RegExp => l.raw.clone(),
        LiteralValue::BigInt => l.bigint.clone().unwrap_or_default(),
    }
}

fn gather_possible_values(node: &Expression, is_class: bool, set: &mut PVSet, is_nested: bool) {
    if set.has_unknown() {
        // no point traversing any further
        return;
    }
    match node {
        Expression::Literal(l) => set.add(PV::S(literal_string(l))),
        Expression::ConditionalExpression(c) => {
            gather_possible_values(&c.consequent, is_class, set, is_nested);
            gather_possible_values(&c.alternate, is_class, set, is_nested);
        }
        Expression::LogicalExpression(l) => {
            if l.operator == LogicalOperator::And {
                // && is a special case, because the only way the left
                // hand value can be included is if it's falsy.
                let mut left = PVSet::default();
                gather_possible_values(&l.left, is_class, &mut left, is_nested);
                if left.has_unknown() {
                    // add all non-nullish falsy values, unless this is a `class` attribute that
                    // will be processed by cslx, in which case falsy values are removed, unless
                    // they're not inside an array/object (TODO 6.0 remove that last part)
                    if !is_class || !is_nested {
                        set.add(PV::S(String::new()));
                        set.add(PV::False);
                        set.add(PV::NaN);
                        set.add(PV::Zero);
                    }
                } else {
                    for v in left.0 {
                        if v.falsy() && (!is_class || !is_nested) {
                            set.add(v);
                        }
                    }
                }
                gather_possible_values(&l.right, is_class, set, is_nested);
            } else {
                gather_possible_values(&l.left, is_class, set, is_nested);
                gather_possible_values(&l.right, is_class, set, is_nested);
            }
        }
        Expression::ArrayExpression(a) if is_class => {
            for entry in a.elements.iter().flatten() {
                match entry {
                    ArrayElement::Expression(e) => gather_possible_values(e, is_class, set, true),
                    ArrayElement::Spread(_) => {
                        if !set.has_unknown() {
                            set.add(PV::Unknown);
                        }
                    }
                }
            }
        }
        Expression::ObjectExpression(o) if is_class => {
            for p in &o.properties {
                match p {
                    ObjectMember::Property(p) if !p.computed => match &p.key {
                        PropertyKey::Expression(k) => match &**k {
                            Expression::Identifier(i) => set.add(PV::S(i.name.clone())),
                            Expression::Literal(l) => set.add(PV::S(literal_string(l))),
                            _ => set.add(PV::Unknown),
                        },
                        _ => set.add(PV::Unknown),
                    },
                    _ => set.add(PV::Unknown),
                }
            }
        }
        _ => set.add(PV::Unknown),
    }
}

enum Chunk<'a> {
    Text(&'a str),
    Expr(&'a Expression),
}

fn get_possible_values(chunk: &Chunk, is_class: bool) -> Option<Vec<String>> {
    let mut values = PVSet::default();
    match chunk {
        Chunk::Text(t) => values.add(PV::S(t.to_string())),
        Chunk::Expr(e) => gather_possible_values(e, is_class, &mut values, false),
    }
    if values.has_unknown() {
        return None;
    }
    Some(values.0.iter().map(PV::to_js_string).collect())
}

fn starts_ws(s: &str) -> bool {
    s.chars().next().is_some_and(is_ws)
}

fn ends_ws(s: &str) -> bool {
    s.chars().last().is_some_and(is_ws)
}

/// An insertion-ordered string set.
#[derive(Default)]
struct StrSet(Vec<String>);

impl StrSet {
    fn add(&mut self, s: String) {
        if !self.0.contains(&s) {
            self.0.push(s);
        }
    }
}

fn attribute_matches(node: NodeRef, name: &str, expected_value: Option<&str>, operator: Option<&str>, case_insensitive: bool) -> bool {
    let name_lower = name.to_lowercase();
    let (_, attributes) = element_parts(node).unwrap();

    for attribute in attributes {
        match attribute {
            AttributeLike::SpreadAttribute(_) => return true,
            AttributeLike::BindDirective(d) if d.name == name => return true,
            // match attributes against the corresponding directive but bail out on exact matching
            AttributeLike::StyleDirective(_) if name_lower == "style" => return true,
            AttributeLike::ClassDirective(d) if name_lower == "class" => {
                if operator == Some("~=") {
                    if Some(d.name.as_str()) == expected_value {
                        return true;
                    }
                } else {
                    return true;
                }
            }
            _ => {}
        }

        let AttributeLike::Attribute(attribute) = attribute else { continue };
        if attribute.name.to_lowercase() != name_lower {
            continue;
        }
        if let AttributeValue::True(_) = attribute.value {
            return operator.is_none();
        }
        let Some(expected_value) = expected_value else { return true };

        let chunks: Vec<Chunk> = match &attribute.value {
            AttributeValue::True(_) => Vec::new(),
            AttributeValue::Expression(ExpressionTagNode::ExpressionTag(t)) => vec![Chunk::Expr(&t.expression)],
            AttributeValue::Sequence(s) => s
                .iter()
                .map(|c| match c {
                    TextOrExpressionTag::Text(t) => Chunk::Text(&t.data),
                    TextOrExpressionTag::ExpressionTag(t) => Chunk::Expr(&t.expression),
                })
                .collect(),
        };

        // is_text_attribute
        if let [Chunk::Text(data)] = chunks.as_slice() {
            let matches = test_attribute(operator, expected_value, case_insensitive, data);
            // continue if we still may match against a class/style directive
            if !matches && (name_lower == "class" || name_lower == "style") {
                continue;
            }
            return matches;
        }

        let mut possible_values = StrSet::default();
        let mut prev_values: Vec<String> = Vec::new();
        for chunk in &chunks {
            let Some(current_possible_values) = get_possible_values(chunk, name_lower == "class") else {
                // impossible to find out all combinations
                return true;
            };

            if !prev_values.is_empty() {
                let mut start_with_space = Vec::new();
                let mut remaining = Vec::new();
                for v in &current_possible_values {
                    if starts_ws(v) {
                        start_with_space.push(v.clone());
                    } else {
                        remaining.push(v.clone());
                    }
                }
                if !remaining.is_empty() {
                    if !start_with_space.is_empty() {
                        for p in &prev_values {
                            possible_values.add(p.clone());
                        }
                    }
                    let mut combined = Vec::new();
                    for p in &prev_values {
                        for v in &remaining {
                            combined.push(format!("{p}{v}"));
                        }
                    }
                    prev_values = combined;
                    for v in start_with_space {
                        if ends_ws(&v) {
                            possible_values.add(v);
                        } else {
                            prev_values.push(v);
                        }
                    }
                    continue;
                } else {
                    for p in &prev_values {
                        possible_values.add(p.clone());
                    }
                    prev_values = Vec::new();
                }
            }
            for v in &current_possible_values {
                if ends_ws(v) {
                    possible_values.add(v.clone());
                } else {
                    prev_values.push(v.clone());
                }
            }
            if prev_values.len() < current_possible_values.len() {
                prev_values.push(" ".into());
            }
            if prev_values.len() > 20 {
                // might grow exponentially, bail out
                return true;
            }
        }
        for p in prev_values {
            possible_values.add(p);
        }

        for v in &possible_values.0 {
            if test_attribute(operator, expected_value, case_insensitive, v) {
                return true;
            }
        }
    }
    false
}
