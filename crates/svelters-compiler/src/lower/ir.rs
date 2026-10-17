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

/// A run of template content. Generated code declares `consts` first (`{@const}`, source order),
/// then `snippets` (hoisted within the fragment, as Svelte does), then builds `items`.
#[derive(Debug, Default)]
pub struct Fragment<'a> {
    pub consts: Vec<ConstDecl<'a>>,
    pub snippets: Vec<Snippet<'a>>,
    pub items: Vec<Item<'a>>,
}

#[derive(Debug)]
pub enum Item<'a> {
    Element(Element<'a>),
    If(IfBlock<'a>),
    Each(EachBlock<'a>),
    Key(KeyBlock<'a>),
    Await(AwaitBlock<'a>),
    Render(RenderCall<'a>),
    Component(ComponentCall<'a>),
}

impl Item<'_> {
    /// The parent slot this item's natives go to.
    pub fn slot(&self) -> &Slot {
        match self {
            Item::Element(e) => &e.slot,
            Item::If(b) => &b.slot,
            Item::Each(b) => &b.slot,
            Item::Key(b) => &b.slot,
            Item::Await(b) => &b.slot,
            Item::Render(r) => &r.slot,
            Item::Component(c) => &c.slot,
        }
    }
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
    /// `class={expr}` (or an interpolated `class`): the element's runtime class set, declared as
    /// `const c<id> = $.derived(() => $n.classes(expr))` and read by style conditions.
    pub class_expr: Option<Dynamic<'a>>,
    pub ops: Vec<Op<'a>>,
    pub children: Fragment<'a>,
}

#[derive(Debug)]
pub enum Op<'a> {
    /// A typed constant known at compile time.
    Set { prop: String, value: Json, span: Span },
    /// A JS value written by a render effect.
    SetDynamic { prop: String, value: Dynamic<'a>, span: Span },
    /// `$n.on(node, signal, handler, flags)`. `flags`: LISTEN_AFTER | LISTEN_CAPTURE (0 = plain).
    /// `sync`: the backend delivers the signal through `call_sync`, so the handler gets
    /// `(event, ...args)` (informational; the runtime decides by how the host calls it).
    Listen { signal: String, handler: Handler<'a>, flags: u32, sync: bool, span: Span },
    /// `{@attach listen(signal, handler, {after, capture})}` with a static signal and phase:
    /// `$.attach(node, () => $n.listen(signal, handler, flags))` (registered when the attachment runs,
    /// as the renderer's `listen` helper).
    ListenAttach { signal: String, handler: &'a Expression, flags: u32, span: Span },
    /// `{@attach expr}`: Svelte's `$.attach(node, () => expr)` with the runtime node.
    Attach { expr: &'a Expression, span: Span },
    /// `parent:<name>=...`: `$n.child_attr(node, name, value)`, sent to the native parent after insertion.
    ChildAttr { name: String, value: AttrValue<'a>, span: Span },
    /// `$n.bind(node, prop, signal, arg, get, set)`; `target` is the bound lvalue.
    Bind { prop: String, signal: String, arg: i64, target: &'a Expression, span: Span },
    /// `bind:this={target}`: Svelte's `$.bind_this(node, set, get)` with the runtime node.
    BindThis { target: &'a Expression, span: Span },
    /// `$n.track(node, prop, signal, arg)` for a one-way prop the native side also edits.
    Track { prop: String, signal: String, arg: i64 },
    /// `$n.token(node, prop, token, cond)`; static when `cond` is empty.
    Token { prop: String, token: String, cond: Cond<'a>, span: Span },
    /// Style-driven prop with conditional candidates, highest precedence first; the first whose
    /// condition holds wins, else `fallback`.
    StyleProp { prop: String, candidates: Vec<(Cond<'a>, Json)>, fallback: Fallback, span: Span },
}

/// An event handler: an expression, or a property of a spread object (`{...events}`), read when
/// the event fires.
#[derive(Debug)]
pub enum Handler<'a> {
    Expr(&'a Expression),
    Member { object: &'a Expression, key: String },
}

#[derive(Debug)]
pub enum AttrValue<'a> {
    Static(Json),
    Dynamic(Dynamic<'a>),
}

/// A conjunction of class conditions; empty = always true.
pub type Cond<'a> = Vec<CondAtom<'a>>;

#[derive(Debug, Clone)]
pub enum CondAtom<'a> {
    /// `class:x={e}`: `!!(e)`.
    Expr(&'a Expression),
    /// Element `element`'s dynamic class set contains `class` (`$.get(c<id>).has(class)`).
    Class { element: usize, class: String },
}

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
    /// `object[key]`: a property of a spread object (`{...props}` on a native element).
    Member { object: &'a Expression, key: String },
}

#[derive(Debug)]
pub enum TemplatePart<'a> {
    Text(String),
    Expr(&'a Expression),
    /// `{#if test}..{:else}..{/if}` holding only text/expressions: `${(test) ? `..` : `..`}`.
    If { test: &'a Expression, consequent: Vec<TemplatePart<'a>>, alternate: Vec<TemplatePart<'a>> },
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

/// `{@const name = init}`: a template-scoped derived (`const name = $.derived(() => init)`),
/// read as `$.get(name)`.
#[derive(Debug)]
pub struct ConstDecl<'a> {
    pub span: Span,
    pub name: String,
    pub init: &'a Expression,
}

/// `{#snippet name(a, b)}`: a native content factory `(anchor, a = $.noop, b = $.noop) => {...}`
/// that builds its body into the region `anchor` it is given; parameters are thunks (read `a()`).
#[derive(Debug)]
pub struct Snippet<'a> {
    pub span: Span,
    pub name: String,
    pub params: Vec<String>,
    pub body: Fragment<'a>,
}

/// `{@render callee(args)}`. When the callee reads reactive state (a snippet prop), codegen
/// renders through `$n.snippet` (re-created when the function changes); otherwise the snippet
/// is called directly with a fresh region. Arguments are passed as thunks.
#[derive(Debug)]
pub struct RenderCall<'a> {
    pub span: Span,
    pub slot: Slot,
    pub callee: &'a Expression,
    pub args: Vec<&'a Expression>,
    /// `{@render f?.()}`: a nullish snippet renders nothing.
    pub optional: bool,
}

/// `{#key expr}`: the body's natives are destroyed and re-created when `expr` changes.
#[derive(Debug)]
pub struct KeyBlock<'a> {
    pub span: Span,
    pub slot: Slot,
    pub key: &'a Expression,
    pub body: Fragment<'a>,
}

/// `{#await expr}..{:then value}..{:catch error}..{/await}`.
#[derive(Debug)]
pub struct AwaitBlock<'a> {
    pub span: Span,
    pub slot: Slot,
    pub expression: &'a Expression,
    pub value: Option<String>,
    pub error: Option<String>,
    pub pending: Option<Fragment<'a>>,
    pub then: Option<Fragment<'a>>,
    pub catch: Option<Fragment<'a>>,
}

#[derive(Debug)]
pub struct ComponentCall<'a> {
    pub span: Span,
    /// The component tag (codegen asks the script analysis whether its name is dynamic); None
    /// for `<svelte:component>`.
    pub node: Option<&'a crate::svelte::ast::Element>,
    /// `<svelte:component this={expr}>`: the component function, always dynamic.
    pub this: Option<&'a crate::estree::Expression>,
    pub slot: Slot,
    pub name: String,
    pub props: Vec<ComponentProp<'a>>,
    /// `{@const}` tags directly inside the component tag: declared before its snippets.
    pub consts: Vec<ConstDecl<'a>>,
    /// Snippets declared directly inside the component tag: declared before the call and passed
    /// as props of the same name.
    pub snippets: Vec<Snippet<'a>>,
    /// The other children: the implicit `children` snippet.
    pub children: Option<Fragment<'a>>,
}

#[derive(Debug)]
pub enum ComponentProp<'a> {
    Static { name: String, value: Json },
    Dynamic { name: String, value: Dynamic<'a> },
    /// `bind:name={target}`: getter + setter.
    Bind { name: String, target: &'a Expression },
    /// `{...expr}`: an argument of Svelte's `$.spread_props` (a thunk when `expr` is reactive).
    Spread(&'a Expression),
}
