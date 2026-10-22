//! Built-in prop get/set: the ABI value conversions, [`set_prop`] and [`get_prop`] over the typed
//! fields of [`Node`], [`default_value`] and [`tag_of`].
//!
//! SG1 turns prop effects into `Dirty` bits from the kind table and adds built-in `PropKey`s
//! (ARCH-SCENE §5.3, §12). Writer lane: WIDGETS.

use super::node::{Dirty, Node, W};
use crate::kinds::{self, Def, P, Ty};
use svelters_abi::Value;

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

/// Writes a prop. `Null` resets it to its default.
pub fn set_prop(n: &mut Node, def: &kinds::PropDef, v: &Value) -> Result<Dirty, String> {
    if let P::Ext(i) = def.key {
        return crate::provider::set_prop(n, i as usize, v); // RA1: a provider kind's field
    }
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
    if let P::Ext(i) = def.key {
        return crate::provider::get_prop(n, i as usize); // RA1: a provider kind's field
    }
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
        // RA1: described per prop by its Rust type (`KindRegistry::provider_member_desc`)
        Ty::Provider => svelters_abi::SVL_NULL,
    }
}
