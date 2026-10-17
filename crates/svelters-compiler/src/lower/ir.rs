//! Typed UI IR: the result of native lowering, consumed by JS generation.
//!
//! The IR separates *what native operations happen* (resolved against a backend specification:
//! kinds, canonical prop names, typed constants, signals, binding plans, slots, style lowering)
//! from *how JavaScript implements them* (codegen + `@svelters/native-runtime`). JavaScript
//! expressions stay as references into the parsed AST; codegen rewrites them with the scope
//! analysis (state reads/writes) and copies them with source maps.
//!
//! Order matters: `Element::ops` are in the order the runtime contract requires (source order of
//! attributes, then text content, then style-derived ops).

use crate::estree::{Expression, Pattern, Span};
use serde_json::Value as Json;

#[derive(Debug)]
pub struct Component<'a> {
    pub root: Fragment<'a>,
    /// Number of element ids allocated (`Element::id` < this).
    pub elements: usize,
}

#[derive(Debug, Default)]
pub struct Fragment<'a> {
    pub items: Vec<Item<'a>>,
}

#[derive(Debug)]
pub enum Item<'a> {
    Element(Element<'a>),
    If(IfBlock<'a>),
    Each(EachBlock<'a>),
    Component(ComponentCall<'a>),
}

/// Where an item is placed in its native parent. `None` = the parent's default slot.
pub type Slot = Option<String>;

#[derive(Debug)]
pub struct Element<'a> {
    pub id: usize,
    pub span: Span,
    /// The tag passed to `host.create`.
    pub kind: String,
    /// Zero-argument method to call once a toplevel is complete (root items only).
    pub present: Option<String>,
    pub slot: Slot,
    pub ops: Vec<Op<'a>>,
    pub children: Fragment<'a>,
}

#[derive(Debug)]
pub enum Op<'a> {
    /// A typed constant known at compile time.
    Set { prop: String, value: Json, span: Span },
    /// A JS value written by a render effect.
    SetDynamic { prop: String, value: Dynamic<'a>, span: Span },
    /// `$n.on(node, signal, handler)`.
    Listen { signal: String, handler: &'a Expression, span: Span },
    /// `$n.bind(node, prop, signal, arg, get, set)`; `target` is the bound lvalue.
    Bind { prop: String, signal: String, arg: i64, target: &'a Expression, span: Span },
    /// `$n.track(node, prop, signal, arg)` for a one-way prop the native side also edits.
    Track { prop: String, signal: String, arg: i64 },
    /// `$n.token(node, prop, token, cond)`; static when `cond` is empty.
    Token { prop: String, token: String, cond: Cond<'a>, span: Span },
    /// Style-driven prop with conditional candidates, highest precedence first; the first whose
    /// condition holds wins, else `fallback`.
    StyleProp { prop: String, candidates: Vec<(Cond<'a>, Json)>, fallback: Fallback, span: Span },
}

/// A conjunction of class conditions (`class:x={e}` expressions); empty = always true.
pub type Cond<'a> = Vec<&'a Expression>;

#[derive(Debug, Clone, PartialEq)]
pub enum Fallback {
    /// The spec's native default.
    Default(Json),
    /// No default known: the runtime captures the native value before the first write.
    Restore,
}

#[derive(Debug)]
pub enum Dynamic<'a> {
    Expr(&'a Expression),
    /// Text content / interpolated attribute: concatenated parts (`` `a ${b ?? ''}` ``).
    Template(Vec<TemplatePart<'a>>),
}

#[derive(Debug)]
pub enum TemplatePart<'a> {
    Text(String),
    Expr(&'a Expression),
}

#[derive(Debug)]
pub struct IfBlock<'a> {
    pub span: Span,
    pub slot: Slot,
    pub test: &'a Expression,
    pub consequent: Fragment<'a>,
    pub alternate: Option<Fragment<'a>>,
}

#[derive(Debug)]
pub struct EachBlock<'a> {
    pub span: Span,
    pub slot: Slot,
    pub collection: &'a Expression,
    /// The item binding (only identifiers are supported by lowering).
    pub context: Option<&'a Pattern>,
    pub index: Option<String>,
    pub key: Option<&'a Expression>,
    pub body: Fragment<'a>,
    pub fallback: Option<Fragment<'a>>,
}

#[derive(Debug)]
pub struct ComponentCall<'a> {
    pub span: Span,
    pub slot: Slot,
    pub name: String,
    pub props: Vec<ComponentProp<'a>>,
}

#[derive(Debug)]
pub enum ComponentProp<'a> {
    Static { name: String, value: Json },
    Dynamic { name: String, value: Dynamic<'a> },
    /// `bind:name={target}`: getter + setter.
    Bind { name: String, target: &'a Expression },
}
