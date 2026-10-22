//! Style sheets: parsing rules, declarations, nesting, `@media` and `@layer` (CSS.md §2.1: our own
//! sheet parser on cssparser's tokenizer, with source positions for diagnostics).
//!
//! The sheet is table-independent: declaration values stay text until a [`Stylist`](crate::Stylist)
//! checks them against a backend's table. Unknown at-rules are dropped with a warning.

use crate::atom::Atom;
use crate::diag::Diagnostic;
use crate::media::MediaList;
use crate::selector::{SelectorList, SelectorParser, StyleParseError, describe};
use crate::state::StateNames;
use crate::var::has_var;
use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, ParseError, Parser, ParserState, QualifiedRuleParser, RuleBodyItemParser,
    RuleBodyParser, StyleSheetParser, Token,
};
use selectors::parser::ParseRelative;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// Parser settings.
#[derive(Clone, Copy, Default)]
pub struct ParseOptions<'s> {
    /// Resolves `:state(ns.name)`; without it, `:state()` is an error.
    pub states: Option<&'s dyn StateNames>,
}

/// One declaration: `name: value [!important]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    /// Lowercased, except custom properties (`--Accent` and `--accent` differ).
    pub name: Atom,
    /// The value as written (trimmed, without `!important`).
    pub value: Box<str>,
    pub important: bool,
    /// Whether the value calls `var()`.
    pub has_var: bool,
    pub line: u32,
    pub column: u32,
}

impl Declaration {
    pub fn is_custom(&self) -> bool {
        self.name.starts_with("--")
    }
}

/// The declarations of one rule or of an inline `style=`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeclBlock {
    pub decls: Vec<Declaration>,
}

impl DeclBlock {
    /// Parses the text of a `style=` attribute.
    pub fn parse_inline(text: &str) -> (DeclBlock, Vec<Diagnostic>) {
        let opts = ParseOptions::default();
        let cx = Ctx::new(&opts);
        let mut input = Parser::new(text);
        let mut p = RulesParser { cx: &cx, nested_in: Some(SelectorList::scope()), layer: None, media: Vec::new(), decls_only: true };
        let decls = collect_body(&mut p, &mut input);
        (DeclBlock { decls }, cx.diags.into_inner())
    }

    pub fn get(&self, name: &str) -> Option<&Declaration> {
        self.decls.iter().rev().find(|d| &*d.name == name)
    }
}

/// A style rule, flattened: nesting is resolved into the selectors, and the enclosing `@media`
/// and `@layer` blocks are recorded on the rule.
#[derive(Clone, Debug)]
pub struct Rule {
    pub selectors: SelectorList,
    pub block: Arc<DeclBlock>,
    /// Every enclosing `@media`; the rule applies when all of them match.
    pub media: Vec<Arc<MediaList>>,
    /// The full layer name (`base.buttons`); anonymous layers get a unique internal name.
    pub layer: Option<Atom>,
    pub line: u32,
    pub column: u32,
}

/// A parsed sheet: rules in source order, the layer names in order of first appearance, and what
/// was dropped.
#[derive(Clone, Debug, Default)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
    pub layers: Vec<Atom>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Stylesheet {
    pub fn parse(text: &str, opts: &ParseOptions<'_>) -> Stylesheet {
        let cx = Ctx::new(opts);
        let mut input = Parser::new(text);
        let mut p = RulesParser { cx: &cx, nested_in: None, layer: None, media: Vec::new(), decls_only: false };
        let iter = StyleSheetParser::new(&mut input, &mut p);
        for r in iter {
            if let Err((e, _, loc)) = r {
                cx.diags.borrow_mut().push(Diagnostic::error(loc, describe(&e)));
            }
        }
        let rules = cx.rules.into_inner().into_iter().flatten().collect();
        Stylesheet { rules, layers: cx.layers.into_inner(), diagnostics: cx.diags.into_inner() }
    }
}

struct Ctx<'o> {
    opts: &'o ParseOptions<'o>,
    /// Rules in source order; a slot is reserved when a style rule starts, so a parent rule comes
    /// before the rules nested in it.
    rules: RefCell<Vec<Option<Rule>>>,
    layers: RefCell<Vec<Atom>>,
    diags: RefCell<Vec<Diagnostic>>,
}

impl<'o> Ctx<'o> {
    fn new(opts: &'o ParseOptions<'o>) -> Self {
        Ctx { opts, rules: RefCell::new(Vec::new()), layers: RefCell::new(Vec::new()), diags: RefCell::new(Vec::new()) }
    }

    fn declare_layer(&self, name: &Atom) {
        let mut l = self.layers.borrow_mut();
        // a sublayer declares its parents first (`@layer a.b` → a, a.b)
        let mut prefix = String::new();
        for part in name.split('.') {
            if !prefix.is_empty() {
                prefix.push('.');
            }
            prefix.push_str(part);
            if !l.iter().any(|x| **x == *prefix) {
                l.push(Atom::new(&prefix));
            }
        }
    }
}

static ANON_LAYERS: AtomicU32 = AtomicU32::new(0);

struct RulesParser<'a, 'o> {
    cx: &'a Ctx<'o>,
    /// The enclosing style rule's selectors: set inside a style rule (declarations allowed,
    /// nested selectors are relative to it).
    nested_in: Option<SelectorList>,
    layer: Option<Atom>,
    media: Vec<Arc<MediaList>>,
    /// `style=`: declarations only.
    decls_only: bool,
}

enum Item {
    Decl(Declaration),
    Done,
}

enum AtPrelude {
    Media(Arc<MediaList>),
    /// Names; empty = anonymous.
    Layer(Vec<Atom>),
    Ignored(String),
}

type PErr<'i> = ParseError<StyleParseError>;

impl<'a, 'o> RulesParser<'a, 'o> {
    fn child(&self, nested_in: Option<SelectorList>, layer: Option<Atom>, media: Vec<Arc<MediaList>>) -> RulesParser<'a, 'o> {
        RulesParser { cx: self.cx, nested_in, layer, media, decls_only: false }
    }

    /// Parses a style block for `selectors` into a reserved slot: its declarations make the rule,
    /// nested rules follow it.
    fn style_block<'i>(&self, selectors: SelectorList, start: &ParserState, input: &mut Parser<'i>) {
        let slot = {
            let mut r = self.cx.rules.borrow_mut();
            r.push(None);
            r.len() - 1
        };
        let mut body = self.child(Some(selectors.clone()), self.layer.clone(), self.media.clone());
        let decls = collect_body(&mut body, input);
        if decls.is_empty() {
            return;
        }
        let loc = start.source_location();
        self.cx.rules.borrow_mut()[slot] = Some(Rule {
            selectors,
            block: Arc::new(DeclBlock { decls }),
            media: self.media.clone(),
            layer: self.layer.clone(),
            line: loc.line + 1,
            column: loc.column,
        });
    }

    /// A group rule's body (`@media`/`@layer`): rules, or inside a style rule, declarations too.
    fn group_block<'i>(&self, layer: Option<Atom>, media: Vec<Arc<MediaList>>, start: &ParserState, input: &mut Parser<'i>) {
        match self.nested_in.clone() {
            // inside a style rule, the group's declarations apply to that rule's selectors
            Some(sel) => self.child(None, layer, media).style_block(sel, start, input),
            None => {
                let mut body = self.child(None, layer, media);
                let iter = RuleBodyParser::new(input, &mut body);
                for r in iter {
                    if let Err((e, _, loc)) = r {
                        self.cx.diags.borrow_mut().push(Diagnostic::error(loc, describe(&e)));
                    }
                }
            }
        }
    }

    fn parse_selectors<'i>(&self, input: &mut Parser<'i>) -> Result<SelectorList, PErr<'i>> {
        let parser = SelectorParser { states: self.cx.opts.states };
        match &self.nested_in {
            None => SelectorList::parse(&parser, input, ParseRelative::No),
            Some(parent) => {
                let list = SelectorList::parse(&parser, input, ParseRelative::ForNesting)?;
                // CSS Nesting §2: a nested selector without `&` is relative to the parent as a
                // descendant; make that explicit, then substitute the parent (`:is(parent)`).
                let mut fixed = Vec::with_capacity(list.len());
                for sel in list.slice() {
                    if sel.has_parent_selector() {
                        fixed.push(sel.clone());
                    } else {
                        let text = format!("& {}", cssparser::ToCss::to_css_string(sel));
                        let mut inner = Parser::new(&text);
                        let reparsed = SelectorList::parse(&parser, &mut inner, ParseRelative::ForNesting)
                            .map_err(|_| ParseError::custom(StyleParseError::Other(format!("can't nest `{text}`"))))?;
                        fixed.extend(reparsed.slice().iter().cloned());
                    }
                }
                Ok(SelectorList::from_iter(fixed.into_iter()).replace_parent_selector(parent))
            }
        }
    }
}

/// Runs a rule body and returns its declarations; errors become diagnostics.
fn collect_body<'i>(p: &mut RulesParser<'_, '_>, input: &mut Parser<'i>) -> Vec<Declaration> {
    let cx = p.cx;
    let mut decls = Vec::new();
    let iter = RuleBodyParser::new(input, p);
    for r in iter {
        match r {
            Ok(Item::Decl(d)) => decls.push(d),
            Ok(Item::Done) => {}
            Err((e, _, loc)) => cx.diags.borrow_mut().push(Diagnostic::error(loc, describe(&e))),
        }
    }
    decls
}

impl<'i> DeclarationParser<'i> for RulesParser<'_, '_> {
    type Declaration = Item;
    type Error = StyleParseError;

    fn parse_value(&mut self, name: CowRcStr<'i>, input: &mut Parser<'i>, start: &ParserState) -> Result<Item, PErr<'i>> {
        let name = if name.starts_with("--") { name.to_string() } else { name.to_ascii_lowercase() };
        let value_start = input.position();
        loop {
            let before = input.state();
            match input.next_including_whitespace() {
                Err(_) => break,
                Ok(Token::Delim('!')) => {
                    input.reset(&before);
                    break;
                }
                Ok(_) => {}
            }
        }
        let value = input.slice_from(value_start).trim();
        let important = input.try_parse(cssparser::parse_important).is_ok();
        input.expect_exhausted()?;
        if value.is_empty() && !name.starts_with("--") {
            return Err(ParseError::custom(StyleParseError::Other(format!("`{name}` has no value"))));
        }
        let loc = start.source_location();
        Ok(Item::Decl(Declaration {
            name: Atom::new(&name),
            value: value.into(),
            important,
            has_var: has_var(value),
            line: loc.line + 1,
            column: loc.column,
        }))
    }
}

impl<'i> QualifiedRuleParser<'i> for RulesParser<'_, '_> {
    type Prelude = SelectorList;
    type QualifiedRule = Item;
    type Error = StyleParseError;

    fn parse_prelude(&mut self, input: &mut Parser<'i>) -> Result<SelectorList, PErr<'i>> {
        self.parse_selectors(input)
    }

    fn parse_block(&mut self, prelude: SelectorList, start: &ParserState, input: &mut Parser<'i>) -> Result<Item, PErr<'i>> {
        self.style_block(prelude, start, input);
        Ok(Item::Done)
    }
}

impl<'i> AtRuleParser<'i> for RulesParser<'_, '_> {
    type Prelude = AtPrelude;
    type AtRule = Item;
    type Error = StyleParseError;

    fn parse_prelude(&mut self, name: CowRcStr<'i>, input: &mut Parser<'i>) -> Result<AtPrelude, PErr<'i>> {
        let start = input.position();
        while input.next().is_ok() {}
        let text = input.slice_from(start);
        match name.to_ascii_lowercase().as_str() {
            "media" => MediaList::parse(text)
                .map(|m| AtPrelude::Media(Arc::new(m)))
                .map_err(|e| ParseError::custom(StyleParseError::Other(format!("@media: {e}")))),
            "layer" => {
                let mut names = Vec::new();
                for part in text.split(',') {
                    let n = part.trim();
                    if n.is_empty() && text.trim().is_empty() {
                        break;
                    }
                    let valid = !n.is_empty()
                        && n.split('.').all(|s| !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
                    if !valid {
                        return Err(ParseError::custom(StyleParseError::Other(format!("@layer: bad layer name `{n}`"))));
                    }
                    names.push(Atom::new(n));
                }
                Ok(AtPrelude::Layer(names))
            }
            other => Ok(AtPrelude::Ignored(other.to_string())),
        }
    }

    fn rule_without_block(&mut self, prelude: AtPrelude, start: &ParserState) -> Result<Item, ()> {
        match prelude {
            AtPrelude::Layer(names) if !names.is_empty() => {
                for n in names {
                    let full = self.full_layer(&n);
                    self.cx.declare_layer(&full);
                }
                Ok(Item::Done)
            }
            AtPrelude::Ignored(name) => {
                self.ignored(&name, start);
                Ok(Item::Done)
            }
            _ => Err(()),
        }
    }

    fn parse_block(&mut self, prelude: AtPrelude, start: &ParserState, input: &mut Parser<'i>) -> Result<Item, PErr<'i>> {
        match prelude {
            AtPrelude::Media(m) => {
                let mut media = self.media.clone();
                media.push(m);
                self.group_block(self.layer.clone(), media, start, input);
            }
            AtPrelude::Layer(names) => {
                let name = match names.len() {
                    0 => Atom::new(&format!("\u{0}anon{}", ANON_LAYERS.fetch_add(1, Ordering::Relaxed))),
                    1 => names[0].clone(),
                    _ => return Err(ParseError::custom(StyleParseError::Other("@layer with a block takes one name".into()))),
                };
                let full = self.full_layer(&name);
                self.cx.declare_layer(&full);
                self.group_block(Some(full), self.media.clone(), start, input);
            }
            AtPrelude::Ignored(name) => self.ignored(&name, start),
        }
        Ok(Item::Done)
    }
}

impl RulesParser<'_, '_> {
    fn full_layer(&self, name: &Atom) -> Atom {
        match &self.layer {
            Some(outer) => Atom::new(&format!("{outer}.{name}")),
            None => name.clone(),
        }
    }

    fn ignored(&self, name: &str, start: &ParserState) {
        self.cx.diags.borrow_mut().push(Diagnostic::warning(start.source_location(), format!("@{name} is not supported; the rule is ignored")));
    }
}

impl<'i> RuleBodyItemParser<'i, Item, StyleParseError> for RulesParser<'_, '_> {
    fn parse_declarations(&self) -> bool {
        self.nested_in.is_some()
    }
    fn parse_qualified(&self) -> bool {
        !self.decls_only
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cssparser::ToCss;

    fn sels(sheet: &Stylesheet) -> Vec<String> {
        sheet.rules.iter().map(|r| r.selectors.slice().iter().map(|s| s.to_css_string()).collect::<Vec<_>>().join(", ")).collect()
    }

    #[test]
    fn rules_and_declarations() {
        let s = Stylesheet::parse(".a, b > c { color: red; --X: 1px 2px ; width: calc(1px + var(--X)) !important }", &Default::default());
        assert!(s.diagnostics.is_empty(), "{:?}", s.diagnostics);
        assert_eq!(sels(&s), [".a, b > c"]);
        let d = &s.rules[0].block.decls;
        assert_eq!((&*d[0].name, &*d[0].value, d[0].important), ("color", "red", false));
        assert_eq!((&*d[1].name, &*d[1].value), ("--X", "1px 2px"));
        assert!(d[2].important && d[2].has_var);
        assert_eq!((s.rules[0].line, s.rules[0].column), (1, 1));
    }

    #[test]
    fn nesting_resolves_parents() {
        let s = Stylesheet::parse(".card { gap: 1px; .title { color: red } &:hover { color: blue } > b { x: y } @media (min-width: 1px) { gap: 2px } }", &Default::default());
        assert!(s.diagnostics.is_empty(), "{:?}", s.diagnostics);
        assert_eq!(sels(&s), [".card", ":is(.card) .title", ":is(.card):hover", ":is(.card) > b", ".card"]);
        assert_eq!(s.rules[4].media.len(), 1);
    }

    #[test]
    fn layers_in_order_of_appearance() {
        let s = Stylesheet::parse("@layer base, ui; @layer ui { @layer buttons { a { x: 1 } } } @layer { b { x: 2 } } c { x: 3 }", &Default::default());
        assert!(s.diagnostics.is_empty(), "{:?}", s.diagnostics);
        let names: Vec<&str> = s.layers.iter().map(|l| &**l).collect();
        assert_eq!(&names[..3], ["base", "ui", "ui.buttons"]);
        assert!(names[3].starts_with('\u{0}'));
        assert_eq!(s.rules[0].layer.as_deref(), Some("ui.buttons"));
        assert!(s.rules[2].layer.is_none());
    }

    #[test]
    fn recovers_with_diagnostics() {
        let s = Stylesheet::parse("a:nope { x: 1 } b { x: 1; y: } @keyframes k { from { x: 1 } } c::before { x: 1 } d { x: 2 }", &Default::default());
        assert_eq!(sels(&s), ["b", "d"]);
        let msgs: Vec<String> = s.diagnostics.iter().map(|d| d.to_string()).collect();
        assert!(msgs[0].contains(":nope"), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("`y` has no value")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("@keyframes")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("::before")), "{msgs:?}");
    }

    #[test]
    fn provider_states_resolve() {
        let names = [("cad.pending", crate::StateMask(1 << 32))];
        let opts = ParseOptions { states: Some(&names) };
        let s = Stylesheet::parse("a:state(cad.pending) { x: 1 } b:state(cad.nope) { x: 1 }", &opts);
        assert_eq!(sels(&s), ["a:state(cad.pending)"]);
        assert!(s.diagnostics[0].message.contains("cad.nope"));
    }

    #[test]
    fn inline_blocks() {
        let (b, d) = DeclBlock::parse_inline("color: red; gap: 2px !important; nested { x: 1 }");
        assert_eq!(b.decls.len(), 2);
        assert!(b.get("gap").unwrap().important);
        assert_eq!(d.len(), 1);
    }
}
