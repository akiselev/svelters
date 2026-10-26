//! W-C2: native templates, the codegen side (skelter `compiler-and-js.md` §3.2).
//!
//! An [`Item::Template`] is written as one table per template at module scope and one call per
//! instance:
//!
//! ```js
//! const $$t0 = $n.instantiate($$T0, $$anchor, [() => title, ($$c) => { $n.if_block($$c, ...) }]);
//! const $$n3 = $n.at($$t0, 2);          // the nodes later code writes to, listens on or binds
//! ...                                    // dynamic writes, as for any element
//! $n.template_listen($$t0, [h0, h1]);   // after the template effect, with the other events
//!
//! const $$T0 = $n.template("<hash>", [[kind, slot, ops, children, loc], ...], [[node, signal, flags], ...]);
//! ```
//!
//! Nodes are numbered in pre-order. A node's `ops` are its static writes in source order (`set`,
//! `class`, `style`, `id`, `cattr`, `token`, `track`) and its one-off writes of values that read no
//! state (`setv`, `idv`, `cattrv`, whose last operand indexes the thunk array); `children` lists
//! child nodes by number and holes (blocks, components, render tags) as `[thunk, slot]`, the thunk
//! a function of the slot's container; `loc` is the node's source offset in a dev build for a
//! backend with `source-loc` (DZ0), else `null`. The runtime (`template.js`) builds the nodes in the order the
//! per-node code would (create, source location, ops, children, insert), so op logs are unchanged;
//! a host with `instantiate` builds them in one call. Listener slots are used only when every
//! listener of the template is written by one `$n.template_listen` (no binding between them in
//! `after_update` order); otherwise the listeners are written one by one and the table has none.

use super::*;

/// A template's module-scope table.
pub(super) struct Table {
    index: usize,
    /// The nodes as JS literal text, in pre-order.
    nodes: Vec<String>,
    /// The listener slots `[node, signal, flags]` as JS literal text, in listener order.
    listeners: Vec<String>,
    /// The listeners are written by `$n.template_listen` (else the table lists none).
    listen: bool,
    /// The listener slots are written one by one (their run was split).
    split: bool,
}

impl Table {
    /// `const $$T<i> = $n.template("<hash>", [...], [...]);`, keyed by a hash of its content.
    pub(super) fn declaration(&self) -> String {
        let nodes = self.nodes.join(", ");
        let listeners = if self.listen { self.listeners.join(", ") } else { String::new() };
        let hash = fnv(&[&nodes, &listeners]);
        format!("\nconst $$T{} = $n.template(\"{hash:016x}\", [{nodes}], [{listeners}]);\n", self.index)
    }
}

/// FNV-1a (64-bit) of `parts`, each followed by a NUL.
fn fnv(parts: &[&str]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.bytes().chain(std::iter::once(0)) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// An entry of the thunk array of `$n.instantiate`.
enum Thunk<'f> {
    /// A one-off value (`() => value`).
    Value(&'f Dynamic<'f>),
    /// A hole (`($$c) => { ... }`, building the item into the container `$$c`).
    Hole(&'f Item<'f>),
}

impl Codegen<'_, '_> {
    /// An `Item::Template`: the instance, the nodes later code uses, their deriveds, and the
    /// element's dynamic writes, events and bindings pushed to the fragment as `element` would.
    pub(super) fn template_item<'f>(&mut self, t: &'f Template<'f>, container: &str, fr: &mut Frag<'f>) {
        let mut nodes = Vec::new();
        let mut thunks = Vec::new();
        let mut listeners = Vec::new();
        let mut locs = false;
        self.table_node(&t.root, &mut nodes, &mut thunks, &mut listeners, &mut locs);
        let var = format!("$$t{}", t.index);
        self.line();
        self.w.map(t.root.span.start);
        self.w.write(&format!("const {var} = $n.instantiate($$T{}, {container}, [", t.index));
        for (i, th) in thunks.iter().enumerate() {
            if i > 0 {
                self.w.write(", ");
            }
            match th {
                Thunk::Value(Dynamic::Expr(e)) => {
                    self.w.write("() => ");
                    self.thunk_body(e);
                }
                Thunk::Value(d) => {
                    self.w.write("() => ");
                    self.dynamic(d);
                }
                Thunk::Hole(item) => {
                    self.w.write("($$c) => {");
                    self.w.indent();
                    // a hole is a block, component or render tag: it writes its own fragments
                    let mut own = Frag::default();
                    self.item(item, "$$c", &mut own);
                    debug_assert!(own.update.is_empty() && own.after.is_empty(), "a template hole writes into its own fragment");
                    self.w.dedent();
                    self.line();
                    self.w.write("}");
                }
            }
        }
        self.w.write("]");
        if locs {
            self.src_used = true;
            self.w.write(", $$src");
        }
        self.w.write(");");
        self.tables.push(Table { index: t.index, nodes, listeners, listen: false, split: false });
        let mut next = 0;
        self.template_refs(&t.root, &var, &mut next, t.index, fr);
    }

    /// The table row of `e` and its element children (pre-order); returns `e`'s node number.
    fn table_node<'f>(&self, e: &'f Element<'f>, nodes: &mut Vec<String>, thunks: &mut Vec<Thunk<'f>>, listeners: &mut Vec<String>, locs: &mut bool) -> usize {
        let k = nodes.len();
        nodes.push(String::new());
        let mut ops = Vec::new();
        let value = |d: &'f Dynamic<'f>, thunks: &mut Vec<Thunk<'f>>| {
            thunks.push(Thunk::Value(d));
            thunks.len() - 1
        };
        for op in &e.ops {
            let init = matches!(self.phase(op), Phase::Init);
            match op {
                Op::Set { prop, value: v, .. } => ops.push(format!("[\"set\", {}, {v}]", js_str(prop))),
                Op::SetDynamic { prop, value: d, .. } if init => ops.push(format!("[\"setv\", {}, {}]", js_str(prop), value(d, thunks))),
                Op::ChildAttr { name, value: AttrValue::Static(v), .. } => ops.push(format!("[\"cattr\", {}, {v}]", js_str(name))),
                Op::ChildAttr { name, value: AttrValue::Dynamic(d), .. } if init => ops.push(format!("[\"cattrv\", {}, {}]", js_str(name), value(d, thunks))),
                Op::WidgetName { value: AttrValue::Static(v), .. } => ops.push(format!("[\"id\", {v}]")),
                Op::WidgetName { value: AttrValue::Dynamic(d), .. } if init => ops.push(format!("[\"idv\", {}]", value(d, thunks))),
                Op::Classes { base, dynamic: None, toggles, .. } if toggles.is_empty() => ops.push(format!("[\"class\", {}]", js_str(base))),
                Op::InlineStyle { base, dynamic: None, directives, .. } if directives.is_empty() => ops.push(format!("[\"style\", {}]", js_str(base))),
                Op::Token { prop, token, cond, .. } if cond.is_empty() => ops.push(format!("[\"token\", {}, {}]", js_str(prop), js_str(token))),
                Op::Track { prop, signal, arg } => ops.push(format!("[\"track\", {}, {}, {arg}]", js_str(prop), js_str(signal))),
                Op::Listen { signal, flags, .. } => listeners.push(format!("[{k}, {}, {flags}]", js_str(signal))),
                // reactive writes, events and bindings: written against the node (`template_refs`)
                _ => {}
            }
        }
        let mut children = Vec::new();
        for item in &e.children.items {
            match item {
                Item::Element(c) => children.push(self.table_node(c, nodes, thunks, listeners, locs).to_string()),
                hole => {
                    thunks.push(Thunk::Hole(hole));
                    let slot = hole.slot().as_deref().map_or("null".to_string(), js_str);
                    children.push(format!("[{}, {slot}]", thunks.len() - 1));
                }
            }
        }
        let slot = e.slot.as_deref().map_or("null".to_string(), js_str);
        let loc = match self.locs {
            Some(has) if has(&e.kind) => {
                *locs = true;
                e.span.start.to_string()
            }
            _ => "null".to_string(),
        };
        nodes[k] = format!("[{}, {slot}, [{}], [{}], {loc}]", js_str(&e.kind), ops.join(", "), children.join(", "));
        k
    }

    /// The nodes of the template later code uses (`const $$n<id> = $n.at(t, k)`), with their class
    /// and id deriveds and handler deriveds; their reactive writes go to the fragment's template
    /// effect, their events and bindings after it, in `element`'s order.
    fn template_refs<'f>(&mut self, e: &'f Element<'f>, var: &str, next: &mut usize, index: usize, fr: &mut Frag<'f>) {
        let k = *next;
        *next += 1;
        let n = format!("$$n{}", e.id + 1);
        let used = e.class_expr.is_some() || e.id_expr.is_some() || e.ops.iter().any(|op| !matches!(self.phase(op), Phase::Init));
        if used {
            self.line();
            self.w.write(&format!("const {n} = $n.at({var}, {k});"));
            self.node_tags.insert(n.clone(), e.kind.clone());
            self.note_shared_props(&n, &e.ops);
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
        let mut own_after = Vec::new();
        for op in &e.ops {
            match self.phase(op) {
                Phase::Update => {
                    self.update_prefix(&n, op);
                    fr.update.push((n.clone(), op));
                }
                Phase::Event => {
                    let memo = self.handler_memo(op);
                    fr.after.push(After { node: n.clone(), op, memo, template: Some(index) });
                }
                Phase::Binding => own_after.push(op),
                // in the table (the lowering admits no element-init ops into a template)
                Phase::Init | Phase::ElementInit => {}
            }
        }
        for item in &e.children.items {
            if let Item::Element(c) = item {
                self.template_refs(c, var, next, index, fr);
            }
        }
        fr.after.extend(own_after.into_iter().map(|op| After { node: n.clone(), op, memo: None, template: None }));
    }

    /// The number of `after` entries from `i` that are template `index`'s listeners, when all of
    /// them come in one run starting at `i`; else None (and the template's listeners are written
    /// one by one).
    pub(super) fn template_listen_run(&mut self, after: &[After<'_>], i: usize, index: usize) -> Option<usize> {
        let pos = self.tables.iter().position(|t| t.index == index)?;
        if self.tables[pos].split {
            return None;
        }
        let ours = |a: &After| a.template == Some(index) && matches!(a.op, Op::Listen { .. });
        let total = after.iter().filter(|a| ours(a)).count();
        let first = after.iter().position(|a| ours(a));
        if first == Some(i) && after[i..].iter().take(total).all(|a| ours(a)) && total == self.tables[pos].listeners.len() {
            return Some(total);
        }
        self.tables[pos].split = true;
        None
    }

    /// `$n.template_listen(t, [handler, ...])`: the template's listeners, in its slot order.
    pub(super) fn template_listen(&mut self, index: usize, run: &[After<'_>]) {
        if let Some(t) = self.tables.iter_mut().find(|t| t.index == index) {
            t.listen = true;
        }
        self.line();
        self.w.write(&format!("$n.template_listen($$t{index}, ["));
        for (i, a) in run.iter().enumerate() {
            if i > 0 {
                self.w.write(", ");
            }
            if let Op::Listen { span, .. } = a.op {
                self.w.map(span.start);
            }
            self.listener(a.op, a.memo.as_deref());
        }
        self.w.write("]);");
    }
}
