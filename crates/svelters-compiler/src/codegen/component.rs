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
    /// Svelte's `ExpressionMetadata.has_call`: `e` calls something that is not a pure global (or
    /// calls anything while it references bindings), outside nested functions. Such template
    /// expressions are memoized in a derived (`Memoizer`), so they run once per change of what they
    /// read, not whenever a sibling expression of the same effect changes.
    fn has_call(&self, _e: &Expression) -> bool {
        false
    }
    /// JS that assigns the JS expression text `value` to the lvalue `target` (bind: setters).
    fn assign(&self, target: &Expression, value: &str) -> String;
    /// Emits the instance script: (hoisted import text, component-body text), both mapped.
    fn script(&self, w: &mut Writer, hoist: bool);
    /// Whether the top-level snippet starting at `start` references nothing of the component
    /// instance, so it is hoisted to module level (Svelte's module_level_snippets).
    fn hoist_snippet(&self, _start: u32) -> bool {
        false
    }
    /// For a component tag whose name is dynamic (Svelte's `Component.metadata.dynamic`: a
    /// non-normal binding such as `$state`/prop/each item, or a member expression), the JS that
    /// reads the current component function; None for a static name.
    fn dynamic_component(&self, _node: &crate::svelte::ast::Element) -> Option<String> {
        None
    }
    /// Ends the component body (`$.pop()`; store cleanup / `return $.pop($$exports)`).
    fn epilogue(&self, w: &mut Writer) {
        w.write("$.pop();");
    }
}

pub struct Codegen<'a, 's> {
    pub src: &'s str,
    pub rw: &'s dyn Rewriter,
    pub w: Writer<'s>,
    tmp: usize,
    /// The next `fragment` is the component root, whose snippets `generate` already emitted.
    root_fragment: bool,
    /// Expressions memoized as `$.template_effect` dependencies while one effect is written
    /// (address -> the effect parameter that holds its value).
    memo: Vec<(usize, String)>,
    _p: std::marker::PhantomData<&'a ()>,
}

/// Generates the whole module. `name` is the component function name.
pub fn generate(src: &str, comp: &Component, rw: &dyn Rewriter, name: &str, filename: &str, imports: &crate::target::RuntimeImports) -> (String, String) {
    let mut g = Codegen { src, rw, w: Writer::new(src), tmp: 0, root_fragment: false, memo: Vec::new(), _p: std::marker::PhantomData };
    g.w.write(&format!("import * as $ from '{}';\nimport * as $n from '{}';\n", imports.core, imports.native));
    rw.script(&mut g.w, true);
    let (hoisted, instance): (Vec<_>, Vec<_>) = comp.root.snippets.iter().partition(|s| rw.hoist_snippet(s.span.start));
    for s in hoisted {
        g.snippet(s);
        g.w.newline();
    }
    g.w.write(&format!("\nexport default function {name}($$anchor, $$props) {{"));
    g.w.indent();
    g.w.newline();
    g.w.write("$.push($$props, true);");
    // Top-level snippets come before the instance script, so it can reference them (Svelte's
    // instance_level_snippets)
    for s in instance {
        g.snippet(s);
    }
    rw.script(&mut g.w, false);
    g.root_fragment = true;
    g.fragment(&comp.root, "$$anchor");
    g.w.newline();
    rw.epilogue(&mut g.w);
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

    /// `get, set` of a binding: the functions of `bind:x={get, set}` (Svelte >= 5.9), else accessors
    /// of the bound lvalue.
    fn bind_accessors(&mut self, target: &Expression) {
        if let Some((get, set)) = function_binding(target) {
            self.paren_expr(get, Mode::Read);
            self.w.write(", ");
            self.paren_expr(set, Mode::Read);
        } else {
            self.w.write("() => ");
            self.expr_text(target, Mode::Read);
            let set = self.rw.assign(target, "$$value");
            self.w.write(&format!(", ($$value) => {{ {set}; }}"));
        }
    }

    /// The `set` half of a binding (read-only bindings never read the target).
    fn bind_setter(&mut self, target: &Expression) {
        if let Some((_, set)) = function_binding(target) {
            self.paren_expr(set, Mode::Read);
        } else {
            let set = self.rw.assign(target, "$$value");
            self.w.write(&format!("($$value) => {{ {set}; }}"));
        }
    }

    fn json(&mut self, v: &Json) {
        self.w.write(&v.to_string());
    }


    /// `{@const}` and snippet declarations of a fragment (before its items).
    fn decls(&mut self, f: &Fragment) {
        for c in &f.consts {
            self.line();
            self.w.map(c.span.start);
            self.w.write(&format!("const {} = $.derived(() => ", c.name));
            self.paren_expr(c.init, Mode::Read);
            self.w.write(");");
        }
        for s in &f.snippets {
            self.snippet(s);
        }
    }

    fn fragment(&mut self, f: &Fragment, container: &str) {
        if std::mem::take(&mut self.root_fragment) {
            for c in &f.consts {
                self.line();
                self.w.map(c.span.start);
                self.w.write(&format!("const {} = $.derived(() => ", c.name));
                self.paren_expr(c.init, Mode::Read);
                self.w.write(");");
            }
        } else {
            self.decls(f);
        }
        for item in &f.items {
            self.item(item, container);
        }
        if DEFER_TEMPLATE_EFFECTS {
            self.deferred_ops(f);
        }
    }

    fn item(&mut self, item: &Item, container: &str) {
        match item {
            Item::Element(e) => self.element(e, container),
            Item::If(b) => self.if_block(b, container),
            Item::Each(b) => self.each_block(b, container),
            Item::Key(b) => self.key_block(b, container),
            Item::Await(b) => self.await_block(b, container),
            Item::Render(r) => self.render(r, container),
            Item::Component(c) => self.component(c, container),
        }
    }

    fn element(&mut self, e: &Element, container: &str) {
        let n = format!("$$n{}", e.id + 1); // `$$` is reserved by Svelte, so user bindings cannot collide
        self.line();
        self.w.map(e.span.start);
        if e.toplevel {
            let present = e.present.as_deref().map_or("null".to_string(), js_str);
            match &e.show {
                Some(s) => self.w.write(&format!("const {n} = $n.toplevel({}, {present}, {});", js_str(&e.kind), js_str(s))),
                None => self.w.write(&format!("const {n} = $n.toplevel({}, {present});", js_str(&e.kind))),
            }
        } else if e.attached {
            self.w.write(&format!("const {n} = $n.attached({});", js_str(&e.kind)));
        } else {
            self.w.write(&format!("const {n} = $n.element({});", js_str(&e.kind)));
        }
        if let Some(c) = &e.class_expr {
            self.line();
            self.w.write(&format!("const c{} = $.derived(() => $n.classes(", e.id + 1));
            self.dynamic(c);
            self.w.write("));");
        }
        if let Some(i) = &e.id_expr {
            self.line();
            self.w.write(&format!("const i{} = $.derived(() => String(", e.id + 1));
            self.dynamic(i);
            self.w.write(" ?? \"\"));");
        }
        for op in &e.ops {
            if !(DEFER_TEMPLATE_EFFECTS && self.is_deferred(op)) {
                self.op(&n, op);
            }
        }
        // children grouped per slot container
        self.fragment_children(&e.children, &n);
        self.line();
        self.w.map(e.span.start);
        self.w.write(&format!("$n.append({container}, {n});"));
    }

    /// Render effects of a fragment's elements are created after its blocks and components, as
    /// Svelte's single `$.template_effect` at the end of each fragment (so a parent's template
    /// expressions are evaluated after its children's components initialised).
    fn is_deferred(&self, op: &Op) -> bool {
        match op {
            Op::SetDynamic { value, .. } => self.dynamic_reactive(value),
            Op::ChildAttr { value: AttrValue::Dynamic(value), .. } => self.dynamic_reactive(value),
            Op::Token { cond, .. } => !cond.is_empty(),
            Op::StyleProp { .. } => true,
            Op::Classes { dynamic, toggles, .. } => dynamic.is_some() || !toggles.is_empty(),
            Op::WidgetName { value: AttrValue::Dynamic(value), .. } => self.dynamic_reactive(value),
            Op::InlineStyle { dynamic, directives, .. } => dynamic.is_some() || !directives.is_empty(),
            Op::StyleSetter { .. } => true,
            _ => false,
        }
    }

    /// The deferred ops of the elements of `f` (and of their children, except children in their
    /// own JS block, which flush their own).
    fn deferred_ops(&mut self, f: &Fragment) {
        for item in &f.items {
            if let Item::Element(e) = item {
                let n = format!("$$n{}", e.id + 1);
                for op in &e.ops {
                    if self.is_deferred(op) {
                        self.op(&n, op);
                    }
                }
                if !scoped(&e.children) {
                    self.deferred_ops(&e.children);
                }
            }
        }
    }

    fn fragment_children(&mut self, f: &Fragment, parent: &str) {
        // snippets declared among an element's children get their own JS block scope
        let scoped = scoped(f);
        if scoped {
            self.line();
            self.w.write("{");
            self.w.indent();
        }
        self.decls(f);
        for item in &f.items {
            let c = format!("$n.slot({parent}, {})", item.slot().as_deref().map_or("null".to_string(), js_str));
            self.item(item, &c);
        }
        if scoped && DEFER_TEMPLATE_EFFECTS {
            self.deferred_ops(f);
        }
        if scoped {
            self.w.dedent();
            self.line();
            self.w.write("}");
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
                self.dynamic_call(&format!("$n.set({n}, {}, ", js_str(prop)), value);
            }
            Op::Listen { signal, handler, flags, span, .. } => {
                self.w.map(span.start);
                self.w.write(&format!("$n.on({n}, {}, ", js_str(signal)));
                match handler {
                    Handler::Expr(h) => self.handler(h),
                    Handler::Member { object, key } => {
                        // a property of a spread object, read when the event fires
                        self.w.write("function (...$$args) { ");
                        self.paren_expr(object, Mode::Read);
                        self.w.write(&format!("[{}]?.apply(this, $$args); }}", js_str(key)));
                    }
                }
                if *flags != 0 {
                    self.w.write(&format!(", {flags}"));
                }
                self.w.write(");");
            }
            Op::ListenAttach { signal, handler, flags, span } => {
                self.w.map(span.start);
                self.w.write(&format!("$.attach({n}, () => $n.listen({}, ", js_str(signal)));
                self.paren_expr(handler, Mode::Read);
                self.w.write(&format!(", {flags}));"));
            }
            Op::Attach { expr, span } => {
                self.w.map(span.start);
                self.w.write(&format!("$.attach({n}, () => "));
                self.paren_expr(expr, Mode::Read);
                self.w.write(");");
            }
            Op::Classes { base, dynamic, toggles, span } => {
                self.w.map(span.start);
                if dynamic.is_none() && toggles.is_empty() {
                    self.w.write(&format!("$n.attr({n}, \"class\", {});", js_str(base)));
                    return;
                }
                self.w.write(&format!("$.template_effect(() => $n.attr({n}, \"class\", $n.class_name({}, ", js_str(base)));
                match dynamic {
                    Some(id) => self.w.write(&format!("$.get(c{})", id + 1)),
                    None => self.w.write("null"),
                }
                self.w.write(", [");
                for (i, (name, e)) in toggles.iter().enumerate() {
                    if i > 0 {
                        self.w.write(", ");
                    }
                    self.w.write(&format!("[{}, !!", js_str(name)));
                    self.paren_expr(e, Mode::Read);
                    self.w.write("]");
                }
                self.w.write("])));");
            }
            Op::WidgetName { value, span } => {
                self.w.map(span.start);
                match value {
                    AttrValue::Static(v) => {
                        self.w.write(&format!("$n.attr({n}, \"id\", "));
                        self.json(v);
                        self.w.write(");");
                    }
                    AttrValue::Dynamic(d) => self.dynamic_call(&format!("$n.attr({n}, \"id\", "), d),
                }
            }
            Op::InlineStyle { base, dynamic, directives, span } => {
                self.w.map(span.start);
                if dynamic.is_none() && directives.is_empty() {
                    self.w.write(&format!("$n.attr({n}, \"style\", {});", js_str(base)));
                    return;
                }
                self.w.write(&format!("$.template_effect(() => $n.attr({n}, \"style\", $n.style_text("));
                match dynamic {
                    Some(d) => self.dynamic(d),
                    None => self.w.write(&js_str(base)),
                }
                self.w.write(", [");
                for (i, (name, v)) in directives.iter().enumerate() {
                    if i > 0 {
                        self.w.write(", ");
                    }
                    self.w.write(&format!("[{}, ", js_str(name)));
                    match v {
                        AttrValue::Static(j) => self.json(j),
                        AttrValue::Dynamic(d) => self.dynamic(d),
                    }
                    self.w.write("]");
                }
                self.w.write("])));");
            }
            Op::StyleSetter { name, target, child_attr, backend, value, fallback: (candidates, fallback), span } => {
                self.w.map(span.start);
                let f = if *child_attr { "child_attr" } else { "set" };
                if *fallback == Fallback::Restore {
                    self.w.write(&format!("$n.capture({n}, {});", js_str(target)));
                    self.line();
                }
                self.w.write(&format!("$.template_effect(() => $n.{f}({n}, {}, $n.setter_value({}, {}, ", js_str(target), js_str(backend), js_str(name)));
                self.dynamic(value);
                self.w.write(")");
                if !candidates.is_empty() || *fallback != Fallback::Default(Json::Null) {
                    self.w.write(" ?? ");
                    self.candidates(candidates, fallback);
                }
                self.w.write("));");
            }
            Op::Explicit { props, attrs } => {
                let list = |v: &[String]| v.iter().map(|x| js_str(x)).collect::<Vec<_>>().join(", ");
                self.w.write(&format!("$n.explicit({n}, [{}], [{}]);", list(props), list(attrs)));
            }
            Op::ChildAttr { name, value, span } => {
                self.w.map(span.start);
                match value {
                    AttrValue::Static(v) => {
                        self.w.write(&format!("$n.child_attr({n}, {}, ", js_str(name)));
                        self.json(v);
                        self.w.write(");");
                    }
                    AttrValue::Dynamic(d) => self.dynamic_call(&format!("$n.child_attr({n}, {}, ", js_str(name)), d),
                }
            }
            Op::Bind { prop, signal, arg, target, span } => {
                self.w.map(span.start);
                self.w.write(&format!("$n.bind({n}, {}, {}, {arg}, ", js_str(prop), js_str(signal)));
                if let Some((get, set)) = function_binding(target) {
                    // `bind:value={get, set}` (Svelte >= 5.9): the functions are the accessors
                    self.paren_expr(get, Mode::Read);
                    self.w.write(", ");
                    self.paren_expr(set, Mode::Read);
                    self.w.write(");");
                } else {
                    self.w.write("() => ");
                    self.expr_text(target, Mode::Read);
                    let set = self.rw.assign(target, "$$value");
                    self.w.write(&format!(", ($$value) => {{ {set}; }});"));
                }
            }
            Op::BindProp { prop, fallback, target, span } => {
                self.w.map(span.start);
                self.w.write(&format!("$n.bind_prop({n}, {}, ", js_str(prop)));
                self.bind_accessors(target);
                if let Some((signal, arg)) = fallback {
                    self.w.write(&format!(", {}, {arg}", js_str(signal)));
                }
                self.w.write(");");
            }
            Op::BindReadonly { prop, target, span } => {
                self.w.map(span.start);
                self.w.write(&format!("$n.bind_readonly({n}, {}, ", js_str(prop)));
                self.bind_setter(target);
                self.w.write(");");
            }
            Op::BindOnce { prop, readonly, target, span } => {
                self.w.map(span.start);
                self.w.write(&format!("$n.bind_once({n}, {}, ", js_str(prop)));
                self.bind_accessors(target);
                self.w.write(&format!(", {readonly});"));
            }
            Op::BindThis { target, span } => {
                self.w.map(span.start);
                if let Some((get, set)) = function_binding(target) {
                    self.w.write(&format!("$.bind_this({n}, "));
                    self.paren_expr(set, Mode::Read);
                    self.w.write(", ");
                    self.paren_expr(get, Mode::Read);
                    self.w.write(");");
                } else {
                    let set = self.rw.assign(target, "$$value");
                    self.w.write(&format!("$.bind_this({n}, ($$value) => {{ {set}; }}, () => "));
                    self.expr_text(target, Mode::Read);
                    self.w.write(");");
                }
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
            Op::StyleProp { prop, child_attr, candidates, fallback, guarded, span } => {
                self.w.map(span.start);
                let f = match (*child_attr, *guarded) {
                    (true, false) => "child_attr",
                    (false, false) => "set",
                    (true, true) => "rule_attr",
                    (false, true) => "rule_set",
                };
                match fallback {
                    Fallback::Restore => self.w.write(&format!("$n.capture({n}, {});", js_str(prop))),
                    // the widget starts at the spec's native default: don't write it redundantly
                    Fallback::Default(d) if !d.is_null() => self.w.write(&format!("$n.assume({n}, {}, {d});", js_str(prop))),
                    Fallback::Default(_) => {}
                }
                if !matches!(fallback, Fallback::Default(d) if d.is_null()) {
                    self.line();
                }
                self.w.write(&format!("$.template_effect(() => $n.{f}({n}, {}, ", js_str(prop)));
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


    /// `c1 ? v1 : c2 ? v2 : fallback` (an unconditional candidate ends the chain), parenthesized.
    fn candidates(&mut self, candidates: &[(Cond, Json)], fallback: &Fallback) {
        self.w.write("(");
        for (c, v) in candidates {
            if c.is_empty() {
                self.json(v);
                self.w.write(")");
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
        self.w.write(")");
    }

    fn cond(&mut self, c: &Cond) {
        for (i, a) in c.iter().enumerate() {
            if i > 0 {
                self.w.write(" && ");
            }
            match a {
                CondAtom::Expr(e) => {
                    self.w.write("!!");
                    self.paren_expr(e, Mode::Read);
                }
                CondAtom::Class { element, class } => self.w.write(&format!("$.get(c{}).has({})", element + 1, js_str(class))),
                CondAtom::Id { element, id } => self.w.write(&format!("$.get(i{}) === {}", element + 1, js_str(id))),
            }
        }
    }

    fn needs_memo(&self, d: &Dynamic) -> bool {
        let simple = |e: &Expression| matches!(e, Expression::Identifier(_) | Expression::MemberExpression(_));
        match d {
            Dynamic::Expr(e) => !simple(e) && self.rw.is_reactive(e),
            Dynamic::Template(parts) => {
                parts.iter().any(|p| matches!(p, TemplatePart::Expr(e) if !simple(e))) && self.dynamic_reactive(d)
            }
            Dynamic::Member { .. } => false,
        }
    }

    fn parts_reactive(&self, parts: &[TemplatePart]) -> bool {
        parts.iter().any(|p| match p {
            TemplatePart::Text(_) => false,
            TemplatePart::Expr(e) => self.rw.is_reactive(e),
            TemplatePart::If { test, consequent, alternate } => self.rw.is_reactive(test) || self.parts_reactive(consequent) || self.parts_reactive(alternate),
        })
    }

    fn dynamic_reactive(&self, d: &Dynamic) -> bool {
        match d {
            Dynamic::Expr(e) => self.rw.is_reactive(e),
            Dynamic::Template(parts) => self.parts_reactive(parts),
            Dynamic::Member { object, .. } => self.rw.is_reactive(object),
        }
    }

    /// `call_head` + the value + `)`: a one-off write, or a render effect when the value is
    /// reactive. Call expressions of the value (`has_call`) are the effect's memoized dependencies
    /// (Svelte's `Memoizer`): `$.template_effect(($0) => .., [() => f(x)])` runs `f(x)` in a
    /// derived of its own, so it is not called again when only another part changes.
    fn dynamic_call(&mut self, call_head: &str, d: &Dynamic) {
        let reactive = self.dynamic_reactive(d);
        let mut memo: Vec<&Expression> = Vec::new();
        if reactive {
            match d {
                Dynamic::Expr(e) if self.rw.has_call(e) => memo.push(e),
                Dynamic::Template(parts) => memo.extend(parts.iter().filter_map(|p| match p {
                    TemplatePart::Expr(e) if self.rw.has_call(e) => Some(*e),
                    _ => None,
                })),
                _ => {}
            }
            let params: Vec<String> = (0..memo.len()).map(|i| format!("${i}")).collect();
            self.w.write(&format!("$.template_effect(({}) => ", params.join(", ")));
        }
        self.memo = memo.iter().enumerate().map(|(i, e)| (*e as *const Expression as usize, format!("${i}"))).collect();
        self.w.write(call_head);
        self.dynamic(d);
        self.memo.clear();
        self.w.write(")");
        if reactive {
            if !memo.is_empty() {
                self.w.write(", [");
                for (i, e) in memo.iter().enumerate() {
                    if i > 0 {
                        self.w.write(", ");
                    }
                    self.w.write("() => ");
                    self.paren_expr(e, Mode::Read);
                }
                self.w.write("]");
            }
            self.w.write(")");
        }
        self.w.write(";");
    }

    /// The effect parameter holding `e`'s memoized value, while `dynamic_call` writes an effect.
    fn memoized(&self, e: &Expression) -> Option<String> {
        let addr = e as *const Expression as usize;
        self.memo.iter().find(|(a, _)| *a == addr).map(|(_, id)| id.clone())
    }

    fn dynamic(&mut self, d: &Dynamic) {
        match d {
            Dynamic::Expr(e) if let Some(id) = self.memoized(e) => self.w.write(&id),
            Dynamic::Expr(e) => self.expr_text(e, Mode::Read),
            Dynamic::Template(parts) => self.template(parts),
            Dynamic::Member { object, key } => {
                self.paren_expr(object, Mode::Read);
                self.w.write(&format!("[{}]", js_str(key)));
            }
        }
    }

    fn template(&mut self, parts: &[TemplatePart]) {
        self.w.write("`");
        for p in parts {
            match p {
                TemplatePart::Text(t) => self.w.write(&escape_template(t)),
                TemplatePart::Expr(e) => {
                    self.w.write("${");
                    match self.memoized(e) {
                        Some(id) => self.w.write(&id),
                        None => self.paren_expr(e, Mode::Read),
                    }
                    self.w.write(" ?? ''}");
                }
                TemplatePart::If { test, consequent, alternate } => {
                    self.w.write("${(");
                    self.expr_text(test, Mode::Read);
                    self.w.write(") ? ");
                    self.template(consequent);
                    self.w.write(" : ");
                    self.template(alternate);
                    self.w.write("}");
                }
            }
        }
        self.w.write("`");
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


    /// `($$anchor, extra) => { body }`, or `null`.
    fn branch_fn(&mut self, f: Option<&Fragment>, extra: Option<&str>) {
        let Some(f) = f else {
            self.w.write("null");
            return;
        };
        match extra {
            Some(x) => self.w.write(&format!("($$anchor, {x}) => {{")),
            None => self.w.write("($$anchor) => {"),
        }
        self.w.indent();
        self.fragment(f, "$$anchor");
        self.w.dedent();
        self.line();
        self.w.write("}");
    }

    fn key_block(&mut self, b: &KeyBlock, container: &str) {
        self.line();
        self.w.map(b.span.start);
        self.w.write(&format!("$n.key_block({container}, () => "));
        self.paren_expr(b.key, Mode::Read);
        self.w.write(", ");
        self.branch_fn(Some(&b.body), None);
        self.w.write(");");
    }

    fn await_block(&mut self, b: &AwaitBlock, container: &str) {
        self.line();
        self.w.map(b.span.start);
        self.w.write(&format!("$n.await_block({container}, () => "));
        self.paren_expr(b.expression, Mode::Read);
        self.w.write(", ");
        self.branch_fn(b.pending.as_ref(), None);
        self.w.write(", ");
        self.branch_fn(b.then.as_ref(), Some(b.value.as_deref().unwrap_or("$$value")));
        self.w.write(", ");
        self.branch_fn(b.catch.as_ref(), Some(b.error.as_deref().unwrap_or("$$error")));
        self.w.write(");");
    }

    /// `const name = ($$anchor, a = $.noop) => { body };` (Svelte's snippet shape).
    fn snippet(&mut self, s: &Snippet) {
        self.snippet_with(s, &[]);
    }

    fn consts(&mut self, consts: &[ConstDecl]) {
        for k in consts {
            self.line();
            self.w.map(k.span.start);
            self.w.write(&format!("const {} = $.derived(() => ", k.name));
            self.paren_expr(k.init, Mode::Read);
            self.w.write(");");
        }
    }

    /// A snippet declaration; `consts` are declared at the start of its body.
    fn snippet_with(&mut self, s: &Snippet, consts: &[ConstDecl]) {
        self.line();
        self.w.map(s.span.start);
        let params: String = s.params.iter().map(|p| format!(", {p} = $.noop")).collect();
        self.w.write(&format!("const {} = ($$anchor{params}) => {{", s.name));
        self.w.indent();
        self.consts(consts);
        self.fragment(&s.body, "$$anchor");
        self.w.dedent();
        self.line();
        self.w.write("};");
    }

    fn render(&mut self, r: &RenderCall, container: &str) {
        // arguments that call something are memoized (Svelte's RenderTag `Memoizer`): the snippet
        // reads them through a derived, so the call runs once per change, not once per read
        let mut memo: Vec<Option<String>> = Vec::new();
        for a in &r.args {
            if self.rw.has_call(a) {
                self.tmp += 1;
                let id = format!("$$arg{}", self.tmp);
                self.line();
                self.w.write(&format!("const {id} = $.derived(() => "));
                self.paren_expr(a, Mode::Read);
                self.w.write(");");
                memo.push(Some(id));
            } else {
                memo.push(None);
            }
        }
        self.line();
        self.w.map(r.span.start);
        if self.rw.is_reactive(r.callee) {
            // the snippet function itself can change (a prop): re-render when it does
            self.w.write(&format!("$n.snippet({container}, () => "));
            if r.optional {
                self.paren_expr(r.callee, Mode::Read);
                self.w.write(" ?? $.noop");
            } else {
                self.expr_text(r.callee, Mode::Read);
            }
        } else {
            self.paren_expr(r.callee, Mode::Read);
            self.w.write(if r.optional { "?.(" } else { "(" });
            self.w.write(&format!("$n.region({container})"));
        }
        for (a, m) in r.args.iter().zip(&memo) {
            self.w.write(", () => ");
            match m {
                Some(id) => self.w.write(&format!("$.get({id})")),
                None => self.paren_expr(a, Mode::Read),
            }
        }
        self.w.write(");");
    }

    fn component(&mut self, c: &ComponentCall, container: &str) {
        let scoped = !c.snippets.is_empty() || !c.consts.is_empty();
        // `<svelte:boundary>`: its `{@const}`s live inside it (so their errors are caught), copied
        // into each of its snippets (Svelte's SvelteBoundary visitor)
        let boundary = c.name == "$n.boundary";
        if scoped {
            self.line();
            self.w.write("{");
            self.w.indent();
            if boundary {
                for s in &c.snippets {
                    self.snippet_with(s, &c.consts);
                }
            } else {
                self.consts(&c.consts);
                for s in &c.snippets {
                    self.snippet(s);
                }
            }
        }
        // Svelte's build_component memoizes non-trivial reactive prop values (anything but an
        // identifier or member expression) in a derived, so a child reading the prop several
        // times evaluates it once per change (`random={Math.random()}`, `active={i === index}`).
        let mut memo: Vec<Option<String>> = Vec::new();
        for p in &c.props {
            let m = match p {
                ComponentProp::Dynamic { value, .. } if self.needs_memo(value) => {
                    self.tmp += 1;
                    let id = format!("$$prop{}", self.tmp);
                    self.line();
                    self.w.write(&format!("const {id} = $.derived(() => "));
                    self.dynamic(value);
                    self.w.write(");");
                    Some(id)
                }
                _ => None,
            };
            memo.push(m);
        }
        self.line();
        self.w.map(c.span.start);
        // `bind:this` on a component binds its exports (Svelte: `$.bind_this(Child(...), set, get)`)
        let this_bind = c.props.iter().find_map(|p| match p {
            ComponentProp::Bind { name, target } if name == "this" => Some(*target),
            _ => None,
        });
        let dynamic_get = if c.this.is_some() { None } else { c.node.and_then(|n| self.rw.dynamic_component(n)) };
        let dynamic = c.this.is_some() || dynamic_get.is_some();
        // a dynamic component binds the exports of each instance inside its branch: cleared when
        // the branch goes, bound again by the next component (Svelte binds inside `$.component`)
        let open = if dynamic && this_bind.is_some() { "$n.component" } else { "$n.snippet" };
        if this_bind.is_some() && !dynamic {
            self.w.write("$.bind_this(");
        }
        if let Some(this) = c.this {
            // `<svelte:component this>`: re-created when the function changes, nothing if nullish
            self.w.write(&format!("{open}({container}, () => "));
            self.paren_expr(this, Mode::Read);
            self.w.write(", ");
        } else {
            match dynamic_get {
                // re-created when the component function changes (Svelte's `$.component`)
                Some(get) => self.w.write(&format!("{open}({container}, () => {get}, ")),
                None => self.w.write(&format!("{}($n.region({container}), ", c.name)),
            }
        }
        let spread = c.props.iter().any(|p| matches!(p, ComponentProp::Spread(_)));
        if spread {
            self.w.write("$.spread_props(");
        }
        // `$.spread_props(obj, () => spread, obj, ...)` / a single object literal
        let mut st = PropsState { open: false, first_arg: true, first_entry: true };
        for (i, p) in c.props.iter().enumerate() {
            if matches!(p, ComponentProp::Bind { name, .. } if name == "this") {
                continue;
            }
            if let ComponentProp::Spread(e) = p {
                if st.open {
                    self.w.write(" }");
                    st.open = false;
                }
                if !st.first_arg {
                    self.w.write(", ");
                }
                st.first_arg = false;
                if self.rw.is_reactive(e) {
                    self.w.write("() => ");
                    self.paren_expr(e, Mode::Read);
                } else {
                    self.expr_text(e, Mode::Read);
                }
                continue;
            }
            self.prop_entry(&mut st);
            match p {
                ComponentProp::Static { name, value } => {
                    self.w.write(&format!("{}: ", js_key(name)));
                    self.json(value);
                }
                ComponentProp::Dynamic { name, value } => {
                    self.w.write(&format!("get {}() {{ return ", js_key(name)));
                    match &memo[i] {
                        Some(id) => self.w.write(&format!("$.get({id})")),
                        None => self.dynamic(value),
                    }
                    self.w.write("; }");
                }
                ComponentProp::Bind { name, target } if function_binding(target).is_some() => {
                    let (get, set) = function_binding(target).unwrap();
                    self.w.write(&format!("get {}() {{ return (", js_key(name)));
                    self.paren_expr(get, Mode::Read);
                    self.w.write(&format!(")(); }}, set {}($$value) {{ (", js_key(name)));
                    self.paren_expr(set, Mode::Read);
                    self.w.write(")($$value); }");
                }
                ComponentProp::Bind { name, target } => {
                    self.w.write(&format!("get {}() {{ return ", js_key(name)));
                    self.expr_text(target, Mode::Read);
                    let set = self.rw.assign(target, "$$value");
                    self.w.write(&format!("; }}, set {}($$value) {{ {set}; }}", js_key(name)));
                }
                ComponentProp::Spread(_) => unreachable!(),
            }
        }
        let mut slots = Vec::new();
        for s in &c.snippets {
            self.prop_entry(&mut st);
            self.w.write(&format!("{}: {}", js_key(&s.name), s.name));
            slots.push(if s.name == "children" { "default".to_string() } else { s.name.clone() });
        }
        if let Some(ch) = &c.children {
            self.prop_entry(&mut st);
            self.w.write("children: ($$anchor, $$slotProps) => {");
            self.w.indent();
            if boundary {
                self.consts(&c.consts);
            }
            self.fragment(ch, "$$anchor");
            self.w.dedent();
            self.line();
            self.w.write("}");
            slots.push("default".to_string());
        }
        if !slots.is_empty() {
            self.prop_entry(&mut st);
            let s: Vec<String> = slots.iter().map(|s| format!("{}: true", js_key(s))).collect();
            self.w.write(&format!("$$slots: {{ {} }}", s.join(", ")));
        }
        if st.open {
            self.w.write(" }");
        } else if st.first_arg {
            self.w.write("{}");
        }
        if spread {
            self.w.write(")");
        }
        if !(dynamic && this_bind.is_some()) {
            self.w.write(")");
        }
        if let Some(target) = this_bind {
            if dynamic {
                self.w.write(", ($$exports) => $.bind_this($$exports");
            }
            match function_binding(target) {
                Some((get, set)) => {
                    self.w.write(", ");
                    self.paren_expr(set, Mode::Read);
                    self.w.write(", ");
                    self.paren_expr(get, Mode::Read);
                }
                None => {
                    let set = self.rw.assign(target, "$$value");
                    self.w.write(&format!(", ($$value) => {{ {set}; }}, () => "));
                    self.expr_text(target, Mode::Read);
                }
            }
            // closes `$.bind_this(`, and for a dynamic component also `$n.component(`
            self.w.write(if dynamic { "))" } else { ")" });
        }
        self.w.write(";");
        if scoped {
            self.w.dedent();
            self.line();
            self.w.write("}");
        }
    }

    fn prop_entry(&mut self, st: &mut PropsState) {
        if !st.open {
            if !st.first_arg {
                self.w.write(", ");
            }
            self.w.write("{");
            st.open = true;
            st.first_arg = false;
            st.first_entry = true;
        }
        if !st.first_entry {
            self.w.write(",");
        }
        self.w.write(" ");
        st.first_entry = false;
    }
}

/// Svelte creates a fragment's render effects after its blocks and components (one
/// `$.template_effect` at the end of the fragment), so a parent's template expressions run after
/// its children's components initialised (Svelte's `lifecycle-render-order-for-children*`).
/// Off: deferring moves every reactive prop write after the element's insertion, which changes
/// the GTK/Qt op logs (`experiments/native-compiler/compiled` expected logs and hand-written
/// references); enabling it is a decision for the op-log owners (deviations.md).
const DEFER_TEMPLATE_EFFECTS: bool = false;

/// Whether an element's children are emitted in their own JS block (they declare snippets or
/// `{@const}`s).
fn scoped(f: &Fragment) -> bool {
    !f.snippets.is_empty() || !f.consts.is_empty()
}

/// `bind:x={get, set}`: the getter and setter of a function binding.
fn function_binding<'e>(target: &'e Expression) -> Option<(&'e Expression, &'e Expression)> {
    match target {
        Expression::SequenceExpression(s) if s.expressions.len() == 2 => Some((&s.expressions[0], &s.expressions[1])),
        _ => None,
    }
}

struct PropsState {
    open: bool,
    first_arg: bool,
    first_entry: bool,
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
