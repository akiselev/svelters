//! The designer's source model (DZ resolver v1): a `.svelte` file read as a tree of designable
//! nodes and rules, and the edits that change it without touching anything else.
//!
//! The visual designer keeps the source as the truth (cadatomic
//! `docs/research/2026-10-25-visual-ui-designer.md` §3.1–3.5). A gesture or an agent call names a
//! node by its start offset (the `file#offset` DZ0 puts in `source-loc`) or a rule by its selector's
//! offset (`style_offset` of DZ0's matched rules), and an [`Intent`]. [`Design::plan`] answers
//! with byte-range [`TextEdit`]s in the file's own style (quotes, separators, indentation), or a
//! [`Refusal`] that says why and points at the span to edit by hand. There is no printer: every
//! edit replaces a value, or inserts or removes one attribute or declaration.
//!
//! Every node, attribute and rule has an [`Editability`]:
//! - **editable**: an element or component whose attributes are literals (`"row"`, `{12}`,
//!   `{true}`, bare); a rule of this file's `<style>` whose selectors are kinds, classes and
//!   pseudo-classes;
//! - **structural**: blocks, `{expr}` text, `{@render}`, `{@html}`, dynamic elements and
//!   components, elements with a spread. They can be selected and moved but their logic is code;
//! - **opaque**: an expression value, a directive, a handler, an interpolated string, a rule with
//!   combinators or `:global`, a rule inside an at-rule. The designer shows it and jumps to it.
//!
//! The parse is strict (no `loose` mode): a file that does not parse is refused whole. Offsets
//! are UTF-8 byte offsets of the file as given; a file starting with a byte order mark is refused,
//! since the parser's offsets skip it.

mod edit;
mod structure;
#[cfg(test)]
mod tests;

pub use edit::{rebase, Intent, Literal, Plan};

use crate::diag::TextEdit;
use crate::estree::{Expression, LiteralValue, UnaryOperator};
use crate::svelte::ast::{AttributeLike, AttributeValue, Element, Fragment, FragmentNode, TextOrExpressionTag};
use crate::svelte::css::ast::{Node as CssNode, SimpleKind, StyleSheet};
use crate::svelte::parse::{parse, ParseOptions};

/// How far the designer may edit a node, an attribute or a rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Editability {
    Editable,
    Structural,
    Opaque,
}

/// Why something is not editable (also why a [`Refusal`] was given).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Why {
    /// A JavaScript expression that isn't a literal.
    Expression,
    /// A quoted value with `{…}` parts.
    Interpolated,
    /// An event handler or a `bind:` (a binding, DZ4).
    Binding,
    /// A directive (`class:`, `use:`, transitions, `{@attach}`, …).
    Directive,
    /// `{...props}`: any attribute may be overridden at run time.
    Spread,
    /// `{#if}`, `{#each}`, `{#await}`, `{#key}`, `{#snippet}`.
    Block,
    /// `<svelte:element>`, `<svelte:component>`: the kind is computed.
    Dynamic,
    /// `<svelte:head>`, `<svelte:window>`, `{@const}`, … : not part of the layout.
    Special,
    /// A selector with combinators, ids, attributes, `:global` or pseudo-class arguments.
    Selector,
    /// A rule inside an at-rule (`@media`, …) or nested in another rule.
    AtRule,
}

/// What kind of template node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Element,
    Component,
    Text,
    Expression,
    Html,
    Render,
    If,
    Each,
    Await,
    Key,
    Snippet,
    Special,
}

/// Which child list of its parent a node is in (blocks have several).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Branch {
    Body,
    Else,
    Pending,
    Then,
    Catch,
    Fallback,
}

/// One template node of the outline. Whitespace-only text and comments are left out.
#[derive(Clone, Debug)]
pub struct DesignNode {
    pub kind: NodeKind,
    /// The tag or component name; the block keyword (`if`, `each`, …) for blocks.
    pub name: String,
    pub start: u32,
    pub end: u32,
    pub parent: Option<usize>,
    pub branch: Branch,
    pub children: Vec<usize>,
    /// Child indices from the root fragment, counting outline nodes only.
    pub path: Vec<u32>,
    pub class: Editability,
    pub why: Option<Why>,
    pub attrs: Vec<DesignAttr>,
    /// End of the tag name (elements and components): where a first attribute goes.
    pub name_end: Option<u32>,
}

/// One attribute or directive of an element or component.
#[derive(Clone, Debug)]
pub struct DesignAttr {
    /// As written: `gap`, `bind:value`, `style:color`, `...` for a spread, `@attach`.
    pub name: String,
    pub start: u32,
    pub end: u32,
    /// The literal value when the attribute is editable.
    pub literal: Option<Literal>,
    pub class: Editability,
    pub why: Option<Why>,
}

/// One rule of the component's `<style>`.
#[derive(Clone, Debug)]
pub struct DesignRule {
    /// The selector text, as written.
    pub selector: String,
    /// Start of the selector: the rule's identity (DZ0's `style_offset`).
    pub start: u32,
    pub end: u32,
    /// The block's braces: `{` at `block_start`, `}` at `block_end - 1`.
    pub block_start: u32,
    pub block_end: u32,
    pub decls: Vec<DesignDecl>,
    pub class: Editability,
    pub why: Option<Why>,
}

/// One declaration of a rule.
#[derive(Clone, Debug)]
pub struct DesignDecl {
    pub property: String,
    pub value: String,
    /// From the property's first byte to the value's last (no `;`).
    pub start: u32,
    pub end: u32,
    pub value_start: u32,
}

/// Why an intent was not planned, and the span to edit by hand when there is one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub kind: RefusalKind,
    pub message: String,
    pub span: Option<(u32, u32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RefusalKind {
    /// The file does not parse (or starts with a byte order mark).
    Parse,
    /// No node starts at the offset.
    NoNode,
    /// No rule's selector starts at the offset.
    NoRule,
    /// The target is not editable; `why` says what it is.
    NotEditable(Why),
    /// The node has no attribute (or the rule no declaration) of that name.
    Missing,
    /// The name or value can't be written as given.
    BadValue,
    /// The edited file failed to parse again, or the target moved (a bug: please report).
    Reparse,
}

impl Refusal {
    fn new(kind: RefusalKind, message: impl Into<String>, span: Option<(u32, u32)>) -> Refusal {
        Refusal { kind, message: message.into(), span }
    }
}

/// A parsed `.svelte` file as the designer sees it.
#[derive(Debug)]
pub struct Design<'s> {
    src: &'s str,
    pub nodes: Vec<DesignNode>,
    /// Top-level outline nodes, in order.
    pub roots: Vec<usize>,
    pub rules: Vec<DesignRule>,
    /// Where the `<style>` element is, and its top-level rules and at-rules (DZR2's `AddRule`).
    style: Option<StyleInfo>,
}

/// The `<style>` element's layout.
#[derive(Debug)]
struct StyleInfo {
    /// The `<` of `<style`.
    start: u32,
    content_start: u32,
    content_end: u32,
    /// (start, end) of each top-level rule or at-rule, in order.
    tops: Vec<(u32, u32)>,
}

impl<'s> Design<'s> {
    pub fn parse(src: &'s str) -> Result<Design<'s>, Refusal> {
        if src.starts_with('\u{feff}') {
            return Err(Refusal::new(RefusalKind::Parse, "the file starts with a byte order mark", None));
        }
        let root = parse(src, &ParseOptions { loose: false, native: true })
            .map_err(|e| Refusal::new(RefusalKind::Parse, e.message, Some((e.start, e.end))))?;
        let sheet = crate::svelte::css::parse_style(src, &root)
            .map_err(|e| Refusal::new(RefusalKind::Parse, e.message, Some((e.start, e.end))))?;
        let mut d = Design { src, nodes: Vec::new(), roots: Vec::new(), rules: Vec::new(), style: None };
        d.roots = d.fragment(&root.fragment, None, Branch::Body, &[], &mut 0);
        if let Some(sheet) = &sheet {
            d.sheet(sheet);
        }
        if let Some(block) = &root.css {
            let tops = sheet.as_ref().map_or_else(Vec::new, |sh| {
                sh.children
                    .iter()
                    .filter_map(|c| match c {
                        CssNode::Rule(r) => Some((r.start as u32, r.end as u32)),
                        CssNode::Atrule(a) => Some((a.start as u32, a.end as u32)),
                        CssNode::Declaration(_) => None,
                    })
                    .collect()
            });
            d.style = Some(StyleInfo { start: block.start, content_start: block.content.start, content_end: block.content.end, tops });
        }
        Ok(d)
    }

    pub fn source(&self) -> &'s str {
        self.src
    }

    /// The node that starts at `offset`.
    pub fn node_at(&self, offset: u32) -> Option<&DesignNode> {
        self.nodes.iter().find(|n| n.start == offset)
    }

    /// The innermost node whose span contains `offset`.
    pub fn node_containing(&self, offset: u32) -> Option<&DesignNode> {
        self.nodes.iter().filter(|n| n.start <= offset && offset < n.end).min_by_key(|n| n.end - n.start)
    }

    /// The rule whose selector starts at `offset`.
    pub fn rule_at(&self, offset: u32) -> Option<&DesignRule> {
        self.rules.iter().find(|r| r.start == offset)
    }

    /// The edits for `intent`, checked by parsing the result again.
    pub fn plan(&self, intent: &Intent) -> Result<Plan, Refusal> {
        edit::plan(self, intent)
    }

    /// DZR2: plans `intents` in order, each against the text the previous ones left (so each
    /// intent's offsets are in that text: an earlier plan's `target`, or an offset passed through
    /// [`rebase`]), as ONE plan: `edits` in this file's offsets (sorted, not overlapping), `text`
    /// the final text, `target` the last intent's target. When the steps' edits can't be mapped
    /// back without overlapping, `edits` is one edit over the changed range (common prefix and
    /// suffix). One undo group for "override here" (`[AddClass, AddRule]`). No intents: no edits.
    pub fn plan_all(&self, intents: &[Intent]) -> Result<Plan, Refusal> {
        edit::plan_all(self, intents)
    }

    fn fragment(&mut self, f: &Fragment, parent: Option<usize>, branch: Branch, path: &[u32], k: &mut u32) -> Vec<usize> {
        let mut out = Vec::new();
        for n in &f.nodes {
            if let Some(i) = self.node(n, parent, branch, path, k) {
                out.push(i);
            }
        }
        out
    }

    fn push(&mut self, n: DesignNode) -> usize {
        self.nodes.push(n);
        self.nodes.len() - 1
    }

    fn node(&mut self, n: &FragmentNode, parent: Option<usize>, branch: Branch, path: &[u32], k: &mut u32) -> Option<usize> {
        use FragmentNode as F;
        let (start, end) = {
            let s = crate::estree::Spanned::span(n);
            (s.start, s.end)
        };
        let idx = *k;
        let blank = |kind, name: &str, class, why| DesignNode {
            kind,
            name: name.to_string(),
            start,
            end,
            parent,
            branch,
            children: Vec::new(),
            path: [path, &[idx]].concat(),
            class,
            why,
            attrs: Vec::new(),
            name_end: None,
        };
        let node = match n {
            F::Text(t) if t.data.trim().is_empty() => return None,
            F::Comment(_) => return None,
            F::Text(_) => blank(NodeKind::Text, "#text", Editability::Editable, None),
            F::ExpressionTag(_) => blank(NodeKind::Expression, "{expression}", Editability::Structural, Some(Why::Expression)),
            F::HtmlTag(_) => blank(NodeKind::Html, "@html", Editability::Structural, Some(Why::Expression)),
            F::RenderTag(_) => blank(NodeKind::Render, "@render", Editability::Structural, Some(Why::Block)),
            F::ConstTag(_) | F::DeclarationTag(_) | F::DebugTag(_) | F::AttachTag(_) => {
                blank(NodeKind::Special, "@tag", Editability::Opaque, Some(Why::Special))
            }
            F::RegularElement(e) => self.element(blank(NodeKind::Element, &e.name, Editability::Editable, None), e),
            F::Component(e) => self.element(blank(NodeKind::Component, &e.name, Editability::Editable, None), e),
            F::SvelteComponent(e) => {
                let mut d = blank(NodeKind::Component, &e.name, Editability::Structural, Some(Why::Dynamic));
                d.attrs = e.attributes.iter().map(|a| self.attr(a)).collect();
                d.name_end = Some(e.name_span.end);
                d
            }
            F::SvelteElement(e) => {
                let mut d = blank(NodeKind::Element, &e.name, Editability::Structural, Some(Why::Dynamic));
                d.attrs = e.attributes.iter().map(|a| self.attr(a)).collect();
                d.name_end = Some(e.name_span.end);
                d
            }
            F::SvelteBoundary(e) | F::SvelteFragment(e) | F::SlotElement(e) | F::SvelteSelf(e) => {
                let mut d = blank(NodeKind::Special, &e.name, Editability::Structural, Some(Why::Special));
                d.attrs = e.attributes.iter().map(|a| self.attr(a)).collect();
                d
            }
            F::TitleElement(e) | F::SvelteBody(e) | F::SvelteDocument(e) | F::SvelteHead(e) | F::SvelteOptionsRaw(e) | F::SvelteWindow(e) => {
                *k += 1;
                let d = blank(NodeKind::Special, &e.name, Editability::Opaque, Some(Why::Special));
                return Some(self.push(d));
            }
            F::IfBlock(_) => blank(NodeKind::If, "if", Editability::Structural, Some(Why::Block)),
            F::EachBlock(_) => blank(NodeKind::Each, "each", Editability::Structural, Some(Why::Block)),
            F::AwaitBlock(_) => blank(NodeKind::Await, "await", Editability::Structural, Some(Why::Block)),
            F::KeyBlock(_) => blank(NodeKind::Key, "key", Editability::Structural, Some(Why::Block)),
            F::SnippetBlock(_) => blank(NodeKind::Snippet, "snippet", Editability::Structural, Some(Why::Block)),
        };
        let my_path = node.path.clone();
        *k += 1;
        let i = self.push(node);
        let mut ck = 0u32;
        let mut kids = Vec::new();
        let mut sub = |d: &mut Self, f: &Fragment, b: Branch, ck: &mut u32| {
            let got = d.fragment(f, Some(i), b, &my_path, ck);
            kids.extend(got);
        };
        match n {
            F::RegularElement(e) | F::Component(e) | F::SvelteBoundary(e) | F::SvelteFragment(e) | F::SlotElement(e) | F::SvelteSelf(e) => {
                sub(self, &e.fragment, Branch::Body, &mut ck)
            }
            F::SvelteComponent(e) => sub(self, &e.fragment, Branch::Body, &mut ck),
            F::SvelteElement(e) => sub(self, &e.fragment, Branch::Body, &mut ck),
            F::IfBlock(b) => {
                sub(self, &b.consequent, Branch::Body, &mut ck);
                if let Some(a) = &b.alternate {
                    sub(self, a, Branch::Else, &mut ck);
                }
            }
            F::EachBlock(b) => {
                sub(self, &b.body, Branch::Body, &mut ck);
                if let Some(f) = &b.fallback {
                    sub(self, f, Branch::Fallback, &mut ck);
                }
            }
            F::AwaitBlock(b) => {
                for (f, br) in [(&b.pending, Branch::Pending), (&b.then, Branch::Then), (&b.catch, Branch::Catch)] {
                    if let Some(f) = f {
                        sub(self, f, br, &mut ck);
                    }
                }
            }
            F::KeyBlock(b) => sub(self, &b.fragment, Branch::Body, &mut ck),
            F::SnippetBlock(b) => sub(self, &b.body, Branch::Body, &mut ck),
            _ => {}
        }
        self.nodes[i].children = kids;
        Some(i)
    }

    fn element(&self, mut d: DesignNode, e: &Element) -> DesignNode {
        d.attrs = e.attributes.iter().map(|a| self.attr(a)).collect();
        d.name_end = Some(e.name_span.end);
        if e.attributes.iter().any(|a| matches!(a, AttributeLike::SpreadAttribute(_))) {
            d.class = Editability::Structural;
            d.why = Some(Why::Spread);
        }
        d
    }

    fn attr(&self, a: &AttributeLike) -> DesignAttr {
        use AttributeLike as A;
        let s = crate::estree::Spanned::span(a);
        let opaque = |name: String, why| DesignAttr { name, start: s.start, end: s.end, literal: None, class: Editability::Opaque, why: Some(why) };
        match a {
            A::Attribute(at) => {
                let handler = at.name.starts_with("on") && at.name.len() > 2;
                match value_literal(&at.value) {
                    Ok(lit) => DesignAttr { name: at.name.clone(), start: s.start, end: s.end, literal: Some(lit), class: Editability::Editable, why: None },
                    Err(Why::Expression) if handler => opaque(at.name.clone(), Why::Binding),
                    Err(why) => opaque(at.name.clone(), why),
                }
            }
            A::StyleDirective(sd) => {
                let name = format!("style:{}", sd.name);
                // the bare `style:color` reads a variable of that name
                match (&sd.value, sd.modifiers.is_empty()) {
                    (AttributeValue::True(_), _) => opaque(name, Why::Expression),
                    (v, true) => match value_literal(v) {
                        Ok(lit) => DesignAttr { name, start: s.start, end: s.end, literal: Some(lit), class: Editability::Editable, why: None },
                        Err(why) => opaque(name, why),
                    },
                    (_, false) => opaque(name, Why::Directive),
                }
            }
            A::SpreadAttribute(_) => opaque("...".into(), Why::Spread),
            A::AttachTag(_) => opaque("@attach".into(), Why::Directive),
            A::BindDirective(d) => opaque(format!("bind:{}", d.name), Why::Binding),
            A::OnDirective(d) => opaque(format!("on:{}", d.name), Why::Binding),
            A::ClassDirective(d) => opaque(format!("class:{}", d.name), Why::Directive),
            A::UseDirective(d) => opaque(format!("use:{}", d.name), Why::Directive),
            A::AnimateDirective(d) => opaque(format!("animate:{}", d.name), Why::Directive),
            A::LetDirective(d) => opaque(format!("let:{}", d.name), Why::Directive),
            A::TransitionDirective(d) => {
                let p = match (d.intro, d.outro) {
                    (true, false) => "in",
                    (false, true) => "out",
                    _ => "transition",
                };
                opaque(format!("{p}:{}", d.name), Why::Directive)
            }
        }
    }

    fn sheet(&mut self, sheet: &StyleSheet) {
        for n in &sheet.children {
            match n {
                CssNode::Rule(r) => self.rule(r, None),
                CssNode::Atrule(a) => {
                    if let Some(b) = &a.block {
                        for c in &b.children {
                            if let CssNode::Rule(r) = c {
                                self.rule(r, Some(Why::AtRule));
                            }
                        }
                    }
                }
                CssNode::Declaration(_) => {}
            }
        }
    }

    fn rule(&mut self, r: &crate::svelte::css::ast::Rule, forced: Option<Why>) {
        let mut decls = Vec::new();
        let mut nested = Vec::new();
        for c in &r.block.children {
            match c {
                CssNode::Declaration(d) => {
                    // without a final `;` the parser's span runs to the `}`: trim the gap
                    let text = self.src[d.start..d.end].trim_end();
                    let colon = text.find(':').map_or(0, |c| c + 1);
                    let ws = text[colon..].len() - text[colon..].trim_start().len();
                    decls.push(DesignDecl {
                        property: d.property.clone(),
                        value: d.value.clone(),
                        start: d.start as u32,
                        end: (d.start + text.len()) as u32,
                        value_start: (d.start + colon + ws) as u32,
                    });
                }
                CssNode::Rule(inner) => nested.push(inner),
                CssNode::Atrule(_) => {}
            }
        }
        let simple = r.prelude.children.iter().all(|c| {
            c.children.len() == 1
                && c.children[0].selectors.iter().all(|s| match s.kind {
                    SimpleKind::TypeSelector | SimpleKind::ClassSelector => true,
                    SimpleKind::PseudoClassSelector => s.name != "global" && (s.args.is_none() || s.name == "state"),
                    _ => false,
                })
        });
        let why = forced.or(if simple { None } else { Some(Why::Selector) });
        self.rules.push(DesignRule {
            selector: self.src[r.prelude.start..r.prelude.end].to_string(),
            start: r.prelude.start as u32,
            end: r.end as u32,
            block_start: r.block.start as u32,
            block_end: r.block.end as u32,
            decls,
            class: if why.is_none() { Editability::Editable } else { Editability::Opaque },
            why,
        });
        for inner in nested {
            self.rule(inner, Some(Why::AtRule));
        }
    }
}

/// The literal an attribute value is, or why it isn't one.
fn value_literal(v: &AttributeValue) -> Result<Literal, Why> {
    match v {
        AttributeValue::True(_) => Ok(Literal::Bool(true)),
        AttributeValue::Sequence(parts) => match parts.as_slice() {
            [] => Ok(Literal::Str(String::new())),
            [TextOrExpressionTag::Text(t)] => Ok(Literal::Str(t.data.clone())),
            _ => Err(Why::Interpolated),
        },
        AttributeValue::Expression(crate::svelte::ast::ExpressionTagNode::ExpressionTag(t)) => expr_literal(&t.expression).ok_or(Why::Expression),
    }
}

fn expr_literal(e: &Expression) -> Option<Literal> {
    match e {
        Expression::Literal(l) => match &l.value {
            LiteralValue::String(s) => Some(Literal::Str(s.clone())),
            LiteralValue::Number(n) => Some(Literal::Num(*n)),
            LiteralValue::Boolean(b) => Some(Literal::Bool(*b)),
            _ => None,
        },
        Expression::UnaryExpression(u) if u.operator == UnaryOperator::Minus => match expr_literal(&u.argument)? {
            Literal::Num(n) => Some(Literal::Num(-n)),
            _ => None,
        },
        _ => None,
    }
}

/// Applies sorted, non-overlapping edits.
pub fn apply(src: &str, edits: &[TextEdit]) -> String {
    let mut out = String::with_capacity(src.len());
    let mut cur = 0usize;
    for e in edits {
        out.push_str(&src[cur..e.start as usize]);
        out.push_str(&e.text);
        cur = e.end as usize;
    }
    out.push_str(&src[cur..]);
    out
}
