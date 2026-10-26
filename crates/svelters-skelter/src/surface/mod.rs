//! The JS-facing side of skelter's kinds: what `skelter.json` publishes and the compiler checks
//! against, beyond what skelter-core's [`skelter_core::KindSpec`] carries yet (coordinator Q3: docs,
//! methods, named signal arguments, nullable props, a11y name sources and a base-kind chain; CORE is
//! adding them, and this table goes when it has). [`crate::kinds`] maps each member onto the real
//! skelter-core kind it belongs to.
//!
//! - [`KindSpec`] and its member specs, [`Value`], [`ValueType`], [`DefaultValue`]: the surface
//!   vocabulary (egui's types: enums by name, typed lists, `null` where nullable).
//! - [`tier0`]: the Tier 0 kinds, transcribed from `metadata/semantics/egui.json` so the JS-facing
//!   names are egui's (coordinator Q2), with skelter's change classes and passive/cancelable flags.
//! - [`ColorScheme`], [`Visual`]: style inputs skelter-core does not take yet (`prefers-color-scheme`,
//!   the TR1 visual channels); the backend keeps them.

pub mod tier0;
pub mod tier1;

/// The built-in surface: [`tier0`] then [`tier1`] (bases first).
pub fn builtin() -> Vec<&'static KindSpec> {
    tier0::ALL.iter().chain(tier1::ALL).copied().collect()
}

use skelter_base::change::ChangeClass;
use skelter_base::color::Color;


/// The type of a prop, signal argument or method parameter (egui's spelling: enums carry their names, lists their element type).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValueType {
    Bool,
    I64,
    F64,
    Str,
    /// One of the listed names.
    Enum(&'static [&'static str]),
    Color,
    StrList,
    NumList,
    IdList,
    Record,
}

impl ValueType {
    /// The spec's spelling (`skelter.json` `type`).
    pub fn name(self) -> &'static str {
        match self {
            ValueType::Bool => "bool",
            ValueType::I64 => "i64",
            ValueType::F64 => "f64",
            ValueType::Str => "string",
            ValueType::Enum(_) => "enum",
            ValueType::Color => "color",
            ValueType::StrList | ValueType::NumList | ValueType::IdList => "list",
            ValueType::Record => "record",
        }
    }

    /// The element type of a list (`skelter.json` `of`).
    pub fn of(self) -> Option<&'static str> {
        match self {
            ValueType::StrList => Some("string"),
            ValueType::NumList => Some("f64"),
            ValueType::IdList => Some("i64"),
            _ => None,
        }
    }
}

/// A prop value as JS sees it (`null` where nullable; skelter-core's [`skelter_core::Value`] has no null:
/// a `null` write resets the prop).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    Str(String),
    /// Index into the prop type's names.
    Enum(u16),
    Color(Color),
    List(Vec<Value>),
    Record(Vec<(String, Value)>),
}

impl Value {
    /// Whether `self` is a value of `ty` (`Null` only where `nullable`).
    pub fn fits(&self, ty: ValueType, nullable: bool) -> bool {
        match (self, ty) {
            (Value::Null, _) => nullable,
            (Value::Bool(_), ValueType::Bool) | (Value::I64(_), ValueType::I64) | (Value::F64(_), ValueType::F64) => true,
            (Value::Str(_), ValueType::Str) | (Value::Color(_), ValueType::Color) | (Value::Record(_), ValueType::Record) => true,
            (Value::Enum(i), ValueType::Enum(names)) => (*i as usize) < names.len(),
            (Value::List(v), ValueType::StrList) => v.iter().all(|x| matches!(x, Value::Str(_))),
            (Value::List(v), ValueType::NumList) => v.iter().all(|x| matches!(x, Value::F64(_) | Value::I64(_))),
            (Value::List(v), ValueType::IdList) => v.iter().all(|x| matches!(x, Value::I64(_))),
            _ => false,
        }
    }
}

/// A prop's default (`const` so specs can be `static`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DefaultValue {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    Str(&'static str),
    /// Index into the enum's names.
    Enum(u16),
}

impl DefaultValue {
    pub fn value(self) -> Value {
        match self {
            DefaultValue::Null => Value::Null,
            DefaultValue::Bool(b) => Value::Bool(b),
            DefaultValue::I64(i) => Value::I64(i),
            DefaultValue::F64(f) => Value::F64(f),
            DefaultValue::Str(s) => Value::Str(s.to_string()),
            DefaultValue::Enum(i) => Value::Enum(i),
        }
    }
}

#[derive(Debug)]
pub struct PropSpec {
    pub name: &'static str,
    pub ty: ValueType,
    pub default: DefaultValue,
    pub nullable: bool,
    /// The work a write causes (architecture §4.3).
    pub change: ChangeClass,
    /// Native-owned (architecture §4.5): the kind changes it itself and announces each change, so it
    /// is observable exactly and bindable.
    pub native: bool,
    pub doc: &'static str,
}

#[derive(Debug)]
pub struct SignalSpec {
    pub name: &'static str,
    pub args: &'static [(&'static str, ValueType)],
    /// Listeners are passive unless they say otherwise (scroll and drag paths, architecture §11.5).
    pub passive: bool,
    /// A listener can cancel the native default (`preventDefault`).
    pub cancelable: bool,
    /// Delivered synchronously, its handler's result read back (the DOM-style events).
    pub sync: bool,
    /// Propagates along the tree with capture and bubble phases (EV1).
    pub bubbles: bool,
    pub doc: &'static str,
}

#[derive(Debug)]
pub struct MethodSpec {
    pub name: &'static str,
    pub params: &'static [(&'static str, ValueType)],
    pub ret: Option<ValueType>,
    pub doc: &'static str,
}

#[derive(Debug)]
pub struct SlotSpec {
    pub name: &'static str,
    /// Kind names (or base kind names) a child in this slot may have.
    pub accepts: &'static [&'static str],
    pub many: bool,
}

#[derive(Debug)]
pub struct A11ySpec {
    /// AccessKit role name (`Button`); empty for no node of its own.
    pub role: &'static str,
    /// Where the accessible name comes from, in order.
    pub name_from: &'static [&'static str],
    /// A missing name is a diagnostic (compiler-and-js §3.9).
    pub name_required: bool,
}

/// What a kind is to JS (architecture §4.2; the parts skelter-core's `KindSpec` lacks, Q3).
#[derive(Debug)]
pub struct KindSpec {
    /// The kind's name (`Button`).
    pub name: &'static str,
    /// The element tag (`button`); `None` for an abstract base.
    pub tag: Option<&'static str>,
    /// The base kind whose members this one has too.
    pub extends: Option<&'static KindSpec>,
    pub doc: &'static str,
    pub props: &'static [PropSpec],
    pub signals: &'static [SignalSpec],
    pub methods: &'static [MethodSpec],
    /// Child slots; the first is the default.
    pub slots: &'static [SlotSpec],
    pub a11y: A11ySpec,
    /// The prop a text child sets (`text`).
    pub text_content: Option<&'static str>,
    /// A window: a root of its own, presented with `present`.
    pub toplevel: bool,
    /// Native-owned state this kind exposes (scroll offset, selection, drag value).
    pub native_state: &'static [&'static str],
}

impl KindSpec {
    /// This kind and its bases, nearest first.
    pub fn chain(&'static self) -> impl Iterator<Item = &'static KindSpec> {
        std::iter::successors(Some(self), |k| k.extends)
    }
}

/// `prefers-color-scheme` (skelter-core has no media state yet: the backend keeps it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorScheme {
    Light,
    Dark,
}

/// A node's property-class channels set from JS (TR1: `style:` opacity, transform, clip, inert); kept by
/// the backend until skelter-core takes effect and transform inputs from JS.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Visual {
    /// `None`: opaque.
    pub opacity: Option<f32>,
    /// A 2D affine `[a, b, c, d, e, f]` (CSS `matrix()` order).
    pub transform: Option<[f32; 6]>,
    /// A clip to the leading fraction of the box, `[x, y]` in 0..=1 (`slide`).
    pub clip: Option<[f32; 2]>,
    /// Takes no input (an element playing its out transition).
    pub inert: bool,
}
