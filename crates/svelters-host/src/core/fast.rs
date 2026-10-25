//! The engine-agnostic halves of the fast host ops (plans/research/skelter/compiler-and-js.md §3.2,
//! §5.4 option (a)): typed scalar prop writes by id with no conversion tree (W-J1), and native
//! templates (W-C2): a static subtree registered once, then built per instance with one host call,
//! by the backend (`instantiate`) or by this module's expansion into today's ops.
//!
//! The engine reads a scalar straight from the JS value's tag and hands it here as a raw
//! `svl_value`; anything the scalar rules below do not cover goes through `op_set_prop` and the
//! descriptor-driven converter, so errors and coercions stay exactly as they are there.

use super::*;
use crate::convert::{self, Arena, Cx, JsReader};
use svelters_abi::*;

/// How a prop's slot takes a scalar JS value without conversion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ScalarSlot {
    /// `SVL_I64` with its range (`NaN` = unbounded) and whether it is unsigned.
    Int { min: f64, max: f64, unsigned: bool },
    /// `SVL_F64` (not a datetime) with its range.
    Float { min: f64, max: f64 },
    Bool,
    /// `SVL_STR`; `nullable` takes `null`.
    Str { nullable: bool },
}

/// A JS scalar as the engine read it from the value's tag.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Scalar<'a> {
    Int(i32),
    Float(f64),
    Bool(bool),
    Null,
    /// UTF-8 (NUL-terminated by the engine), borrowed for the call.
    Str(&'a [u8]),
}

const MAX_SAFE: f64 = 9_007_199_254_740_992.0;

impl ScalarSlot {
    fn of(slot: &SlotDesc) -> Option<ScalarSlot> {
        match slot.tag {
            SVL_I64 => Some(ScalarSlot::Int { min: slot.min, max: slot.max, unsigned: slot.has(SVL_D_UNSIGNED) }),
            SVL_F64 if slot.ty != SVL_T_DATETIME => Some(ScalarSlot::Float { min: slot.min, max: slot.max }),
            SVL_BOOL => Some(ScalarSlot::Bool),
            SVL_STR => Some(ScalarSlot::Str { nullable: slot.has(SVL_D_NULLABLE) }),
            _ => None,
        }
    }

    /// The raw value of `v` for this slot, or `None` when `v` needs the converter (a coercion, a
    /// range error, a type error: the converter words them).
    pub(crate) fn encode(self, v: Scalar<'_>) -> Option<svl_value> {
        let within = |n: f64, min: f64, max: f64| !(n < min || n > max); // NaN bounds: unbounded
        match (self, v) {
            (ScalarSlot::Int { min, max, unsigned }, Scalar::Int(i)) if within(i as f64, min, max) => {
                let mut out = svl_value::new(SVL_I64, svl_value_u { i: i as i64 });
                if unsigned {
                    out.flags |= SVL_F_UNSIGNED;
                }
                Some(out)
            }
            (ScalarSlot::Int { min, max, unsigned }, Scalar::Float(f))
                if f.is_finite() && f.fract() == 0.0 && f.abs() < MAX_SAFE && within(f, min, max) =>
            {
                let mut out = svl_value::new(SVL_I64, svl_value_u { i: f as i64 });
                if unsigned {
                    out.flags |= SVL_F_UNSIGNED;
                }
                Some(out)
            }
            (ScalarSlot::Float { min, max }, Scalar::Int(i)) if within(i as f64, min, max) => {
                Some(svl_value::new(SVL_F64, svl_value_u { f: i as f64 }))
            }
            (ScalarSlot::Float { min, max }, Scalar::Float(f)) if within(f, min, max) => {
                Some(svl_value::new(SVL_F64, svl_value_u { f }))
            }
            (ScalarSlot::Bool, Scalar::Bool(b)) => Some(svl_value::new(SVL_BOOL, svl_value_u { b })),
            (ScalarSlot::Str { .. }, Scalar::Str(s)) => {
                let mut out = svl_value::new(SVL_STR, svl_value_u { s: svl_str { ptr: s.as_ptr().cast(), len: s.len() } });
                out.flags |= SVL_F_NUL_TERM;
                Some(out)
            }
            (ScalarSlot::Str { nullable: true }, Scalar::Null) => Some(svl_value::null()),
            _ => None,
        }
    }
}

/// What `host.instantiate` built: contiguous handles from the backend, or the handles of an
/// expansion, in template node order.
pub(crate) enum Instance {
    Base(svl_handle),
    Handles(Vec<svl_handle>),
}

/// A registered template: the raw table and, per listener, the signal name events need.
pub(crate) struct HostTemplate {
    pub(crate) table: OwnedTemplate,
    signal_names: Vec<String>,
    refs: Cell<u32>,
}

/// The registered templates of a host, by id (`host.template` returns it).
#[derive(Default)]
pub(crate) struct Templates {
    by_id: RefCell<HashMap<u32, Rc<HostTemplate>>>,
    by_hash: RefCell<HashMap<u64, u32>>,
    next: Cell<u32>,
}

/// One template node as the engine read it: kind and slot by name or id, parent index + 1.
pub(crate) struct TplNodeIn<'a> {
    pub kind: Name<'a>,
    pub parent: u32,
    pub slot: Option<Name<'a>>,
}

impl Core {
    /// The scalar slot of prop `p` of kind `k`, when it has a descriptor that takes scalars as
    /// they are.
    fn scalar_slot(&self, k: svl_id, p: svl_id) -> Option<ScalarSlot> {
        let d = self.member(NameKind::Prop, k, p)?;
        ScalarSlot::of(d.params.first()?)
    }

    /// The scalar slot of prop `prop` on live handle `h` (resolving a name), for an engine about to
    /// read a value: `Ok(None)` means "use `op_set_prop`" (a dead or app handle, a prop with no
    /// scalar descriptor). Errors are the ones `op_set_prop` gives for the same handle and name.
    pub(crate) fn fast_prop(&self, h: svl_handle, prop: Name<'_>) -> Result<Option<(svl_id, ScalarSlot)>, OpError> {
        if crate::exports::is_app_handle(h) {
            return Ok(None);
        }
        let Some(k) = self.live_kind(h)? else { return Ok(None) };
        let p = self.resolve_name(NameKind::Prop, k, prop)?;
        Ok(self.scalar_slot(k, p).map(|s| (p, s)))
    }

    /// W-J1: writes a scalar the engine encoded with [`ScalarSlot::encode`] for prop `p` (from
    /// [`Core::fast_prop`]) of `h`: no arena, no callbacks, the same backend call and ABI v2.6
    /// read-back as `op_set_prop`. Nothing ran between `fast_prop` and this, so `h` is still live.
    pub(crate) fn op_set_scalar(&self, h: svl_handle, p: svl_id, raw: &svl_value) -> Result<(), OpError> {
        self.timed("op_set_scalar", || {
            let res = self.backend.set_prop_raw(h, p, raw).map_err(|e| self.backend_err(e));
            if res.is_ok() && self.st.borrow().observed.contains_key(&(h, p)) {
                self.after_set(h, p, Some(unsafe { Value::from_raw(raw) }));
            }
            res
        })
    }

    // ---- templates (W-C2) ----

    /// Whether the backend builds templates itself (`host.instantiate` then returns a base handle).
    pub(crate) fn has_native_templates(&self) -> bool {
        self.backend.has_templates()
    }

    /// Registers a template (`host.template`): `nodes` in pre-order, `props` as (node index,
    /// prop, value) in any order (kept in order per node), `listens` as (node index, signal,
    /// flags). Names resolve now and values convert now with the props' descriptors, so an
    /// instantiation does neither. The same `hash` registered again returns the same id (with one
    /// more reference, `template_release` drops one).
    pub(crate) fn op_template<R: JsReader>(
        &self,
        r: &R,
        hash: u64,
        nodes: &[TplNodeIn<'_>],
        props: &[(u32, Name<'_>, R::V)],
        listens: &[(u32, Name<'_>, u32)],
    ) -> Result<u32, OpError> {
        self.timed("op_template", || {
            if let Some(&id) = self.templates.by_hash.borrow().get(&hash)
                && let Some(t) = self.templates.by_id.borrow().get(&id)
            {
                t.refs.set(t.refs.get() + 1);
                return Ok(id);
            }
            let mut kinds = Vec::with_capacity(nodes.len());
            let mut raw_nodes = Vec::with_capacity(nodes.len());
            for (i, n) in nodes.iter().enumerate() {
                let kind = match n.kind {
                    Name::Str(s) => self.op_kind_id(s)?,
                    Name::Id(id) => id,
                };
                let slot = match (n.slot, (n.parent as usize).checked_sub(1)) {
                    (Some(s), Some(p)) if p < i => self.resolve_name(NameKind::Slot, kinds[p], s)?,
                    (_, Some(p)) if p >= i => return Err(format!("template node {i}: its parent {p} does not come before it").into()),
                    _ => 0,
                };
                kinds.push(kind);
                raw_nodes.push(svl_tpl_node { kind, parent: n.parent, slot, first_prop: 0, n_props: 0 });
            }
            // props grouped by node, in their order within a node
            let mut order: Vec<usize> = (0..props.len()).collect();
            order.sort_by_key(|&i| props[i].0);
            let mut values: Vec<(svl_id, Value)> = Vec::with_capacity(props.len());
            for &i in &order {
                let (node, prop, v) = &props[i];
                let k = *kinds.get(*node as usize).ok_or_else(|| format!("template prop {i}: no node {node}"))?;
                let p = self.resolve_name(NameKind::Prop, k, *prop)?;
                let desc = self.member(NameKind::Prop, k, p);
                let any = SlotDesc::new(SVL_NULL);
                let slot = desc.as_ref().and_then(|d| d.params.first()).unwrap_or(&any);
                let mut ar = Arena::new();
                // static props are constants: no callbacks, no handles of their own
                let cx = Cx { types: self, utf16: false, callbacks: None, owner: 0 };
                let raw = convert::encode(r, v, slot, &cx, &mut ar)
                    .map_err(|e| self.conv_err(&format!("template <{}> prop #{p}", self.kind_name(k)), e))?;
                let owned = unsafe { Value::from_raw(&raw) };
                drop(ar);
                let n = &mut raw_nodes[*node as usize];
                if n.n_props == 0 {
                    n.first_prop = values.len() as u32;
                }
                n.n_props += 1;
                values.push((p, owned));
            }
            let mut raw_listens = Vec::with_capacity(listens.len());
            let mut signal_names = Vec::with_capacity(listens.len());
            for (i, (node, signal, flags)) in listens.iter().enumerate() {
                let k = *kinds.get(*node as usize).ok_or_else(|| format!("template listener {i}: no node {node}"))?;
                let s = self.resolve_name(NameKind::Signal, k, *signal)?;
                let name = match signal {
                    Name::Str(n) => n.to_string(),
                    Name::Id(_) => self
                        .member(NameKind::Signal, k, s)
                        .map(|d| d.name.clone())
                        .ok_or_else(|| "a template listener by id needs the signal's descriptor".to_string())?,
                };
                raw_listens.push(svl_tpl_listen { node: *node, signal: s, flags: *flags });
                signal_names.push(name);
            }
            let table = OwnedTemplate::new(hash, raw_nodes, values, raw_listens);
            if !table.as_ref().is_well_formed() {
                return Err("template nodes are not in pre-order".to_string().into());
            }
            let id = self.templates.next.get() + 1;
            self.templates.next.set(id);
            self.templates.by_id.borrow_mut().insert(id, Rc::new(HostTemplate { table, signal_names, refs: Cell::new(1) }));
            self.templates.by_hash.borrow_mut().insert(hash, id);
            Ok(id)
        })
    }

    /// Drops one reference to template `id`; the last one frees it (instances keep their nodes).
    pub(crate) fn op_template_release(&self, id: u32) {
        let gone = match self.templates.by_id.borrow().get(&id) {
            Some(t) if t.refs.get() > 1 => {
                t.refs.set(t.refs.get() - 1);
                None
            }
            Some(t) => Some(t.table.id()),
            None => None,
        };
        if let Some(hash) = gone {
            self.templates.by_id.borrow_mut().remove(&id);
            self.templates.by_hash.borrow_mut().remove(&hash);
        }
    }

    /// `host.instantiate(id)`: builds one instance of template `id`. A backend with
    /// `instantiate` builds it in one call (contiguous handles); otherwise the template expands
    /// here into `create`, `set_prop`, `insert` and `listen_ex`, in the order `svl_template`
    /// documents, which is the order the runtime would issue them one by one (the op log shows
    /// the same lines). A failure destroys what was built and reports the first error.
    pub(crate) fn op_instantiate(&self, id: u32) -> Result<Instance, OpError> {
        self.timed("op_instantiate", || {
            self.tree_ops_allowed()?;
            let t = self.templates.by_id.borrow().get(&id).cloned().ok_or_else(|| format!("unknown template {id}"))?;
            let tr = t.table.as_ref();
            if self.backend.has_templates() {
                let raw = t.table.raw();
                if let Some(base) = self.backend.instantiate(&raw).map_err(|e| self.backend_err(e))? {
                    if base.saturating_add(tr.nodes.len() as u64) >= MAX_HANDLE {
                        self.backend.destroy_range(base, tr.nodes.len() as u32);
                        return Err(format!("backend issued handles from {base}, which JS cannot represent exactly").into());
                    }
                    let hs: Vec<svl_handle> = (0..tr.nodes.len() as u64).map(|k| base + k).collect();
                    self.adopt_instance(&t, &hs);
                    return Ok(Instance::Base(base));
                }
            }
            let mut hs: Vec<svl_handle> = Vec::with_capacity(tr.nodes.len());
            let built = (|| -> Result<(), OpError> {
                for step in tr.expansion() {
                    match step {
                        TplStep::Create(k) => {
                            let h = self.create_kind(tr.nodes[k].kind)?;
                            hs.push(h);
                            for p in tr.node_props(k) {
                                self.backend.set_prop_raw(h, p.prop, &p.value).map_err(|e| self.backend_err(e))?;
                            }
                        }
                        TplStep::Place(k) => {
                            let n = &tr.nodes[k];
                            if let Some(parent) = (n.parent as usize).checked_sub(1) {
                                self.backend.insert(hs[parent], hs[k], None, n.slot).map_err(|e| self.backend_err(e))?;
                                self.st.borrow_mut().tree.insert(hs[k], hs[parent]);
                            }
                            for (i, l) in tr.listens.iter().enumerate().filter(|(_, l)| l.node as usize == k) {
                                self.backend.listen_ex(hs[k], l.signal, true, l.flags).map_err(|e| self.backend_err(e))?;
                                let kind = tr.nodes[k].kind;
                                self.st.borrow_mut().signal_names.entry((kind, l.signal)).or_insert_with(|| t.signal_names[i].clone());
                            }
                        }
                    }
                }
                Ok(())
            })();
            if let Err(e) = built {
                for &h in hs.iter().rev() {
                    if self.forget_handle(h).unwrap_or(false) {
                        self.backend.destroy(h);
                    }
                }
                return Err(e);
            }
            Ok(Instance::Handles(hs))
        })
    }

    /// The handle table entries, tree links and signal names of an instance the backend built.
    fn adopt_instance(&self, t: &HostTemplate, hs: &[svl_handle]) {
        let tr = t.table.as_ref();
        let mut st = self.st.borrow_mut();
        for (k, n) in tr.nodes.iter().enumerate() {
            st.handles.insert(hs[k], n.kind);
            if let Some(p) = (n.parent as usize).checked_sub(1) {
                st.tree.insert(hs[k], hs[p]);
            }
        }
        for (i, l) in tr.listens.iter().enumerate() {
            let kind = tr.nodes[l.node as usize].kind;
            st.signal_names.entry((kind, l.signal)).or_insert_with(|| t.signal_names[i].clone());
        }
    }

    /// `host.destroyRange(base, n)`: destroys the handles of one native instantiation, children
    /// first, in one backend call (`destroy_range`). Handles already dead are skipped; when one is
    /// pinned by a queued event, every handle goes through `destroy` on its own instead.
    pub(crate) fn op_destroy_range(&self, base: svl_handle, n: u32) -> Result<(), OpError> {
        self.timed("op_destroy_range", || {
            self.tree_ops_allowed()?;
            let range = base..base.saturating_add(n as u64);
            if range.clone().any(|h| self.pinned(h)) {
                for h in range.clone().rev() {
                    if self.forget_handle(h)? {
                        self.backend.destroy(h);
                    }
                }
                return Ok(());
            }
            let mut live = Vec::with_capacity(n as usize);
            for h in range.rev() {
                if self.forget_handle(h)? {
                    live.push(h);
                }
            }
            if live.len() == n as usize {
                self.backend.destroy_range(base, n);
            } else {
                // some died natively already: destroy the others one by one
                live.into_iter().for_each(|h| self.backend.destroy(h));
            }
            Ok(())
        })
    }
}
