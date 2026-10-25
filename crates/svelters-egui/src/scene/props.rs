//! Built-in prop get/set: the ABI value conversions, [`set_prop`] and [`get_prop`] over the typed
//! fields of a node ([`Common`] and [`W`]), [`default_value`] and [`tag_of`].
//!
//! SG1: `set_prop` returns [`Dirty`] bits (LAYOUT for layout props, MEASURE for content a leaf
//! measures); the scene's write path (`access.rs`, `Scene::set_value`) applies them, restyles and
//! announces. The typed built-in keys are in `keys.rs`. Writer lane: WIDGETS.

use super::node::{Common, Dirty, Node, W};
use crate::kinds::{self, Def, P, Ty};
use svelters_abi::Value;

// ---- values ----

pub(crate) fn as_bool(v: &Value) -> Result<bool, String> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::I64(i) => Ok(*i != 0),
        other => Err(format!("expected a boolean, got {other:?}")),
    }
}

pub(crate) fn as_f64(v: &Value) -> Result<f64, String> {
    match v {
        Value::F64(f) => Ok(*f),
        Value::I64(i) => Ok(*i as f64),
        Value::U64(u) => Ok(*u as f64),
        other => Err(format!("expected a number, got {other:?}")),
    }
}

pub(crate) fn as_i64(v: &Value) -> Result<i64, String> {
    match v {
        Value::I64(i) => Ok(*i),
        Value::F64(f) if f.fract() == 0.0 => Ok(*f as i64),
        other => Err(format!("expected an integer, got {other:?}")),
    }
}

pub(crate) fn as_str(v: &Value) -> Result<String, String> {
    match v {
        Value::Str(s) => Ok(s.clone()),
        Value::Null => Ok(String::new()),
        Value::I64(i) => Ok(i.to_string()),
        Value::F64(f) => Ok(f.to_string()),
        other => Err(format!("expected a string, got {other:?}")),
    }
}

pub(crate) fn as_enum(v: &Value, values: &'static [&'static str]) -> Result<&'static str, String> {
    let s = as_str(v)?;
    values.iter().copied().find(|x| *x == s).ok_or_else(|| format!("`{s}` is not one of {}", values.join(", ")))
}

pub(crate) fn as_len(v: &Value) -> Result<Option<f64>, String> {
    match v {
        Value::Null => Ok(None),
        v => as_f64(v).map(Some),
    }
}

pub(crate) fn as_str_list(v: &Value) -> Result<Vec<String>, String> {
    match v {
        Value::Null => Ok(Vec::new()),
        Value::List(items) => items.iter().map(as_str).collect(),
        // a static attribute: whitespace-separated (the compiler's `strlist` form)
        Value::Str(s) => Ok(s.split_ascii_whitespace().map(str::to_string).collect()),
        other => Err(format!("expected a list of strings, got {other:?}")),
    }
}

pub(crate) fn as_weight(v: &Value) -> Result<i64, String> {
    let w = as_f64(v)?;
    if !(1.0..=1000.0).contains(&w) {
        return Err(format!("font-weight {w} is outside 1..=1000"));
    }
    Ok(w.round() as i64)
}

pub(crate) fn len_value(v: Option<f64>) -> Value {
    v.map(Value::F64).unwrap_or(Value::Null)
}

/// Writes a prop. `Null` resets it to its default.
/// True for the props stored in [`Common`] (every kind has them).
pub(crate) fn is_common(key: P) -> bool {
    use P::*;
    matches!(
        key,
        Enabled | Visible | Tooltip | AccessibleName | Anchor | Width | Height | MinWidth | MinHeight | MaxWidth | MaxHeight | FlexGrow | FlexShrink | FlexBasis | AlignSelf | Margin | Position | Left | Top | Right | Bottom | FlexDirection | FlexWrap | Gap | RowGap | ColumnGap | Padding | AlignItems | JustifyContent | Invalid | DragType | DragData | DropTypes | Autofocus | FocusScope | Default | Cancel | DialogRole | StatusTip | StatusSection | AnchorScope
    )
}

/// Writes a prop into a node's common props `c` and widget fields `w`. `Null` resets it to its
/// default. Returns what the node needs (the caller restyles on LAYOUT).
pub fn set_prop(c: &mut Common, w: &mut W, def: &kinds::PropDef, v: &Value) -> Result<Dirty, String> {
    if let P::Ext(i) = def.key {
        // RA1: a provider kind's field; OW0: an own widget's extra prop, which the built-in egui
        // wrapper of the same kind ignores
        return if matches!(w, W::Ext(_)) { crate::provider::set_prop(w, i as usize, v) } else { Ok(Dirty::EMPTY) };
    }
    if matches!(w, W::Ext(_)) && !is_common(def.key) {
        if matches!(def.ty, Ty::StrList) {
            // OW3: a list prop takes the row's forms (null, a list, a static whitespace-separated string)
            let l = Value::List(as_str_list(v)?.into_iter().map(Value::Str).collect());
            return crate::provider::set_replaced_prop(w, def, &l);
        }
        return crate::provider::set_replaced_prop(w, def, v); // OW0: an own widget's field
    }
    let v = if matches!(v, Value::Null) && !def.nullable { default_value(def) } else { v.clone() };
    let v = &v;
    // WG2b: its kinds' own props (`crate::elements`); the `Widget`/`Box` props fall through
    if let Some(r) = crate::elements::set_prop(w, def, v) {
        return r.map(|mut d| {
            if def.layout {
                d.insert(Dirty::LAYOUT);
            }
            d
        });
    }
    let mut d = Dirty::default();
    match (def.key, &mut *w) {
        (P::Enabled, _) => c.enabled = as_bool(v)?,
        (P::Visible, _) => {
            c.visible = as_bool(v)?;
            d.insert(Dirty::LAYOUT);
        }
        (P::Tooltip, _) => c.tooltip = as_str(v)?,
        (P::AccessibleName, _) => c.accessible_name = as_str(v)?,
        (P::Anchor, _) => c.anchor = as_str(v)?,
        (P::AnchorScope, _) => c.anchor_scope = Some(as_str(v)?).filter(|t| !t.is_empty()).map(String::into_boxed_str),
        (P::DialogRole, _) => c.dialog_role = as_enum(v, crate::kinds::DIALOG_ROLE)?,
        (P::StatusTip, _) => c.status_tip = Some(as_str(v)?).filter(|t| !t.is_empty()).map(String::into_boxed_str),
        (P::StatusSection, _) => c.status_section = as_enum(v, crate::kinds::STATUS_SECTION)?,
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
        (P::Position, _) => c.position = as_enum(v, kinds::POSITION)?,
        (P::Left, _) => c.left = as_len(v)?,
        (P::Top, _) => c.top = as_len(v)?,
        (P::Right, _) => c.right = as_len(v)?,
        (P::Bottom, _) => c.bottom = as_len(v)?,
        (P::FlexDirection, _) => c.flex_direction = as_enum(v, kinds::DIRECTION)?,
        (P::FlexWrap, _) => c.flex_wrap = as_enum(v, kinds::WRAP)?,
        (P::Gap, _) => c.gap = as_f64(v)?,
        (P::RowGap, _) => c.row_gap = as_len(v)?,
        (P::ColumnGap, _) => c.column_gap = as_len(v)?,
        (P::Padding, _) => c.padding = as_f64(v)?,
        (P::AlignItems, _) => c.align_items = as_enum(v, kinds::ALIGN)?,
        (P::JustifyContent, _) => c.justify_content = as_enum(v, kinds::JUSTIFY)?,
        (P::Autofocus, _) => c.autofocus = as_bool(v)?,
        (P::FocusScope, _) => c.focus_scope = as_enum(v, kinds::FOCUS_SCOPE)?,
        (P::Default, W::Button { .. } | W::Ext(_)) => c.default_button = as_bool(v)?,
        (P::Cancel, W::Button { .. } | W::Ext(_)) => c.cancel_button = as_bool(v)?,
        (P::Invalid, _) => c.invalid = as_bool(v)?,
        // WG4b: drag and drop on any node (`kinds/dnd.rs`), before the per-payload catch-alls
        (P::DragType | P::DragData | P::DropTypes, _) => crate::kinds::dnd::set(c, def.key, as_str(v)?),
        // WG2a: the menu family and the overlays (`crate::menus`, `crate::overlays`)
        (key, W::Menu(m)) => d = crate::menus::set_prop(m, key, v)?,
        (key, W::Overlay(o)) => crate::overlays::set_prop(o, key, v)?,
        (P::Title, W::Window { title, .. }) => *title = as_str(v)?,
        (P::DefaultWidth, W::Window { default_w, .. }) => *default_w = as_f64(v)?,
        (P::DefaultHeight, W::Window { default_h, .. }) => *default_h = as_f64(v)?,
        (
            P::Text,
            W::Label { text, .. }
            | W::Button { text, .. }
            | W::Checkbox { text, .. }
            | W::TextEdit { text, .. }
            | W::ToggleButton { text, .. }
            | W::CollapsingHeader { text, .. },
        ) => {
            *text = as_str(v)?;
            d.insert(Dirty::MEASURE);
        }
        // RT2: runs (`crate::rich`; `Scene::set_value` remeasures their label)
        (P::Text, W::Run { text, .. }) => *text = as_str(v)?,
        (P::FontWeight, W::Run { weight, .. }) => *weight = if as_i64(v)? == 0 { 0 } else { as_weight(v)? },
        (P::Href, W::Run { href, .. }) => *href = as_str(v)?,
        (P::WrapMode, W::Label { wrap, .. }) => {
            *wrap = as_enum(v, kinds::WRAP_MODE)?;
            d.insert(Dirty::MEASURE);
        }
        (P::Selectable, W::Label { selectable, .. }) => *selectable = as_bool(v)?,
        (P::Selected, W::Button { selected, .. } | W::Icon { selected, .. }) => *selected = as_bool(v)?,
        (P::Frame, W::Button { frame, .. } | W::ToggleButton { frame, .. }) => *frame = as_bool(v)?,
        (P::Small, W::Button { small, .. }) => {
            *small = as_bool(v)?;
            d.insert(Dirty::MEASURE);
        }
        (P::Checked, W::Checkbox { checked, .. } | W::ToggleButton { checked, .. }) => *checked = as_bool(v)?,
        (P::HintText, W::TextEdit { hint, .. } | W::ComboBox { hint, .. }) => *hint = as_str(v)?,
        (P::Multiline, W::TextEdit { multiline, .. }) => {
            *multiline = as_bool(v)?;
            d.insert(Dirty::MEASURE);
        }
        (P::Password, W::TextEdit { password, .. }) => *password = as_bool(v)?,
        (P::CharLimit, W::TextEdit { char_limit, .. }) => *char_limit = as_i64(v)?.max(0),
        (P::ListSelected | P::ListSelection | P::ListLayout | P::SelectedIds | P::RowDragType | P::RowDropTypes | P::ExpandOn | P::RootDecorated | P::CurrentItem | P::InternalMove, W::VirtualList(l)) => crate::kinds::virtuallist::set(l, def.key, v)?,
        (P::RowHeight | P::TreeIndent | P::UniformRows, W::VirtualList(l)) => {
            crate::kinds::virtuallist::set(l, def.key, v)?;
            d.insert(Dirty::LAYOUT);
        }
        // ---- WG1 ----
        (P::FontWeight, W::Label { weight, .. } | W::Button { weight, .. } | W::ToggleButton { weight, .. } | W::Icon { weight, .. }) => {
            *weight = as_weight(v)?;
            d.insert(Dirty::MEASURE);
        }
        (P::Radio, W::ToggleButton { radio, .. }) => *radio = as_bool(v)?,
        (P::Options, W::ComboBox { options, .. }) => {
            *options = as_str_list(v)?;
            d.insert(Dirty::MEASURE);
        }
        (P::SelectedIndex, W::ComboBox { selected, .. }) => *selected = as_i64(v)?,
        (P::Orientation, W::Separator { vertical }) => {
            *vertical = as_enum(v, kinds::ORIENTATION)? == "vertical";
            d.insert(Dirty::MEASURE);
        }
        (P::Name, W::Icon { name, .. }) => {
            *name = as_str(v)?;
            d.insert(Dirty::MEASURE);
        }
        (P::Glyph, W::Icon { glyph, .. }) => {
            *glyph = as_str(v)?;
            d.insert(Dirty::MEASURE);
        }
        (P::Family, W::Icon { family, .. }) => {
            *family = as_str(v)?;
            d.insert(Dirty::MEASURE);
        }
        (P::Size, W::Icon { size, .. }) => {
            *size = as_len(v)?.filter(|s| *s > 0.0);
            d.insert(Dirty::MEASURE);
        }
        (P::Source, W::Icon { source, .. }) => {
            *source = as_str(v)?;
            d.insert(Dirty::MEASURE);
        }
        (P::Tint, W::Icon { tint, .. }) => *tint = as_bool(v)?,
        (P::Frame, W::Icon { frame, .. }) => {
            *frame = as_bool(v)?;
            d.insert(Dirty::MEASURE);
        }
        (P::GridTemplateColumns, W::Grid { columns, .. }) => {
            let s = as_str(v)?;
            crate::layout::parse_grid_template(&s)?;
            *columns = s;
        }
        (P::JustifyItems, W::Grid { justify_items, .. }) => *justify_items = as_enum(v, kinds::JUSTIFY_ITEMS)?,
        (P::Open, W::CollapsingHeader { open, .. }) => {
            *open = as_bool(v)?;
            // its own size changes: restyle and relayout from its parent (SG1's style change)
            d.insert(Dirty::LAYOUT);
        }
        // ---- WG4a (`kinds/views.rs`, `crate::views`) ----
        (key, W::Table(t)) => d.insert(t.set(key, v)?),
        (key, W::ToastHost(t)) => t.set(key, v)?,
        (key, W::Toast(t)) => t.set(key, v)?,
        (P::ButtonOrder, W::DialogButtons(d)) => d.order = as_enum(v, crate::kinds::BUTTON_ORDER)?,
        (P::Message, W::StatusBar(b)) => b.set_message(as_str(v)?, None),
        (P::Title, W::Tab { title, .. }) => *title = as_str(v)?,
        // OW5b: the column's props (`views::table::ColumnState`)
        (key, W::TableColumn(c)) => c.set(key, v)?,
        (P::SelectedIndex, W::TabView { selected }) => {
            *selected = as_i64(v)?;
            // another page is laid out
            d.insert(Dirty::LAYOUT);
        }
        (P::Closable, W::Tab { closable, .. }) => *closable = as_bool(v)?,
        (key, w) => return Err(format!("{key:?} is not a prop of {w:?}")),
    }
    if def.layout {
        d.insert(Dirty::LAYOUT);
    }
    Ok(d)
}

pub fn get_prop(n: &Node, def: &kinds::PropDef) -> Result<Value, String> {
    if let P::Ext(i) = def.key {
        // RA1: a provider kind's field; OW0: an own widget's extra prop (the wrapper reads the default)
        return if matches!(n.w, W::Ext(_)) { crate::provider::get_prop(n, i as usize) } else { Ok(default_value(def)) };
    }
    if let W::Ext(x) = n.w
        && !is_common(def.key)
    {
        return crate::provider::get_replaced_prop(x, def); // OW0: an own widget's field
    }
    if let Some(v) = crate::elements::get_prop(n.w, def) {
        return Ok(v); // WG2b: its kinds' own props
    }
    let c = &n.c;
    let s = |x: &str| Value::Str(x.to_string());
    Ok(match (def.key, &n.w) {
        (P::Enabled, _) => Value::Bool(c.enabled),
        (P::Visible, _) => Value::Bool(c.visible),
        (P::Tooltip, _) => s(&c.tooltip),
        (P::AccessibleName, _) => s(&c.accessible_name),
        (P::Anchor, _) => s(&c.anchor),
        (P::AnchorScope, _) => Value::Str(c.anchor_scope.as_deref().unwrap_or("").to_string()),
        (P::DialogRole, _) => Value::Str(c.dialog_role.to_string()),
        (P::StatusTip, _) => Value::Str(c.status_tip.as_deref().unwrap_or("").to_string()),
        (P::StatusSection, _) => Value::Str(c.status_section.to_string()),
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
        (P::Position, _) => s(c.position),
        (P::Left, _) => len_value(c.left),
        (P::Top, _) => len_value(c.top),
        (P::Right, _) => len_value(c.right),
        (P::Bottom, _) => len_value(c.bottom),
        (P::FlexDirection, _) => s(c.flex_direction),
        (P::FlexWrap, _) => s(c.flex_wrap),
        (P::Gap, _) => Value::F64(c.gap),
        (P::RowGap, _) => len_value(c.row_gap),
        (P::ColumnGap, _) => len_value(c.column_gap),
        (P::Padding, _) => Value::F64(c.padding),
        (P::AlignItems, _) => s(c.align_items),
        (P::JustifyContent, _) => s(c.justify_content),
        (P::Autofocus, _) => Value::Bool(c.autofocus),
        (P::FocusScope, _) => s(c.focus_scope),
        (P::Default, W::Button { .. } | W::Ext(_)) => Value::Bool(c.default_button),
        (P::Cancel, W::Button { .. } | W::Ext(_)) => Value::Bool(c.cancel_button),
        (P::Invalid, _) => Value::Bool(c.invalid),
        (P::DragType | P::DragData | P::DropTypes, _) => s(crate::kinds::dnd::get(c, def.key)), // WG4b
        // WG2a
        (key, W::Menu(m)) => crate::menus::get_prop(m, key)?,
        (key, W::Overlay(o)) => crate::overlays::get_prop(o, key)?,
        (P::Title, W::Window { title, .. }) => s(title),
        (P::DefaultWidth, W::Window { default_w, .. }) => Value::F64(*default_w),
        (P::DefaultHeight, W::Window { default_h, .. }) => Value::F64(*default_h),
        (P::Text, _) => s(n.text()),
        (P::FontWeight, W::Run { weight, .. }) => Value::I64(*weight),
        (P::Href, W::Run { href, .. }) => s(href),
        (P::WrapMode, W::Label { wrap, .. }) => s(wrap),
        (P::Selectable, W::Label { selectable, .. }) => Value::Bool(*selectable),
        (P::Selected, W::Button { selected, .. } | W::Icon { selected, .. }) => Value::Bool(*selected),
        (P::Frame, W::Button { frame, .. } | W::ToggleButton { frame, .. } | W::Icon { frame, .. }) => Value::Bool(*frame),
        (P::Small, W::Button { small, .. }) => Value::Bool(*small),
        (P::Checked, W::Checkbox { checked, .. } | W::ToggleButton { checked, .. }) => Value::Bool(*checked),
        (P::HintText, W::TextEdit { hint, .. } | W::ComboBox { hint, .. }) => s(hint),
        (P::Multiline, W::TextEdit { multiline, .. }) => Value::Bool(*multiline),
        (P::Password, W::TextEdit { password, .. }) => Value::Bool(*password),
        (P::CharLimit, W::TextEdit { char_limit, .. }) => Value::I64(*char_limit),
        (P::ListSelected | P::ListSelection | P::ListLayout | P::SelectedIds | P::RowHeight | P::TreeIndent | P::RowDragType | P::RowDropTypes | P::ExpandOn | P::RootDecorated | P::CurrentItem | P::InternalMove | P::UniformRows, W::VirtualList(l)) => {
            crate::kinds::virtuallist::get(l, def.key).ok_or_else(|| format!("{:?} is not a prop of <virtual-list>", def.key))?
        }
        // ---- WG1 ----
        (P::FontWeight, W::Label { weight, .. } | W::Button { weight, .. } | W::ToggleButton { weight, .. } | W::Icon { weight, .. }) => Value::I64(*weight),
        (P::Radio, W::ToggleButton { radio, .. }) => Value::Bool(*radio),
        (P::Options, W::ComboBox { options, .. }) => Value::List(options.iter().map(|o| s(o)).collect()),
        (P::SelectedIndex, W::ComboBox { selected, .. }) => Value::I64(*selected),
        (P::Orientation, W::Separator { vertical }) => s(if *vertical { "vertical" } else { "horizontal" }),
        (P::Name, W::Icon { name, .. }) => s(name),
        (P::Glyph, W::Icon { glyph, .. }) => s(glyph),
        (P::Source, W::Icon { source, .. }) => if source.is_empty() { Value::Null } else { s(source) },
        (P::Tint, W::Icon { tint, .. }) => Value::Bool(*tint),
        (P::Family, W::Icon { family, .. }) => s(family),
        (P::Size, W::Icon { size, .. }) => len_value(*size),
        (P::GridTemplateColumns, W::Grid { columns, .. }) => s(columns),
        (P::JustifyItems, W::Grid { justify_items, .. }) => s(justify_items),
        (P::Open, W::CollapsingHeader { open, .. }) => Value::Bool(*open),
        // ---- WG4a ----
        (key, W::Table(t)) => t.get(key).ok_or_else(|| format!("{key:?} is not a prop of <table>"))?,
        (key, W::ToastHost(t)) => t.get(key).ok_or_else(|| format!("{key:?} is not a prop of <toast-host>"))?,
        (key, W::Toast(t)) => t.get(key).ok_or_else(|| format!("{key:?} is not a prop of <toast>"))?,
        (P::ButtonOrder, W::DialogButtons(d)) => Value::Str(d.order.to_string()),
        (P::Message, W::StatusBar(b)) => s(&b.message),
        (P::Title, W::Tab { title, .. }) => s(title),
        (key, W::TableColumn(c)) => c.get(key).ok_or_else(|| format!("{key:?} is not a prop of <table-column>"))?,
        (P::SelectedIndex, W::TabView { selected }) => Value::I64(*selected),
        (P::Closable, W::Tab { closable, .. }) => Value::Bool(*closable),
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
        Ty::StrList | Ty::IdList | Ty::NumList => svelters_abi::SVL_LIST,
        Ty::Snippet => svelters_abi::SVL_BOOL,
        // RA1: described per prop by its Rust type (`KindRegistry::provider_member_desc`)
        Ty::Provider => svelters_abi::SVL_NULL,
    }
}
