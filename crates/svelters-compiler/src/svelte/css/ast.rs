//! Svelte's CSS AST (`AST.CSS` of `svelte/src/compiler/types/css.d.ts`, Svelte 5.57.2), lossless
//! enough for the scoping transform: every node keeps its byte span in the component source.
//!
//! SVELTERS: `metadata` is not stored on the nodes. Rules, complex selectors and relative
//! selectors carry an `id` (their index in parse order) into the side tables of
//! [`super::Meta`], which the analysis and prune passes mutate through `Cell`s. Comments
//! (`StyleSheet.comments`) are not kept: nothing in the scoping pipeline reads them.

/// `AST.CSS.StyleSheet` (`content` only; the `<style>` attributes stay on `ast::StyleBlock`).
#[derive(Clone, Debug)]
pub struct StyleSheet {
    pub start: usize,
    pub end: usize,
    pub children: Vec<Node>,
    /// `content.start` / `content.end`: the text between `<style ...>` and `</style>`.
    pub content_start: usize,
    pub content_end: usize,
    /// Number of rules / complex selectors / relative selectors (the `id` ranges).
    pub counts: (usize, usize, usize),
}

/// A top-level or block child: `Rule | Atrule | Declaration` (declarations only inside blocks).
#[derive(Clone, Debug)]
pub enum Node {
    Rule(Rule),
    Atrule(Atrule),
    Declaration(Declaration),
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub id: usize,
    pub start: usize,
    pub end: usize,
    pub prelude: SelectorList,
    pub block: Block,
}

#[derive(Clone, Debug)]
pub struct Atrule {
    pub start: usize,
    pub end: usize,
    pub name: String,
    pub prelude: String,
    pub block: Option<Block>,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub start: usize,
    pub end: usize,
    pub children: Vec<Node>,
}

#[derive(Clone, Debug)]
pub struct Declaration {
    pub start: usize,
    pub end: usize,
    pub property: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct SelectorList {
    pub start: usize,
    pub end: usize,
    pub children: Vec<ComplexSelector>,
}

#[derive(Clone, Debug)]
pub struct ComplexSelector {
    pub id: usize,
    pub start: usize,
    pub end: usize,
    pub children: Vec<RelativeSelector>,
}

#[derive(Clone, Debug)]
pub struct RelativeSelector {
    pub id: usize,
    pub start: usize,
    pub end: usize,
    pub combinator: Option<Combinator>,
    pub selectors: Vec<SimpleSelector>,
}

#[derive(Clone, Debug)]
pub struct Combinator {
    pub start: usize,
    pub end: usize,
    /// `' '`, `+`, `~`, `>` or `||`
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SimpleKind {
    TypeSelector,
    IdSelector,
    ClassSelector,
    AttributeSelector,
    PseudoElementSelector,
    PseudoClassSelector,
    Percentage,
    Nth,
    NestingSelector,
}

/// Every `AST.CSS.SimpleSelector` in one struct: `kind` is its `type`; `name` holds `value` for
/// `Percentage` / `Nth`.
#[derive(Clone, Debug)]
pub struct SimpleSelector {
    pub kind: SimpleKind,
    pub start: usize,
    pub end: usize,
    pub name: String,
    /// `TypeSelector.namespace`
    pub namespace: Option<String>,
    /// `PseudoClassSelector.args` / `PseudoElementSelector.args`
    pub args: Option<SelectorList>,
    /// `AttributeSelector.matcher`
    pub matcher: Option<String>,
    /// `AttributeSelector.value`
    pub value: Option<String>,
    /// `AttributeSelector.flags`
    pub flags: Option<String>,
}

impl SimpleSelector {
    pub fn new(kind: SimpleKind, start: usize, end: usize, name: String) -> SimpleSelector {
        SimpleSelector { kind, start, end, name, namespace: None, args: None, matcher: None, value: None, flags: None }
    }

    pub fn is_pseudo_class(&self, name: &str) -> bool {
        self.kind == SimpleKind::PseudoClassSelector && self.name == name
    }

    pub fn is_pseudo(&self) -> bool {
        matches!(self.kind, SimpleKind::PseudoClassSelector | SimpleKind::PseudoElementSelector)
    }
}
