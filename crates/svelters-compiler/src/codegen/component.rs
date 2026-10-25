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
use crate::semantics::ChangeClass;
use serde_json::Value as Json;
use std::collections::{BTreeMap, HashMap};

/// W-C2: `Item::Template` emission.
mod template;

/// What codegen emits beyond today's ops, from the backend's capabilities (presto
/// `compiler-and-js.md` §3). The default is today's output; template items come from the
/// lowering (`lower::templates`), which makes them only for a backend with `templates`.
#[derive(Default)]
pub struct Features<'s> {
    /// W-C1 (`ids`): element kinds and prop writes carry the spec's stable ids.
    pub ids: Option<&'s dyn IdSource>,
    /// W-C4 (`change_classes`): reactive writes carry their change class.
    pub change_classes: bool,
}

/// W-C1: the stable ids codegen writes, answered from the backend spec.
pub trait IdSource {
    fn backend(&self) -> &str;
    fn spec_hash(&self) -> &str;
    /// The kind id of the elements `tag` creates, and the kind's name (for the constant's name).
    fn kind(&self, tag: &str) -> Option<(u32, String)>;
    /// The id of prop `prop` (its canonical JS name) of elements `tag`, and its declaring kind.
    fn prop(&self, tag: &str, prop: &str) -> Option<(u32, String)>;
}

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
    /// Whether `e` reads a store subscription (`$store`): an `{#each}` over it gets mutable items.
    fn uses_store(&self, _e: &Expression) -> bool {
        false
    }
    /// Svelte's `ExpressionMetadata.has_call`: `e` calls something that is not a pure global (or
    /// calls anything while it references bindings), outside nested functions. Such template
    /// expressions are memoized in a derived (`Memoizer`), so they run once per change of what they
    /// read, not whenever a sibling expression of the same effect changes.
    fn has_call(&self, _e: &Expression) -> bool {
        false
    }
    /// JS that assigns the JS expression text `value` to the lvalue `target` (bind: setters).
    fn assign(&self, target: &Expression, value: &str) -> String;
    /// The read of an identifier of the template (`{@debug name}`).
    fn identifier(&self, id: &crate::estree::Identifier) -> String {
        id.name.clone()
    }
    /// A declaration tag's `let`/`const` declaration, transformed as in a script (runes become
    /// runtime calls), without a trailing semicolon.
    fn declaration(&self, _d: &crate::estree::VariableDeclaration) -> String {
        String::new()
    }
    /// The JS that reads a directive name (`use:name`, possibly dotted), which the parser keeps
    /// as text rather than an expression.
    fn directive_name(&self, name: &str) -> String {
        name.to_string()
    }
    /// Svelte's `extract_paths(pattern, initial)` for a destructuring template binding (snippet
    /// parameter, `{#each}` context, `{@const}`, `{:then}`/`{:catch}`), as JS over the JS text
    /// `initial`: default values and computed keys are rewritten like any template expression.
    /// `derived_arrays`: the `$$array` intermediates are read as deriveds (`$.get`), else as plain
    /// variables.
    fn destructure(&self, _pattern: &Pattern, _initial: &str, _derived_arrays: bool) -> Destructured {
        Destructured::default()
    }
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
    /// Whether the component uses `$inspect.trace` in dev: the module first imports
    /// `<core>/tracing`, which turns on the dependency recording `$.trace` reports.
    fn tracing(&self) -> bool {
        false
    }
    /// `--hmr`: the component's `$state` signature as a JS literal (`"<hash>"`, or `null` for
    /// `// @hmr:reset`); `None` when the component has no keyed declarations.
    fn hmr_signature(&self) -> Option<String> {
        None
    }
    /// Svelte's `dev` option: snippets get Svelte's dev wrapper (`$.wrap_snippet`, argument
    /// validation, no stringification).
    fn dev(&self) -> bool {
        false
    }
    /// Dev: `file:line:column` of a source offset (1-based line, 0-based column, Svelte's
    /// `locator`), for runtime warnings; None outside dev.
    fn dev_location(&self, _pos: u32) -> Option<String> {
        None
    }
    /// MOTION-CSS (`native_transitions`): the svelte/transition built-in (`fade`, `fly`, `slide`,
    /// `scale`, `blur`, `draw`) a `transition:`/`in:`/`out:` name is imported as, emitted as
    /// `$n.transition_native`; `None` for anything else (`$n.transition`).
    fn native_transition(&self, _name: &str) -> Option<&'static str> {
        None
    }
    /// MOTION-CSS (`native_transitions`): an `animate:` name is svelte/animate's `flip`, emitted as
    /// `$n.flip_native`.
    fn native_flip(&self, _name: &str) -> bool {
        false
    }
    /// Ends the component body (`$.pop()`; store cleanup / `return $.pop($$exports)`).
    fn epilogue(&self, w: &mut Writer) {
        w.write("$.pop();");
    }
}

/// The result of [`Rewriter::destructure`].
#[derive(Debug, Default)]
pub struct Destructured {
    /// `$$array` intermediates of array patterns: (name, value); declared as deriveds, read `$.get(name)`.
    pub arrays: Vec<(String, String)>,
    /// Every leaf: (name, expression, has a default value).
    pub leaves: Vec<(String, String, bool)>,
}

/// How [`Codegen::destructure`] declares the leaves of a pattern, matching how the transform reads
/// the binding kind: snippet parameters are thunks (`a()`), every other template binding a source
/// (`$.get(a)`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Leaf {
    Thunk,
    Source,
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
    /// The component function name (`$.wrap_snippet(Name, ...)` in dev).
    name: &'s str,
    /// CX3: the component's draft bindings (`Component::drafts`).
    drafts: Vec<(usize, Draft<'a>)>,
    /// DZ0: in a dev build for a backend with the `source-loc` prop, whether a kind takes it. Its
    /// elements get `$n.source_loc(node, $$src, offset)` and component calls `$n.region(container,
    /// $$src, offset)`, so the runtime can tell each native node where in this file it was declared
    /// and through which component call sites it was rendered.
    locs: Option<&'s dyn Fn(&str) -> bool>,
    /// DZ0: some code read `$$src` (the module then declares it).
    src_used: bool,
    features: &'s Features<'s>,
    /// W-C1: the id constants the module declares: name -> (id, tag, prop).
    id_consts: BTreeMap<String, (u32, String, Option<String>)>,
    /// W-C1: the tag of each element variable (prop ids depend on the element's kind).
    node_tags: HashMap<String, String>,
    /// W-C2: the template tables the module declares, by template number.
    tables: Vec<template::Table>,
    _p: std::marker::PhantomData<&'a ()>,
}

/// Generates the whole module. `name` is the component function name.
#[allow(clippy::too_many_arguments)]
pub fn generate(src: &str, comp: &Component, rw: &dyn Rewriter, name: &str, filename: &str, imports: &crate::target::RuntimeImports, hmr: bool, locs: Option<&dyn Fn(&str) -> bool>, features: &Features) -> (String, String) {
    let drafts = comp.drafts.clone();
    let mut g = Codegen::new(src, rw, Writer::new(src), name, drafts, locs, features);
    g.module(comp, imports, hmr.then_some(filename));
    if g.src_used {
        // `var`: hoisted, and assigned before any component function of the module runs
        g.w.write(&format!("\nvar $$src = {};\n", js_str(filename)));
    }
    let out = filename.rsplit('/').next().unwrap_or(filename).replace(".svelte", ".js");
    g.w.finish(filename, src, &out)
}

/// [`generate`]'s traversal with no output (CX1's diagnostics-only check): the rune transform
/// reports some diagnostics (assignments to constants, invalid bindings, ...) while it rewrites
/// the template's expressions, so the walk runs, but nothing is written and no source map is built.
pub fn traverse(src: &str, comp: &Component, rw: &dyn Rewriter, name: &str, imports: &crate::target::RuntimeImports) {
    let drafts = comp.drafts.clone();
    let features = Features::default();
    let mut g = Codegen::new(src, rw, Writer::sink(), name, drafts, None, &features);
    g.module(comp, imports, None);
}

impl<'a, 's> Codegen<'a, 's> {
    fn new(src: &'s str, rw: &'s dyn Rewriter, w: Writer<'s>, name: &'s str, drafts: Vec<(usize, Draft<'a>)>, locs: Option<&'s dyn Fn(&str) -> bool>, features: &'s Features<'s>) -> Self {
        Codegen {
            src,
            rw,
            w,
            tmp: 0,
            root_fragment: false,
            memo: Vec::new(),
            name,
            drafts,
            locs,
            src_used: false,
            features,
            id_consts: BTreeMap::new(),
            node_tags: HashMap::new(),
            tables: Vec::new(),
            _p: std::marker::PhantomData,
        }
    }
}

impl<'a> Codegen<'a, '_> {
    /// CX3: the draft binding whose bound expression is `target`, if it is one.
    fn draft(&self, target: &Expression) -> Option<Draft<'a>> {
        let at = target as *const Expression as usize;
        self.drafts.iter().find(|(k, _)| *k == at).map(|(_, d)| d.clone())
    }
}

impl Codegen<'_, '_> {
    /// The whole module: imports, hoisted snippets, the component function.
    fn module(&mut self, comp: &Component, imports: &crate::target::RuntimeImports, hmr_id: Option<&str>) {
        let (rw, name) = (self.rw, self.name);
        let g = self;
        if rw.tracing() {
            g.w.write(&format!("import '{}/tracing';\n", imports.core));
        }
        g.w.write(&format!("import * as $ from '{}';\nimport * as $n from '{}';\n", imports.core, imports.native));
        if comp.kinds {
            // the spec tables of runtime-resolved attributes (spreads, <svelte:element>)
            g.w.write(&format!("import * as $$kinds from '{}/kinds';\n", imports.native));
        }
        // CX3: the modules of draft bind sources (their `get`/`set` exports)
        for (i, m) in comp.bind_imports.iter().enumerate() {
            g.w.write(&format!("import * as $$bind_source_{i} from {};\n", js_str(m)));
        }
        rw.script(&mut g.w, true);
        let (hoisted, instance): (Vec<_>, Vec<_>) = comp.root.snippets.iter().partition(|s| rw.hoist_snippet(s.span.start));
        for s in hoisted {
            g.snippet(s);
            g.w.newline();
        }
        g.w.write(&format!("\n{}function {name}($$anchor, $$props) {{", if hmr_id.is_some() { "" } else { "export default " }));
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
        g.module_tables();
        if let Some(id) = hmr_id {
            let id = serde_json::to_string(id).expect("string serializes");
            let sig = rw.hmr_signature().map_or(String::new(), |s| format!(", {s}"));
            g.w.write(&format!(
                "\nif (import.meta.hot) {{\n\t{name} = $n.hmr({name}, {id}{sig});\n\timport.meta.hot.acceptExports([\"default\"], (m) => m && {name}[$n.HMR].update(m.default));\n}}\n\nexport default {name};\n"
            ));
        }
    }
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

    /// The body of an arrow `() => e`: an object literal is parenthesized (`{#each { length: 5 }}`),
    /// else it would be read as a block.
    fn thunk_body(&mut self, e: &Expression) {
        if matches!(e, Expression::ObjectExpression(_)) {
            self.w.write("(");
            self.expr_text(e, Mode::Read);
            self.w.write(")");
        } else {
            self.expr_text(e, Mode::Read);
        }
    }

    /// `get, set` of a binding: the functions of `bind:x={get, set}` (Svelte >= 5.9), else accessors
    /// of the bound lvalue.
    fn bind_accessors(&mut self, target: &Expression) {
        if let Some(d) = self.draft(target) {
            self.w.write("() => ");
            self.draft_call(&d, &d.get, false);
            self.w.write(", ($$value) => ");
            self.draft_call(&d, &d.set, true);
            return;
        }
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
        if let Some(d) = self.draft(target) {
            self.w.write("($$value) => ");
            self.draft_call(&d, &d.set, true);
            return;
        }
        if let Some((_, set)) = function_binding(target) {
            self.paren_expr(set, Mode::Read);
        } else {
            let set = self.rw.assign(target, "$$value");
            self.w.write(&format!("($$value) => {{ {set}; }}"));
        }
    }

    /// CX3: `$$bind_source_<ns>.<f>(...args[, $$value])`.
    fn draft_call(&mut self, d: &Draft<'_>, f: &str, value: bool) {
        let ident = f.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$') && f.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
        let member = if ident { format!(".{f}") } else { format!("[{}]", js_str(f)) };
        self.w.write(&format!("$$bind_source_{}{member}(", d.ns));
        for (i, a) in d.args.iter().enumerate() {
            if i > 0 {
                self.w.write(", ");
            }
            self.expr_text(a, Mode::Read);
        }
        if value {
            self.w.write(if d.args.is_empty() { "$$value" } else { ", $$value" });
        }
        self.w.write(")");
    }

    fn json(&mut self, v: &Json) {
        self.w.write(&v.to_string());
    }


    /// `{@const}` and snippet declarations of a fragment (before its items).
    fn decls(&mut self, f: &Fragment) {
        self.consts(&f.consts);
        for s in &f.snippets {
            self.snippet(s);
        }
    }

    /// A fragment as Svelte's Fragment visitor builds it: the items in place (`init`: elements with
    /// their static writes, blocks, components), then the fragment's single `$.template_effect`
    /// with every reactive write of its elements (`update`), then their events and bindings
    /// (`after_update`) (see [`Frag`]).
    fn fragment(&mut self, f: &Fragment, container: &str) {
        if std::mem::take(&mut self.root_fragment) {
            self.consts(&f.consts);
        } else {
            self.decls(f);
        }
        let mut fr = Frag::default();
        for item in &f.items {
            self.item(item, container, &mut fr);
        }
        self.debugs(f);
        self.close(fr);
    }

    /// The end of a fragment: its template effect, then its events and bindings.
    fn close(&mut self, fr: Frag) {
        self.template_effect(&fr.update);
        let mut i = 0;
        while i < fr.after.len() {
            let a = &fr.after[i];
            match a.op {
                Op::Listen { .. } => {
                    // W-C2: a template's listeners, when they come in one run, in one call
                    if let Some(t) = a.template
                        && let Some(run) = self.template_listen_run(&fr.after, i, t)
                    {
                        self.template_listen(t, &fr.after[i..i + run]);
                        i += run;
                        continue;
                    }
                    self.line();
                    self.listen(&a.node, a.op, a.memo.as_deref());
                }
                op => self.op(&a.node, op),
            }
            i += 1;
        }
    }

    /// `{@debug}` tags of a fragment (Svelte's DebugTag visitor).
    fn debugs(&mut self, f: &Fragment) {
        for d in &f.debugs {
            self.line();
            self.w.map(d.start);
            let props: Vec<String> = d.identifiers.iter().map(|i| format!("{}: $.snapshot({})", i.name, self.rw.identifier(i))).collect();
            let object = if props.is_empty() { "{}".to_string() } else { format!("{{ {} }}", props.join(", ")) };
            self.w.write(&format!("$.template_effect(() => {{ console.log({object}); debugger; }});"));
        }
    }

    fn item<'f>(&mut self, item: &'f Item<'f>, container: &str, fr: &mut Frag<'f>) {
        match item {
            Item::Element(e) => self.element(e, container, fr),
            Item::If(b) => self.if_block(b, container),
            Item::Each(b) => self.each_block(b, container),
            Item::Key(b) => self.key_block(b, container),
            Item::Await(b) => self.await_block(b, container),
            Item::Render(r) => self.render(r, container),
            Item::Component(c) => self.component(c, container),
            Item::Dynamic(d) => self.dynamic_element(d, container),
            Item::Template(t) => self.template_item(t, container, fr),
        }
    }

    /// `<svelte:element this={tag}>`, as Svelte's SvelteElement visitor builds the element's
    /// callback: its `init` (the spread's attribute effect, attachments, actions and `bind:this`,
    /// in attribute order), then its own template effect (reactive text of a backend without
    /// text nodes) and events and bindings, then its body, which is a fragment of its own.
    fn dynamic_element<'f>(&mut self, d: &'f DynamicElement<'f>, container: &str) {
        let n = format!("$$n{}", d.id + 1);
        self.line();
        self.w.map(d.span.start);
        self.w.write(&format!("$n.dynamic_element({container}, () => "));
        self.paren_expr(d.tag, Mode::Read);
        self.w.write(&format!(", ({n}) => {{"));
        self.w.indent();
        let mut own = Frag::default();
        for op in &d.ops {
            match self.phase(op) {
                Phase::Init | Phase::ElementInit => self.op(&n, op),
                Phase::Update => {
                    self.update_prefix(&n, op);
                    own.update.push((n.clone(), op));
                }
                Phase::Event => {
                    let memo = self.handler_memo(op);
                    own.after.push(After { node: n.clone(), op, memo, template: None });
                }
                Phase::Binding => own.after.push(After { node: n.clone(), op, memo: None, template: None }),
            }
        }
        self.close(own);
        self.decls(&d.children);
        let mut body = Frag::default();
        self.children(&d.children, &n, &mut body);
        self.close(body);
        self.w.dedent();
        self.line();
        self.w.write("}, $$kinds);");
    }

    /// The object of an `Op::Spread`, in source order; inline handler functions are hoisted
    /// before the call (`hoisted`), so a re-run passes the same function.
    fn spread_object(&mut self, entries: &[SpreadEntry], hoisted: &[(usize, String)]) {
        self.w.write("({");
        let mut first = true;
        let mut sep = |w: &mut Writer| {
            w.write(if first { " " } else { ", " });
            first = false;
        };
        for (i, e) in entries.iter().enumerate() {
            match e {
                SpreadEntry::Attr { key, value } => {
                    sep(&mut self.w);
                    self.w.write(&format!("{}: ", js_key(key)));
                    match value {
                        AttrValue::Static(j) => self.json(j),
                        AttrValue::Dynamic(d) => self.dynamic(d),
                    }
                }
                SpreadEntry::Event { key, handler } => {
                    sep(&mut self.w);
                    self.w.write(&format!("{}: ", js_key(key)));
                    match hoisted.iter().find(|(j, _)| *j == i) {
                        Some((_, id)) => self.w.write(id),
                        None => self.paren_expr(handler, Mode::Read),
                    }
                }
                // FW1: the component's own callback prop, re-read with the spread object
                SpreadEntry::Forward { key } => {
                    sep(&mut self.w);
                    self.w.write(&format!("{}: {}", js_key(key), props_member(key)));
                }
                SpreadEntry::Spread(e) => {
                    sep(&mut self.w);
                    match self.memoized(e) {
                        Some(id) => self.w.write(&format!("...{id}")),
                        None => {
                            self.w.write("...(");
                            self.expr_text(e, Mode::Read);
                            self.w.write(")");
                        }
                    }
                }
                SpreadEntry::Class { .. } | SpreadEntry::Style { .. } => {}
            }
        }
        for (sym, class) in [("$n.CLASS", true), ("$n.STYLE", false)] {
            let list: Vec<&SpreadEntry> = entries.iter().filter(|e| if class { matches!(e, SpreadEntry::Class { .. }) } else { matches!(e, SpreadEntry::Style { .. }) }).collect();
            if list.is_empty() {
                continue;
            }
            sep(&mut self.w);
            self.w.write(&format!("[{sym}]: {{ "));
            for (k, e) in list.iter().enumerate() {
                if k > 0 {
                    self.w.write(", ");
                }
                match e {
                    SpreadEntry::Class { name, expr } => {
                        self.w.write(&format!("{}: ", js_key(name)));
                        match self.memoized(expr) {
                            Some(id) => self.w.write(&id),
                            None => self.paren_expr(expr, Mode::Read),
                        }
                    }
                    SpreadEntry::Style { name, value } => {
                        self.w.write(&format!("{}: ", js_key(name)));
                        match value {
                            AttrValue::Static(j) => self.json(j),
                            AttrValue::Dynamic(d) => self.dynamic(d),
                        }
                    }
                    _ => {}
                }
            }
            self.w.write(" }");
        }
        self.w.write(if first { "})" } else { " })" });
    }

    /// An element, as Svelte's RegularElement visitor orders it: its static writes now, its
    /// reactive writes into the fragment's template effect and its events after it (both before
    /// its children's), then the children, then its actions, attachments and `bind:this`, and its
    /// bindings after the template effect (after its children's). Children that declare snippets
    /// or `{@const}`s get a JS block of their own, which is a fragment of its own (Svelte: a block
    /// with its own memoizer and template effect).
    fn element<'f>(&mut self, e: &'f Element<'f>, container: &str, fr: &mut Frag<'f>) {
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
            let kind = self.kind_ref(&e.kind);
            self.w.write(&format!("const {n} = {kind};"));
        }
        self.node_tags.insert(n.clone(), e.kind.clone());
        if self.locs.is_some_and(|has| has(&e.kind)) {
            self.src_used = true;
            self.w.write(&format!(" $n.source_loc({n}, $$src, {});", e.span.start));
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
        let mut own_init = Vec::new();
        let mut own_after = Vec::new();
        for op in &e.ops {
            match self.phase(op) {
                Phase::Init => self.op(&n, op),
                Phase::Update => {
                    self.update_prefix(&n, op);
                    fr.update.push((n.clone(), op));
                }
                Phase::Event => {
                    let memo = self.handler_memo(op);
                    fr.after.push(After { node: n.clone(), op, memo, template: None });
                }
                Phase::ElementInit => own_init.push(op),
                Phase::Binding => own_after.push(op),
            }
        }
        // (a toplevel appended at the root is shown when the outermost build returns: the
        // runtime's `showTop`, so after its fragment's template effect, in document order)
        let append = |g: &mut Self| {
            g.line();
            g.w.map(e.span.start);
            g.w.write(&format!("$n.append({container}, {n});"));
        };
        if scoped(&e.children) {
            self.line();
            self.w.write("{");
            self.w.indent();
            self.decls(&e.children);
            let mut inner = Frag::default();
            self.children(&e.children, &n, &mut inner);
            self.element_init(&n, &own_init);
            append(self);
            inner.after.extend(own_after.into_iter().map(|op| After { node: n.clone(), op, memo: None, template: None }));
            self.close(inner);
            self.w.dedent();
            self.line();
            self.w.write("}");
        } else {
            self.children(&e.children, &n, fr);
            self.element_init(&n, &own_init);
            append(self);
            fr.after.extend(own_after.into_iter().map(|op| After { node: n.clone(), op, memo: None, template: None }));
        }
    }

    /// The items of an element's children, each placed in its slot of `parent`.
    fn children<'f>(&mut self, f: &'f Fragment<'f>, parent: &str, fr: &mut Frag<'f>) {
        for item in &f.items {
            let c = format!("$n.slot({parent}, {})", item.slot().as_deref().map_or("null".to_string(), js_str));
            self.item(item, &c, fr);
        }
        self.debugs(f);
    }

    fn element_init(&mut self, n: &str, ops: &[&Op]) {
        for op in ops {
            self.op(n, op);
        }
    }

    /// Where an op goes in its fragment (Svelte's `init` / `update` / `after_update`).
    fn phase(&self, op: &Op) -> Phase {
        match op {
            Op::SetDynamic { value, .. } | Op::ChildAttr { value: AttrValue::Dynamic(value), .. } | Op::WidgetName { value: AttrValue::Dynamic(value), .. } | Op::Text { value: AttrValue::Dynamic(value), .. } => {
                if self.dynamic_reactive(value) { Phase::Update } else { Phase::Init }
            }
            // Svelte's build_attribute_effect: a spread's `$.attribute_effect` is an effect of its
            // own, written with the element (`init`), not part of the fragment's template effect
            Op::Spread { .. } => Phase::Init,
            Op::Token { cond, .. } if !cond.is_empty() => Phase::Update,
            Op::Classes { dynamic, toggles, .. } if dynamic.is_some() || !toggles.is_empty() => Phase::Update,
            Op::InlineStyle { dynamic, directives, .. } if dynamic.is_some() || !directives.is_empty() => Phase::Update,
            Op::StyleProp { .. } | Op::StyleSetter { .. } => Phase::Update,
            Op::Listen { .. } => Phase::Event,
            Op::ListenAttach { .. } | Op::Attach { .. } | Op::Action { .. } | Op::BindThis { .. } => Phase::ElementInit,
            Op::Bind { .. } | Op::BindProp { .. } | Op::BindReadonly { .. } | Op::BindOnce { .. } | Op::BindCommit { .. } | Op::BindLink { .. } => Phase::Binding,
            // Svelte's TransitionDirective / AnimateDirective push to the element's `after_update`,
            // with its bindings, in attribute order
            Op::Transition { .. } | Op::Animate { .. } => Phase::Binding,
            _ => Phase::Init,
        }
    }

    /// The fragment's single `$.template_effect` over its reactive writes, in element order. Call
    /// expressions are its memoized dependencies (Svelte's `Memoizer`):
    /// `$.template_effect(($0) => .., [() => f(x)])` runs `f(x)` in a derived of its own, so it is
    /// not called again when only another value of the fragment changes.
    fn template_effect(&mut self, update: &[(String, &Op)]) {
        if update.is_empty() {
            return;
        }
        let mut memo: Vec<&Expression> = Vec::new();
        for (_, op) in update {
            self.memo_exprs(op, &mut memo);
        }
        self.memo = memo.iter().enumerate().map(|(i, e)| (*e as *const Expression as usize, format!("${i}"))).collect();
        let params: Vec<String> = (0..memo.len()).map(|i| format!("${i}")).collect();
        self.line();
        self.w.write(&format!("$.template_effect(({}) => ", params.join(", ")));
        if let [(n, op)] = update {
            self.update(n, op);
        } else {
            self.w.write("{");
            self.w.indent();
            for (n, op) in update {
                self.line();
                self.update(n, op);
                self.w.write(";");
            }
            self.w.dedent();
            self.line();
            self.w.write("}");
        }
        self.memo.clear();
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
        self.w.write(");");
    }

    /// The call expressions an update op memoizes (Svelte's `has_call`), each once.
    fn memo_exprs<'e>(&self, op: &'e Op, out: &mut Vec<&'e Expression>) {
        let mut add = |e: &'e Expression, out: &mut Vec<&'e Expression>| {
            if self.rw.has_call(e) && !out.iter().any(|x| std::ptr::eq(*x, e)) {
                out.push(e);
            }
        };
        let dynamic = |d: &'e Dynamic, out: &mut Vec<&'e Expression>, add: &mut dyn FnMut(&'e Expression, &mut Vec<&'e Expression>)| match d {
            Dynamic::Expr(e) => add(e, out),
            Dynamic::Template(parts) => {
                for p in parts {
                    if let TemplatePart::Expr(e) = p {
                        add(e, out);
                    }
                }
            }
            Dynamic::Member { .. } => {}
        };
        let cond = |c: &'e Cond, out: &mut Vec<&'e Expression>, add: &mut dyn FnMut(&'e Expression, &mut Vec<&'e Expression>)| {
            for a in c {
                if let CondAtom::Expr(e) = a {
                    add(e, out);
                }
            }
        };
        match op {
            Op::SetDynamic { value, .. } | Op::ChildAttr { value: AttrValue::Dynamic(value), .. } | Op::WidgetName { value: AttrValue::Dynamic(value), .. } | Op::Text { value: AttrValue::Dynamic(value), .. } => {
                dynamic(value, out, &mut add)
            }
            Op::Classes { toggles, .. } => {
                for (_, e) in toggles {
                    add(e, out);
                }
            }
            Op::InlineStyle { dynamic: d, directives, .. } => {
                if let Some(d) = d {
                    dynamic(d, out, &mut add);
                }
                for (_, v) in directives {
                    if let AttrValue::Dynamic(d) = v {
                        dynamic(d, out, &mut add);
                    }
                }
            }
            Op::StyleSetter { value, fallback: (candidates, _), .. } => {
                dynamic(value, out, &mut add);
                for (c, _) in candidates {
                    cond(c, out, &mut add);
                }
            }
            Op::Token { cond: c, .. } => cond(c, out, &mut add),
            Op::StyleProp { candidates, .. } => {
                for (c, _) in candidates {
                    cond(c, out, &mut add);
                }
            }
            _ => {}
        }
    }

    /// Svelte's build_event_handler: a handler expression that calls something is memoized
    /// (`onclick={make(x)}` runs `make` once per change, not once per event). The derived is
    /// declared with the element (`init`), the listener comes after the template effect.
    fn handler_memo(&mut self, op: &Op) -> Option<String> {
        match op {
            Op::Listen { handler: Handler::Expr(h), .. } if !is_function(h) && self.rw.has_call(h) => {
                self.tmp += 1;
                let id = format!("$$handler{}", self.tmp);
                self.line();
                self.w.write(&format!("const {id} = $.derived(() => "));
                self.paren_expr(h, Mode::Read);
                self.w.write(");");
                Some(id)
            }
            _ => None,
        }
    }

    /// `$n.on(node, signal, handler[, flags]);` of an `Op::Listen` (`memo`: its handler derived).
    fn listen(&mut self, n: &str, op: &Op, memo: Option<&str>) {
        let Op::Listen { signal, flags, span, .. } = op else { unreachable!() };
        self.w.map(span.start);
        self.w.write(&format!("$n.on({n}, {}, ", js_str(signal)));
        self.listener(op, memo);
        if *flags != 0 {
            self.w.write(&format!(", {flags}"));
        }
        self.w.write(");");
    }

    /// The handler function of an `Op::Listen` (`memo`: its handler derived).
    fn listener(&mut self, op: &Op, memo: Option<&str>) {
        let Op::Listen { signal, handler, flags, .. } = op else { unreachable!() };
        match handler {
            Handler::Expr(h) => {
                // the event as Svelte's `$.apply` names it: type, plus `capture`
                let event = if flags & 2 != 0 { format!("{signal}capture") } else { signal.clone() };
                self.handler(h, memo, &event)
            }
            Handler::Member { object, key } => {
                // a property of a spread object, read when the event fires
                self.w.write("function (...$$args) { ");
                self.paren_expr(object, Mode::Read);
                self.w.write(&format!("[{}]?.apply(this, $$args); }}", js_str(key)));
            }
            // FW1: the component's own callback prop, read when the event fires (Svelte's
            // handler for `onx={$$props.onx}`)
            Handler::Forward { key } => {
                self.w.write(&format!("function (...$$args) {{ {}?.apply(this, $$args); }}", props_member(key)));
            }
        }
    }

    /// W-C1: the creation of an element `tag`: `$n.element("tag")`, or with ids
    /// `$n.element_id($$K_Kind)`.
    fn kind_ref(&mut self, tag: &str) -> String {
        match self.features.ids.and_then(|ids| ids.kind(tag)) {
            Some((id, kind)) => {
                let c = self.id_const(format!("$$K_{}", ident(&kind)), id, tag, None);
                format!("$n.element_id({c})")
            }
            None => format!("$n.element({})", js_str(tag)),
        }
    }

    /// W-C1: the head of a prop write on element variable `n`, up to its value:
    /// `$n.set(n, "prop", `, or with ids `$n.setp(n, $$P_Kind_prop, `.
    fn set_head(&mut self, n: &str, prop: &str) -> String {
        let found = self.features.ids.zip(self.node_tags.get(n).cloned()).and_then(|(ids, tag)| ids.prop(&tag, prop).map(|p| (p, tag)));
        match found {
            Some(((id, owner), tag)) => {
                let c = self.id_const(format!("$$P_{}_{}", ident(&owner), ident(prop)), id, &tag, Some(prop));
                format!("$n.setp({n}, {c}, ")
            }
            None => format!("$n.set({n}, {}, ", js_str(prop)),
        }
    }

    /// W-C1: declares the id constant `name` (a name already taken by another id gets the id
    /// appended) and returns its name.
    fn id_const(&mut self, name: String, id: u32, tag: &str, prop: Option<&str>) -> String {
        let name = match self.id_consts.get(&name) {
            Some((other, ..)) if *other != id => format!("{name}_{id}"),
            _ => name,
        };
        self.id_consts.entry(name.clone()).or_insert_with(|| (id, tag.to_string(), prop.map(str::to_string)));
        name
    }

    /// W-C4: `, <class code>` after a reactive write's value, with the `change_classes` feature
    /// and a known class.
    fn change_arg(&self, c: Option<ChangeClass>) -> String {
        match c {
            Some(c) if self.features.change_classes => format!(", {}", c.code()),
            _ => String::new(),
        }
    }

    /// The module-scope declarations codegen collected: W-C2's template tables, then W-C1's id
    /// constants and their check (`$n.ids`, which verifies them against the host once per module).
    fn module_tables(&mut self) {
        for t in std::mem::take(&mut self.tables) {
            self.w.write(&t.declaration());
        }
        let Some(ids) = self.features.ids else { return };
        if self.id_consts.is_empty() {
            return;
        }
        self.w.write("\n");
        let mut check = Vec::new();
        for (name, (id, tag, prop)) in &self.id_consts {
            self.w.write(&format!("const {name} = {id};\n"));
            check.push(match prop {
                Some(p) => format!("[{name}, {}, {}]", js_str(tag), js_str(p)),
                None => format!("[{name}, {}]", js_str(tag)),
            });
        }
        self.w.write(&format!("$n.ids({}, {}, [{}]);\n", js_str(ids.backend()), js_str(ids.spec_hash()), check.join(", ")));
    }

    /// The init statements of an update op, written with its element: the native value a
    /// style-driven write falls back to is captured before the template effect writes it.
    fn update_prefix(&mut self, n: &str, op: &Op) {
        match op {
            Op::StyleSetter { target, fallback: (_, Fallback::Restore), .. } => {
                self.line();
                self.w.write(&format!("$n.capture({n}, {});", js_str(target)));
            }
            Op::StyleProp { prop, fallback: Fallback::Restore, .. } => {
                self.line();
                self.w.write(&format!("$n.capture({n}, {});", js_str(prop)));
            }
            // the widget starts at the spec's native default: don't write it redundantly
            Op::StyleProp { prop, fallback: Fallback::Default(d), .. } if !d.is_null() => {
                self.line();
                self.w.write(&format!("$n.assume({n}, {}, {d});", js_str(prop)));
            }
            _ => {}
        }
    }

    /// The statement of an update op in the fragment's template effect (no `;`).
    fn update(&mut self, n: &str, op: &Op) {
        match op {
            Op::SetDynamic { prop, value, change, span } => {
                self.w.map(span.start);
                let head = self.set_head(n, prop);
                self.w.write(&head);
                self.dynamic(value);
                let change = self.change_arg(*change);
                self.w.write(&format!("{change})"));
            }
            Op::ChildAttr { name, value: AttrValue::Dynamic(d), span } => {
                self.w.map(span.start);
                self.w.write(&format!("$n.child_attr({n}, {}, ", js_str(name)));
                self.dynamic(d);
                self.w.write(")");
            }
            Op::WidgetName { value: AttrValue::Dynamic(d), span } => {
                self.w.map(span.start);
                self.w.write(&format!("$n.attr({n}, \"id\", "));
                self.dynamic(d);
                self.w.write(")");
            }
            Op::Text { value: AttrValue::Dynamic(d), span } => {
                self.w.map(span.start);
                self.w.write(&format!("$n.text({n}, $$kinds, "));
                self.dynamic(d);
                self.w.write(")");
            }
            Op::Classes { base, dynamic, toggles, change, span } => {
                self.w.map(span.start);
                self.w.write(&format!("$n.attr({n}, \"class\", $n.class_name({}, ", js_str(base)));
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
                    self.memo_or_paren(e);
                    self.w.write("]");
                }
                let change = self.change_arg(*change);
                self.w.write(&format!("]){change})"));
            }
            Op::InlineStyle { base, dynamic, directives, change, span } => {
                self.w.map(span.start);
                self.w.write(&format!("$n.attr({n}, \"style\", $n.style_text("));
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
                let change = self.change_arg(*change);
                self.w.write(&format!("]){change})"));
            }
            Op::StyleSetter { name, target, child_attr, backend, value, fallback: (candidates, fallback), span } => {
                self.w.map(span.start);
                let f = if *child_attr { "child_attr" } else { "set" };
                self.w.write(&format!("$n.{f}({n}, {}, $n.setter_value({}, {}, ", js_str(target), js_str(backend), js_str(name)));
                self.dynamic(value);
                self.w.write(")");
                if !candidates.is_empty() || *fallback != Fallback::Default(Json::Null) {
                    self.w.write(" ?? ");
                    self.candidates(candidates, fallback);
                }
                self.w.write(")");
            }
            Op::Token { prop, token, cond, span } => {
                self.w.map(span.start);
                self.w.write(&format!("$n.token({n}, {}, {}, ", js_str(prop), js_str(token)));
                self.cond(cond);
                self.w.write(")");
            }
            Op::StyleProp { prop, child_attr, candidates, fallback, guarded, span } => {
                self.w.map(span.start);
                let f = match (*child_attr, *guarded) {
                    (true, false) => "child_attr",
                    (false, false) => "set",
                    (true, true) => "rule_attr",
                    (false, true) => "rule_set",
                };
                self.w.write(&format!("$n.{f}({n}, {}, ", js_str(prop)));
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
            _ => unreachable!("not an update op"),
        }
    }

    /// `(e)`, or the effect parameter holding its memoized value.
    fn memo_or_paren(&mut self, e: &Expression) {
        match self.memoized(e) {
            Some(id) => self.w.write(&id),
            None => self.paren_expr(e, Mode::Read),
        }
    }

    fn op(&mut self, n: &str, op: &Op) {
        self.line();
        match op {
            Op::Set { prop, value, span } => {
                self.w.map(span.start);
                let head = self.set_head(n, prop);
                self.w.write(&head);
                self.json(value);
                self.w.write(");");
            }
            Op::SetDynamic { prop, value, span, .. } => {
                self.w.map(span.start);
                let head = self.set_head(n, prop);
                self.dynamic_call(&head, value);
            }
            Op::Listen { .. } => unreachable!("events are written after the template effect"),
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
            Op::Action { name, arg, span } => {
                self.w.map(span.start);
                let f = self.rw.directive_name(name);
                match arg {
                    Some(a) => {
                        self.w.write(&format!("$.action({n}, ($$node, $$action_arg) => {f}?.($$node, $$action_arg), () => "));
                        self.paren_expr(a, Mode::Read);
                        self.w.write(");");
                    }
                    None => self.w.write(&format!("$.action({n}, ($$node) => {f}?.($$node));")),
                }
            }
            // TR1: Svelte's `$.transition(flags, node, () => fn, () => params)` / `$.animation`, on the
            // runtime node (after `bind:this`, as Svelte's `after_update`)
            Op::Transition { flags, name, arg, span } => {
                self.w.map(span.start);
                // MOTION-CSS: a built-in the backend evaluates natively from its parameters
                if let Some(kind) = self.rw.native_transition(name) {
                    self.w.write(&format!("$n.transition_native({flags}, {n}, {}", js_str(kind)));
                } else {
                    let f = self.rw.directive_name(name);
                    self.w.write(&format!("$n.transition({flags}, {n}, () => {f}"));
                }
                if let Some(a) = arg {
                    self.w.write(", () => ");
                    self.paren_expr(a, Mode::Read);
                }
                self.w.write(");");
            }
            Op::Animate { name, arg, span } => {
                self.w.map(span.start);
                if self.rw.native_flip(name) {
                    self.w.write(&format!("$n.flip_native({n}, "));
                } else {
                    let f = self.rw.directive_name(name);
                    self.w.write(&format!("$n.animation({n}, () => {f}, "));
                }
                match arg {
                    Some(a) => {
                        self.w.write("() => ");
                        self.paren_expr(a, Mode::Read);
                    }
                    None => self.w.write("null"),
                }
                self.w.write(");");
            }
            Op::Classes { base, dynamic, toggles, span, .. } => {
                self.w.map(span.start);
                if dynamic.is_none() && toggles.is_empty() {
                    self.w.write(&format!("$n.attr({n}, \"class\", {});", js_str(base)));
                    return;
                }
                unreachable!("a dynamic class is an update");
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
            Op::InlineStyle { base, dynamic, directives, span, .. } => {
                self.w.map(span.start);
                if dynamic.is_none() && directives.is_empty() {
                    self.w.write(&format!("$n.attr({n}, \"style\", {});", js_str(base)));
                    return;
                }
                unreachable!("a dynamic inline style is an update");
            }
            Op::StyleSetter { .. } => unreachable!("a style setter is an update"),
            Op::Spread { entries, css, span } => {
                let mut hoisted = Vec::new();
                for (i, e) in entries.iter().enumerate() {
                    if let SpreadEntry::Event { handler, .. } = e
                        && is_function(handler)
                    {
                        self.tmp += 1;
                        let id = format!("$$handler{}", self.tmp);
                        self.w.write(&format!("const {id} = "));
                        self.expr_text(handler, Mode::Read);
                        self.w.write(";");
                        self.line();
                        hoisted.push((i, id));
                    }
                }
                // Svelte's Memoizer: call expressions are the effect's dependencies (deriveds), so
                // `{...get_rest()}` is not called again when another attribute changes
                let mut memo: Vec<&Expression> = Vec::new();
                for e in entries {
                    match e {
                        SpreadEntry::Spread(x) | SpreadEntry::Attr { value: AttrValue::Dynamic(Dynamic::Expr(x)), .. } if self.rw.has_call(x) => memo.push(x),
                        SpreadEntry::Attr { value: AttrValue::Dynamic(Dynamic::Template(parts)), .. } | SpreadEntry::Style { value: AttrValue::Dynamic(Dynamic::Template(parts)), .. } => {
                            memo.extend(parts.iter().filter_map(|p| match p {
                                TemplatePart::Expr(x) if self.rw.has_call(x) => Some(*x),
                                _ => None,
                            }))
                        }
                        SpreadEntry::Style { value: AttrValue::Dynamic(Dynamic::Expr(x)), .. } | SpreadEntry::Class { expr: x, .. } if self.rw.has_call(x) => memo.push(x),
                        _ => {}
                    }
                }
                let params: Vec<String> = (0..memo.len()).map(|i| format!("${i}")).collect();
                self.w.map(span.start);
                self.w.write(&format!("$n.spread({n}, $$kinds, ({}) => ", params.join(", ")));
                self.memo = memo.iter().enumerate().map(|(i, e)| (*e as *const Expression as usize, format!("${i}"))).collect();
                self.spread_object(entries, &hoisted);
                self.memo.clear();
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
                } else if css.is_some() {
                    self.w.write(", null");
                }
                if let Some(c) = css {
                    self.w.write(&format!(", {}", js_str(c)));
                }
                self.w.write(");");
            }
            Op::Text { value, span } => {
                self.w.map(span.start);
                match value {
                    AttrValue::Static(v) => {
                        self.w.write(&format!("$n.text({n}, $$kinds, "));
                        self.json(v);
                        self.w.write(");");
                    }
                    AttrValue::Dynamic(d) => self.dynamic_call(&format!("$n.text({n}, $$kinds, "), d),
                }
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
            Op::BindCommit { prop, signal, arg, target, span } => {
                self.w.map(span.start);
                self.w.write(&format!("$n.bind_commit({n}, {}, ", js_str(prop)));
                self.bind_accessors(target);
                self.w.write(&format!(", {}, {arg});", js_str(signal)));
            }
            Op::BindLink { prop, obj, field, fallback, span } => {
                self.w.map(span.start);
                self.w.write(&format!("$n.bind_link({n}, {}, () => ", js_str(prop)));
                self.expr_text(obj, Mode::Read);
                self.w.write(&format!(", {}, () => {{", js_str(field)));
                self.w.indent();
                self.op(n, fallback);
                self.w.dedent();
                self.line();
                self.w.write("});");
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
                assert!(cond.is_empty(), "a conditional token is an update");
                self.w.write(&format!("$n.token({n}, {}, {}, true);", js_str(prop), js_str(token)));
            }
            Op::StyleProp { .. } => unreachable!("a style-driven prop is an update"),
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
                    self.memo_or_paren(e);
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

    /// `call_head` + the value + `);`: the one-off write of a value that reads no state (a
    /// reactive one is written by the fragment's template effect, see `update`).
    fn dynamic_call(&mut self, call_head: &str, d: &Dynamic) {
        self.w.write(call_head);
        self.dynamic(d);
        self.w.write(");");
    }

    /// The effect parameter holding `e`'s memoized value, while `template_effect` writes an effect.
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

    /// An event handler value; `memo` is the derived holding it (a handler with a call), `event`
    /// the event name for dev warnings.
    fn handler(&mut self, h: &Expression, memo: Option<&str>, event: &str) {
        if is_function(h) {
            self.expr_text(h, Mode::Read);
            return;
        }
        // evaluated at event time, so a reactive handler value is read when the event fires
        self.w.write("function (...$$args) { ");
        match self.rw.dev_location(h.span().start) {
            // dev: Svelte's `$.apply` (warns `event_handler_invalid` for a value that is not a
            // function), natively: the event name and location are static
            Some(loc) => {
                self.w.write("$n.apply(() => ");
                match memo {
                    Some(id) => self.w.write(&format!("$.get({id})")),
                    None => self.paren_expr(h, Mode::Read),
                }
                self.w.write(&format!(", this, $$args, {}, {}", js_str(event), js_str(&loc)));
                let side = has_side_effects(h);
                let parens = matches!(unparen(h), Expression::CallExpression(c) if c.arguments.is_empty() && matches!(unparen(&c.callee), Expression::Identifier(_)));
                if side || parens {
                    self.w.write(&format!(", {side}"));
                }
                if parens {
                    self.w.write(", true");
                }
                self.w.write("); }");
            }
            None => {
                match memo {
                    Some(id) => self.w.write(&format!("$.get({id})")),
                    None => self.paren_expr(h, Mode::Read),
                }
                self.w.write("?.apply(this, $$args); }");
            }
        }
    }

    fn if_block(&mut self, b: &IfBlock, container: &str) {
        self.line();
        self.w.map(b.span.start);
        self.w.write(&format!("$n.if_block({container}, () => "));
        self.thunk_body(b.test);
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
                self.w.write("}");
            }
            None => self.w.write("null"),
        }
        // TR1: an `{:else if}` block is transparent for local transitions
        self.w.write(if b.elseif { ", true);" } else { ");" });
    }

    fn each_block(&mut self, b: &EachBlock, container: &str) {
        let item = match b.context {
            Some(Pattern::Identifier(id)) => id.name.clone(),
            _ => {
                self.tmp += 1;
                format!("$$item{}", self.tmp)
            }
        };
        let pattern = b.context.filter(|p| !matches!(p, Pattern::Identifier(_)));
        let index = b.index.clone().unwrap_or_else(|| "$$index".to_string());
        self.line();
        self.w.map(b.span.start);
        self.w.write(&format!("$n.each_block({container}, () => "));
        self.thunk_body(b.collection);
        self.w.write(", ");
        match b.key {
            Some(k) => {
                // a destructured item is destructured inside the key function (Svelte:
                // `b.arrow([node.context, index], key)`, the visited pattern as the parameter)
                match pattern {
                    Some(p) => {
                        let d = self.rw.destructure(p, "$$key", false);
                        self.w.write(&format!("($$key, {index}) => {{ "));
                        for (id, value) in &d.arrays {
                            self.w.write(&format!("const {id} = {value}; "));
                        }
                        for (name, expr, _) in &d.leaves {
                            self.w.write(&format!("const {name} = {expr}; "));
                        }
                        self.w.write("return ");
                        self.paren_expr(k, Mode::EachKey);
                        self.w.write("; }");
                    }
                    None => {
                        self.w.write(&format!("({item}, {index}) => "));
                        self.paren_expr(k, Mode::EachKey);
                    }
                }
            }
            None => self.w.write("null"),
        }
        self.w.write(&format!(", ($$anchor, {item}, {index}) => {{"));
        self.w.indent();
        if let Some(p) = pattern {
            self.destructure(p, &format!("$.get({item})"), Leaf::Source);
        }
        self.fragment(&b.body, "$$anchor");
        self.w.dedent();
        self.line();
        self.w.write("}, ");
        // Svelte's EachBlock: items are immutable sources unless the collection reads a store
        let flags = if self.rw.uses_store(b.collection) { ", $n.EACH_ITEM_MUTABLE" } else { "" };
        match &b.fallback {
            Some(f) => {
                self.w.write("($$anchor) => {");
                self.w.indent();
                self.fragment(f, "$$anchor");
                self.w.dedent();
                self.line();
                self.w.write(&format!("}}{flags});"));
            }
            None => self.w.write(&format!("null{flags});")),
        }
    }


    /// `($$anchor, extra) => { body }`, or `null`. `extra` is the `{:then}`/`{:catch}` binding
    /// (`fallback`: the parameter name when there is none or it is destructured).
    fn branch_fn(&mut self, f: Option<&Fragment>, extra: Option<(Option<&Param>, &str)>) {
        let Some(f) = f else {
            self.w.write("null");
            return;
        };
        let mut pattern = None;
        match extra {
            Some((Some(Param::Id(x)), _)) => self.w.write(&format!("($$anchor, {x}) => {{")),
            Some((p, fallback)) => {
                if let Some(Param::Pattern(p)) = p {
                    pattern = Some(*p);
                }
                self.w.write(&format!("($$anchor, {fallback}) => {{"))
            }
            None => self.w.write("($$anchor) => {"),
        }
        self.w.indent();
        if let (Some(p), Some((_, fallback))) = (pattern, extra) {
            self.destructure(p, &format!("$.get({fallback})"), Leaf::Source);
        }
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
        self.branch_fn(b.then.as_ref(), Some((b.value.as_ref(), "$$value")));
        self.w.write(", ");
        self.branch_fn(b.catch.as_ref(), Some((b.error.as_ref(), "$$error")));
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
            if let Some(d) = k.declaration {
                let text = self.rw.declaration(d);
                self.w.write(&format!("{text};"));
                continue;
            }
            let Some(init) = k.init else { continue };
            let name = match k.pattern {
                Some(_) => {
                    self.tmp += 1;
                    format!("$$const{}", self.tmp)
                }
                None => k.name.clone(),
            };
            self.w.write(&format!("const {name} = $.derived(() => "));
            self.paren_expr(init, Mode::Read);
            self.w.write(");");
            if let Some(p) = k.pattern {
                self.destructure(p, &format!("$.get({name})"), Leaf::Source);
            }
        }
    }

    /// Declares the leaves of a destructuring template binding read from the JS text `initial`
    /// (Svelte's EachBlock / SnippetBlock / ConstTag / AwaitBlock visitors): `$$array`
    /// intermediates as deriveds, then each leaf as a thunk (snippet parameters; a leaf with a
    /// default value goes through `$.derived_safe_equal`, so the default is evaluated once per
    /// change) or as a `$.derived_safe_equal` source (objects always propagate, like the item).
    fn destructure(&mut self, pattern: &Pattern, initial: &str, leaf: Leaf) {
        let d = self.rw.destructure(pattern, initial, true);
        for (id, value) in &d.arrays {
            self.line();
            self.w.write(&format!("const {id} = $.derived(() => {value});"));
        }
        for (name, expr, has_default) in &d.leaves {
            self.line();
            match leaf {
                Leaf::Thunk if *has_default => {
                    self.tmp += 1;
                    let d = format!("$$d{}", self.tmp);
                    self.w.write(&format!("const {d} = $.derived_safe_equal(() => {expr});"));
                    self.line();
                    self.w.write(&format!("const {name} = () => $.get({d});"));
                }
                Leaf::Thunk => self.w.write(&format!("const {name} = () => {expr};")),
                Leaf::Source => self.w.write(&format!("const {name} = $.derived_safe_equal(() => {expr});")),
            }
        }
    }

    /// A snippet declaration; `consts` are declared at the start of its body.
    fn snippet_with(&mut self, s: &Snippet, consts: &[ConstDecl]) {
        self.line();
        self.w.map(s.span.start);
        // a destructured or defaulted parameter is `$$argN`, its leaves declared from `$$argN?.()`
        // (Svelte's SnippetBlock visitor)
        let mut params = String::new();
        let mut patterns = Vec::new();
        for (i, p) in s.params.iter().enumerate() {
            match p {
                Param::Id(name) => params.push_str(&format!(", {name} = $.noop")),
                Param::Pattern(pat) => {
                    params.push_str(&format!(", $$arg{i} = $.noop"));
                    patterns.push((*pat, format!("$$arg{i}?.()")));
                }
            }
        }
        // dev (Svelte's SnippetBlock visitor): a function expression, so `arguments` can be
        // validated, wrapped so the snippet cannot be stringified (`{snippet}` without `{@render}`)
        let dev = self.rw.dev();
        if dev {
            self.w.write(&format!("const {} = $.wrap_snippet({}, function ($$anchor{params}) {{", s.name, self.name));
        } else {
            self.w.write(&format!("const {} = ($$anchor{params}) => {{", s.name));
        }
        self.w.indent();
        if dev {
            self.line();
            self.w.write("$n.validate_snippet_args(...arguments);");
        }
        for (p, initial) in patterns {
            self.destructure(p, &initial, Leaf::Thunk);
        }
        self.consts(consts);
        self.fragment(&s.body, "$$anchor");
        self.w.dedent();
        self.line();
        self.w.write(if dev { "});" } else { "};" });
    }

    /// The region a component call or `{@render}` at `start` is placed in: `$n.region(container)`,
    /// with the call site (`$$src`, `start`) in a dev build for a backend with `source-loc`.
    /// `block`: a dynamic component or a reactive snippet, whose block takes the container itself
    /// unless there is a site to carry (DZ0b2).
    fn site_region(&mut self, container: &str, start: u32, block: bool) -> String {
        if self.locs.is_some() {
            self.src_used = true;
            format!("$n.region({container}, $$src, {start})")
        } else if block {
            container.to_string()
        } else {
            format!("$n.region({container})")
        }
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
        // DZ0b2: the render tag is a call site, as a component call is
        let reactive = self.rw.is_reactive(r.callee);
        let site = self.site_region(container, r.span.start, reactive);
        if reactive {
            // the snippet function itself can change (a prop): re-render when it does
            self.w.write(&format!("$n.snippet({site}, () => "));
            if r.optional {
                self.paren_expr(r.callee, Mode::Read);
                self.w.write(" ?? $.noop");
            } else {
                self.expr_text(r.callee, Mode::Read);
            }
        } else {
            self.paren_expr(r.callee, Mode::Read);
            self.w.write(if r.optional { "?.(" } else { "(" });
            self.w.write(&site);
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
        // DZ0: in a dev build for a backend with `source-loc`, the call site goes on the region the
        // component (or, DZ0b2, the dynamic component's block) is placed in
        let site = self.site_region(container, c.span.start, dynamic);
        if let Some(this) = c.this {
            // `<svelte:component this>`: re-created when the function changes, nothing if nullish
            self.w.write(&format!("{open}({site}, () => "));
            self.paren_expr(this, Mode::Read);
            self.w.write(", ");
        } else {
            match dynamic_get {
                // re-created when the component function changes (Svelte's `$.component`)
                Some(get) => self.w.write(&format!("{open}({site}, () => {get}, ")),
                None => self.w.write(&format!("{}({site}, ", c.name)),
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
                // a value that reads no state is a plain property, evaluated once (Svelte's
                // build_component: `b.init` unless `has_state`)
                ComponentProp::Dynamic { name, value } if !self.dynamic_reactive(value) => {
                    self.w.write(&format!("{}: ", js_key(name)));
                    self.dynamic(value);
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
                    // `bind:x={$store}`: the child's `$.prop` learns it is bound to a store, so a
                    // mutation through it is written back (`store_set`) (Svelte's build_component)
                    let mark = if matches!(target, Expression::Identifier(_)) && self.rw.uses_store(target) { "$.mark_store_binding(); " } else { "" };
                    self.w.write(&format!("get {}() {{ {mark}return ", js_key(name)));
                    self.expr_text(target, Mode::Read);
                    let set = self.rw.assign(target, "$$value");
                    self.w.write(&format!("; }}, set {}($$value) {{ {set}; }}", js_key(name)));
                }
                // FW1: this component's own callback prop, passed through as a getter
                ComponentProp::Forward { name } => {
                    self.w.write(&format!("get {}() {{ return {}; }}", js_key(name), props_member(name)));
                }
                ComponentProp::Attach(e) => {
                    // Svelte's build_component: a reactive attachment is re-read by the attach effect
                    self.w.write("[$.attachment()]: ");
                    if self.rw.is_reactive(e) {
                        self.w.write("($$node) => (");
                        if is_function(e) {
                            self.paren_expr(e, Mode::Read);
                        } else {
                            self.paren_expr(e, Mode::Read);
                            self.w.write(" || $.noop");
                        }
                        self.w.write(")($$node)");
                    } else {
                        self.paren_expr(e, Mode::Read);
                    }
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
            // dev: Svelte wraps the implicit `children` snippet too (shared/component.js)
            if self.rw.dev() {
                self.w.write(&format!("children: $.wrap_snippet({}, ($$anchor, $$slotProps) => {{", self.name));
            } else {
                self.w.write("children: ($$anchor, $$slotProps) => {");
            }
            self.w.indent();
            if boundary {
                self.consts(&c.consts);
            }
            self.fragment(ch, "$$anchor");
            self.w.dedent();
            self.line();
            self.w.write(if self.rw.dev() { "})" } else { "}" });
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

/// What a fragment writes after its items, as Svelte's Fragment visitor: one `$.template_effect`
/// with every reactive write of its elements (`update`), created after its blocks and components,
/// so a parent's template expressions run after its children's components initialised (Svelte's
/// `lifecycle-render-order-for-children*`), and an unrelated change re-runs them all (each
/// runtime write skips an unchanged value); then the events and bindings (`after_update`), so a
/// binding's first write sees the element's other props.
#[derive(Default)]
struct Frag<'f> {
    /// (element variable, reactive op), in element order.
    update: Vec<(String, &'f Op<'f>)>,
    after: Vec<After<'f>>,
}

/// An event or binding of an element, written after the template effect.
struct After<'f> {
    node: String,
    op: &'f Op<'f>,
    /// The handler derived of an event (declared with the element).
    memo: Option<String>,
    /// W-C2: the template (number) whose listener slot this event fills.
    template: Option<usize>,
}

/// Where an op is written (Svelte's `init` / `update` / `after_update`, element-level `init`).
enum Phase {
    /// With the element, in attribute order.
    Init,
    /// In the fragment's template effect.
    Update,
    /// After the template effect, before the children's events (Svelte: event attributes).
    Event,
    /// After the element's children (Svelte: actions, attachments, `bind:this`).
    ElementInit,
    /// After the template effect, after the children's events and bindings (Svelte: `bind:`).
    Binding,
}

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

/// FW1: `$$props.name` / `$$props["na-me"]`, the component's own (internal) props parameter.
fn props_member(name: &str) -> String {
    let key = js_key(name);
    if key == name { format!("$$props.{name}") } else { format!("$$props[{key}]") }
}

/// A JS identifier fragment for a name (`text-edit` -> `text_edit`).
fn ident(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '$' { c } else { '_' }).collect()
}

fn escape_template(s: &str) -> String {
    s.replace('\\', "\\\\").replace('`', "\\`").replace("${", "\\${")
}

#[allow(dead_code)]
fn span_of(s: Span) -> (u32, u32) {
    (s.start, s.end)
}

fn is_function(h: &Expression) -> bool {
    matches!(h, Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_))
}

fn unparen(e: &Expression) -> &Expression {
    match e {
        Expression::ParenthesizedExpression(p) => unparen(&p.expression),
        e => e,
    }
}

/// Svelte's `has_side_effects` (events.js): a call, `new`, assignment or update, also inside a sequence.
fn has_side_effects(e: &Expression) -> bool {
    match unparen(e) {
        Expression::CallExpression(_) | Expression::NewExpression(_) | Expression::AssignmentExpression(_) | Expression::UpdateExpression(_) => true,
        Expression::SequenceExpression(s) => s.expressions.iter().any(has_side_effects),
        _ => false,
    }
}
