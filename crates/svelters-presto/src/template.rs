//! Native templates (compiler-and-js §3.2): a fragment's static part instantiated in one native call.
//! Handles are allocated contiguously, so node `k` of a template is `base + k` and the generated code
//! needs no navigation; `destroy_range(base, n)` releases a non-hole subtree in one call.
//!
//! The work is today's op sequence, in CF2's init order: every node is created and its static props
//! and classes set, then the children are inserted into their parents (before the template's roots
//! are), then the listeners are registered. The roots are left for the caller to insert
//! (they go before an anchor the template does not know). A failure frees what was created, so a
//! failed instantiation leaves no trace (HMR's `dispose` path).
//!
//! The wire form of a table and the host/ABI op (`host.instantiate`) are lanes COMPILER's and
//! RUNTIME's; this is the backend's side, on the decoded [`Template`]. Templates are module-scoped
//! data registered by content hash with refcounts, so a module version's templates go when it does.

use crate::backend::PrestoBackend;
use std::collections::HashMap;
use svelters_abi::*;

/// One node of a template.
#[derive(Clone, Debug, PartialEq)]
pub struct TemplateNode {
    /// Wire kind id.
    pub kind: svl_id,
    /// Index of the parent node in the template; `None` for a root.
    pub parent: Option<u32>,
    /// Wire slot id in the parent (0: the default slot).
    pub slot: svl_id,
    /// Static props: (wire prop id, constant).
    pub props: Vec<(svl_id, Value)>,
    /// Static classes (the scope class included), space-separated.
    pub classes: Option<String>,
    /// Listener registrations: (wire signal id, `listen_ex` flags).
    pub listens: Vec<(svl_id, u32)>,
}

/// A decoded template table: nodes in creation order, every parent before its children.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Template {
    pub nodes: Vec<TemplateNode>,
}

impl Template {
    /// Checks the shape: parents come first; at least one node.
    pub fn validate(&self) -> StrResult<()> {
        if self.nodes.is_empty() {
            return Err("an empty template".into());
        }
        for (i, n) in self.nodes.iter().enumerate() {
            if let Some(p) = n.parent
                && p as usize >= i
            {
                return Err(format!("template node {i}: parent {p} does not come before it"));
            }
        }
        Ok(())
    }

    /// The indices of the roots (nodes without a parent), in order.
    pub fn roots(&self) -> impl Iterator<Item = u32> + '_ {
        self.nodes.iter().enumerate().filter(|(_, n)| n.parent.is_none()).map(|(i, _)| i as u32)
    }
}

/// Registered templates by content hash, with refcounts.
#[derive(Default)]
pub struct Templates {
    by_hash: HashMap<u64, (std::rc::Rc<Template>, u32)>,
}

impl Templates {
    pub fn register(&mut self, hash: u64, t: Template) -> StrResult<()> {
        t.validate()?;
        let e = self.by_hash.entry(hash).or_insert_with(|| (std::rc::Rc::new(t), 0));
        e.1 += 1;
        Ok(())
    }

    /// Drops one reference; the template goes with the last.
    pub fn release(&mut self, hash: u64) {
        if let Some(e) = self.by_hash.get_mut(&hash) {
            e.1 -= 1;
            if e.1 == 0 {
                self.by_hash.remove(&hash);
            }
        }
    }

    pub fn get(&self, hash: u64) -> Option<std::rc::Rc<Template>> {
        self.by_hash.get(&hash).map(|e| e.0.clone())
    }

    pub fn len(&self) -> usize {
        self.by_hash.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_hash.is_empty()
    }
}

impl PrestoBackend {
    /// Instantiates `t`: returns the base handle; node `k` is `base + k`.
    pub fn instantiate(&self, t: &Template) -> StrResult<svl_handle> {
        t.validate()?;
        let n = t.nodes.len() as u64;
        let base = self.0.handles.borrow_mut().reserve(n);
        let mut made = 0u64;
        let r = (|| -> StrResult<()> {
            for (k, node) in t.nodes.iter().enumerate() {
                let h = base + k as u64;
                self.create_at(h, node.kind)?;
                made += 1;
                for (prop, v) in &node.props {
                    self.write_prop(h, *prop, v)?;
                }
                if let Some(c) = &node.classes {
                    self.style_op(SVL_STYLE_CLASSES, h, Some(c))?;
                }
            }
            // children into their parents in template order (appending keeps sibling order); the
            // roots are the caller's to insert, so the whole instance is built before it is attached
            for (k, node) in t.nodes.iter().enumerate() {
                if let Some(p) = node.parent {
                    self.write_insert(base + p as u64, base + k as u64, None, node.slot)?;
                }
            }
            for (k, node) in t.nodes.iter().enumerate() {
                for &(sig, flags) in &node.listens {
                    self.write_listen(base + k as u64, sig, true, flags)?;
                }
            }
            Ok(())
        })();
        if let Err(e) = r {
            for k in (0..made).rev() {
                self.free(base + k);
            }
            return Err(e);
        }
        Ok(base)
    }

    /// Frees handles `base .. base + n` (a template instance's non-hole subtree). Handles already
    /// gone are skipped.
    pub fn destroy_range(&self, base: svl_handle, n: u64) {
        for k in (0..n).rev() {
            if self.0.borrowed.borrow().contains(&(base + k)) {
                continue;
            }
            self.free(base + k);
        }
    }
}
