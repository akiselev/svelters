//! The icon registry (being ported from svelters-egui `icons.rs`).

use super::super::{PaintCx, StyleView};
use presto_base::color::Color;
use presto_base::geom::Point;
use std::sync::Arc;

pub struct IconRegistry;

impl IconRegistry {
    pub fn contains(&self, _key: &str) -> bool {
        false
    }
}

pub struct Slots;

impl Slots {
    pub fn of(_s: &StyleView) -> Slots {
        Slots
    }
}

/// The global registry when it has `key` (none is registered yet).
pub fn registered(_key: &super::IconKey) -> Option<Arc<IconRegistry>> {
    None
}

/// Whether an icon font of `family` is registered (none yet).
pub fn has_font(_family: &str) -> bool {
    false
}

pub fn paint(_cx: &mut PaintCx, _reg: &IconRegistry, _key: &str, _center: Point, _side: f32, _color: Color, _slots: &Slots) -> bool {
    false
}
