//! The selector dialect: [`Impl`] for the `selectors` crate, our pseudo-classes and the parser.
//!
//! Supported: type, class, id and attribute selectors, all four combinators, `:is/:where/:not/:has`,
//! `:nth-child(… of S)` and the other structural pseudo-classes (from `selectors`), the state
//! pseudo-classes of [`STATE_PSEUDO_CLASSES`](crate::state::STATE_PSEUDO_CLASSES), `:focus-visible`,
//! provider states as `:state(ns.name)`, `&` inside nested rules, and the pseudo-elements
//! `::part(name)`, `::placeholder` and `::selection` ([`PseudoElement`]).

use crate::atom::{Atom, AttrValue};
use crate::state::{STATE_PSEUDO_CLASSES, StateMask, StateNames};
use cssparser::{CowRcStr, ParseError, Parser as CssParser, ToCss, Token};
use selectors::SelectorImpl;
use selectors::parser::{ParseRelative, SelectorParseErrorKind};
use std::fmt;

pub type SelectorList = selectors::SelectorList<Impl>;
pub type Selector = selectors::parser::Selector<Impl>;

/// The `SelectorImpl` of svelters' sheets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Impl;

impl SelectorImpl for Impl {
    type ExtraMatchingData<'a> = ();
    type AttrValue = AttrValue;
    type Identifier = Atom;
    type LocalName = Atom;
    type NamespaceUrl = Atom;
    type NamespacePrefix = Atom;
    type BorrowedNamespaceUrl = str;
    type BorrowedLocalName = str;
    type NonTSPseudoClass = PseudoClass;
    type PseudoElement = PseudoElement;
}

/// A non-tree-structural pseudo-class.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PseudoClass {
    /// A built-in state: matches when `bit` is set (or clear, when `negated`).
    State { name: &'static str, bit: StateMask, negated: bool },
    /// `:focus-visible`.
    FocusVisible,
    /// `:state(name)`: a provider state, resolved to its bit at parse time.
    Provider { name: Atom, bit: StateMask },
}

impl PseudoClass {
    /// The state bits the pseudo-class reads.
    pub fn bits(&self) -> StateMask {
        match self {
            PseudoClass::State { bit, .. } | PseudoClass::Provider { bit, .. } => *bit,
            PseudoClass::FocusVisible => StateMask(StateMask::FOCUSED.0 | StateMask::FOCUS_VISIBLE.0),
        }
    }
}

impl ToCss for PseudoClass {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        match self {
            PseudoClass::State { name, .. } => write!(dest, ":{name}"),
            PseudoClass::FocusVisible => dest.write_str(":focus-visible"),
            PseudoClass::Provider { name, .. } => write!(dest, ":state({name})"),
        }
    }
}

impl selectors::parser::NonTSPseudoClass for PseudoClass {
    fn is_active_or_hover(&self) -> bool {
        matches!(self, PseudoClass::State { name: "hover" | "active", .. })
    }

    fn is_user_action_state(&self) -> bool {
        matches!(self, PseudoClass::State { name: "hover" | "active" | "focus" | "focus-within", .. } | PseudoClass::FocusVisible)
    }
}

/// A pseudo-element (ARCH-WIDGETS W3): a styled piece of an element that is not a light-tree node.
///
/// Its style is computed on the host element with [`Stylist::compute_pseudo`](crate::Stylist::compute_pseudo),
/// inheriting from the host's style. State pseudo-classes go on the host (`button:hover::part(label)`);
/// none may follow the pseudo-element, as in CSS for `::part()` of a paint part.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PseudoElement {
    /// `::part(name …)`: a named part of a widget, either a shadow node or paint-only. A part
    /// carries one or more names (WIDGET-PARITY P10: `day today`); a selector's names must all be
    /// among the part's. Kept sorted and deduplicated.
    Part(Box<[Atom]>),
    /// `::placeholder`: a text field's placeholder text.
    Placeholder,
    /// `::selection`: selected text (`color`, `background-color`).
    Selection,
}

impl PseudoElement {
    /// `::part(name)`.
    pub fn part(name: &str) -> PseudoElement {
        PseudoElement::Part(Box::new([Atom::new(name)]))
    }

    /// A part with several names (`::part(day today weekend)` matches rules for any subset).
    pub fn parts(names: &[&str]) -> PseudoElement {
        let mut v: Vec<Atom> = names.iter().map(|n| Atom::new(n)).collect();
        v.sort();
        v.dedup();
        PseudoElement::Part(v.into())
    }

    /// Whether a selector's pseudo-element `sel` matches this one (the element's): the same kind,
    /// and for parts, every name of `sel` among this part's names.
    pub fn matched_by(&self, sel: &PseudoElement) -> bool {
        match (self, sel) {
            (PseudoElement::Part(mine), PseudoElement::Part(want)) => want.iter().all(|w| mine.contains(w)),
            (a, b) => a == b,
        }
    }
}

impl ToCss for PseudoElement {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        match self {
            PseudoElement::Part(n) => {
                dest.write_str("::part(")?;
                for (i, a) in n.iter().enumerate() {
                    if i > 0 {
                        dest.write_str(" ")?;
                    }
                    write!(dest, "{a}")?;
                }
                dest.write_str(")")
            }
            PseudoElement::Placeholder => dest.write_str("::placeholder"),
            PseudoElement::Selection => dest.write_str("::selection"),
        }
    }
}

impl selectors::parser::PseudoElement for PseudoElement {}

/// Why a selector or sheet failed to parse.
#[derive(Clone, Debug, PartialEq)]
pub enum StyleParseError {
    Selector(SelectorParseErrorKind),
    /// `:state(name)` with a name the [`StateNames`] resolver doesn't know.
    UnknownState(String),
    /// A pseudo-class or pseudo-element this dialect doesn't have.
    UnknownPseudo(String),
    Other(String),
}

impl From<SelectorParseErrorKind> for StyleParseError {
    fn from(k: SelectorParseErrorKind) -> Self {
        StyleParseError::Selector(k)
    }
}

impl fmt::Display for StyleParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StyleParseError::Selector(k) => write!(f, "invalid selector ({k:?})"),
            StyleParseError::UnknownState(n) => write!(f, "unknown provider state `{n}` in :state()"),
            StyleParseError::UnknownPseudo(n) => write!(f, "unsupported pseudo-class or pseudo-element `{n}`"),
            StyleParseError::Other(m) => f.write_str(m),
        }
    }
}

/// The `selectors::Parser` of the dialect. `states` resolves `:state()` names.
pub struct SelectorParser<'s> {
    pub states: Option<&'s dyn StateNames>,
}

impl<'i> selectors::Parser<'i> for SelectorParser<'_> {
    type Impl = Impl;
    type Error = StyleParseError;

    fn parse_nth_child_of(&self) -> bool {
        true
    }
    fn parse_is_and_where(&self) -> bool {
        true
    }
    fn parse_has(&self) -> bool {
        true
    }
    fn parse_parent_selector(&self) -> bool {
        true
    }

    fn parse_non_ts_pseudo_class(&self, name: CowRcStr<'i>) -> Result<PseudoClass, ParseError<StyleParseError>> {
        let lower = name.to_ascii_lowercase();
        if lower == "focus-visible" {
            return Ok(PseudoClass::FocusVisible);
        }
        if let Some((n, bit, negated)) = STATE_PSEUDO_CLASSES.iter().find(|(n, _, _)| *n == lower) {
            return Ok(PseudoClass::State { name: n, bit: *bit, negated: *negated });
        }
        Err(ParseError::custom(StyleParseError::UnknownPseudo(format!(":{name}"))))
    }

    fn parse_non_ts_functional_pseudo_class(
        &self,
        name: CowRcStr<'i>,
        input: &mut CssParser<'i>,
        _after_part: bool,
    ) -> Result<PseudoClass, ParseError<StyleParseError>> {
        if !name.eq_ignore_ascii_case("state") {
            return Err(ParseError::custom(StyleParseError::UnknownPseudo(format!(":{name}()"))));
        }
        // `ns.name` is three tokens (ident, `.`, ident); `name` alone is one.
        let start = input.position();
        input.expect_ident()?;
        while input.try_parse(|i| i.expect_delim('.')).is_ok() {
            input.expect_ident()?;
        }
        let text = input.slice_from(start).trim().to_string();
        match self.states.and_then(|s| s.lookup(&text)) {
            Some(bit) => Ok(PseudoClass::Provider { name: Atom::new(&text), bit }),
            None => Err(ParseError::custom(StyleParseError::UnknownState(text))),
        }
    }

    fn parse_pseudo_element(&self, name: CowRcStr<'i>) -> Result<PseudoElement, ParseError<StyleParseError>> {
        match &*name.to_ascii_lowercase() {
            "placeholder" => Ok(PseudoElement::Placeholder),
            "selection" => Ok(PseudoElement::Selection),
            _ => Err(ParseError::custom(StyleParseError::UnknownPseudo(format!("::{name}")))),
        }
    }

    fn parse_functional_pseudo_element(&self, name: CowRcStr<'i>, input: &mut CssParser<'i>) -> Result<PseudoElement, ParseError<StyleParseError>> {
        if !name.eq_ignore_ascii_case("part") {
            return Err(ParseError::custom(StyleParseError::UnknownPseudo(format!("::{name}()"))));
        }
        // one or more names: the part must have all of them (P10)
        let mut names = vec![input.expect_ident()?.to_string()];
        while let Ok(n) = input.try_parse(|i| i.expect_ident_cloned()) {
            names.push(n.to_string());
        }
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        Ok(PseudoElement::parts(&names))
    }
}

/// Parses a selector list (`a, b > c:hover`) on its own, e.g. for a query.
pub fn parse_selector_list(text: &str, states: Option<&dyn StateNames>) -> Result<SelectorList, String> {
    let mut input = CssParser::new(text);
    let parser = SelectorParser { states };
    let list = SelectorList::parse(&parser, &mut input, ParseRelative::No).map_err(|e| describe(&e))?;
    input.expect_exhausted().map_err(|e| format!("unexpected {:?} after the selector", e.kind))?;
    Ok(list)
}

/// A readable message for a parse error.
pub(crate) fn describe(e: &ParseError<StyleParseError>) -> String {
    match &e.kind {
        cssparser::ParseErrorKind::Custom(c) => c.to_string(),
        cssparser::ParseErrorKind::Basic(b) => match b {
            cssparser::BasicParseErrorKind::UnexpectedToken => "unexpected token".into(),
            cssparser::BasicParseErrorKind::EndOfInput => "unexpected end of input".into(),
            other => format!("{other:?}"),
        },
    }
}

pub(crate) fn token_name(t: &Token<'_>) -> String {
    match t {
        Token::Ident(s) => format!("`{s}`"),
        Token::Delim(c) => format!("`{c}`"),
        Token::Function(f) => format!("`{f}(`"),
        other => other.to_css_string(),
    }
}
