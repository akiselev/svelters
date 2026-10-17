//! Port of `svelte/src/compiler/phases/1-parse/index.js` (Svelte 5.57.2 @ f3de3f2). Owner: agent D.
//!
//! Layout mirrors upstream: `state/` (fragment, element, tag, text), `read/` (context,
//! expression, options, script, style), `utils/` (bracket, entities, html, whitespace),
//! `acorn.rs` (the acorn wrapper). Helpers that live elsewhere upstream (`src/utils.js`,
//! `html-tree-validation.js`, `phases/nodes.js`, `utils/ast.js`, `utils/string.js`) are in
//! `shared/`.
//!
//! Representation: upstream builds a mutable object graph where a node is both in its parent's
//! `fragment.nodes` and on `parser.stack`, and `parser.fragments` is a separate stack of fragment
//! objects. The port keeps exactly that structure with indices into two arenas (`nodes` for
//! nodes that own fragments, `frags` for fragments) and converts to the typed `ast::Root` at the
//! end. Positions are byte offsets; every place upstream steps one UTF-16 unit over an ASCII
//! delimiter steps one byte here, and steps over arbitrary characters advance by the character.

pub mod acorn;
pub mod read;
pub mod shared;
pub mod state;
pub mod utils;

#[cfg(test)]
mod tests;

use std::collections::HashSet;

use super::ast::{
    self, AttributeLike, AwaitBlock, EachBlock, Element, Fragment, FragmentNode, IfBlock, KeyBlock, Root, Script,
    SnippetBlock, StyleBlock, SvelteComponent, SvelteElement,
};
use super::errors::{self as e, CompileError, CompileWarning};
use crate::estree::{Comment as JsComment, Expression, Identifier, Pattern, Span};
use shared::utils::is_reserved;
use utils::whitespace::is_whitespace;

pub type PResult<T> = Result<T, CompileError>;

#[derive(Clone, Debug, Default)]
pub struct ParseOptions {
    /// Svelte's `loose` mode (error-tolerant parsing for editors).
    pub loose: bool,
    /// Native element mode (svelters deviation): no HTML-only rules. Element names are native
    /// names: no void elements, no implicit closing (`<p>`, `<li>`...), no SVG/MathML by name.
    /// The full list is in docs/notes/native-compiler/deviations.md.
    pub native: bool,
}

pub type NodeId = usize;
pub type FragId = usize;

/// An entry of a fragment: a finished leaf node, or a node that owns fragments (in `nodes`).
#[derive(Debug)]
pub enum Item {
    Leaf(FragmentNode),
    Node(NodeId),
}

/// `AST.Fragment` (metadata other than `transparent` is not kept).
#[derive(Debug, Default)]
pub struct FragData {
    pub nodes: Vec<Item>,
    pub transparent: bool,
}

/// Which `ElementLike` an element is (the `type` upstream).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementType {
    Component,
    TitleElement,
    SlotElement,
    RegularElement,
    SvelteBody,
    SvelteComponent,
    SvelteDocument,
    SvelteElement,
    SvelteFragment,
    SvelteBoundary,
    SvelteHead,
    SvelteOptions,
    SvelteSelf,
    SvelteWindow,
}

/// Upstream uses `end: -1` for nodes that are still open; in loose mode such nodes can survive
/// into the output. SVELTERS: positions are `u32`, so `-1` is represented by `u32::MAX`.
pub const OPEN_END: u32 = u32::MAX;

#[derive(Debug)]
pub struct ElementData {
    pub type_: ElementType,
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub name_span: Span,
    pub attributes: Vec<AttributeLike>,
    pub fragment: FragId,
    /// `SvelteComponent.expression`
    pub expression: Option<Expression>,
    /// `SvelteElement.tag`
    pub tag: Option<Expression>,
}

#[derive(Debug)]
pub struct IfData {
    pub elseif: bool,
    pub start: u32,
    pub end: u32,
    pub test: Expression,
    pub consequent: FragId,
    pub alternate: Option<FragId>,
}

#[derive(Debug)]
pub struct EachData {
    pub start: u32,
    pub end: u32,
    pub expression: Expression,
    pub context: Option<Pattern>,
    pub body: FragId,
    pub fallback: Option<FragId>,
    pub index: Option<String>,
    pub key: Option<Expression>,
}

#[derive(Debug)]
pub struct AwaitData {
    pub start: u32,
    pub end: u32,
    pub expression: Expression,
    pub value: Option<Pattern>,
    pub error: Option<Pattern>,
    pub pending: Option<FragId>,
    pub then: Option<FragId>,
    pub catch: Option<FragId>,
}

#[derive(Debug)]
pub struct KeyData {
    pub start: u32,
    pub end: u32,
    pub expression: Expression,
    pub fragment: FragId,
}

#[derive(Debug)]
pub struct SnippetData {
    pub start: u32,
    pub end: u32,
    pub expression: Identifier,
    pub type_params: Option<String>,
    pub parameters: Vec<Pattern>,
    pub body: FragId,
}

/// `AST.TemplateNode`s that can be on `parser.stack`.
#[derive(Debug)]
pub enum TemplateNode {
    Root,
    Element(ElementData),
    IfBlock(IfData),
    EachBlock(EachData),
    AwaitBlock(AwaitData),
    KeyBlock(KeyData),
    SnippetBlock(SnippetData),
}

impl TemplateNode {
    pub fn type_name(&self) -> &'static str {
        match self {
            TemplateNode::Root => "Root",
            TemplateNode::Element(el) => match el.type_ {
                ElementType::Component => "Component",
                ElementType::TitleElement => "TitleElement",
                ElementType::SlotElement => "SlotElement",
                ElementType::RegularElement => "RegularElement",
                ElementType::SvelteBody => "SvelteBody",
                ElementType::SvelteComponent => "SvelteComponent",
                ElementType::SvelteDocument => "SvelteDocument",
                ElementType::SvelteElement => "SvelteElement",
                ElementType::SvelteFragment => "SvelteFragment",
                ElementType::SvelteBoundary => "SvelteBoundary",
                ElementType::SvelteHead => "SvelteHead",
                ElementType::SvelteOptions => "SvelteOptions",
                ElementType::SvelteSelf => "SvelteSelf",
                ElementType::SvelteWindow => "SvelteWindow",
            },
            TemplateNode::IfBlock(_) => "IfBlock",
            TemplateNode::EachBlock(_) => "EachBlock",
            TemplateNode::AwaitBlock(_) => "AwaitBlock",
            TemplateNode::KeyBlock(_) => "KeyBlock",
            TemplateNode::SnippetBlock(_) => "SnippetBlock",
        }
    }

    pub fn start(&self) -> u32 {
        match self {
            TemplateNode::Root => 0,
            TemplateNode::Element(n) => n.start,
            TemplateNode::IfBlock(n) => n.start,
            TemplateNode::EachBlock(n) => n.start,
            TemplateNode::AwaitBlock(n) => n.start,
            TemplateNode::KeyBlock(n) => n.start,
            TemplateNode::SnippetBlock(n) => n.start,
        }
    }

    pub fn end(&self) -> u32 {
        match self {
            TemplateNode::Root => 0,
            TemplateNode::Element(n) => n.end,
            TemplateNode::IfBlock(n) => n.end,
            TemplateNode::EachBlock(n) => n.end,
            TemplateNode::AwaitBlock(n) => n.end,
            TemplateNode::KeyBlock(n) => n.end,
            TemplateNode::SnippetBlock(n) => n.end,
        }
    }

    pub fn set_end(&mut self, end: u32) {
        match self {
            TemplateNode::Root => {}
            TemplateNode::Element(n) => n.end = end,
            TemplateNode::IfBlock(n) => n.end = end,
            TemplateNode::EachBlock(n) => n.end = end,
            TemplateNode::AwaitBlock(n) => n.end = end,
            TemplateNode::KeyBlock(n) => n.end = end,
            TemplateNode::SnippetBlock(n) => n.end = end,
        }
    }

    /// `node.name` (elements only; `undefined` for blocks and the root).
    pub fn name(&self) -> Option<&str> {
        match self {
            TemplateNode::Element(n) => Some(&n.name),
            _ => None,
        }
    }

    pub fn as_element(&self) -> Option<&ElementData> {
        match self {
            TemplateNode::Element(n) => Some(n),
            _ => None,
        }
    }
}

/// `LastAutoClosedTag`
#[derive(Clone, Debug)]
pub struct LastAutoClosedTag {
    pub tag: String,
    pub reason: String,
    pub depth: usize,
}

/// Root-level data (`this.root` minus the fragment, which is `frags[0]`).
#[derive(Debug, Default)]
pub struct RootData {
    pub css: Option<StyleBlock>,
    pub instance: Option<Script>,
    pub module: Option<Script>,
    pub comments: Vec<JsComment>,
    pub options: Option<ast::SvelteOptions>,
}

/// `ParserState`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserState {
    Fragment,
    Element,
    Tag,
    Text,
}

/// A sticky pattern (upstream regex with the `y` flag, or with a `^` anchor): returns the length
/// of the match at byte offset `at`, if any.
pub type Pattern_ = fn(&str, usize) -> Option<usize>;

pub struct Parser {
    pub template: String,
    /// Whether or not we're in loose parsing mode, in which case we try to continue parsing as
    /// much as possible
    pub loose: bool,
    /// SVELTERS: native element mode, see [`ParseOptions::native`].
    pub native: bool,
    pub index: usize,
    /// Whether we're parsing in TypeScript mode (always false: TypeScript is rejected up front).
    pub ts: bool,
    pub stack: Vec<NodeId>,
    pub fragments: Vec<FragId>,
    pub nodes: Vec<TemplateNode>,
    pub frags: Vec<FragData>,
    pub root: RootData,
    pub meta_tags: HashSet<String>,
    pub last_auto_closed_tag: Option<LastAutoClosedTag>,
    /// Warnings (`w.*` calls) in emission order.
    pub warnings: Vec<CompileWarning>,
    /// `has_lf_line_breaks_only` cache: (template length, result).
    lf_cache: std::cell::Cell<Option<(usize, bool)>>,
}

impl Parser {
    /// `new Parser(template, loose)`; returns the parser after parsing the whole template.
    pub fn new(template: &str, opts: &ParseOptions) -> PResult<Parser> {
        let mut parser = Parser {
            template: trim_end(template).to_string(),
            loose: opts.loose,
            native: opts.native,
            index: 0,
            ts: false,
            stack: Vec::new(),
            fragments: Vec::new(),
            nodes: Vec::new(),
            frags: Vec::new(),
            root: RootData::default(),
            meta_tags: HashSet::new(),
            last_auto_closed_tag: None,
            warnings: Vec::new(),
            lf_cache: std::cell::Cell::new(None),
        };

        let match_lang = regex_lang_attribute(template);
        parser.ts = match_lang.as_ref().is_some_and(|(_, lang)| lang == "ts");

        // SVELTERS: acorn-typescript is not ported (deviations.md).
        if parser.ts {
            let (pos, _) = match_lang.unwrap();
            return Err(e::svelters_typescript_unsupported(pos));
        }

        parser.nodes.push(TemplateNode::Root);
        let root_fragment = parser.create_fragment(false);
        parser.stack.push(0);
        parser.fragments.push(root_fragment);

        let mut state = ParserState::Fragment;

        while parser.index < parser.template.len() {
            state = match state {
                ParserState::Fragment => state::fragment::fragment(&mut parser),
                ParserState::Element => {
                    state::element::element(&mut parser)?;
                    ParserState::Fragment
                }
                ParserState::Tag => {
                    state::tag::tag(&mut parser)?;
                    ParserState::Fragment
                }
                ParserState::Text => {
                    state::text::text(&mut parser);
                    ParserState::Fragment
                }
            };
        }

        if parser.stack.len() > 1 {
            let current = parser.current();
            let template_len = parser.template.len() as u32;
            let node = &mut parser.nodes[current];

            if parser.loose {
                node.set_end(template_len);
            } else if let TemplateNode::Element(el) = node
                && el.type_ == ElementType::RegularElement
            {
                el.end = el.start + 1;
                return Err(e::element_unclosed((el.start, el.end), &el.name));
            } else {
                let start = node.start();
                node.set_end(start + 1);
                return Err(e::block_unclosed((start, start + 1)));
            }
        }

        if state != ParserState::Fragment {
            return Err(e::unexpected_eof(parser.index));
        }

        let root_frag = 0;
        let options_index = parser.frags[root_frag].nodes.iter().position(|thing| match thing {
            Item::Node(id) => matches!(&parser.nodes[*id], TemplateNode::Element(el) if el.type_ == ElementType::SvelteOptions),
            Item::Leaf(_) => false,
        });
        if let Some(options_index) = options_index {
            let Item::Node(id) = parser.frags[root_frag].nodes.remove(options_index) else { unreachable!() };
            let TemplateNode::Element(options) = std::mem::replace(&mut parser.nodes[id], TemplateNode::Root) else {
                unreachable!()
            };
            let fragment = options.fragment;
            parser.root.options = Some(read::options::read_options(options)?);

            shared::nodes::disallow_children(&parser, fragment, "svelte:options")?;
        }

        Ok(parser)
    }

    pub fn current(&self) -> NodeId {
        *self.stack.last().expect("parser stack is never empty while parsing")
    }

    /// `create_fragment(transparent)`: allocates a fragment in the arena.
    pub fn create_fragment(&mut self, transparent: bool) -> FragId {
        self.frags.push(FragData { nodes: Vec::new(), transparent });
        self.frags.len() - 1
    }

    pub fn eat(&mut self, s: &str, required: bool, required_in_loose: bool) -> PResult<bool> {
        if self.match_str(s) {
            self.index += s.len();
            return Ok(true);
        }

        if required && (!self.loose || required_in_loose) {
            return Err(e::expected_token(self.index, s));
        }

        Ok(false)
    }

    /// `eat(str)` (not required).
    pub fn eat_opt(&mut self, s: &str) -> bool {
        if self.match_str(s) {
            self.index += s.len();
            return true;
        }
        false
    }

    /// `eat(str, true)`
    pub fn eat_required(&mut self, s: &str) -> PResult<bool> {
        self.eat(s, true, true)
    }

    /// `match(str)`
    pub fn match_str(&self, s: &str) -> bool {
        self.template.as_bytes().get(self.index..).is_some_and(|rest| rest.starts_with(s.as_bytes()))
    }

    /// Match a sticky pattern at the current index
    pub fn match_regex(&self, pattern: Pattern_) -> Option<&str> {
        if self.index > self.template.len() {
            return None;
        }
        pattern(&self.template, self.index).map(|len| &self.template[self.index..self.index + len])
    }

    /// The character at byte offset `i` (`charCodeAt`/`codePointAt`).
    pub fn char_at(&self, i: usize) -> Option<char> {
        self.template.get(i..).and_then(|s| s.chars().next())
    }

    /// The byte at `i` (`template[i]` for ASCII comparisons).
    pub fn byte_at(&self, i: usize) -> Option<u8> {
        self.template.as_bytes().get(i).copied()
    }

    pub fn allow_whitespace(&mut self) {
        while self.index < self.template.len() {
            match self.char_at(self.index) {
                Some(c) if is_whitespace(c as u32) => self.index += c.len_utf8(),
                _ => break,
            }
        }
    }

    /// Search for a regex starting at the current index and return the result if it matches
    pub fn read(&mut self, pattern: Pattern_) -> Option<String> {
        let result = self.match_regex(pattern).map(str::to_string);
        if let Some(r) = &result {
            self.index += r.len();
        }
        result
    }

    pub fn read_identifier(&mut self) -> PResult<Identifier> {
        let start = self.index;
        let mut end = start;
        let mut name = String::new();

        if let Some(code) = self.char_at(self.index)
            && shared::utils::is_identifier_start(code)
        {
            end += code.len_utf8();

            while end < self.template.len() {
                let code = self.char_at(end).unwrap();

                if !shared::utils::is_identifier_char(code) {
                    break;
                }
                end += code.len_utf8();
            }

            name = self.template[start..end].to_string();
            self.index = end;

            if is_reserved(&name) {
                return Err(e::unexpected_reserved_word(start, &name));
            }
        }

        Ok(Identifier { start: start as u32, end: end as u32, name })
    }

    pub fn read_until(&mut self, delimiter: &str) -> PResult<String> {
        if self.index >= self.template.len() {
            if self.loose {
                return Ok(String::new());
            }
            return Err(e::unexpected_eof(self.template.len()));
        }

        let start = self.index;
        match self.template[start..].find(delimiter) {
            Some(i) => {
                self.index = start + i;
                Ok(self.template[start..self.index].to_string())
            }
            None => {
                self.index = self.template.len();
                Ok(self.template[start..].to_string())
            }
        }
    }

    /// `read_until_regex(pattern)`: `search` returns the offset of the first match in `haystack`.
    pub fn read_until_regex(&mut self, search: fn(&str) -> Option<usize>) -> PResult<String> {
        if self.index >= self.template.len() {
            if self.loose {
                return Ok(String::new());
            }
            return Err(e::unexpected_eof(self.template.len()));
        }

        let start = self.index;
        match search(&self.template[start..]) {
            Some(i) => {
                self.index = start + i;
                Ok(self.template[start..self.index].to_string())
            }
            None => {
                self.index = self.template.len();
                Ok(self.template[start..].to_string())
            }
        }
    }

    pub fn require_whitespace(&mut self) -> PResult<()> {
        match self.char_at(self.index) {
            Some(c) if is_whitespace(c as u32) => {
                self.index += c.len_utf8();
            }
            _ => return Err(e::expected_whitespace(self.index)),
        }
        self.allow_whitespace();
        Ok(())
    }

    pub fn pop(&mut self) -> Option<NodeId> {
        if let Some(fragment) = self.fragments.pop() {
            let f = &self.frags[fragment];
            if f.transparent && f.nodes.iter().any(|n| matches!(n, Item::Leaf(FragmentNode::DeclarationTag(_)))) {
                self.frags[fragment].transparent = false;
            }
        }
        self.stack.pop()
    }

    pub fn append(&mut self, node: Item) {
        if let Some(&f) = self.fragments.last() {
            self.frags[f].nodes.push(node);
        }
    }

    /// Appends a node that owns fragments; returns its id.
    pub fn append_node(&mut self, node: TemplateNode) -> NodeId {
        self.nodes.push(node);
        let id = self.nodes.len() - 1;
        self.append(Item::Node(id));
        id
    }

    pub fn item_span(&self, item: &Item) -> Span {
        use crate::estree::Spanned;
        match item {
            Item::Leaf(n) => n.span(),
            Item::Node(id) => Span { start: self.nodes[*id].start(), end: self.nodes[*id].end() },
        }
    }

    pub fn element(&self, id: NodeId) -> &ElementData {
        match &self.nodes[id] {
            TemplateNode::Element(el) => el,
            _ => panic!("not an element"),
        }
    }

    pub fn element_mut(&mut self, id: NodeId) -> &mut ElementData {
        match &mut self.nodes[id] {
            TemplateNode::Element(el) => el,
            _ => panic!("not an element"),
        }
    }

    /// `has_lf_line_breaks_only(parser)` of acorn.js (cached per template).
    pub fn has_lf_line_breaks_only(&self) -> bool {
        let len = self.template.len();
        if let Some((l, v)) = self.lf_cache.get()
            && l == len
        {
            return v;
        }
        let v = acorn::has_lf_line_breaks_only(&self.template);
        self.lf_cache.set(Some((len, v)));
        v
    }

    // -----------------------------------------------------------------------------------------
    // conversion of the arena into the typed AST

    fn build_fragment(&mut self, f: FragId) -> Fragment {
        let items = std::mem::take(&mut self.frags[f].nodes);
        let mut nodes = Vec::with_capacity(items.len());
        for item in items {
            nodes.push(match item {
                Item::Leaf(n) => n,
                Item::Node(id) => self.build_node(id),
            });
        }
        Fragment { nodes }
    }

    fn build_opt_fragment(&mut self, f: Option<FragId>) -> Option<Fragment> {
        f.map(|f| self.build_fragment(f))
    }

    fn build_node(&mut self, id: NodeId) -> FragmentNode {
        match std::mem::replace(&mut self.nodes[id], TemplateNode::Root) {
            TemplateNode::Root => unreachable!("root or a node converted twice"),
            TemplateNode::Element(el) => {
                let fragment = self.build_fragment(el.fragment);
                let ElementData { type_, start, end, name, name_span, attributes, expression, tag, .. } = el;
                let variant: fn(Element) -> FragmentNode = match type_ {
                    ElementType::SvelteComponent => {
                        return FragmentNode::SvelteComponent(SvelteComponent {
                            start,
                            end,
                            name,
                            name_span,
                            attributes,
                            fragment,
                            expression: expression.expect("svelte:component expression"),
                        });
                    }
                    ElementType::SvelteElement => {
                        return FragmentNode::SvelteElement(SvelteElement {
                            start,
                            end,
                            name,
                            name_span,
                            attributes,
                            fragment,
                            tag: tag.expect("svelte:element tag"),
                        });
                    }
                    ElementType::Component => FragmentNode::Component,
                    ElementType::TitleElement => FragmentNode::TitleElement,
                    ElementType::SlotElement => FragmentNode::SlotElement,
                    ElementType::RegularElement => FragmentNode::RegularElement,
                    ElementType::SvelteBody => FragmentNode::SvelteBody,
                    ElementType::SvelteDocument => FragmentNode::SvelteDocument,
                    ElementType::SvelteFragment => FragmentNode::SvelteFragment,
                    ElementType::SvelteBoundary => FragmentNode::SvelteBoundary,
                    ElementType::SvelteHead => FragmentNode::SvelteHead,
                    ElementType::SvelteOptions => FragmentNode::SvelteOptionsRaw,
                    ElementType::SvelteSelf => FragmentNode::SvelteSelf,
                    ElementType::SvelteWindow => FragmentNode::SvelteWindow,
                };
                variant(Element { start, end, name, name_span, attributes, fragment })
            }
            TemplateNode::IfBlock(b) => FragmentNode::IfBlock(IfBlock {
                start: b.start,
                end: b.end,
                elseif: b.elseif,
                test: b.test,
                consequent: self.build_fragment(b.consequent),
                alternate: self.build_opt_fragment(b.alternate),
            }),
            TemplateNode::EachBlock(b) => FragmentNode::EachBlock(EachBlock {
                start: b.start,
                end: b.end,
                expression: b.expression,
                context: b.context,
                body: self.build_fragment(b.body),
                fallback: self.build_opt_fragment(b.fallback),
                index: b.index,
                key: b.key,
            }),
            TemplateNode::AwaitBlock(b) => FragmentNode::AwaitBlock(AwaitBlock {
                start: b.start,
                end: b.end,
                expression: b.expression,
                value: b.value,
                error: b.error,
                pending: self.build_opt_fragment(b.pending),
                then: self.build_opt_fragment(b.then),
                catch: self.build_opt_fragment(b.catch),
            }),
            TemplateNode::KeyBlock(b) => FragmentNode::KeyBlock(KeyBlock {
                start: b.start,
                end: b.end,
                expression: b.expression,
                fragment: self.build_fragment(b.fragment),
            }),
            TemplateNode::SnippetBlock(b) => FragmentNode::SnippetBlock(SnippetBlock {
                start: b.start,
                end: b.end,
                expression: b.expression,
                parameters: b.parameters,
                type_params: b.type_params,
                body: self.build_fragment(b.body),
            }),
        }
    }

    fn into_root(mut self, source_len: usize) -> (Root, Vec<CompileWarning>) {
        let fragment = self.build_fragment(0);
        let root = Root {
            start: 0,
            end: source_len as u32,
            options: self.root.options.take(),
            fragment,
            css: self.root.css.take(),
            instance: self.root.instance.take(),
            module: self.root.module.take(),
            comments: std::mem::take(&mut self.root.comments),
            ts: self.ts,
        };
        (root, self.warnings)
    }
}

/// `String.prototype.trimEnd` (JS WhiteSpace + LineTerminator, which is `is_whitespace`).
fn trim_end(s: &str) -> &str {
    s.trim_end_matches(|c: char| is_whitespace(c as u32))
}

/// `regex_lang_attribute`:
/// `/<!--[^]*?-->|<script\s+(?:[^>]*|(?:[^=>'"/]+=(?:"[^"]*"|'[^']*'|[^>\s]+)\s+)*)lang=(["'])?([^"' >]+)\1[^>]*>/g`,
/// executed repeatedly until a match starts with `<s`. Returns (match start, group 2).
///
/// Hand-written matcher. Backtracking is only observable through which `lang=` is captured;
/// the derivation of the search order is in the comments below.
fn regex_lang_attribute(template: &str) -> Option<(usize, String)> {
    let b = template.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'<' {
            if b[i..].starts_with(b"<!--") {
                // `[^]*?-->`: lazily up to the first `-->`
                if let Some(end) = template[i + 4..].find("-->") {
                    i = i + 4 + end + 3;
                    continue;
                }
            } else if b[i..].starts_with(b"<script")
                && let Some(lang) = lang_after_script(template, i + 7)
            {
                return Some((i, lang));
            }
        }
        i += template[i..].chars().next().map_or(1, char::len_utf8);
    }
    None
}

fn is_js_space(c: char) -> bool {
    is_whitespace(c as u32)
}

/// Matches `\s+(?:...)lang=...>` at `p` (just after `<script`).
fn lang_after_script(template: &str, p: usize) -> Option<String> {
    let s = &template[p..];
    // `\s+`: possessive is equivalent here (a shorter run only lets the group start on
    // whitespace, which neither `lang=` nor a new attribute iteration can use differently).
    let ws: usize = s.chars().take_while(|c| is_js_space(*c)).map(char::len_utf8).sum();
    if ws == 0 {
        return None;
    }
    let p = p + ws;

    // Alternative 1, `[^>]*`: greedy, so candidate positions are tried from the first `>` (or
    // the end) downwards.
    let q = template[p..].find('>').map_or(template.len(), |i| p + i);
    let mut i = q;
    loop {
        if let Some(lang) = lang_tail(template, i) {
            return Some(lang);
        }
        if i == p {
            break;
        }
        i -= 1;
        while !template.is_char_boundary(i) {
            i -= 1;
        }
    }

    // Alternative 2, `(?:[^=>'"/]+=(?:"[^"]*"|'[^']*'|[^>\s]+)\s+)*`: every sub-match is forced
    // (each greedy run is followed by a character it cannot contain), so the iterations are
    // deterministic; the star tries the longest repetition first.
    let mut positions = vec![p];
    let mut at = p;
    while let Some(next) = attribute_iteration(template, at) {
        positions.push(next);
        at = next;
    }
    for &pos in positions.iter().rev() {
        if let Some(lang) = lang_tail(template, pos) {
            return Some(lang);
        }
    }
    None
}

fn attribute_iteration(template: &str, at: usize) -> Option<usize> {
    let s = &template[at..];
    let name: usize = s.chars().take_while(|c| !matches!(c, '=' | '>' | '\'' | '"' | '/')).map(char::len_utf8).sum();
    if name == 0 || !s[name..].starts_with('=') {
        return None;
    }
    let mut i = at + name + 1;
    let rest = &template[i..];
    if let Some(r) = rest.strip_prefix('"') {
        i += 1 + r.find('"')? + 1;
    } else if let Some(r) = rest.strip_prefix('\'') {
        i += 1 + r.find('\'')? + 1;
    } else {
        let v: usize = rest.chars().take_while(|c| *c != '>' && !is_js_space(*c)).map(char::len_utf8).sum();
        if v == 0 {
            return None;
        }
        i += v;
    }
    let ws: usize = template[i..].chars().take_while(|c| is_js_space(*c)).map(char::len_utf8).sum();
    if ws == 0 {
        return None;
    }
    Some(i + ws)
}

/// `lang=(["'])?([^"' >]+)\1[^>]*>` at `at`.
fn lang_tail(template: &str, at: usize) -> Option<String> {
    let rest = template[at..].strip_prefix("lang=")?;
    let value_run = |s: &str| -> usize { s.chars().take_while(|c| !matches!(c, '"' | '\'' | ' ' | '>')).map(char::len_utf8).sum() };
    let (quote, s) = match rest.chars().next() {
        Some(q @ ('"' | '\'')) => (Some(q), &rest[1..]),
        _ => (None, rest),
    };
    let n = value_run(s);
    if n == 0 {
        return None;
    }
    let value = &s[..n];
    let mut after = &s[n..];
    if let Some(q) = quote {
        after = after.strip_prefix(q)?;
    }
    // `[^>]*>`
    after.find('>')?;
    Some(value.to_string())
}

/// Remove the byte order mark from a string if it's present (`remove_bom` of compiler/index.js)
fn remove_bom(source: &str) -> &str {
    source.strip_prefix('\u{feff}').unwrap_or(source)
}

/// `parse(source, { modern: true })` of `svelte/compiler` (BOM removal + `phases/1-parse`).
pub fn parse(src: &str, opts: &ParseOptions) -> Result<Root, CompileError> {
    parse_with_warnings(src, opts).map(|(root, _)| root)
}

/// Like [`parse`], also returning the warnings the parser emitted.
pub fn parse_with_warnings(src: &str, opts: &ParseOptions) -> Result<(Root, Vec<CompileWarning>), CompileError> {
    let template = remove_bom(src);
    let parser = Parser::new(template, opts)?;
    Ok(parser.into_root(template.len()))
}
