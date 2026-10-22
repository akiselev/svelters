//! Port of `phases/2-analyze/css/css-analyze.js` (Svelte 5.57.2): selector metadata and the
//! `:global` validation errors.
//!
//! The zimmerframe walk becomes explicit recursion: `rules` is the `Rule` part of
//! `context.path` (the only part the visitors read besides "is this selector list the argument
//! of a pseudo-class", passed as `in_pseudo`).

use super::ast::*;
use super::errors as e;
use super::utils::{is_global_rel, is_keyframes_node, is_unscoped_pseudo_class};
use super::Css;
use crate::svelte::errors::CompileError;

type R = Result<(), CompileError>;

/// `analyze_css(stylesheet, analysis)`
pub fn analyze_css(css: &mut Css) -> R {
    let sheet = css.sheet;
    let mut a = Analyzer { css, rules: Vec::new() };
    for n in &sheet.children {
        a.node(n, None)?;
    }
    Ok(())
}

struct Analyzer<'c, 's> {
    css: &'c mut Css<'s>,
    /// the `Rule` ancestors of the current node
    rules: Vec<usize>,
}

/// True if is `:global`
fn is_global_block_selector(s: &SimpleSelector) -> bool {
    s.is_pseudo_class("global") && s.args.is_none()
}

/// Marks the complex selectors directly inside `s` (`walk(s, null, { ComplexSelector(node) { used = true } })`).
fn mark_outer_complex(css: &Css, s: &SimpleSelector) {
    if let Some(args) = &s.args {
        for c in &args.children {
            css.meta.complex[c.id].used.set(true);
        }
    }
}

/// Marks every complex selector inside `s`, at any depth.
pub(super) fn mark_all_complex_in_simple(css: &Css, s: &SimpleSelector) {
    if let Some(args) = &s.args {
        for c in &args.children {
            mark_all_complex(css, c);
        }
    }
}

pub(super) fn mark_all_complex(css: &Css, c: &ComplexSelector) {
    css.meta.complex[c.id].used.set(true);
    for r in &c.children {
        for s in &r.selectors {
            mark_all_complex_in_simple(css, s);
        }
    }
}

impl<'c, 's> Analyzer<'c, 's> {
    fn is_unscoped(&self) -> bool {
        self.rules.iter().all(|r| self.css.meta.rules[*r].has_global_selectors.get())
    }

    fn is_in_global_block(&self) -> bool {
        self.rules.iter().any(|r| self.css.meta.rules[*r].is_global_block.get())
    }

    fn node(&mut self, n: &'s Node, rule: Option<usize>) -> R {
        match n {
            Node::Rule(r) => self.rule(r, rule),
            Node::Atrule(a) => {
                if is_keyframes_node(a) {
                    if !a.prelude.starts_with("-global-") && !self.is_in_global_block() {
                        self.css.keyframes.push(a.prelude.clone());
                    } else if a.prelude.starts_with("-global-") {
                        // we don't check if the block.children.length because the keyframe is still added even if empty
                        let u = self.is_unscoped();
                        self.css.has_global |= u;
                    }
                }
                if let Some(b) = &a.block {
                    for c in &b.children {
                        self.node(c, rule)?;
                    }
                }
                Ok(())
            }
            Node::Declaration(_) => Ok(()),
        }
    }

    fn rule(&mut self, node: &'s Rule, state_rule: Option<usize>) -> R {
        let meta = &self.css.meta;
        meta.rules[node.id].parent_rule.set(state_rule);
        let parent_rule = state_rule;

        // We gotta allow :global x, :global y because CSS preprocessors might generate that from :global { x, y {...} }
        for complex in &node.prelude.children {
            let mut is_global_block = false;
            for (selector_idx, child) in complex.children.iter().enumerate() {
                let idx = child.selectors.iter().position(is_global_block_selector);
                if is_global_block {
                    // All selectors after :global are unscoped
                    meta.rel[child.id].is_global_like.set(true);
                }
                if idx == Some(0) {
                    if child.selectors.len() > 1 && selector_idx == 0 && parent_rule.is_none() {
                        let s = &child.selectors[1];
                        return Err(e::css_global_block_invalid_modifier_start((s.start, s.end)));
                    }
                    // `child` starts with `:global`
                    meta.rules[node.id].is_global_block.set(true);
                    is_global_block = true;
                    for s in &child.selectors[1..] {
                        mark_outer_complex(self.css, s);
                    }
                    if let Some(c) = &child.combinator {
                        if c.name != " " {
                            return Err(e::css_global_block_invalid_combinator((child.start, child.end), &c.name));
                        }
                    }
                    let declaration = node.block.children.iter().find_map(|c| match c {
                        Node::Declaration(d) => Some(d),
                        _ => None,
                    });
                    // just `:global`, not e.g. `:global x`
                    let is_lone_global = complex.children.len() == 1 && complex.children[0].selectors.len() == 1;
                    if is_lone_global && node.prelude.children.len() > 1 {
                        // `:global, :global x { z { ... } }` would become `x { z { ... } }` which means `z` is always
                        // constrained by `x`, which is not what the user intended
                        return Err(e::css_global_block_invalid_list((node.prelude.start, node.prelude.end)));
                    }
                    if let Some(d) = declaration {
                        // :global { color: red; } is invalid, but foo :global { color: red; } is valid
                        if node.prelude.children.len() == 1 && is_lone_global {
                            return Err(e::css_global_block_invalid_declaration((d.start, d.end)));
                        }
                    }
                } else if let Some(i) = idx {
                    let s = &child.selectors[i];
                    return Err(e::css_global_block_invalid_modifier((s.start, s.end)));
                }
            }
            if meta.rules[node.id].is_global_block.get() && !is_global_block {
                return Err(e::css_global_block_invalid_list((node.prelude.start, node.prelude.end)));
            }
        }

        // visit selector list first, to populate child selector metadata
        self.rules.push(node.id);
        for c in &node.prelude.children {
            self.complex(c, Some(node.id), false)?;
        }
        self.rules.pop();

        let meta = &self.css.meta;
        for s in &node.prelude.children {
            let g = meta.complex[s.id].is_global.get();
            let rm = &meta.rules[node.id];
            rm.has_global_selectors.set(rm.has_global_selectors.get() || g);
            rm.has_local_selectors.set(rm.has_local_selectors.get() || !g);
        }

        // if this rule has a ComplexSelector whose RelativeSelector children are all
        // `:global(...)`, and the rule contains declarations (rather than just
        // nested rules) then the component as a whole includes global CSS
        let has_decl = node.block.children.iter().any(|c| matches!(c, Node::Declaration(_)));
        if meta.rules[node.id].has_global_selectors.get() && has_decl && self.is_unscoped() {
            self.css.has_global = true;
        }

        // visit block list, so parent rule metadata is populated
        self.rules.push(node.id);
        for c in &node.block.children {
            self.node(c, Some(node.id))?;
        }
        self.rules.pop();
        Ok(())
    }

    /// `in_pseudo`: the selector list holding `node` is the argument of a pseudo-class
    /// (`context.path.at(-2)?.type === 'PseudoClassSelector'`).
    fn complex(&mut self, node: &'s ComplexSelector, rule: Option<usize>, in_pseudo: bool) -> R {
        // analyse relevant selectors first
        for (i, r) in node.children.iter().enumerate() {
            self.relative(r, i == 0, rule, in_pseudo)?;
        }

        let css = &*self.css;
        let meta = &css.meta;
        if let Some(gi) = node.children.iter().position(is_global_rel) {
            let global = &node.children[gi];
            let g0 = &global.selectors[0];
            if in_pseudo && g0.args.is_none() {
                return Err(e::css_global_block_invalid_placement((g0.start, g0.end)));
            }
            if g0.args.is_some() && gi != 0 && gi != node.children.len() - 1 {
                // ensure `:global(...)` is not used in the middle of a selector (but multiple `global(...)` in sequence are ok)
                for c in &node.children[gi + 1..] {
                    if !is_global_rel(c) {
                        return Err(e::css_global_invalid_placement((g0.start, g0.end)));
                    }
                }
            }
        }

        // ensure `:global(...)` do not lead to invalid css after `:global()` is removed
        for relative in &node.children {
            for (i, selector) in relative.selectors.iter().enumerate() {
                if selector.is_pseudo_class("global") {
                    let child = selector.args.as_ref().and_then(|a| a.children[0].children.first());
                    // ensure `:global(element)` to be at the first position in a compound selector
                    if let Some(child) = child {
                        if child.selectors.first().is_some_and(|s| s.kind == SimpleKind::TypeSelector) && i != 0 {
                            return Err(e::css_global_invalid_selector_list((selector.start, selector.end)));
                        }
                    }
                    // ensure `:global(.class)` is not followed by a type selector, eg: `:global(.class)element`
                    if let Some(next) = relative.selectors.get(i + 1) {
                        if next.kind == SimpleKind::TypeSelector {
                            return Err(e::css_type_selector_invalid_placement((next.start, next.end)));
                        }
                    }
                    // ensure `:global(...)`contains a single selector
                    // (standalone :global() with multiple selectors is OK)
                    if let Some(args) = &selector.args {
                        if args.children.len() > 1 && (node.children.len() > 1 || relative.selectors.len() > 1) {
                            return Err(e::css_global_invalid_selector((selector.start, selector.end)));
                        }
                    }
                }
            }
        }

        let cm = &meta.complex[node.id];
        cm.rule.set(rule);
        let is_global = node.children.iter().all(|r| meta.rel[r.id].is_global.get() || meta.rel[r.id].is_global_like.get());
        cm.is_global.set(is_global);
        cm.used.set(cm.used.get() || is_global);

        if let Some(rule_id) = rule {
            if let Some(parent) = meta.rules[rule_id].parent_rule.get() {
                if node.children.first().and_then(|r| r.selectors.first()).is_some_and(|s| s.kind == SimpleKind::NestingSelector) {
                    let first = node.children[0].selectors.get(1);
                    let no_nesting_scope = first.is_none_or(|f| f.kind != SimpleKind::PseudoClassSelector || is_unscoped_pseudo_class(f));
                    let parent_is_global = css.rules[parent]
                        .prelude
                        .children
                        .iter()
                        .any(|c| c.children.len() == 1 && meta.rel[c.children[0].id].is_global.get());
                    // mark `&:hover` in `:global(.foo) { &:hover { color: green }}` as used
                    if no_nesting_scope && parent_is_global {
                        cm.used.set(true);
                    }
                }
            }
        }
        Ok(())
    }

    fn relative(&mut self, node: &'s RelativeSelector, is_first: bool, rule: Option<usize>, in_pseudo: bool) -> R {
        let css = &*self.css;
        let meta = &css.meta;
        if let Some(c) = &node.combinator {
            let has_parent_rule = rule.is_some_and(|r| meta.rules[r].parent_rule.get().is_some());
            if !has_parent_rule && is_first && !in_pseudo {
                return Err(e::css_selector_invalid((c.start, c.end)));
            }
        }

        let rm = &meta.rel[node.id];
        rm.is_global.set(!node.selectors.is_empty() && is_global_rel(node));

        if !node.selectors.is_empty() && node.selectors.iter().all(|s| s.is_pseudo()) {
            let first = &node.selectors[0];
            let like = first.is_pseudo_class("host")
                || (first.kind == SimpleKind::PseudoElementSelector
                    && matches!(
                        first.name.as_str(),
                        "view-transition" | "view-transition-group" | "view-transition-old" | "view-transition-new" | "view-transition-image-pair"
                    ));
            rm.is_global_like.set(rm.is_global_like.get() || like);
        }

        let root_like = node.selectors.iter().any(|s| s.is_pseudo_class("root"))
            // :root.y:has(.x) is not a global selector because while .y is unscoped, .x inside `:has(...)` should be scoped
            && !node.selectors.iter().any(|s| s.is_pseudo_class("has"));
        rm.is_global_like.set(rm.is_global_like.get() || root_like);

        if rm.is_global_like.get() || rm.is_global.get() {
            // So that nested selectors like `:root:not(.x)` are not marked as unused
            for child in &node.selectors {
                mark_all_complex_in_simple(css, child);
            }
        }

        // context.next(): the simple selectors
        for s in &node.selectors {
            match s.kind {
                SimpleKind::NestingSelector => self.nesting(s, rule)?,
                SimpleKind::PseudoElementSelector => {} // Don't analyse these
                _ => {
                    if let Some(args) = &s.args {
                        for c in &args.children {
                            self.complex(c, rule, s.kind == SimpleKind::PseudoClassSelector)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn nesting(&mut self, node: &SimpleSelector, rule: Option<usize>) -> R {
        let css = &*self.css;
        let meta = &css.meta;
        // a `&` outside any rule cannot happen (selectors only exist in rules)
        let Some(rule_id) = rule else { return Ok(()) };
        let rule = css.rules[rule_id];
        match meta.rules[rule_id].parent_rule.get() {
            None => {
                // https://developer.mozilla.org/en-US/docs/Web/CSS/Nesting_selector#using_outside_nested_rule
                let children = &rule.prelude.children;
                let selectors = &children[0].children[0].selectors;
                let ok = children.len() == 1
                    && selectors.len() == 1
                    && selectors[0].is_pseudo_class("global")
                    && selectors[0]
                        .args
                        .as_ref()
                        .and_then(|a| a.children.first())
                        .and_then(|c| c.children.first())
                        .and_then(|r| r.selectors.first())
                        .is_some_and(|s| std::ptr::eq(s, node));
                if !ok {
                    return Err(e::css_nesting_selector_invalid_placement((node.start, node.end)));
                }
            }
            Some(parent) => {
                let pm = &meta.rules[parent];
                let p = css.rules[parent];
                // :global { &.foo { ... } } is invalid
                if pm.is_global_block.get()
                    && pm.parent_rule.get().is_none()
                    && p.prelude.children[0].children.len() == 1
                    && p.prelude.children[0].children[0].selectors.len() == 1
                {
                    return Err(e::css_global_block_invalid_modifier_start((node.start, node.end)));
                }
            }
        }
        Ok(())
    }
}
