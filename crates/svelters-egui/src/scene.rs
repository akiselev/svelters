//! The retained scene: a generational arena of typed nodes (plans/research/EGUI.md §1.1).
//!
//! Host ops only edit this arena; they never touch egui. A handle is `generation << 24 | idx` (below 2^53,
//! never 0): a slot whose generation reaches 2^29 is retired instead of recycled, so handles are
//! never reused and a stale one fails with an error.

use crate::kinds::{self, Def, KINDS, P, Ty};
use std::collections::HashMap;
use svelters_abi::{Value, svl_handle};

pub type Handle = svl_handle;

const IDX_BITS: u32 = 24;
const IDX_MASK: u64 = (1 << IDX_BITS) - 1;
const MAX_GEN: u32 = 1 << 29;

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
}

impl Node {
    pub fn listens(&self, signal: u32) -> bool {
        signal != 0 && self.listen.contains(&signal)
    }
    pub fn observes(&self, prop: u32) -> bool {
        prop != 0 && self.observe.get(&prop).copied().unwrap_or(0) > 0
    }
    pub fn tag(&self) -> &'static str {
        KINDS[self.kind].tag.unwrap_or("?")
    }
    pub fn is_window(&self) -> bool {
        matches!(self.w, W::Window { .. })
    }
    pub fn text(&self) -> &str {
        match &self.w {
            W::Label { text, .. } | W::Button { text, .. } | W::Checkbox { text, .. } | W::TextEdit { text, .. } | W::Slider { text, .. } => text,
            W::Window { title, .. } => title,
            W::Box => "",
        }
    }
}

struct Slot {
    generation: u32,
    node: Option<Node>,
}

#[derive(Default)]
pub struct Arena {
    slots: Vec<Slot>,
    free: Vec<u32>,
}

impl Arena {
    fn index(&self, h: Handle) -> Option<usize> {
        let idx = (h & IDX_MASK) as usize;
        let generation = (h >> IDX_BITS) as u32;
        let s = self.slots.get(idx)?;
        (s.generation == generation && s.node.is_some()).then_some(idx)
    }

    pub fn get(&self, h: Handle) -> Option<&Node> {
        self.index(h).and_then(|i| self.slots[i].node.as_ref())
    }

    pub fn get_mut(&mut self, h: Handle) -> Option<&mut Node> {
        let i = self.index(h)?;
        self.slots[i].node.as_mut()
    }

    pub fn n(&self, h: Handle) -> &Node {
        self.get(h).expect("live handle")
    }

    pub fn n_mut(&mut self, h: Handle) -> &mut Node {
        self.get_mut(h).expect("live handle")
    }

    pub fn contains(&self, h: Handle) -> bool {
        self.index(h).is_some()
    }

    /// Allocates a slot and builds the node for its handle.
    pub fn alloc(&mut self, make: impl FnOnce(Handle) -> Node) -> Handle {
        let idx = match self.free.pop() {
            Some(i) => i as usize,
            None => {
                self.slots.push(Slot { generation: 0, node: None });
                self.slots.len() - 1
            }
        };
        let s = &mut self.slots[idx];
        s.generation += 1;
        let h = ((s.generation as u64) << IDX_BITS) | idx as u64;
        s.node = Some(make(h));
        h
    }

    /// Frees a node's slot (its handle becomes stale forever).
    pub fn free(&mut self, h: Handle) -> Option<Node> {
        let i = self.index(h)?;
        let s = &mut self.slots[i];
        let node = s.node.take();
        if s.generation < MAX_GEN {
            self.free.push(i as u32);
        }
        node
    }

    pub fn len(&self) -> usize {
        self.slots.iter().filter(|s| s.node.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn handles(&self) -> Vec<Handle> {
        self.slots.iter().filter_map(|s| s.node.as_ref().map(|n| n.handle)).collect()
    }
}

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
        id: egui::Id::unique(("svl", handle)),
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
    };
    crate::layout::restyle(&mut n);
    n
}

// ---- values ----

fn as_bool(v: &Value) -> Result<bool, String> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::I64(i) => Ok(*i != 0),
        other => Err(format!("expected a boolean, got {other:?}")),
    }
}

fn as_f64(v: &Value) -> Result<f64, String> {
    match v {
        Value::F64(f) => Ok(*f),
        Value::I64(i) => Ok(*i as f64),
        Value::U64(u) => Ok(*u as f64),
        other => Err(format!("expected a number, got {other:?}")),
    }
}

fn as_i64(v: &Value) -> Result<i64, String> {
    match v {
        Value::I64(i) => Ok(*i),
        Value::F64(f) if f.fract() == 0.0 => Ok(*f as i64),
        other => Err(format!("expected an integer, got {other:?}")),
    }
}

fn as_str(v: &Value) -> Result<String, String> {
    match v {
        Value::Str(s) => Ok(s.clone()),
        Value::Null => Ok(String::new()),
        Value::I64(i) => Ok(i.to_string()),
        Value::F64(f) => Ok(f.to_string()),
        other => Err(format!("expected a string, got {other:?}")),
    }
}

fn as_enum(v: &Value, values: &'static [&'static str]) -> Result<&'static str, String> {
    let s = as_str(v)?;
    values.iter().copied().find(|x| *x == s).ok_or_else(|| format!("`{s}` is not one of {}", values.join(", ")))
}

fn as_len(v: &Value) -> Result<Option<f64>, String> {
    match v {
        Value::Null => Ok(None),
        v => as_f64(v).map(Some),
    }
}

fn len_value(v: Option<f64>) -> Value {
    v.map(Value::F64).unwrap_or(Value::Null)
}

/// What a node needs after a prop write.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dirty {
    /// The node's taffy style changed: relayout from here up.
    pub style: bool,
    /// The leaf's measured content may have changed: measure again, relayout if it did.
    pub measure: bool,
}

/// Writes a prop. `Null` resets it to its default.
pub fn set_prop(n: &mut Node, def: &kinds::PropDef, v: &Value) -> Result<Dirty, String> {
    let v = if matches!(v, Value::Null) && !def.nullable { default_value(def) } else { v.clone() };
    let v = &v;
    let mut d = Dirty::default();
    let c = &mut n.c;
    match (def.key, &mut n.w) {
        (P::Enabled, _) => c.enabled = as_bool(v)?,
        (P::Visible, _) => {
            c.visible = as_bool(v)?;
            d.style = true;
        }
        (P::Tooltip, _) => c.tooltip = as_str(v)?,
        (P::AccessibleName, _) => c.accessible_name = as_str(v)?,
        (P::Width, _) => c.width = as_len(v)?,
        (P::Height, _) => c.height = as_len(v)?,
        (P::MinWidth, _) => c.min_width = as_len(v)?,
        (P::MinHeight, _) => c.min_height = as_len(v)?,
        (P::MaxWidth, _) => c.max_width = as_len(v)?,
        (P::MaxHeight, _) => c.max_height = as_len(v)?,
        (P::FlexGrow, _) => c.flex_grow = as_f64(v)?,
        (P::FlexShrink, _) => c.flex_shrink = as_f64(v)?,
        (P::FlexBasis, _) => c.flex_basis = as_len(v)?,
        (P::AlignSelf, _) => c.align_self = as_enum(v, kinds::ALIGN_SELF)?,
        (P::Margin, _) => c.margin = as_f64(v)?,
        (P::FlexDirection, _) => c.flex_direction = as_enum(v, kinds::DIRECTION)?,
        (P::FlexWrap, _) => c.flex_wrap = as_enum(v, kinds::WRAP)?,
        (P::Gap, _) => c.gap = as_f64(v)?,
        (P::RowGap, _) => c.row_gap = as_len(v)?,
        (P::ColumnGap, _) => c.column_gap = as_len(v)?,
        (P::Padding, _) => c.padding = as_f64(v)?,
        (P::AlignItems, _) => c.align_items = as_enum(v, kinds::ALIGN)?,
        (P::JustifyContent, _) => c.justify_content = as_enum(v, kinds::JUSTIFY)?,
        (P::Title, W::Window { title, .. }) => *title = as_str(v)?,
        (P::DefaultWidth, W::Window { default_w, .. }) => *default_w = as_f64(v)?,
        (P::DefaultHeight, W::Window { default_h, .. }) => *default_h = as_f64(v)?,
        (P::Text, W::Label { text, .. } | W::Button { text, .. } | W::Checkbox { text, .. } | W::TextEdit { text, .. } | W::Slider { text, .. }) => {
            *text = as_str(v)?;
            d.measure = true;
        }
        (P::WrapMode, W::Label { wrap, .. }) => {
            *wrap = as_enum(v, kinds::WRAP_MODE)?;
            d.measure = true;
        }
        (P::Selectable, W::Label { selectable, .. }) => *selectable = as_bool(v)?,
        (P::Selected, W::Button { selected, .. }) => *selected = as_bool(v)?,
        (P::Frame, W::Button { frame, .. }) => *frame = as_bool(v)?,
        (P::Small, W::Button { small, .. }) => {
            *small = as_bool(v)?;
            d.measure = true;
        }
        (P::Checked, W::Checkbox { checked, .. }) => *checked = as_bool(v)?,
        (P::HintText, W::TextEdit { hint, .. }) => *hint = as_str(v)?,
        (P::Multiline, W::TextEdit { multiline, .. }) => {
            *multiline = as_bool(v)?;
            d.measure = true;
        }
        (P::Password, W::TextEdit { password, .. }) => *password = as_bool(v)?,
        (P::CharLimit, W::TextEdit { char_limit, .. }) => *char_limit = as_i64(v)?.max(0),
        (P::Value, W::Slider { value, .. }) => *value = as_f64(v)?,
        (P::Min, W::Slider { min, .. }) => *min = as_f64(v)?,
        (P::Max, W::Slider { max, .. }) => *max = as_f64(v)?,
        (P::Step, W::Slider { step, .. }) => *step = as_f64(v)?.max(0.0),
        (P::ShowValue, W::Slider { show_value, .. }) => {
            *show_value = as_bool(v)?;
            d.measure = true;
        }
        (key, _) => return Err(format!("{key:?} is not a prop of <{}>", n.tag())),
    }
    if def.layout {
        d.style = true;
        crate::layout::restyle(n);
    }
    Ok(d)
}

pub fn get_prop(n: &Node, def: &kinds::PropDef) -> Result<Value, String> {
    let c = &n.c;
    let s = |x: &str| Value::Str(x.to_string());
    Ok(match (def.key, &n.w) {
        (P::Enabled, _) => Value::Bool(c.enabled),
        (P::Visible, _) => Value::Bool(c.visible),
        (P::Tooltip, _) => s(&c.tooltip),
        (P::AccessibleName, _) => s(&c.accessible_name),
        (P::Width, _) => len_value(c.width),
        (P::Height, _) => len_value(c.height),
        (P::MinWidth, _) => len_value(c.min_width),
        (P::MinHeight, _) => len_value(c.min_height),
        (P::MaxWidth, _) => len_value(c.max_width),
        (P::MaxHeight, _) => len_value(c.max_height),
        (P::FlexGrow, _) => Value::F64(c.flex_grow),
        (P::FlexShrink, _) => Value::F64(c.flex_shrink),
        (P::FlexBasis, _) => len_value(c.flex_basis),
        (P::AlignSelf, _) => s(c.align_self),
        (P::Margin, _) => Value::F64(c.margin),
        (P::FlexDirection, _) => s(c.flex_direction),
        (P::FlexWrap, _) => s(c.flex_wrap),
        (P::Gap, _) => Value::F64(c.gap),
        (P::RowGap, _) => len_value(c.row_gap),
        (P::ColumnGap, _) => len_value(c.column_gap),
        (P::Padding, _) => Value::F64(c.padding),
        (P::AlignItems, _) => s(c.align_items),
        (P::JustifyContent, _) => s(c.justify_content),
        (P::Title, W::Window { title, .. }) => s(title),
        (P::DefaultWidth, W::Window { default_w, .. }) => Value::F64(*default_w),
        (P::DefaultHeight, W::Window { default_h, .. }) => Value::F64(*default_h),
        (P::Text, _) => s(n.text()),
        (P::WrapMode, W::Label { wrap, .. }) => s(wrap),
        (P::Selectable, W::Label { selectable, .. }) => Value::Bool(*selectable),
        (P::Selected, W::Button { selected, .. }) => Value::Bool(*selected),
        (P::Frame, W::Button { frame, .. }) => Value::Bool(*frame),
        (P::Small, W::Button { small, .. }) => Value::Bool(*small),
        (P::Checked, W::Checkbox { checked, .. }) => Value::Bool(*checked),
        (P::HintText, W::TextEdit { hint, .. }) => s(hint),
        (P::Multiline, W::TextEdit { multiline, .. }) => Value::Bool(*multiline),
        (P::Password, W::TextEdit { password, .. }) => Value::Bool(*password),
        (P::CharLimit, W::TextEdit { char_limit, .. }) => Value::I64(*char_limit),
        (P::Value, W::Slider { value, .. }) => Value::F64(*value),
        (P::Min, W::Slider { min, .. }) => Value::F64(*min),
        (P::Max, W::Slider { max, .. }) => Value::F64(*max),
        (P::Step, W::Slider { step, .. }) => Value::F64(*step),
        (P::ShowValue, W::Slider { show_value, .. }) => Value::Bool(*show_value),
        (key, _) => return Err(format!("{key:?} is not a prop of <{}>", n.tag())),
    })
}

pub fn default_value(def: &kinds::PropDef) -> Value {
    match def.default {
        Def::Null => Value::Null,
        Def::Bool(b) => Value::Bool(b),
        Def::F64(f) => Value::F64(f),
        Def::I64(i) => Value::I64(i),
        Def::Str(s) => Value::Str(s.to_string()),
    }
}

/// The ABI tag of a prop type.
pub fn tag_of(ty: Ty) -> u16 {
    match ty {
        Ty::Bool => svelters_abi::SVL_BOOL,
        Ty::F64 => svelters_abi::SVL_F64,
        Ty::I64 => svelters_abi::SVL_I64,
        Ty::Str | Ty::Enum(_) => svelters_abi::SVL_STR,
    }
}
