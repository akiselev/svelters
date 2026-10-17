//! Svelte's modern template AST (`AST` namespace of `svelte/src/compiler/types/template.d.ts`,
//! Svelte 5.57.2 @ f3de3f2), as produced by `parse(source, { modern: true })`.
//!
//! Contract for the port:
//! - Node/field names and shapes follow template.d.ts. `metadata` fields are NOT stored here;
//!   analysis keeps its own side tables keyed by node span or id.
//! - Positions are byte offsets (see `estree` module docs); `name_loc` is reduced to a span.
//! - Deviation: `Root.css` is the raw `<style>` block (`StyleBlock`), not Svelte's CSS AST. The
//!   native style subset is parsed separately by `crate::style` (we only implement the styling
//!   we can lower to native properties; see docs/notes/native-compiler/styling.md).
//! - Serialization matches `svelte/compiler`'s JSON for the oracle tests (they ignore `loc`,
//!   `name_loc`, `metadata`, and the CSS AST).

use crate::estree::{Comment as JsComment, Expression, Identifier, Pattern, Program, Span, Spanned, VariableDeclaration};
use serde::Serialize;

macro_rules! node {
    ($(#[$m:meta])* pub struct $name:ident { $($(#[$fm:meta])* pub $f:ident : $t:ty),* $(,)? }) => {
        #[derive(Clone, Debug, PartialEq, Serialize)]
        $(#[$m])*
        pub struct $name {
            pub start: u32,
            pub end: u32,
            $($(#[$fm])* pub $f: $t,)*
        }
        impl Spanned for $name {
            fn span(&self) -> Span { Span { start: self.start, end: self.end } }
        }
    };
}

node! {
    #[serde(tag = "type", rename = "Root")]
    pub struct Root {
        pub options: Option<SvelteOptions>,
        pub fragment: Fragment,
        pub css: Option<StyleBlock>,
        /// Absent (not null) in Svelte's JSON when there is no instance script.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub instance: Option<Script>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub module: Option<Script>,
        /// Comments found in `<script>` and `{expressions}`.
        pub comments: Vec<JsComment>,
        /// Not serialized by Svelte as a field; kept for consumers (`metadata.ts` upstream).
        #[serde(skip)]
        pub ts: bool,
    }
}

/// `<svelte:options>`; only the fields Svelte interprets, plus the raw attributes.
node! {
    pub struct SvelteOptions {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub runes: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub immutable: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub accessors: Option<bool>,
        #[serde(rename = "preserveWhitespace", skip_serializing_if = "Option::is_none")]
        pub preserve_whitespace: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub namespace: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub css: Option<String>,
        /// `customRenderer` option of the custom-renderer fork: a module path, or null to opt out.
        #[serde(rename = "customRenderer", skip_serializing_if = "Option::is_none")]
        pub custom_renderer: Option<Option<String>>,
        /// `customElement` is accepted by the parser but unsupported by this compiler: kept raw.
        #[serde(skip)]
        pub custom_element: Option<Span>,
        pub attributes: Vec<Attribute>,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename = "Fragment")]
pub struct Fragment {
    pub nodes: Vec<FragmentNode>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum FragmentNode {
    Text(Text),
    Comment(Comment),
    // tags
    ExpressionTag(ExpressionTag),
    HtmlTag(HtmlTag),
    ConstTag(ConstTag),
    DeclarationTag(DeclarationTag),
    DebugTag(DebugTag),
    RenderTag(RenderTag),
    AttachTag(AttachTag),
    // element-likes
    Component(Element),
    TitleElement(Element),
    SlotElement(Element),
    RegularElement(Element),
    SvelteBody(Element),
    SvelteComponent(SvelteComponent),
    SvelteDocument(Element),
    SvelteElement(SvelteElement),
    SvelteFragment(Element),
    SvelteBoundary(Element),
    SvelteHead(Element),
    #[serde(rename = "SvelteOptions")]
    SvelteOptionsRaw(Element),
    SvelteSelf(Element),
    SvelteWindow(Element),
    // blocks
    EachBlock(EachBlock),
    IfBlock(IfBlock),
    AwaitBlock(AwaitBlock),
    KeyBlock(KeyBlock),
    SnippetBlock(SnippetBlock),
}

impl Spanned for FragmentNode {
    fn span(&self) -> Span {
        use FragmentNode::*;
        match self {
            Text(n) => n.span(),
            Comment(n) => n.span(),
            ExpressionTag(n) => n.span(),
            HtmlTag(n) => n.span(),
            ConstTag(n) => n.span(),
            DeclarationTag(n) => n.span(),
            DebugTag(n) => n.span(),
            RenderTag(n) => n.span(),
            AttachTag(n) => n.span(),
            Component(n) | TitleElement(n) | SlotElement(n) | RegularElement(n) | SvelteBody(n) | SvelteDocument(n)
            | SvelteFragment(n) | SvelteBoundary(n) | SvelteHead(n) | SvelteOptionsRaw(n) | SvelteSelf(n)
            | SvelteWindow(n) => n.span(),
            SvelteComponent(n) => n.span(),
            SvelteElement(n) => n.span(),
            EachBlock(n) => n.span(),
            IfBlock(n) => n.span(),
            AwaitBlock(n) => n.span(),
            KeyBlock(n) => n.span(),
            SnippetBlock(n) => n.span(),
        }
    }
}

node! {
    /// Static text. `data` has entities decoded, `raw` is the source text.
    pub struct Text { pub data: String, pub raw: String }
}
node! { pub struct Comment { pub data: String } }
node! { pub struct ExpressionTag { pub expression: Expression } }
node! { pub struct HtmlTag { pub expression: Expression } }
node! {
    /// `{@const x = y}`: exactly one declarator with an init.
    pub struct ConstTag { pub declaration: VariableDeclaration }
}
node! { pub struct DeclarationTag { pub declaration: VariableDeclaration } }
node! { pub struct DebugTag { pub identifiers: Vec<Identifier> } }
node! {
    /// `expression` is a CallExpression or a ChainExpression wrapping one.
    pub struct RenderTag { pub expression: Expression }
}
node! { pub struct AttachTag { pub expression: Expression } }

node! {
    /// Shared shape of every element-like node (RegularElement, Component, SvelteHead, ...);
    /// the enum variant in `FragmentNode` carries the `type`.
    pub struct Element {
        pub name: String,
        #[serde(skip)]
        pub name_span: Span,
        pub attributes: Vec<AttributeLike>,
        pub fragment: Fragment,
    }
}
node! {
    pub struct SvelteComponent {
        pub name: String,
        #[serde(skip)]
        pub name_span: Span,
        pub attributes: Vec<AttributeLike>,
        pub fragment: Fragment,
        pub expression: Expression,
    }
}
node! {
    pub struct SvelteElement {
        pub name: String,
        #[serde(skip)]
        pub name_span: Span,
        pub attributes: Vec<AttributeLike>,
        pub fragment: Fragment,
        pub tag: Expression,
    }
}

node! {
    pub struct EachBlock {
        pub expression: Expression,
        pub context: Option<Pattern>,
        pub body: Fragment,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub fallback: Option<Fragment>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub index: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub key: Option<Expression>,
    }
}
node! {
    pub struct IfBlock {
        pub elseif: bool,
        pub test: Expression,
        pub consequent: Fragment,
        pub alternate: Option<Fragment>,
    }
}
node! {
    pub struct AwaitBlock {
        pub expression: Expression,
        pub value: Option<Pattern>,
        pub error: Option<Pattern>,
        pub pending: Option<Fragment>,
        pub then: Option<Fragment>,
        pub catch: Option<Fragment>,
    }
}
node! { pub struct KeyBlock { pub expression: Expression, pub fragment: Fragment } }
node! {
    pub struct SnippetBlock {
        pub expression: Identifier,
        pub parameters: Vec<Pattern>,
        #[serde(rename = "typeParams", skip_serializing_if = "Option::is_none")]
        pub type_params: Option<String>,
        pub body: Fragment,
    }
}

// ---------------------------------------------------------------------------------------------
// attributes and directives

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum AttributeLike {
    Attribute(Attribute),
    SpreadAttribute(SpreadAttribute),
    AttachTag(AttachTag),
    AnimateDirective(Directive),
    BindDirective(Directive),
    ClassDirective(Directive),
    LetDirective(Directive),
    OnDirective(Directive),
    StyleDirective(StyleDirective),
    TransitionDirective(TransitionDirective),
    UseDirective(Directive),
}

impl Spanned for AttributeLike {
    fn span(&self) -> Span {
        use AttributeLike::*;
        match self {
            Attribute(n) => n.span(),
            SpreadAttribute(n) => n.span(),
            AttachTag(n) => n.span(),
            AnimateDirective(n) | BindDirective(n) | ClassDirective(n) | LetDirective(n) | OnDirective(n) | UseDirective(n) => n.span(),
            StyleDirective(n) => n.span(),
            TransitionDirective(n) => n.span(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum AttributeValue {
    /// Bare attribute (`disabled`).
    True(bool),
    /// `name={expr}`.
    Expression(ExpressionTagNode),
    /// Quoted or unquoted text with interpolations.
    Sequence(Vec<TextOrExpressionTag>),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum ExpressionTagNode {
    ExpressionTag(ExpressionTag),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum TextOrExpressionTag {
    Text(Text),
    ExpressionTag(ExpressionTag),
}

node! {
    pub struct Attribute {
        pub name: String,
        #[serde(skip)]
        pub name_span: Option<Span>,
        pub value: AttributeValue,
    }
}
node! { pub struct SpreadAttribute { pub expression: Expression } }
node! {
    /// Animate/Bind/Class/Let/On/Use directives. `expression` is None for the bare form
    /// (`on:click`, `use:action`); for `bind:x` / `class:x` shorthands the parser synthesises the
    /// Identifier as upstream does. `modifiers` is empty except for `on:` (legacy).
    pub struct Directive {
        pub name: String,
        #[serde(skip)]
        pub name_span: Option<Span>,
        pub expression: Option<Expression>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        pub modifiers: Vec<String>,
    }
}
node! {
    pub struct StyleDirective {
        pub name: String,
        #[serde(skip)]
        pub name_span: Option<Span>,
        pub value: AttributeValue,
        pub modifiers: Vec<String>,
    }
}
node! {
    pub struct TransitionDirective {
        pub name: String,
        #[serde(skip)]
        pub name_span: Option<Span>,
        pub expression: Option<Expression>,
        pub modifiers: Vec<String>,
        pub intro: bool,
        pub outro: bool,
    }
}

// ---------------------------------------------------------------------------------------------
// script / style

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScriptContext {
    Default,
    Module,
}

node! {
    #[serde(tag = "type", rename = "Script")]
    pub struct Script {
        pub context: ScriptContext,
        pub content: Program,
        pub attributes: Vec<Attribute>,
    }
}

node! {
    /// Raw `<style>` block (deviation, see module docs). `content` spans the text between the tags.
    #[serde(tag = "type", rename = "StyleSheet")]
    pub struct StyleBlock {
        pub attributes: Vec<Attribute>,
        pub content: StyleContent,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StyleContent {
    pub start: u32,
    pub end: u32,
    pub styles: String,
}
