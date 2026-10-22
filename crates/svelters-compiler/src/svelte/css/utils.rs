//! Port of `phases/2-analyze/css/utils.js` and `phases/css.js` (Svelte 5.57.2).
//!
//! `get_possible_values` lives in `prune.rs` (it reads template expressions).

use super::ast::*;
use super::Meta;

/// `remove_css_prefix`: strips `/^-((webkit)|(moz)|(o)|(ms))-/`.
pub fn remove_css_prefix(name: &str) -> &str {
    for p in ["-webkit-", "-moz-", "-o-", "-ms-"] {
        if let Some(rest) = name.strip_prefix(p) {
            return rest;
        }
    }
    name
}

pub fn is_keyframes_node(node: &Atrule) -> bool {
    remove_css_prefix(&node.name) == "keyframes"
}

/// `is_global(relative_selector)`: true if is `:global(...)` or `:global` and no pseudo class
/// that is scoped. Takes the relative selector's `selectors`.
pub fn is_global(selectors: &[&SimpleSelector]) -> bool {
    let Some(first) = selectors.first() else { return false };
    first.is_pseudo_class("global")
        && (first.args.is_none()
            // Only these two selector types keep the whole selector global, because e.g.
            // :global(button).x means that the selector is still scoped because of the .x
            || selectors.iter().all(|s| is_unscoped_pseudo_class(s) || s.kind == SimpleKind::PseudoElementSelector))
}

pub fn rel_selectors(r: &RelativeSelector) -> Vec<&SimpleSelector> {
    r.selectors.iter().collect()
}

pub fn is_global_rel(r: &RelativeSelector) -> bool {
    is_global(&rel_selectors(r))
}

/// `true` if is a pseudo class that cannot be or is not scoped
pub fn is_unscoped_pseudo_class(selector: &SimpleSelector) -> bool {
    if selector.kind != SimpleKind::PseudoClassSelector {
        return false;
    }
    let n = selector.name.as_str();
    // These make the selector scoped
    (n != "has"
        && n != "is"
        && n != "where"
        // Not is special because we want to scope as specific as possible, but because :not
        // inverses the result, we want to leave the unscoped, too. The exception is more than
        // one selector in the :not (.e.g :not(.x .y)), then .x and .y should be scoped
        && (n != "not" || selector.args.as_ref().is_none_or(|a| a.children.iter().all(|c| c.children.len() == 1))))
        // selectors with has/is/where/not can also be global if all their children are global
        || selector.args.as_ref().is_none_or(|a| a.children.iter().all(|c| c.children.iter().all(is_global_rel)))
}

/// `is_outer_global`: `:global(x):has(y)` is `true` here but `false` for [`is_global`].
pub fn is_outer_global(selectors: &[&SimpleSelector]) -> bool {
    let Some(first) = selectors.first() else { return false };
    first.is_pseudo_class("global") && (first.args.is_none() || selectors.iter().all(|s| s.is_pseudo()))
}

/// `get_parent_rules(rule)`: the rule and its ancestors; root is last.
pub fn get_parent_rules(meta: &Meta, rule: Option<usize>) -> Vec<usize> {
    let mut rules = Vec::new();
    let mut r = rule;
    while let Some(id) = r {
        rules.push(id);
        r = meta.rules[id].parent_rule.get();
    }
    rules
}
