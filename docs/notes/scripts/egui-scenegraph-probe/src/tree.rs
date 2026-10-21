//! The retained node arena: generation-checked handles that fit a JS number (< 2^53) and are never
//! reused (a slot whose generation reaches the limit is retired instead of recycled).

use egui::{Rect, Vec2};

pub type Handle = u64;

const IDX_BITS: u32 = 24; // 16M live nodes
const IDX_MASK: u64 = (1 << IDX_BITS) - 1;
const GEN_LIMIT: u32 = 1 << 29; // 24 + 29 = 53 bits

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Column,
    Row,
    Label,
    Button,
    Checkbox,
    TextEdit,
}

impl Kind {
    pub fn is_container(self) -> bool {
        matches!(self, Kind::Column | Kind::Row)
    }
}

pub struct Node {
    pub kind: Kind,
    pub parent: Option<Handle>,
    pub children: Vec<Handle>,
    /// Stable identity (component path + key), the source of the `egui::Id`.
    pub identity: u64,
    pub text: String,
    pub checked: bool,
    pub listen_clicked: bool,
    pub observe_text: bool,
    /// Taffy mirror node (taffy mode only).
    pub tnode: Option<taffy::NodeId>,
    /// Size this node took last frame (native-layout culling).
    pub last_size: Vec2,
    /// Screen rect last frame (event injection in tests, hit testing).
    pub screen_rect: Rect,
    /// Taffy mode: rect relative to the scroll content origin.
    pub layout_rect: Rect,
    /// Taffy mode: cached no-wrap text size (cleared when the text changes).
    pub intrinsic: std::cell::Cell<Option<Vec2>>,
}

impl Node {
    fn new(kind: Kind, identity: u64) -> Node {
        Node {
            kind,
            parent: None,
            children: Vec::new(),
            identity,
            text: String::new(),
            checked: false,
            listen_clicked: false,
            observe_text: false,
            tnode: None,
            last_size: Vec2::ZERO,
            screen_rect: Rect::NOTHING,
            layout_rect: Rect::NOTHING,
            intrinsic: std::cell::Cell::new(None),
        }
    }
}

struct Slot {
    gen_: u32,
    node: Option<Node>,
}

#[derive(Default)]
pub struct Arena {
    slots: Vec<Slot>,
    free: Vec<u32>,
    pub live: usize,
}

impl Arena {
    pub fn create(&mut self, kind: Kind, identity: u64) -> Handle {
        let idx = match self.free.pop() {
            Some(i) => i as usize,
            None => {
                self.slots.push(Slot { gen_: 0, node: None });
                self.slots.len() - 1
            }
        };
        let s = &mut self.slots[idx];
        s.gen_ += 1;
        s.node = Some(Node::new(kind, identity));
        self.live += 1;
        ((s.gen_ as u64) << IDX_BITS) | idx as u64
    }

    fn idx(&self, h: Handle) -> Option<usize> {
        let idx = (h & IDX_MASK) as usize;
        let gen_ = (h >> IDX_BITS) as u32;
        let s = self.slots.get(idx)?;
        (s.gen_ == gen_ && s.node.is_some()).then_some(idx)
    }

    pub fn get(&self, h: Handle) -> Option<&Node> {
        self.idx(h).and_then(|i| self.slots[i].node.as_ref())
    }

    pub fn get_mut(&mut self, h: Handle) -> Option<&mut Node> {
        let i = self.idx(h)?;
        self.slots[i].node.as_mut()
    }

    pub fn n(&self, h: Handle) -> &Node {
        self.get(h).expect("live handle")
    }

    pub fn n_mut(&mut self, h: Handle) -> &mut Node {
        self.get_mut(h).expect("live handle")
    }

    /// Destroys one handle: detaches it from its parent and orphans its children (the host destroys
    /// subtrees itself, child first or parent first).
    pub fn destroy(&mut self, h: Handle) -> Option<Node> {
        let i = self.idx(h)?;
        let node = self.slots[i].node.take()?;
        self.live -= 1;
        if self.slots[i].gen_ + 1 < GEN_LIMIT {
            self.free.push(i as u32);
        } // else: retired, so the handle space is never reused
        if let Some(p) = node.parent {
            if let Some(pn) = self.get_mut(p) {
                pn.children.retain(|&c| c != h);
            }
        }
        for &c in &node.children {
            if let Some(cn) = self.get_mut(c) {
                cn.parent = None;
            }
        }
        Some(node)
    }

    pub fn insert(&mut self, parent: Handle, child: Handle, before: Option<Handle>) -> Result<(), String> {
        if self.get(parent).is_none() {
            return Err(format!("stale or foreign handle {parent}"));
        }
        let cn = self.get(child).ok_or_else(|| format!("stale or foreign handle {child}"))?;
        if cn.parent.is_some() {
            return Err("child already has a parent".into());
        }
        let pos = match before {
            None => self.n(parent).children.len(),
            Some(b) => self
                .n(parent)
                .children
                .iter()
                .position(|&c| c == b)
                .ok_or("`before` is not a child of the parent")?,
        };
        self.n_mut(parent).children.insert(pos, child);
        self.n_mut(child).parent = Some(parent);
        Ok(())
    }

    pub fn remove(&mut self, parent: Handle, child: Handle) -> Result<(), String> {
        if self.get(child).and_then(|c| c.parent) != Some(parent) {
            return Err(format!("handle {child} is not inserted in {parent}"));
        }
        self.n_mut(parent).children.retain(|&c| c != child);
        self.n_mut(child).parent = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_handles_are_detected_and_never_reused() {
        let mut a = Arena::default();
        let h1 = a.create(Kind::Label, 1);
        a.destroy(h1);
        let h2 = a.create(Kind::Label, 2);
        assert_ne!(h1, h2);
        assert!(a.get(h1).is_none());
        assert!(a.get(h2).is_some());
        assert!(h2 < (1u64 << 53));
    }
}
