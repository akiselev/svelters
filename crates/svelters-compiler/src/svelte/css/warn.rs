//! Port of `phases/2-analyze/css/css-warn.js` (Svelte 5.57.2): `css_unused_selector`.

use super::ast::*;
use super::utils::is_keyframes_node;
use super::Css;
use crate::svelte::errors::CompileWarning;

pub fn warn_unused(css: &Css, source: &str, out: &mut Vec<CompileWarning>) {
    let mut w = Warn { css, source, out };
    for n in &css.sheet.children {
        w.node(n);
    }
}

struct Warn<'c, 's, 'o> {
    css: &'c Css<'s>,
    source: &'c str,
    out: &'o mut Vec<CompileWarning>,
}

impl Warn<'_, '_, '_> {
    fn node(&mut self, n: &Node) {
        match n {
            Node::Atrule(a) => {
                if !is_keyframes_node(a) {
                    if let Some(b) = &a.block {
                        for c in &b.children {
                            self.node(c);
                        }
                    }
                }
            }
            Node::Rule(r) => {
                for c in &r.prelude.children {
                    self.complex(c, None);
                }
                if !self.css.meta.rules[r.id].is_global_block.get() {
                    for c in &r.block.children {
                        self.node(c);
                    }
                }
            }
            Node::Declaration(_) => {}
        }
    }

    /// `outer`: the complex selector whose `:is(...)` / `:where(...)` holds `node`.
    fn complex(&mut self, node: &ComplexSelector, outer: Option<&ComplexSelector>) {
        let meta = &self.css.meta;
        // prevent double-marking of `.unused:is(.unused)`
        if !meta.complex[node.id].used.get() && outer.is_none_or(|o| meta.complex[o.id].used.get()) {
            let text = &self.source[node.start..node.end];
            self.out.push(CompileWarning {
                code: "css_unused_selector",
                message: format!("Unused CSS selector \"{text}\""),
                start: node.start as u32,
                end: node.end as u32,
            });
        }
        for r in &node.children {
            for s in &r.selectors {
                if s.kind == SimpleKind::PseudoClassSelector && (s.name == "is" || s.name == "where") {
                    if let Some(args) = &s.args {
                        for c in &args.children {
                            self.complex(c, Some(node));
                        }
                    }
                }
            }
        }
    }
}
