//! IR -> JavaScript module targeting `@svelters/native-runtime` (docs/notes/native-compiler/runtime-contract.md).
//!
//! Expression meaning is delegated to a [`Rewriter`] (implemented over Svelte's scope analysis in
//! `crate::transform`): it turns user expressions into span edits (`count` -> `$.get(count)`,
//! `count++` -> `$.update(count)`), answers whether an expression reads reactive state (to decide
//! between a one-off write and a render effect), and transforms the instance script.

use super::edit::Edits;
use super::writer::Writer;
use crate::estree::{Expression, Pattern, Span, Spanned};
use crate::lower::ir::*;
use serde_json::Value as Json;

/// How an expression is evaluated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Ordinary read context (template expressions, handlers).
    Read,
    /// The key function of a keyed each: the each-block item and index are plain values there.
    EachKey,
}

pub trait Rewriter {
    /// Span edits that make `e` valid JS against the runtime.
    fn expr(&self, e: &Expression, mode: Mode, edits: &mut Edits);
    /// Whether evaluating `e` can read reactive state (decides `$.template_effect`).
    fn is_reactive(&self, e: &Expression) -> bool;
    /// JS that assigns the JS expression text `value` to the lvalue `target` (bind: setters).
    fn assign(&self, target: &Expression, value: &str) -> String;
    /// Emits the instance script: (hoisted import text, component-body text), both mapped.
    fn script(&self, w: &mut Writer, hoist: bool);
}

pub struct Codegen<'a, 's> {
    pub src: &'s str,
    pub rw: &'s dyn Rewriter,
    pub w: Writer<'s>,
    tmp: usize,
    _p: std::marker::PhantomData<&'a ()>,
}

/// Generates the whole module. `name` is the component function name.
pub fn generate(src: &str, comp: &Component, rw: &dyn Rewriter, name: &str, filename: &str) -> (String, String) {
    let mut g = Codegen { src, rw, w: Writer::new(src), tmp: 0, _p: std::marker::PhantomData };
    g.w.write("import * as $ from 'svelte/internal/client';\nimport * as $n from '@svelters/native-runtime';\n");
    rw.script(&mut g.w, true);
    g.w.write(&format!("\nexport default function {name}($$anchor, $$props) {{"));
    g.w.indent();
    g.w.newline();
    g.w.write("$.push($$props, true);");
    rw.script(&mut g.w, false);
    g.fragment(&comp.root, "$$anchor");
    g.w.newline();
    g.w.write("$.pop();");
    g.w.dedent();
    g.w.newline();
    g.w.write("}\n");
    let out = filename.rsplit('/').next().unwrap_or(filename).replace(".svelte", ".js");
    g.w.finish(filename, src, &out)
}

impl Codegen<'_, '_> {
    fn line(&mut self) {
        self.w.newline();
    }

    fn expr_text(&mut self, e: &Expression, mode: Mode) {
        let mut edits = Edits::new();
        self.rw.expr(e, mode, &mut edits);
        let s = e.span();
        edits.emit(&mut self.w, self.src, s.start, s.end);
    }

    /// `(expr)`, parenthesised unless it already is.
    fn paren_expr(&mut self, e: &Expression, mode: Mode) {
        let wrap = !matches!(e, Expression::ParenthesizedExpression(_) | Expression::Identifier(_) | Expression::Literal(_) | Expression::MemberExpression(_) | Expression::CallExpression(_));
        if wrap {
            self.w.write("(");
        }
        self.expr_text(e, mode);
        if wrap {
            self.w.write(")");
        }
    }

    fn json(&mut self, v: &Json) {
        self.w.write(&v.to_string());
    }

    fn fragment(&mut self, f: &Fragment, container: &str) {
        for item in &f.items {
            match item {
                Item::Element(e) => self.element(e, container),
                Item::If(b) => self.if_block(b, container),
                Item::Each(b) => self.each_block(b, container),
                Item::Component(c) => self.component(c, container),
            }
        }
    }

    fn element(&mut self, e: &Element, container: &str) {
        let n = format!("n{}", e.id + 1);
        self.line();
        self.w.map(e.span.start);
        match &e.present {
            Some(p) => self.w.write(&format!("const {n} = $n.element({}, {});", js_str(&e.kind), js_str(p))),
            None => self.w.write(&format!("const {n} = $n.element({});", js_str(&e.kind))),
        }
        for op in &e.ops {
            self.op(&n, op);
        }
        // children grouped per slot container
        self.fragment_children(&e.children, &n);
        self.line();
        self.w.map(e.span.start);
        self.w.write(&format!("$n.append({container}, {n});"));
    }

    fn fragment_children(&mut self, f: &Fragment, parent: &str) {
        for item in &f.items {
            let slot = match item {
                Item::Element(e) => &e.slot,
                Item::If(b) => &b.slot,
                Item::Each(b) => &b.slot,
                Item::Component(c) => &c.slot,
            };
            let c = format!("$n.slot({parent}, {})", slot.as_deref().map_or("null".to_string(), js_str));
            match item {
                Item::Element(e) => self.element(e, &c),
                Item::If(b) => self.if_block(b, &c),
                Item::Each(b) => self.each_block(b, &c),
                Item::Component(cc) => self.component(cc, &c),
            }
        }
    }

    fn op(&mut self, n: &str, op: &Op) {
        self.line();
        match op {
            Op::Set { prop, value, span } => {
                self.w.map(span.start);
                self.w.write(&format!("$n.set({n}, {}, ", js_str(prop)));
                self.json(value);
                self.w.write(");");
            }
            Op::SetDynamic { prop, value, span } => {
                self.w.map(span.start);
                let reactive = self.dynamic_reactive(value);
                if reactive {
                    self.w.write("$.template_effect(() => ");
                }
                self.w.write(&format!("$n.set({n}, {}, ", js_str(prop)));
                self.dynamic(value);
                self.w.write(")");
                if reactive {
                    self.w.write(")");
                }
                self.w.write(";");
            }
            Op::Listen { signal, handler, span } => {
                self.w.map(span.start);
                self.w.write(&format!("$n.on({n}, {}, ", js_str(signal)));
                self.handler(handler);
                self.w.write(");");
            }
            Op::Bind { prop, signal, arg, target, span } => {
                self.w.map(span.start);
                self.w.write(&format!("$n.bind({n}, {}, {}, {arg}, () => ", js_str(prop), js_str(signal)));
                self.expr_text(target, Mode::Read);
                let set = self.rw.assign(target, "$$value");
                self.w.write(&format!(", ($$value) => {{ {set}; }});"));
            }
            Op::Track { prop, signal, arg } => {
                self.w.write(&format!("$n.track({n}, {}, {}, {arg});", js_str(prop), js_str(signal)));
            }
            Op::Token { prop, token, cond, span } => {
                self.w.map(span.start);
                if cond.is_empty() {
                    self.w.write(&format!("$n.token({n}, {}, {}, true);", js_str(prop), js_str(token)));
                } else {
                    self.w.write(&format!("$.template_effect(() => $n.token({n}, {}, {}, ", js_str(prop), js_str(token)));
                    self.cond(cond);
                    self.w.write("));");
                }
            }
            Op::StyleProp { prop, candidates, fallback, span } => {
                self.w.map(span.start);
                if *fallback == Fallback::Restore {
                    self.w.write(&format!("$n.capture({n}, {});", js_str(prop)));
                    self.line();
                }
                self.w.write(&format!("$.template_effect(() => $n.set({n}, {}, ", js_str(prop)));
                for (c, v) in candidates {
                    if c.is_empty() {
                        self.json(v);
                        self.w.write("));");
                        return;
                    }
                    self.cond(c);
                    self.w.write(" ? ");
                    self.json(v);
                    self.w.write(" : ");
                }
                match fallback {
                    Fallback::Default(d) => self.json(d),
                    Fallback::Restore => self.w.write("$n.RESTORE"),
                }
                self.w.write("));");
            }
        }
    }

    fn cond(&mut self, c: &Cond) {
        for (i, e) in c.iter().enumerate() {
            if i > 0 {
                self.w.write(" && ");
            }
            self.w.write("!!");
            self.paren_expr(e, Mode::Read);
        }
    }

    fn dynamic_reactive(&self, d: &Dynamic) -> bool {
        match d {
            Dynamic::Expr(e) => self.rw.is_reactive(e),
            Dynamic::Template(parts) => parts.iter().any(|p| matches!(p, TemplatePart::Expr(e) if self.rw.is_reactive(e))),
        }
    }

    fn dynamic(&mut self, d: &Dynamic) {
        match d {
            Dynamic::Expr(e) => self.expr_text(e, Mode::Read),
            Dynamic::Template(parts) => {
                self.w.write("`");
                for p in parts {
                    match p {
                        TemplatePart::Text(t) => self.w.write(&escape_template(t)),
                        TemplatePart::Expr(e) => {
                            self.w.write("${");
                            self.paren_expr(e, Mode::Read);
                            self.w.write(" ?? ''}");
                        }
                    }
                }
                self.w.write("`");
            }
        }
    }

    fn handler(&mut self, h: &Expression) {
        if matches!(h, Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)) {
            self.expr_text(h, Mode::Read);
        } else {
            // evaluated at event time, so a reactive handler value is read when the event fires
            self.w.write("function (...$$args) { ");
            self.paren_expr(h, Mode::Read);
            self.w.write("?.apply(this, $$args); }");
        }
    }

    fn if_block(&mut self, b: &IfBlock, container: &str) {
        self.line();
        self.w.map(b.span.start);
        self.w.write(&format!("$n.if_block({container}, () => "));
        self.expr_text(b.test, Mode::Read);
        self.w.write(", ($$anchor) => {");
        self.w.indent();
        self.fragment(&b.consequent, "$$anchor");
        self.w.dedent();
        self.line();
        self.w.write("}, ");
        match &b.alternate {
            Some(alt) => {
                self.w.write("($$anchor) => {");
                self.w.indent();
                self.fragment(alt, "$$anchor");
                self.w.dedent();
                self.line();
                self.w.write("});");
            }
            None => self.w.write("null);"),
        }
    }

    fn each_block(&mut self, b: &EachBlock, container: &str) {
        let item = match b.context {
            Some(Pattern::Identifier(id)) => id.name.clone(),
            _ => {
                self.tmp += 1;
                format!("$$item{}", self.tmp)
            }
        };
        let index = b.index.clone().unwrap_or_else(|| "$$index".to_string());
        self.line();
        self.w.map(b.span.start);
        self.w.write(&format!("$n.each_block({container}, () => "));
        self.expr_text(b.collection, Mode::Read);
        self.w.write(", ");
        match b.key {
            Some(k) => {
                self.w.write(&format!("({item}, {index}) => "));
                self.paren_expr(k, Mode::EachKey);
            }
            None => self.w.write("null"),
        }
        self.w.write(&format!(", ($$anchor, {item}, {index}) => {{"));
        self.w.indent();
        self.fragment(&b.body, "$$anchor");
        self.w.dedent();
        self.line();
        self.w.write("}, ");
        match &b.fallback {
            Some(f) => {
                self.w.write("($$anchor) => {");
                self.w.indent();
                self.fragment(f, "$$anchor");
                self.w.dedent();
                self.line();
                self.w.write("});");
            }
            None => self.w.write("null);"),
        }
    }

    fn component(&mut self, c: &ComponentCall, container: &str) {
        self.line();
        self.w.map(c.span.start);
        self.w.write(&format!("{}($n.region({container}), {{", c.name));
        for (i, p) in c.props.iter().enumerate() {
            if i > 0 {
                self.w.write(",");
            }
            self.w.write(" ");
            match p {
                ComponentProp::Static { name, value } => {
                    self.w.write(&format!("{}: ", js_key(name)));
                    self.json(value);
                }
                ComponentProp::Dynamic { name, value } => {
                    self.w.write(&format!("get {}() {{ return ", js_key(name)));
                    self.dynamic(value);
                    self.w.write("; }");
                }
                ComponentProp::Bind { name, target } => {
                    self.w.write(&format!("get {}() {{ return ", js_key(name)));
                    self.expr_text(target, Mode::Read);
                    let set = self.rw.assign(target, "$$value");
                    self.w.write(&format!("; }}, set {}($$value) {{ {set}; }}", js_key(name)));
                }
            }
        }
        self.w.write(" });");
    }
}

pub fn js_str(s: &str) -> String {
    serde_json::to_string(s).expect("string serializes")
}

fn js_key(s: &str) -> String {
    let ident = s.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    if ident { s.to_string() } else { js_str(s) }
}

fn escape_template(s: &str) -> String {
    s.replace('\\', "\\\\").replace('`', "\\`").replace("${", "\\${")
}

#[allow(dead_code)]
fn span_of(s: Span) -> (u32, u32) {
    (s.start, s.end)
}
