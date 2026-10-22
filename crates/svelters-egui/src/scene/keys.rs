//! Typed prop keys (ARCH-SCENE §6.1): [`PropKey<T>`] statics for the built-in kinds, one module per
//! kind (`keys::label::TEXT`, `keys::widget::VISIBLE`, …). A read through a key borrows the field
//! (`NodeRef::get`, no `Value` conversion); a write (`NodeMut::set`) goes through the exact write
//! path of `NodeMut::set_value`.
//!
//! The design doc names this module `scene::props`; it is `scene::keys` because `props.rs` is the
//! WIDGETS lane's value-conversion file. RA1 generates keys for provider kinds the same way.
//! Writer lane: SCENE (built-in keys follow the kind table, WIDGETS).

use super::node::{Common, W};
use svelters_abi::Value;

/// A value a prop key carries, convertible to the ABI value of the write path.
pub trait PropValue: 'static {
    fn into_value(self) -> Value;
}

impl PropValue for bool {
    fn into_value(self) -> Value {
        Value::Bool(self)
    }
}
impl PropValue for f64 {
    fn into_value(self) -> Value {
        Value::F64(self)
    }
}
impl PropValue for i64 {
    fn into_value(self) -> Value {
        Value::I64(self)
    }
}
impl PropValue for String {
    fn into_value(self) -> Value {
        Value::Str(self)
    }
}
impl PropValue for &'static str {
    fn into_value(self) -> Value {
        Value::Str(self.to_string())
    }
}
/// A nullable length: `None` is `null` (auto).
impl PropValue for Option<f64> {
    fn into_value(self) -> Value {
        self.map(Value::F64).unwrap_or(Value::Null)
    }
}

/// A static descriptor of one prop: its name and a typed accessor.
pub struct PropKey<T: 'static> {
    /// The prop's name (`text`, `flex-grow`).
    pub name: &'static str,
    get: for<'a> fn(&'a Common, &'a W) -> Option<&'a T>,
}

impl<T: 'static> PropKey<T> {
    pub const fn new(name: &'static str, get: for<'a> fn(&'a Common, &'a W) -> Option<&'a T>) -> PropKey<T> {
        PropKey { name, get }
    }

    pub(crate) fn read<'a>(&self, c: &'a Common, w: &'a W) -> Option<&'a T> {
        (self.get)(c, w)
    }
}

impl<T> std::fmt::Debug for PropKey<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PropKey({})", self.name)
    }
}

macro_rules! common {
    ($($N:ident: $T:ty = $name:literal, $f:ident;)*) => {$(
        pub static $N: PropKey<$T> = {
            fn g<'a>(c: &'a Common, _: &'a W) -> Option<&'a $T> {
                Some(&c.$f)
            }
            PropKey::new($name, g)
        };
    )*};
}

macro_rules! field {
    ($($N:ident: $T:ty = $name:literal, $V:ident . $f:ident;)*) => {$(
        pub static $N: PropKey<$T> = {
            fn g<'a>(_: &'a Common, w: &'a W) -> Option<&'a $T> {
                match w {
                    W::$V { $f, .. } => Some($f),
                    _ => None,
                }
            }
            PropKey::new($name, g)
        };
    )*};
}

/// Every element inside a window (the `Widget` base).
pub mod widget {
    use super::*;
    common! {
        ENABLED: bool = "enabled", enabled;
        VISIBLE: bool = "visible", visible;
        TOOLTIP: String = "tooltip", tooltip;
        ACCESSIBLE_NAME: String = "accessible-name", accessible_name;
        WIDTH: Option<f64> = "width", width;
        HEIGHT: Option<f64> = "height", height;
        MIN_WIDTH: Option<f64> = "min-width", min_width;
        MIN_HEIGHT: Option<f64> = "min-height", min_height;
        MAX_WIDTH: Option<f64> = "max-width", max_width;
        MAX_HEIGHT: Option<f64> = "max-height", max_height;
        FLEX_GROW: f64 = "flex-grow", flex_grow;
        FLEX_SHRINK: f64 = "flex-shrink", flex_shrink;
        FLEX_BASIS: Option<f64> = "flex-basis", flex_basis;
        ALIGN_SELF: &'static str = "align-self", align_self;
        MARGIN: f64 = "margin", margin;
        ANCHOR: String = "anchor", anchor;
        POSITION: &'static str = "position", position;
        LEFT: Option<f64> = "left", left;
        TOP: Option<f64> = "top", top;
        RIGHT: Option<f64> = "right", right;
        BOTTOM: Option<f64> = "bottom", bottom;
    }
}

/// `box` (and the content area of `window`).
pub mod container {
    use super::*;
    common! {
        FLEX_DIRECTION: &'static str = "flex-direction", flex_direction;
        FLEX_WRAP: &'static str = "flex-wrap", flex_wrap;
        GAP: f64 = "gap", gap;
        ROW_GAP: Option<f64> = "row-gap", row_gap;
        COLUMN_GAP: Option<f64> = "column-gap", column_gap;
        PADDING: f64 = "padding", padding;
        ALIGN_ITEMS: &'static str = "align-items", align_items;
        JUSTIFY_CONTENT: &'static str = "justify-content", justify_content;
    }
}

pub mod window {
    use super::*;
    field! {
        TITLE: String = "title", Window.title;
        DEFAULT_WIDTH: f64 = "default-width", Window.default_w;
        DEFAULT_HEIGHT: f64 = "default-height", Window.default_h;
    }
}

pub mod label {
    use super::*;
    field! {
        TEXT: String = "text", Label.text;
        WRAP_MODE: &'static str = "wrap-mode", Label.wrap;
        SELECTABLE: bool = "selectable", Label.selectable;
    }
}

pub mod button {
    use super::*;
    field! {
        TEXT: String = "text", Button.text;
        SELECTED: bool = "selected", Button.selected;
        FRAME: bool = "frame", Button.frame;
        SMALL: bool = "small", Button.small;
    }
}

pub mod checkbox {
    use super::*;
    field! {
        TEXT: String = "text", Checkbox.text;
        CHECKED: bool = "checked", Checkbox.checked;
    }
}

pub mod text_edit {
    use super::*;
    field! {
        TEXT: String = "text", TextEdit.text;
        HINT_TEXT: String = "hint-text", TextEdit.hint;
        MULTILINE: bool = "multiline", TextEdit.multiline;
        PASSWORD: bool = "password", TextEdit.password;
        CHAR_LIMIT: i64 = "char-limit", TextEdit.char_limit;
    }
}

pub mod slider {
    use super::*;
    field! {
        VALUE: f64 = "value", Slider.value;
        MIN: f64 = "min", Slider.min;
        MAX: f64 = "max", Slider.max;
        STEP: f64 = "step", Slider.step;
        TEXT: String = "text", Slider.text;
        SHOW_VALUE: bool = "show-value", Slider.show_value;
    }
}
