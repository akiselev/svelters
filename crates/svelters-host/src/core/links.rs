//! Native links (CX3, `plans/milestones/CX3-bind-targets.md`; CADATOMIC.md SV3): `host.link(store,
//! field, node, prop)` ties a writable field of a Rust store (E3) to a native prop, both ways, in Rust.
//! This is what `bind:prop={store.field}` lowers to on a backend whose spec declares the `link` bind
//! source (egui): typing or dragging on the node costs no JS, and neither does a Rust write to the store.
//!
//! - **Both ends are observed** through the ordinary refcounted `observed` table (a store field with
//!   `app_observe`, the node prop with `op_observe`), each counting the link in `Observed::links`. A
//!   pair that only links observe is never delivered to JS (`count == links`); one JS also observes (a
//!   template reading `store.field`) is delivered as before.
//! - **Propagation** happens in `take_changes`: a changed value of one end is written to the other, the
//!   store through its generated setter (`on_write` runs; an `Err` writes the store's value back to the
//!   node), the node through `set_prop`. Both writes take R1's read-back (`after_set`), so the echo
//!   compares equal and a normalized value (a clamp in `on_write`) comes back once.
//! - **At link time** the store's value is written to the node (the store is the source of truth).
//! - **Lifetime**: `host.unlink(id)` (the binding's effect teardown) releases both observations; a
//!   destroyed node or released store drops its links.
//! - **Refused** (`link` answers 0, and the runtime falls back to the JS binding): the source is not an
//!   app object, the field is not a writable, non-proportional store field, or the backend cannot
//!   observe the prop.

use super::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Link {
    pub id: u32,
    pub store: (svl_handle, svl_id),
    pub node: (svl_handle, svl_id),
}

impl Core {
    /// `host.link(store, field, node, prop)`: the link id, or 0 when the pair cannot be linked.
    pub(crate) fn op_link(&self, store: svl_handle, field: &str, node: svl_handle, prop: &str) -> Result<u32, OpError> {
        self.timed("op_link", || {
            if !crate::exports::is_app_handle(store) || crate::exports::is_app_handle(node) {
                return Ok(0);
            }
            let Some(kind) = self.apps.kind(store) else { return Ok(0) };
            let fid = crate::exports::schema().resolve(NameKind::Prop, kind, field);
            if fid == 0 || !self.app_store_field_writable(store, fid) {
                return Ok(0);
            }
            let nk = self.kind_of(node).map_err(OpError::from)?;
            let pid = self.resolve_name(NameKind::Prop, nk, Name::Str(prop)).map_err(OpError::from)?;
            if !self.app_observe(store, Name::Str(field), true)? {
                return Ok(0);
            }
            if self.st.borrow().observed.get(&(store, fid)).is_some_and(|o| o.proportional) || !self.op_observe(node, Name::Str(prop), true)? {
                self.app_observe(store, Name::Id(fid), false)?;
                return Ok(0);
            }
            let id = {
                let mut st = self.st.borrow_mut();
                for key in [(store, fid), (node, pid)] {
                    if let Some(o) = st.observed.get_mut(&key) {
                        o.links += 1;
                    }
                }
                st.next_link += 1;
                let id = st.next_link;
                st.links.push(Link { id, store: (store, fid), node: (node, pid) });
                id
            };
            // the store is the source of truth: the node starts with its value
            if let Some(v) = self.app_prop_value(store, fid) {
                self.link_write_node((node, pid), v);
            }
            Ok(id)
        })
    }

    /// `host.unlink(id)`: releases both observations. Unknown ids (already dropped with their node or
    /// store) are a no-op.
    pub(crate) fn op_unlink(&self, id: u32) -> Result<(), OpError> {
        let link = {
            let mut st = self.st.borrow_mut();
            let Some(i) = st.links.iter().position(|l| l.id == id) else { return Ok(()) };
            let l = st.links.remove(i);
            for key in [l.store, l.node] {
                if let Some(o) = st.observed.get_mut(&key) {
                    o.links = o.links.saturating_sub(1);
                }
            }
            l
        };
        self.app_observe(link.store.0, Name::Id(link.store.1), false)?;
        self.op_observe(link.node.0, Name::Id(link.node.1), false)?;
        Ok(())
    }

    /// The links of a destroyed node or a released store go with it; the other end's observation is
    /// released.
    pub(crate) fn links_gone(&self, h: svl_handle) {
        let gone: Vec<Link> = {
            let mut st = self.st.borrow_mut();
            if st.links.is_empty() {
                return;
            }
            let (gone, keep): (Vec<Link>, Vec<Link>) = std::mem::take(&mut st.links).into_iter().partition(|l| l.store.0 == h || l.node.0 == h);
            st.links = keep;
            for l in &gone {
                for key in [l.store, l.node] {
                    if let Some(o) = st.observed.get_mut(&key) {
                        o.links = o.links.saturating_sub(1);
                    }
                }
            }
            gone
        };
        for l in gone {
            if l.store.0 != h {
                let _ = self.app_observe(l.store.0, Name::Id(l.store.1), false);
            }
            if l.node.0 != h {
                let _ = self.op_observe(l.node.0, Name::Id(l.node.1), false);
            }
        }
    }

    /// Whether JS observes `key` itself (not only links): its changes go into the JS batch.
    pub(crate) fn js_observes(&self, key: (svl_handle, svl_id)) -> bool {
        self.st.borrow().observed.get(&key).is_some_and(|o| o.count > o.links)
    }

    /// A changed value `v` of `key`, read at the checkpoint: written to the other end of each link.
    pub(crate) fn link_propagate(&self, key: (svl_handle, svl_id), v: &Value) {
        let targets: Vec<Link> = {
            let st = self.st.borrow();
            if st.links.is_empty() {
                return;
            }
            st.links.iter().filter(|l| l.store == key || l.node == key).copied().collect()
        };
        for l in targets {
            self.reactive.stat(|s| s.link_writes += 1);
            if l.store == key {
                self.link_write_node(l.node, v.clone());
            } else if let Err(e) = self.app_set_value(l.store.0, l.store.1, v.clone()) {
                // refused by the store (`on_write`, a type mismatch): the node shows the store's value
                self.emit("warn", Some("link"), format!("link {}: the store refused the node's value: {}", l.id, e.msg));
                if let Some(back) = self.app_prop_value(l.store.0, l.store.1) {
                    self.link_write_node(l.node, back);
                }
            }
        }
    }

    fn link_write_node(&self, (h, p): (svl_handle, svl_id), v: Value) {
        match self.backend.set_prop(h, p, &v) {
            Ok(()) => self.after_set(h, p, Some(v)),
            Err(e) => self.emit("warn", Some("link"), format!("link: writing the store's value to the node failed: {}", e.message)),
        }
    }
}
