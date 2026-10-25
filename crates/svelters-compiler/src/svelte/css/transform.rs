//! Port of `phases/3-transform/css/index.js` (Svelte 5.57.2): `render_stylesheet`, the scoped
//! stylesheet text (the `css.code` of `compile`).
//!
//! SVELTERS: no source map, and no `sourceMappingURL` comment (upstream appends one only for
//! `dev` with injected styles). zimmerframe's `path` becomes explicit parameters: the `Rule`
//! ancestors (`rules`), whether a selector list belongs to a rule, and whether an enclosing
//! complex selector is unused.

use super::ast::*;
use super::magic_string::MagicString;
use super::utils::{is_keyframes_node, remove_css_prefix};
use super::Css;
use crate::svelte::parse::utils::whitespace::is_whitespace;
use std::cell::Cell;
use std::rc::Rc;

fn is_ws(c: char) -> bool {
    is_whitespace(c as u32)
}

struct State<'m, 's> {
    code: &'m mut MagicString<'s>,
    hash: &'m str,
    minify: bool,
    dev: bool,
    selector: String,
    keyframes: &'m [String],
}

/// `render_stylesheet(source, analysis, options)`: `inject` is `analysis.inject_styles`.
pub fn render_stylesheet(source: &str, css: &Css, hash: &str, inject: bool, dev: bool) -> String {
    render_stylesheet_mapped(source, css, hash, inject, dev).0
}

/// [`render_stylesheet`], with DZ0's map: the runs of the output that are untouched text of
/// `<style>`, as `(output offset, offset from the style content's start, length)` bytes.
pub fn render_stylesheet_mapped(source: &str, css: &Css, hash: &str, inject: bool, dev: bool) -> (String, Vec<(u32, u32, u32)>) {
    let mut code = MagicString::new(source);
    let sheet = css.sheet;
    {
        let mut state = State {
            code: &mut code,
            hash,
            minify: inject && !dev,
            dev,
            selector: format!(".{hash}"),
            keyframes: &css.keyframes,
        };
        let mut rules = Vec::new();
        let spec = Rc::new(Cell::new(false));
        for n in &sheet.children {
            visit_node(n, css, &mut state, &mut rules, &spec);
        }
    }
    code.remove(0, sheet.content_start);
    code.remove(sheet.content_end, source.len());
    if inject && !dev {
        remove_preceding_whitespace(sheet.content_end, &mut code);
    }
    let start = sheet.content_start;
    let map = code.segments().into_iter().filter(|&(_, s, _)| s >= start).map(|(o, s, l)| (o as u32, (s - start) as u32, l as u32)).collect();
    (code.to_string(), map)
}

fn char_before(s: &str, i: usize) -> Option<char> {
    s.get(..i).and_then(|p| p.chars().next_back())
}

/// Walk backwards until we find a non-whitespace character
fn remove_preceding_whitespace(end: usize, code: &mut MagicString) {
    let o = code.original();
    let mut start = end;
    while let Some(c) = char_before(o, start) {
        if !is_ws(c) {
            break;
        }
        start -= c.len_utf8();
    }
    if start < end {
        code.remove(start, end);
    }
}

fn is_in_global_block(css: &Css, rules: &[usize]) -> bool {
    rules.iter().any(|r| css.meta.rules[*r].is_global_block.get())
}

fn is_used(css: &Css, rule: &Rule) -> bool {
    rule.prelude.children.iter().any(|s| css.meta.complex[s.id].used.get())
}

fn is_empty(css: &Css, rule: &Rule, in_global_block: bool) -> bool {
    if css.meta.rules[rule.id].is_global_block.get() {
        return rule.block.children.is_empty();
    }
    for child in &rule.block.children {
        match child {
            Node::Declaration(_) => return false,
            Node::Rule(r) => {
                if (is_used(css, r) || in_global_block) && !is_empty(css, r, in_global_block) {
                    return false;
                }
            }
            Node::Atrule(a) => {
                if a.block.as_ref().is_none_or(|b| !b.children.is_empty()) {
                    return false;
                }
            }
        }
    }
    true
}

fn escape_comment_close(node: &Rule, code: &mut MagicString) {
    let o = code.original().as_bytes();
    let mut escaped = false;
    let mut in_comment = false;
    let mut i = node.start;
    while i < node.end {
        if escaped {
            escaped = false;
        } else {
            let c = o[i];
            if in_comment {
                if c == b'*' && o.get(i + 1) == Some(&b'/') {
                    i += 1;
                    code.prepend_right(i, "\\");
                    in_comment = false;
                }
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'/' {
                i += 1;
                if o.get(i) == Some(&b'*') {
                    in_comment = true;
                }
            }
        }
        i += 1;
    }
}

fn visit_node(n: &Node, css: &Css, state: &mut State, rules: &mut Vec<usize>, spec: &Rc<Cell<bool>>) {
    match n {
        Node::Atrule(a) => {
            if is_keyframes_node(a) {
                let o = state.code.original().as_bytes();
                let mut start = a.start + a.name.len() + 1;
                while o.get(start) == Some(&b' ') {
                    start += 1;
                }
                if a.prelude.starts_with("-global-") {
                    state.code.remove(start, start + 8);
                } else if !is_in_global_block(css, rules) {
                    let h = format!("{}-", state.hash);
                    state.code.prepend_right(start, &h);
                }
                return; // don't transform anything within
            }
            if let Some(b) = &a.block {
                for c in &b.children {
                    visit_node(c, css, state, rules, spec);
                }
            }
        }
        Node::Declaration(d) => declaration(d, state),
        Node::Rule(r) => rule(r, css, state, rules, spec),
    }
}

fn declaration(node: &Declaration, state: &mut State) {
    let lower = node.property.to_lowercase();
    let property = remove_css_prefix(&lower);
    if property == "animation" || property == "animation-name" {
        let o = state.code.original();
        let mut index = node.start + node.property.len() + 1;
        let mut name = String::new();
        let mut inserts = Vec::new();
        while index < o.len() {
            let c = o[index..].chars().next().unwrap();
            if is_ws(c) || c == ',' || c == ';' || c == '}' {
                if state.keyframes.contains(&name) {
                    inserts.push(index - name.len());
                }
                if c == ';' || c == '}' {
                    break;
                }
                name.clear();
            } else {
                name.push(c);
            }
            index += c.len_utf8();
        }
        let h = format!("{}-", state.hash);
        for i in inserts {
            state.code.prepend_right(i, &h);
        }
    } else if state.minify {
        remove_preceding_whitespace(node.start, state.code);
        // Don't minify whitespace in custom properties, since some browsers (Chromium < 99)
        // treat --foo: ; and --foo:; differently
        if !node.property.starts_with("--") {
            let o = state.code.original();
            let start = node.start + node.property.len() + 1;
            let mut end = start;
            while let Some(c) = o.get(end..).and_then(|s| s.chars().next()) {
                if !is_ws(c) {
                    break;
                }
                end += c.len_utf8();
            }
            if end > start {
                state.code.remove(start, end);
            }
        }
    }
}

fn rule(node: &Rule, css: &Css, state: &mut State, rules: &mut Vec<usize>, spec: &Rc<Cell<bool>>) {
    if state.minify {
        remove_preceding_whitespace(node.start, state.code);
        remove_preceding_whitespace(node.block.end - 1, state.code);
    }
    let in_global = is_in_global_block(css, rules);

    // keep empty rules in dev, because it's convenient to
    // see them in devtools
    if !state.dev && is_empty(css, node, in_global) {
        if state.minify {
            state.code.remove(node.start, node.end);
        } else {
            state.code.prepend_right(node.start, "/* (empty) ");
            state.code.append_left(node.end, "*/");
            escape_comment_close(node, state.code);
        }
        return;
    }

    if !is_used(css, node) && !in_global {
        if state.minify {
            state.code.remove(node.start, node.end);
        } else {
            state.code.prepend_right(node.start, "/* (unused) ");
            state.code.append_left(node.end, "*/");
            escape_comment_close(node, state.code);
        }
        return;
    }

    rules.push(node.id);
    if css.meta.rules[node.id].is_global_block.get() {
        let selector = &node.prelude.children[0];
        if node.prelude.children.len() == 1 && selector.children.len() == 1 && selector.children[0].selectors.len() == 1 {
            // `:global {...}`
            if state.minify {
                state.code.remove(node.start, node.block.start + 1);
                state.code.remove(node.block.end - 1, node.end);
            } else {
                state.code.prepend_right(node.start, "/* ");
                state.code.append_left(node.block.start + 1, "*/");
                state.code.prepend_right(node.block.end - 1, "/*");
                state.code.append_left(node.block.end, "*/");
            }
            // don't recurse into selectors but visit the body
            for c in &node.block.children {
                visit_node(c, css, state, rules, spec);
            }
            rules.pop();
            return;
        }
    }

    selector_list(&node.prelude, Some(node), css, state, rules, false, spec);
    for c in &node.block.children {
        visit_node(c, css, state, rules, spec);
    }
    rules.pop();
}

/// `rule`: the parent when it is a `Rule` (`path.at(-1)`); `in_unused`: an enclosing complex
/// selector is unused. `rules` includes the owning rule.
fn selector_list(
    node: &SelectorList,
    rule: Option<&Rule>,
    css: &Css,
    state: &mut State,
    rules: &mut Vec<usize>,
    in_unused: bool,
    spec: &Rc<Cell<bool>>,
) {
    let meta = &css.meta;
    let in_global = is_in_global_block(css, rules);
    // Only add comments if we're not inside a complex selector that itself is unused or a global block
    if (!in_global || (node.children.len() > 1 && rule.is_some_and(|r| meta.rules[r.id].is_global_block.get()))) && !in_unused {
        let children = &node.children;
        let mut pruning = false;
        let mut prune_start = children[0].start;
        let mut last = prune_start;
        let mut has_previous_used = false;

        for (i, selector) in children.iter().enumerate() {
            let used = meta.complex[selector.id].used.get();
            if used == pruning {
                if pruning {
                    let o = state.code.original().as_bytes();
                    let mut j = selector.start;
                    while o[j] != b',' {
                        j -= 1;
                    }
                    if state.minify {
                        state.code.remove(prune_start, if has_previous_used { j } else { j + 1 });
                    } else {
                        state.code.append_right(if has_previous_used { j } else { j + 1 }, "*/");
                    }
                } else if i == 0 {
                    if state.minify {
                        prune_start = selector.start;
                    } else {
                        state.code.prepend_right(selector.start, "/* (unused) ");
                    }
                } else if state.minify {
                    prune_start = last;
                } else {
                    state.code.overwrite(last, selector.start, " /* (unused) ");
                }
                pruning = !pruning;
            }
            if !pruning && used {
                has_previous_used = true;
            }
            last = selector.end;
        }

        if pruning {
            if state.minify {
                state.code.remove(prune_start, last);
            } else {
                state.code.append_left(last, "*/");
            }
        }
    }

    // if we're in a `:is(...)` or whatever, keep existing specificity bump state
    let mut specificity = spec.clone();
    // if this selector list belongs to a rule, require a specificity bump for the
    // first scoped selector but only if we're at the top level
    if let Some(rule) = rule {
        specificity = Rc::new(Cell::new(false));
        let mut r = meta.rules[rule.id].parent_rule.get();
        while let Some(id) = r {
            if meta.rules[id].has_local_selectors.get() {
                specificity = Rc::new(Cell::new(true));
                break;
            }
            r = meta.rules[id].parent_rule.get();
        }
    }

    for c in &node.children {
        complex(c, css, state, rules, in_unused, &specificity);
    }
}

fn remove_global_pseudo_class(selector: &SimpleSelector, combinator: Option<&Combinator>, code: &mut MagicString) {
    if selector.args.is_none() {
        let mut start = selector.start;
        if combinator.is_some_and(|c| c.name == " ") {
            // div :global.x becomes div.x
            let o = code.original();
            while let Some(c) = char_before(o, start) {
                if !is_ws(c) {
                    break;
                }
                start -= c.len_utf8();
            }
        }
        // update(...), not remove(...) because there could be a closing unused comment at the end
        code.update(start, selector.start + ":global".len(), "");
    } else {
        code.remove(selector.start, selector.start + ":global(".len());
        code.remove(selector.end - 1, selector.end);
    }
}

fn complex(node: &ComplexSelector, css: &Css, state: &mut State, rules: &mut Vec<usize>, in_unused: bool, spec: &Rc<Cell<bool>>) {
    let meta = &css.meta;
    let before_bumped = spec.get();

    for relative in &node.children {
        if meta.rel[relative.id].is_global.get() {
            let global = &relative.selectors[0];
            remove_global_pseudo_class(global, relative.combinator.as_ref(), state.code);

            let parent_rule = meta.complex[node.id].rule.get().and_then(|r| meta.rules[r].parent_rule.get());
            if let Some(parent_rule) = parent_rule {
                if global.args.is_none() {
                    if relative.combinator.is_none() {
                        // div { :global.x { ... } } becomes div { &.x { ... } }
                        state.code.prepend_right(global.start, "&");
                    }
                    // In case of multiple :global selectors in a selector list we gotta delete the comma, too, but only if
                    // the next selector is used; if it's unused then the comma deletion happens as part of removal of that next selector
                    let prelude = &css.rules[parent_rule].prelude;
                    let idx = node.children.iter().position(|s| std::ptr::eq(s, relative)).map_or(-1, |i| i as isize);
                    if prelude.children.len() > 1 && node.children.len() as isize == idx - 1 {
                        if let Some(next) = prelude.children.iter().find(|s| s.start > global.end) {
                            if meta.complex[next.id].used.get() {
                                state.code.update(global.end, next.start, "");
                            }
                        }
                    }
                }
            }
            continue;
        } else {
            // for any :global() or :global at the middle of compound selector
            for selector in &relative.selectors {
                if selector.is_pseudo_class("global") {
                    remove_global_pseudo_class(selector, None, state.code);
                }
            }
        }

        if meta.rel[relative.id].scoped.get() {
            if relative.selectors.len() == 1 {
                // skip standalone :is/:where/& selectors
                let s = &relative.selectors[0];
                if s.kind == SimpleKind::PseudoClassSelector && (s.name == "is" || s.name == "where") {
                    continue;
                }
            }
            if relative.selectors.iter().any(|s| s.kind == SimpleKind::NestingSelector) {
                continue;
            }

            // for the first occurrence, we use a classname selector, so that every
            // encapsulated selector gets a +0-1-0 specificity bump. thereafter,
            // we use a `:where` selector, which does not affect specificity
            let mut modifier = state.selector.clone();
            if spec.get() {
                modifier = format!(":where({modifier})");
            }
            spec.set(true);

            let mut i = relative.selectors.len();
            while i > 0 {
                i -= 1;
                let selector = &relative.selectors[i];
                if selector.is_pseudo() {
                    if selector.name != "root" && selector.name != "host" && i == 0 {
                        state.code.prepend_right(selector.start, &modifier);
                    }
                    continue;
                }
                if selector.kind == SimpleKind::TypeSelector && selector.name == "*" && selector.namespace.is_none() {
                    state.code.update(selector.start, selector.end, &modifier);
                } else {
                    state.code.append_left(selector.end, &modifier);
                }
                break;
            }
        }
    }

    // context.next(): the pseudo-class arguments
    let unused_here = in_unused || !meta.complex[node.id].used.get();
    for relative in &node.children {
        for s in &relative.selectors {
            if s.kind == SimpleKind::PseudoClassSelector && matches!(s.name.as_str(), "is" | "where" | "has" | "not") {
                if let Some(args) = &s.args {
                    selector_list(args, None, css, state, rules, unused_here, spec);
                }
            }
        }
    }

    spec.set(before_bumped);
}
