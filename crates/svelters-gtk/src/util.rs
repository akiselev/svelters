use gtk::prelude::*;

/// Widget after which a child inserted before `before` must be placed: the previous sibling of
/// `before`, or the last child of `parent` when appending.
pub fn prev_sibling(parent: &gtk::Widget, before: Option<&gtk::Widget>) -> Option<gtk::Widget> {
    match before {
        Some(b) => b.prev_sibling(),
        None => parent.last_child(),
    }
}

/// Position of the direct child of `parent` that is `w` or contains it.
#[allow(dead_code)] // used by generated slot code once a slot takes an index
pub fn direct_child_index(parent: &gtk::Widget, w: &gtk::Widget) -> Option<u32> {
    let mut cur = w.clone();
    while cur.parent().as_ref() != Some(parent) {
        cur = cur.parent()?;
    }
    let mut n = 0;
    let mut s = cur.prev_sibling();
    while let Some(x) = s {
        n += 1;
        s = x.prev_sibling();
    }
    Some(n)
}

/// Normalises the index types gtk4-rs returns (`i32` with -1, `u32`, `Option<u32>`).
#[allow(dead_code)]
pub trait Idx {
    fn idx(self) -> Option<u32>;
}
impl Idx for i32 {
    fn idx(self) -> Option<u32> {
        u32::try_from(self).ok()
    }
}
impl Idx for u32 {
    fn idx(self) -> Option<u32> {
        Some(self)
    }
}
impl Idx for Option<u32> {
    fn idx(self) -> Option<u32> {
        self
    }
}

/// `-1` (append) for `None`, as GTK's `gint position` parameters expect.
#[allow(dead_code)]
pub fn idx_i32(i: Option<u32>) -> i32 {
    i.map_or(-1, |x| x as i32)
}
