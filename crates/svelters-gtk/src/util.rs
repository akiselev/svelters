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

/// The direct child of `parent` that is `w` or contains it (the wrapper row of a GtkListBox child).
#[allow(dead_code)]
pub fn direct_child(parent: &gtk::Widget, w: &gtk::Widget) -> Option<gtk::Widget> {
    let mut cur = w.clone();
    while cur.parent().as_ref() != Some(parent) {
        cur = cur.parent()?;
    }
    Some(cur)
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

/// Position of `child` in a `GtkFixed`, read from its layout child's transform.
/// (`gtk_fixed_get_child_position` reports 0,0 on GTK 4.22 even after `gtk_fixed_move`.)
pub fn fixed_position(fixed: &gtk::Fixed, child: &gtk::Widget) -> (f64, f64) {
    fixed
        .layout_manager()
        .and_then(|lm| lm.layout_child(child).downcast::<gtk::FixedLayoutChild>().ok())
        .and_then(|lc| lc.transform())
        .map(|t| {
            let (x, y) = t.to_translate();
            (x as f64, y as f64)
        })
        .unwrap_or((0.0, 0.0))
}

/// A child-attribute value on its way to or from a native call (ABI v2.2).
#[derive(Clone, Debug, PartialEq)]
pub enum AttrV {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

#[allow(dead_code)] // each accessor is used by the generated attribute code that needs it
impl AttrV {
    pub fn as_bool(&self) -> Result<bool, String> {
        match self {
            AttrV::Bool(b) => Ok(*b),
            AttrV::Int(i) => Ok(*i != 0),
            other => Err(format!("expected a boolean, got {other:?}")),
        }
    }

    pub fn as_i64(&self) -> Result<i64, String> {
        match self {
            AttrV::Int(i) => Ok(*i),
            AttrV::Float(f) if f.fract() == 0.0 && f.abs() < 9.0e15 => Ok(*f as i64),
            other => Err(format!("expected an integer, got {other:?}")),
        }
    }

    pub fn as_i32(&self) -> Result<i32, String> {
        i32::try_from(self.as_i64()?).map_err(|_| "integer out of range".to_string())
    }

    pub fn as_u32(&self) -> Result<u32, String> {
        u32::try_from(self.as_i64()?).map_err(|_| "integer out of range".to_string())
    }

    pub fn as_f64(&self) -> Result<f64, String> {
        match self {
            AttrV::Float(f) => Ok(*f),
            AttrV::Int(i) => Ok(*i as f64),
            other => Err(format!("expected a number, got {other:?}")),
        }
    }

    pub fn as_f32(&self) -> Result<f32, String> {
        Ok(self.as_f64()? as f32)
    }

    pub fn as_str(&self) -> Result<&str, String> {
        match self {
            AttrV::Str(s) => Ok(s),
            other => Err(format!("expected a string, got {other:?}")),
        }
    }
}
