//! Node creation and tree edits: [`new_node`] and the scene half of the host's insert, remove and
//! destroy ops (their kind checks, repaint and window bookkeeping stay in `ops.rs`), plus freeing a
//! closed window's subtree.
//!
//! SG1 routes every edit through one path (`SceneMut` with an `Origin`: cycle check, owner check,
//! root cache, roots table, journal; ARCH-SCENE §6.4, §12) and makes `new_node` `tree::create`
//! (no egui id until attach). Writer lane: SCENE.

use super::dirty::invalidate;
use super::node::{Common, Node, Visual, W};
use super::{Arena, Handle};
use crate::kinds::KINDS;
use std::collections::HashMap;

pub fn new_node(kind: usize, handle: Handle) -> Node {
    let w = match KINDS[kind].id {
        "Window" => W::Window { title: String::new(), default_w: 480.0, default_h: 360.0 },
        "Box" => W::Box,
        "Label" => W::Label { text: String::new(), wrap: "wrap", selectable: false },
        "Button" => W::Button { text: String::new(), selected: false, frame: true, small: false },
        "Checkbox" => W::Checkbox { text: String::new(), checked: false },
        "TextEdit" => W::TextEdit { text: String::new(), hint: String::new(), multiline: false, password: false, char_limit: 0 },
        "Slider" => W::Slider { value: 0.0, min: 0.0, max: 100.0, step: 0.0, text: String::new(), show_value: true },
        other => unreachable!("not creatable: {other}"),
    };
    let mut c = Common::default();
    if matches!(w, W::Window { .. }) {
        c.padding = 8.0;
    }
    let mut n = Node {
        kind,
        handle,
        parent: None,
        children: Vec::new(),
        id: crate::ids::node_id(handle),
        c,
        w,
        listen: Vec::new(),
        observe: HashMap::new(),
        presented: false,
        want_focus: false,
        style: taffy::Style::default(),
        cache: taffy::Cache::new(),
        unrounded: taffy::Layout::with_order(0),
        layout: taffy::Layout::with_order(0),
        intrinsic: None,
        rect: egui::Rect::NOTHING,
        drawn: egui::Rect::NOTHING,
        had_focus: false,
        vis: Visual::default(),
    };
    crate::layout::restyle(&mut n);
    n
}

/// Takes `child` out of its parent's children (if it has a parent), invalidating the parent's layout.
pub(crate) fn detach(scene: &mut Arena, child: Handle) {
    if let Some(p) = scene.get(child).and_then(|n| n.parent) {
        if let Some(pn) = scene.get_mut(p) {
            pn.children.retain(|&c| c != child);
        }
        invalidate(scene, p);
        scene.n_mut(child).parent = None;
    }
}

/// Moves `child` under `parent`, before `before` (last when `None`). The kind checks are the
/// caller's. `Ok(false)`: nothing changed (`child` is `before` itself).
pub(crate) fn insert(scene: &mut Arena, parent: Handle, child: Handle, before: Option<Handle>) -> Result<bool, String> {
    // A parent inside the child's own subtree would make a cycle.
    let mut a = Some(parent);
    while let Some(x) = a {
        if x == child {
            return Err("cannot insert an element under its own descendant".into());
        }
        a = scene.n(x).parent;
    }
    if let Some(b) = before {
        if b == child {
            return Ok(false); // already in place
        }
        if scene.get(b).and_then(|n| n.parent) != Some(parent) {
            return Err(format!("handle #{b} is not a child of #{parent}"));
        }
    }
    detach(scene, child);
    let pn = scene.n_mut(parent);
    let at = before.and_then(|b| pn.children.iter().position(|&c| c == b)).unwrap_or(pn.children.len());
    pn.children.insert(at, child);
    scene.n_mut(child).parent = Some(parent);
    invalidate(scene, parent);
    Ok(true)
}

/// Takes `child` out of `parent`.
pub(crate) fn remove(scene: &mut Arena, parent: Handle, child: Handle) -> Result<(), String> {
    if !scene.contains(parent) || !scene.contains(child) {
        return Err("stale or foreign handle".into());
    }
    if scene.n(child).parent != Some(parent) {
        return Err(format!("handle #{child} is not a child of #{parent}"));
    }
    detach(scene, child);
    Ok(())
}

/// Detaches and frees `h`; `None` if it is not live. Children still attached are orphaned: the
/// host destroys them itself.
pub(crate) fn destroy(scene: &mut Arena, h: Handle) -> Option<Node> {
    if !scene.contains(h) {
        return None;
    }
    detach(scene, h);
    let kids = std::mem::take(&mut scene.n_mut(h).children);
    for c in kids {
        if let Some(cn) = scene.get_mut(c) {
            cn.parent = None;
        }
    }
    scene.free(h)
}

/// Frees `w` and its whole subtree; returns the freed handles in pre-order (empty if `w` is not
/// live).
pub(crate) fn free_subtree(scene: &mut Arena, w: Handle) -> Vec<Handle> {
    let mut dead = Vec::new();
    if !scene.contains(w) {
        return dead;
    }
    fn collect(s: &Arena, h: Handle, out: &mut Vec<Handle>) {
        out.push(h);
        for &c in &s.n(h).children {
            collect(s, c, out);
        }
    }
    collect(scene, w, &mut dead);
    for &h in &dead {
        scene.free(h);
    }
    dead
}
