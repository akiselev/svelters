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
    /// `{@debug a, b}`: `$.template_effect(() => { console.log({ a: $.snapshot(a), .. }); debugger; })`
    /// after the items (no natives).
    pub debugs: Vec<&'a crate::svelte::ast::DebugTag>,
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
    /// The spec's zero-argument method that shows a toplevel modelessly (None: the runtime picks
    /// one, as for Qt dialogs).
    pub present: Option<String>,
    /// A toplevel kind (`$n.toplevel`): shown by its `show` policy at the root, or made transient
    /// for the enclosing toplevel when declared inside an element (X7).
    pub toplevel: bool,
    /// A static `show="modal|modeless|none"` on a toplevel.
    pub show: Option<String>,
    /// Attached to its parent element instead of placed in a slot (`$n.attached`: event
    /// controllers, context menus).
    pub attached: bool,
    pub slot: Slot,
    /// `class={expr}` (or an interpolated `class`): the element's runtime class set, declared as
    /// `const c<id> = $.derived(() => $n.classes(expr))` and read by style conditions.
    pub class_expr: Option<Dynamic<'a>>,
    /// A dynamic `id`: matched at runtime against the rules' `#id` conditions (`i<id>`).
    pub id_expr: Option<Dynamic<'a>>,
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
    /// `use:name={arg}`: Svelte's `$.action(node, ($$node, $$action_arg) => name?.($$node,
    /// $$action_arg), () => arg)` with the runtime node (`name` may be dotted: `use:a.b`).
    Action { name: String, arg: Option<&'a Expression>, span: Span },
    /// TR1 `transition:` / `in:` / `out:` (`flags`: Svelte's TRANSITION_IN 1 | TRANSITION_OUT 2 |
    /// TRANSITION_GLOBAL 4): `$n.transition(flags, node, () => name, () => arg)`, after `bind:this`.
    Transition { flags: u32, name: String, arg: Option<&'a Expression>, span: Span },
    /// TR1 `animate:name={arg}` on the only element of a keyed each item: `$n.animation(node, () => name, () => arg)`.
    Animate { name: String, arg: Option<&'a Expression>, span: Span },
    /// `parent:<name>=...`: `$n.child_attr(node, name, value)`, sent to the native parent after insertion.
    ChildAttr { name: String, value: AttrValue<'a>, span: Span },
    /// `$n.bind(node, prop, signal, arg, get, set)`; `target` is the bound lvalue.
    Bind { prop: String, signal: String, arg: i64, target: &'a Expression, span: Span },
    /// `$n.bind_prop(node, prop, get, set[, signal, arg])`: two-way on observation (R2); the signal
    /// plan is the fallback the runtime uses only when the host cannot observe the prop.
    BindProp { prop: String, fallback: Option<(String, i64)>, target: &'a Expression, span: Span },
    /// `$n.bind_readonly(node, prop, set)`: a read-only observable prop.
    BindReadonly { prop: String, target: &'a Expression, span: Span },
    /// `$n.bind_once(node, prop, get, set, readonly)`: `bind:prop|nonreactive`.
    BindOnce { prop: String, readonly: bool, target: &'a Expression, span: Span },
    /// `bind:this={target}`: Svelte's `$.bind_this(node, set, get)` with the runtime node.
    BindThis { target: &'a Expression, span: Span },
    /// `$n.track(node, prop, signal, arg)` for a one-way prop the native side also edits.
    Track { prop: String, signal: String, arg: i64 },
    /// `$n.token(node, prop, token, cond)`; static when `cond` is empty.
    Token { prop: String, token: String, cond: Cond<'a>, span: Span },
    /// Style-driven prop (or child attribute, `child_attr`) with conditional candidates, highest
    /// precedence first; the first whose condition holds wins, else `fallback`.
    /// `guarded`: the element has a dynamic `style={...}`, which wins over rules at runtime
    /// (`$n.rule_set` / `$n.rule_attr`).
    StyleProp { prop: String, child_attr: bool, candidates: Vec<(Cond<'a>, Json)>, fallback: Fallback, guarded: bool, span: Span },
    /// The explicit props and `parent:` attributes of an element with a dynamic `style={...}`:
    /// they win over its setter declarations at runtime (`$n.explicit`).
    Explicit { props: Vec<String>, attrs: Vec<String> },
    /// The classes the X6 styling sees (`$n.attr(node, "class", ...)` -> `host.style(CLASSES)`):
    /// `base` (static classes and the component's CSS scope class), then the dynamic class set of
    /// element `dynamic` (`c<id>`), then the `class:` toggles.
    Classes { base: String, dynamic: Option<usize>, toggles: Vec<(String, &'a Expression)>, span: Span },
    /// `id`: the widget name (`$n.attr(node, "id", value)` -> `host.style(NAME)`).
    WidgetName { value: AttrValue<'a>, span: Span },
    /// The inline style the X6 styling sees (`$n.attr(node, "style", ...)`): the static CSS
    /// declarations of `style=`, or its dynamic value, then the `style:` directives of CSS
    /// properties (a nullish value drops the declaration).
    InlineStyle { base: String, dynamic: Option<Dynamic<'a>>, directives: Vec<(String, AttrValue<'a>)>, span: Span },
    /// `style:<setter>={value}`: `$n.set(node, target, $n.setter_value(backend, name, value) ?? <fallback>)`
    /// (`$n.child_attr` for a child attribute); the value is parsed at runtime like `style=`. A
    /// nullish (or bad) value falls back to the static inline value or the matching rules
    /// (`fallback.0`, highest precedence first), then to `fallback.1`.
    StyleSetter { name: String, target: String, child_attr: bool, backend: String, value: Dynamic<'a>, fallback: (Vec<(Cond<'a>, Json)>, Fallback), span: Span },
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
    /// Element `element`'s dynamic `id` is `id` (`$.get(i<id>) === id`).
    Id { element: usize, id: String },
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
    /// `{:else if}`: transparent for local transitions (TR1, `$n.if_block(..., true)`).
    pub elseif: bool,
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
    /// The item binding: an identifier, or a destructuring pattern (its leaves are declared from
    /// the item at the start of the body, `Codegen::destructure`).
    pub context: Option<&'a Pattern>,
    pub index: Option<String>,
    pub key: Option<&'a Expression>,
    pub body: Fragment<'a>,
    pub fallback: Option<Fragment<'a>>,
}

/// `{@const name = init}`: a template-scoped derived (`const name = $.derived(() => init)`),
/// read as `$.get(name)`. `{@const { a, b } = init}` (`pattern`): one derived of `init`, then a
/// derived per leaf of the pattern (`name` is unused). A declaration tag (`{let x = $state(0)}`,
/// `declaration`) is emitted as the script would emit that declaration (`init` unused); both kinds
/// keep their source order (Svelte's `state.consts`).
#[derive(Debug)]
pub struct ConstDecl<'a> {
    pub span: Span,
    pub name: String,
    pub init: Option<&'a Expression>,
    pub pattern: Option<&'a Pattern>,
    pub declaration: Option<&'a crate::estree::VariableDeclaration>,
}

/// A binding a template block introduces: a snippet parameter, `{:then v}`, `{:catch e}`.
#[derive(Debug)]
pub enum Param<'a> {
    Id(String),
    /// Destructuring and/or a default value (`{#snippet s({ a, b = 1 })}`, `{:then [x, y]}`).
    Pattern(&'a Pattern),
}

/// `{#snippet name(a, b)}`: a native content factory `(anchor, a = $.noop, b = $.noop) => {...}`
/// that builds its body into the region `anchor` it is given; parameters are thunks (read `a()`).
#[derive(Debug)]
pub struct Snippet<'a> {
    pub span: Span,
    pub name: String,
    pub params: Vec<Param<'a>>,
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
    pub value: Option<Param<'a>>,
    pub error: Option<Param<'a>>,
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
