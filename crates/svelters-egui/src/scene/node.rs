//! The node: today's one fat [`Node`] with its widget payload [`W`], common props [`Common`] and TR1
//! visual channels [`Visual`], plus [`Dirty`] (what a prop write needs).
//!
//! SG1 (ARCH-SCENE §4.3) splits it into the `NodeCore`, `LayoutBox`, `Payload` and `NodeCold` columns
//! and adds `NodeFlags`, `StateBits` and `Owner`. Writer lane: SCENE (RA1 adds payload variants).

use super::Handle;
use std::collections::HashMap;

/// Which egui widget a node shows, with the props the widget borrows mutably.
#[derive(Clone, Debug, PartialEq)]
pub enum W {
    Window { title: String, default_w: f64, default_h: f64 },
    Box,
    Label { text: String, wrap: &'static str, selectable: bool },
    Button { text: String, selected: bool, frame: bool, small: bool },
    Checkbox { text: String, checked: bool },
    TextEdit { text: String, hint: String, multiline: bool, password: bool, char_limit: i64 },
    Slider { value: f64, min: f64, max: f64, step: f64, text: String, show_value: bool },
    /// RA1: a provider kind's widget struct (`#[svelters::widget]`, `crate::provider`).
    Ext(crate::provider::ExtWidget),
}

/// Props every in-window node has (`Widget`), plus the box/window container props.
#[derive(Clone, Debug, PartialEq)]
pub struct Common {
    pub enabled: bool,
    pub visible: bool,
    pub tooltip: String,
    pub accessible_name: String,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
    pub max_width: Option<f64>,
    pub max_height: Option<f64>,
    pub flex_grow: f64,
    pub flex_shrink: f64,
    pub flex_basis: Option<f64>,
    pub align_self: &'static str,
    pub margin: f64,
    pub flex_direction: &'static str,
    pub flex_wrap: &'static str,
    pub gap: f64,
    pub row_gap: Option<f64>,
    pub column_gap: Option<f64>,
    pub padding: f64,
    pub align_items: &'static str,
    pub justify_content: &'static str,
}

impl Default for Common {
    fn default() -> Self {
        Common {
            enabled: true,
            visible: true,
            tooltip: String::new(),
            accessible_name: String::new(),
            width: None,
            height: None,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: None,
            align_self: "auto",
            margin: 0.0,
            flex_direction: "column",
            flex_wrap: "nowrap",
            gap: 0.0,
            row_gap: None,
            column_gap: None,
            padding: 0.0,
            align_items: "stretch",
            justify_content: "start",
        }
    }
}

pub struct Node {
    pub kind: usize,
    pub handle: Handle,
    pub parent: Option<Handle>,
    pub children: Vec<Handle>,
    pub id: egui::Id,
    pub c: Common,
    pub w: W,
    /// Listened signal ids.
    pub listen: Vec<u32>,
    /// Observed prop ids with their nesting count.
    pub observe: HashMap<u32, u32>,
    /// Window: `present` was called.
    pub presented: bool,
    /// TextEdit: focus requested by `request-focus`, applied next pass.
    pub want_focus: bool,
    // ---- layout (taffy on the arena, see layout.rs) ----
    pub style: taffy::Style,
    pub cache: taffy::Cache,
    pub unrounded: taffy::Layout,
    pub layout: taffy::Layout,
    /// No-wrap size of the leaf's content, measured with egui's fonts; None = measure again.
    pub intrinsic: Option<egui::Vec2>,
    /// Rect relative to the window's content origin (set by the layout pass).
    pub rect: egui::Rect,
    /// What the widget actually took last frame (screen coordinates).
    pub drawn: egui::Rect,
    /// Last frame's focus, for gained/lost-focus.
    pub had_focus: bool,
    /// TR1 visual channels (ABI v2.8 style ops): what a running transition shows.
    pub vis: Visual,
    /// RA1: state bits (ARCH-EMBED §8.4; accessors in `provider/state.rs`).
    pub state: crate::provider::StateBits,
    /// RA1: provider data (ARCH-EMBED §8.4; accessors in `scene/ext.rs`).
    pub ext: super::ext::ExtBlock,
}

/// The visual channels of a node (plans/milestones/TR1-transitions.md section 3.2). Paint-only
/// except `clip`, which also caps the node's layout size.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Visual {
    /// `SVL_STYLE_OPACITY`: multiplies the subtree's opacity.
    pub opacity: Option<f32>,
    /// `SVL_STYLE_TRANSFORM`: tx, ty (points), sx, sy, origin x, y (fractions of the size).
    pub transform: Option<[f32; 6]>,
    /// `SVL_STYLE_CLIP`: width, height the node occupies and is cut at (< 0: that axis free).
    pub clip: Option<[f32; 2]>,
    /// `SVL_STYLE_INERT`: the subtree reports no events.
    pub inert: bool,
}

impl Visual {
    pub fn is_identity(&self) -> bool {
        *self == Visual::default()
    }
}

impl Node {
    pub fn listens(&self, signal: u32) -> bool {
        signal != 0 && self.listen.contains(&signal)
    }
    pub fn observes(&self, prop: u32) -> bool {
        prop != 0 && self.observe.get(&prop).copied().unwrap_or(0) > 0
    }
    pub fn tag(&self) -> &'static str {
        crate::kinds::kind(self.kind).tag.unwrap_or("?")
    }
    pub fn is_window(&self) -> bool {
        matches!(self.w, W::Window { .. })
    }
    pub fn text(&self) -> &str {
        match &self.w {
            W::Label { text, .. } | W::Button { text, .. } | W::Checkbox { text, .. } | W::TextEdit { text, .. } | W::Slider { text, .. } => text,
            W::Window { title, .. } => title,
            W::Box | W::Ext(_) => "",
        }
    }
}

/// What a node needs after a prop write.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dirty {
    /// The node's taffy style changed: relayout from here up.
    pub style: bool,
    /// The leaf's measured content may have changed: measure again, relayout if it did.
    pub measure: bool,
}
