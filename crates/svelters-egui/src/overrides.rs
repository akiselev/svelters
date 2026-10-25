//! DZ0 (SV30): a tool's overrides of node props, for a designer's two-speed edit: while a gesture
//! lasts (a drag, a slider) the designer overrides the props it previews on the live nodes; on
//! release it writes the source once, and the swap that follows replaces the nodes (or, for a
//! style-only edit, the designer clears the overrides).
//!
//! While a prop is overridden the runtime's writes to it are held back: the last one is kept, and
//! clearing the override writes it, so the node shows what the app says again. Overrides belong
//! to nodes: a node freed by a swap takes its overrides with it. An override write is a host write
//! (`Origin::Js`): it announces nothing to `bind:`, so previewing a bound prop never changes app
//! state. CSS values a rule decides are forced with [`EguiBackend::force_style`].

use crate::backend::EguiBackend;
use crate::kinds;
use crate::scene::{self, Handle, Origin};
use svelters_abi::Value;

impl EguiBackend {
    /// Overrides prop `prop` (by name) of node `h` with `v` until [`EguiBackend::clear_override`];
    /// the runtime's writes meanwhile are held back. Overriding again replaces the value.
    pub fn set_override(&self, h: Handle, prop: &str, v: &Value) -> Result<(), String> {
        {
            let mut s = self.0.scene.borrow_mut();
            let id = s.live(h)?;
            let n = s.get(h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
            let (pid, def) = kinds::find_prop(n.kind as usize, prop).ok_or_else(|| format!("<{}> has no prop {prop}", n.tag()))?;
            let current = scene::get_prop(&n, def)?;
            s.set_value(id, pid, v, Origin::Js).map_err(|e| e.to_string())?;
            self.0.overrides.borrow_mut().entry((id, pid)).or_insert(current);
        }
        self.touched(self.root_of(h));
        Ok(())
    }

    /// Ends the override of `prop` on `h`: the node takes the value the runtime last wrote (or had
    /// before the override). `false` if it was not overridden.
    pub fn clear_override(&self, h: Handle, prop: &str) -> Result<bool, String> {
        let under = {
            let mut s = self.0.scene.borrow_mut();
            let id = s.live(h)?;
            let n = s.get(h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
            let (pid, _) = kinds::find_prop(n.kind as usize, prop).ok_or_else(|| format!("<{}> has no prop {prop}", n.tag()))?;
            let Some(under) = self.0.overrides.borrow_mut().remove(&(id, pid)) else { return Ok(false) };
            s.set_value(id, pid, &under, Origin::Js).map_err(|e| e.to_string())?;
            true
        };
        self.touched(self.root_of(h));
        Ok(under)
    }

    /// Ends every override (each live node takes its runtime value again) and every forced style.
    pub fn clear_overrides(&self) {
        let all: Vec<_> = self.0.overrides.borrow_mut().drain().collect();
        let mut roots = Vec::new();
        {
            let mut s = self.0.scene.borrow_mut();
            for ((id, pid), under) in all {
                if s.idx(id).is_some() && s.set_value(id, pid, &under, Origin::Js).is_ok() {
                    roots.push(s.core[s.ix(id)].root);
                }
            }
            roots.extend(s.clear_forced());
        }
        for r in roots {
            self.touched(Some(r));
        }
    }

    /// The live nodes' overridden props, as (node handle, prop name).
    pub fn overrides(&self) -> Vec<(Handle, &'static str)> {
        let s = self.0.scene.borrow();
        let mut out: Vec<_> = self
            .0
            .overrides
            .borrow()
            .keys()
            .filter_map(|&(id, pid)| {
                let i = s.idx(id)?;
                let def = kinds::prop_def(s.core[i].kind as usize, pid)?;
                Some((id.to_raw(), def.name))
            })
            .collect();
        out.sort();
        out
    }
}
