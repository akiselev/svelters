//! svl values <-> windows-rs types, for the generated dispatch code. The string forms live in
//! `parse.rs` (platform-neutral, unit-tested on Linux).
use crate::bindings::Microsoft::UI::Xaml::Thickness;
use crate::bindings::Windows::Foundation::{IPropertyValue, PropertyValue};
use crate::parse;
use crate::table::Enum;
use svelters_abi::Value;
use windows_core::{HSTRING, IInspectable, Interface};

pub use crate::parse::{R, arg, arity, is_true};

/// A windows-rs error as the text `last_error` reports.
pub fn err(e: windows_core::Error) -> String {
    let msg = e.message();
    let code = e.code().0 as u32;
    if msg.is_empty() { format!("HRESULT 0x{code:08X}") } else { format!("{msg} (HRESULT 0x{code:08X})") }
}

pub fn hstring(v: &Value) -> R<HSTRING> {
    Ok(HSTRING::from(parse::to_string(v)?))
}

pub fn bool_(v: &Value) -> R<bool> {
    parse::to_bool(v)
}

pub fn i32_(v: &Value) -> R<i32> {
    parse::to_i32(v)
}

pub fn f64_(v: &Value) -> R<f64> {
    parse::to_f64(v)
}

pub fn enum_(v: &Value, e: &Enum) -> R<i32> {
    parse::enum_value(v, e)
}

pub fn enum_name(e: &Enum, value: i32) -> Value {
    parse::enum_name(e, value)
}

pub fn thickness(v: &Value) -> R<Thickness> {
    let [l, t, r, b] = parse::thickness(v)?;
    Ok(Thickness { Left: l, Top: t, Right: r, Bottom: b })
}

pub fn thickness_value(t: Thickness) -> Value {
    Value::Str(parse::thickness_text([t.Left, t.Top, t.Right, t.Bottom]))
}

/// `IReference<bool>`: `null` is indeterminate.
pub fn nullable_bool(v: &Value) -> R<Option<bool>> {
    match v {
        Value::Null => Ok(None),
        other => parse::to_bool(other).map(Some),
    }
}

pub fn nullable_bool_value(v: Option<bool>) -> Value {
    v.map_or(Value::Null, Value::Bool)
}

/// Text for an `object` property: a boxed HSTRING. `null` clears it.
pub fn boxed_str(v: &Value) -> R<Option<IInspectable>> {
    match v {
        Value::Null => Ok(None),
        other => PropertyValue::CreateString(&hstring(other)?).map(Some).map_err(err),
    }
}

/// An `object` property read back: text when it holds a boxed string, the handle when it holds one
/// of our elements, else null.
pub fn unboxed_value(o: Option<IInspectable>) -> Value {
    let Some(o) = o else { return Value::Null };
    if let Ok(pv) = o.cast::<IPropertyValue>()
        && let Ok(s) = pv.GetString()
    {
        return Value::Str(s.to_string());
    }
    match crate::backend::handle_of(&o) {
        Some(h) => Value::Handle(h),
        None => Value::Null,
    }
}
