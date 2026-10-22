//! svl values <-> windows-rs types, for the generated dispatch code. The string forms live in
//! `parse.rs` (platform-neutral, unit-tested on Linux).
use crate::bindings::Microsoft::UI::Xaml::Controls::{FontIcon, IconElement, Symbol, SymbolIcon};
use crate::bindings::Microsoft::UI::Xaml::Media::Imaging::BitmapImage;
use crate::bindings::Microsoft::UI::Xaml::Media::{Brush, FontFamily, ImageSource, SolidColorBrush};
use crate::bindings::Microsoft::UI::Xaml::{CornerRadius, Thickness};
use crate::bindings::Windows::Foundation::{IPropertyValue, PropertyValue, Uri};
use crate::bindings::Windows::UI::Color;
use crate::bindings::Windows::UI::Text::FontWeight;
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
        return Value::Str(s.to_string_lossy());
    }
    match crate::backend::handle_of(&o) {
        Some(h) => Value::Handle(h),
        None => Value::Null,
    }
}

pub fn f32_(v: &Value) -> R<f32> {
    parse::to_f64(v).map(|f| f as f32)
}

pub fn corner_radius(v: &Value) -> R<CornerRadius> {
    let [tl, tr, br, bl] = parse::corner_radius(v)?;
    Ok(CornerRadius { TopLeft: tl, TopRight: tr, BottomRight: br, BottomLeft: bl })
}

pub fn corner_radius_value(c: CornerRadius) -> Value {
    Value::Str(parse::thickness_text([c.TopLeft, c.TopRight, c.BottomRight, c.BottomLeft]))
}

pub fn color(v: &Value) -> R<Color> {
    let [a, r, g, b] = parse::color(v)?;
    Ok(Color { A: a, R: r, G: g, B: b })
}

pub fn color_value(c: Color) -> Value {
    Value::Str(parse::color_text([c.A, c.R, c.G, c.B]))
}

/// A brush prop: a color string becomes a new SolidColorBrush; `null` clears the local value (the theme brush
/// applies again).
pub fn brush(v: &Value) -> R<Option<Brush>> {
    Ok(solid_brush(v)?.map(|b| b.cast::<Brush>()).transpose().map_err(err)?)
}

pub fn solid_brush(v: &Value) -> R<Option<SolidColorBrush>> {
    match v {
        Value::Null => Ok(None),
        other => SolidColorBrush::CreateInstanceWithColor(color(other)?).map(Some).map_err(err),
    }
}

/// A solid brush reads back as its color; another brush (gradient, acrylic) or none as null.
pub fn brush_value(b: Option<IInspectable>) -> Value {
    b.and_then(|b| b.cast::<SolidColorBrush>().ok()).and_then(|b| b.Color().ok()).map_or(Value::Null, color_value)
}

pub fn solid_brush_value(b: Option<IInspectable>) -> Value {
    brush_value(b)
}

pub fn font_weight(v: &Value) -> R<FontWeight> {
    Ok(FontWeight { Weight: parse::font_weight(v)? })
}

pub fn font_weight_value(w: FontWeight) -> Value {
    Value::I64(w.Weight as i64)
}

pub fn font_family(v: &Value) -> R<Option<FontFamily>> {
    match v {
        Value::Null => Ok(None),
        other => FontFamily::CreateInstanceWithName(&hstring(other)?).map(Some).map_err(err),
    }
}

pub fn font_family_value(f: Option<IInspectable>) -> Value {
    f.and_then(|f| f.cast::<FontFamily>().ok()).and_then(|f| f.Source().ok()).map_or(Value::Null, |s| Value::Str(s.to_string_lossy()))
}

pub fn datetime(v: &Value) -> R<windows_time::DateTime> {
    Ok(windows_time::DateTime::from_unix_millis(parse::datetime_ms(v)?.round() as i64))
}

pub fn datetime_value(d: windows_time::DateTime) -> Value {
    Value::DateTime(d.unix_millis() as f64)
}

pub fn timespan(v: &Value) -> R<windows_time::TimeSpan> {
    Ok(windows_time::TimeSpan::from_millis(parse::timespan_ms(v)?.round() as i64))
}

pub fn timespan_value(t: windows_time::TimeSpan) -> Value {
    Value::F64(t.whole_millis() as f64)
}

/// `IReference<T>` props: `null` clears, anything else converts.
pub fn nullable<T>(v: &Value, f: impl Fn(&Value) -> R<T>) -> R<Option<T>> {
    match v {
        Value::Null => Ok(None),
        other => f(other).map(Some),
    }
}

pub fn uri(v: &Value) -> R<Option<Uri>> {
    match v {
        Value::Null => Ok(None),
        Value::Str(s) if s.is_empty() => Ok(None),
        other => Uri::CreateUri(&hstring(other)?).map(Some).map_err(err),
    }
}

pub fn uri_value(u: Option<IInspectable>) -> Value {
    u.and_then(|u| u.cast::<Uri>().ok()).and_then(|u| u.AbsoluteUri().ok()).map_or(Value::Null, |s| Value::Str(s.to_string_lossy()))
}

/// An image source: a URI (`https:`, `ms-appx:`, `file:`), written as a BitmapImage (decoded when shown).
pub fn image_source(v: &Value) -> R<Option<ImageSource>> {
    let Some(u) = uri(v)? else { return Ok(None) };
    BitmapImage::CreateInstanceWithUriSource(&u).and_then(|b| b.cast::<ImageSource>()).map(Some).map_err(err)
}

/// A BitmapImage reads back as its URI; other sources (a WriteableBitmap, an SVG source) as null.
pub fn image_source_value(s: Option<IInspectable>) -> Value {
    s.and_then(|s| s.cast::<BitmapImage>().ok()).and_then(|b| b.UriSource().ok()).map_or(Value::Null, |u| uri_value(Some(u.into())))
}

/// An icon: a `Symbol` name (`accept`, `Save`) becomes a SymbolIcon, any other text a FontIcon with that glyph.
pub fn icon(v: &Value, symbols: &Enum) -> R<Option<IconElement>> {
    match v {
        Value::Null => Ok(None),
        other => match parse::enum_value(other, symbols) {
            Ok(n) => SymbolIcon::CreateInstanceWithSymbol(Symbol(n)).and_then(|i| i.cast::<IconElement>()).map(Some).map_err(err),
            Err(_) => {
                let f = FontIcon::new().map_err(err)?;
                f.SetGlyph(&hstring(other)?).map_err(err)?;
                f.cast::<IconElement>().map(Some).map_err(err)
            }
        },
    }
}

/// A SymbolIcon reads back as its symbol's kebab name, a FontIcon as its glyph, anything else as null.
pub fn icon_value(i: Option<IInspectable>, symbols: &Enum) -> Value {
    let Some(i) = i else { return Value::Null };
    if let Ok(s) = i.cast::<SymbolIcon>().and_then(|s| s.Symbol()) {
        return parse::enum_name(symbols, s.0);
    }
    if let Ok(g) = i.cast::<FontIcon>().and_then(|f| f.Glyph()) {
        return Value::Str(g.to_string_lossy());
    }
    Value::Null
}
