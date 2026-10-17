//! Native style subset parser (svelters extension, not Svelte's CSS parser). Owner: agent C.
//!
//! A component's `<style>` holds two kinds of declarations (the backend's style table, ST1):
//! `css` ones are the X6 translator's (GTK CSS / QSS, run by the Vite plugin on Svelte's scoped
//! CSS), setter and child-attr ones are the compiler's: it matches their rules at compile time and
//! lowers them to `$n.set` / `$n.child_attr`. This parser reads the whole sheet but only reports
//! problems that touch setter declarations (`is_setter`): a CSS construct it does not model is
//! not its business. Grammar and diagnostic codes: `docs/notes/native-compiler/styling-syntax.md`.
//!
//! All spans are absolute byte offsets into the component source (`content_start` + local offset).

use crate::diag::Diagnostics;
use serde::Serialize;

/// `(start, end)` byte offsets into the component source.
pub type Span = (u32, u32);

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct StyleSheet {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Rule {
    /// From the first selector to the closing `}`.
    pub span: Span,
    /// Only the selectors that parsed; a rule whose selectors all failed is dropped.
    pub selectors: Vec<ComplexSelector>,
    pub declarations: Vec<Declaration>,
}

/// The combinator that precedes a compound selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Combinator {
    /// The first compound of a complex selector.
    Start,
    /// Whitespace: any ancestor.
    Descendant,
    /// `>`: the direct parent.
    Child,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ComplexSelector {
    pub span: Span,
    /// Non-empty; the first entry's combinator is [`Combinator::Start`].
    pub parts: Vec<(Combinator, Compound)>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Compound {
    pub span: Span,
    pub element: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub states: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Declaration {
    /// From the property name to the end of the value (excluding `;`).
    pub span: Span,
    pub name: String,
    pub name_span: Span,
    /// Empty for custom properties (`--x`), whose value is only kept as `raw_value`.
    pub value: Vec<ValueToken>,
    pub value_span: Span,
    /// The source text of `value_span`.
    pub raw_value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ValueToken {
    Ident(String),
    /// `8dp`, `0`, `1.5`, `50%` (unit `%`).
    Number { value: f64, unit: Option<String> },
    /// Unescaped contents of a quoted string.
    String(String),
    /// Including the `#`, e.g. `#ff0000`.
    Color(String),
    /// `name(args)`; `raw` is the trimmed text between the parentheses.
    Function { name: String, raw: String },
}

impl Compound {
    /// `(ids, classes + states, element types)`; compare lexicographically.
    pub fn specificity(&self) -> (u32, u32, u32) {
        (self.id.is_some() as u32, (self.classes.len() + self.states.len()) as u32, self.element.is_some() as u32)
    }
}

impl ComplexSelector {
    /// CSS specificity: `(id count, class/pseudo-class count, type count)`.
    pub fn specificity(&self) -> (u32, u32, u32) {
        self.parts.iter().fold((0, 0, 0), |(a, b, c), (_, x)| {
            let (xa, xb, xc) = x.specificity();
            (a + xa, b + xb, c + xc)
        })
    }

    /// The compound the rule applies to (the last one).
    pub fn subject(&self) -> &Compound {
        &self.parts.last().expect("complex selector has at least one compound").1
    }
}

pub const SYNTAX: &str = "style_syntax";
pub const UNSUPPORTED_SELECTOR: &str = "style_unsupported_selector";
pub const UNSUPPORTED_COMBINATOR: &str = "style_unsupported_combinator";
pub const UNSUPPORTED_AT_RULE: &str = "style_unsupported_at_rule";
pub const UNSUPPORTED_NESTING: &str = "style_unsupported_nesting";
pub const UNSUPPORTED_IMPORTANT: &str = "style_unsupported_important";
pub const UNSUPPORTED_VALUE: &str = "style_unsupported_value";

/// Parses the text of a `<style>` element; `content_start` is its absolute byte offset.
/// `is_setter(name)`: whether a declaration is the compiler's (a setter or child-attr property);
/// only problems touching those are reported.
pub fn parse_style(source: &str, content_start: u32, diags: &mut Diagnostics, is_setter: &dyn Fn(&str) -> bool) -> StyleSheet {
    let mut p = Parser { src: source, b: source.as_bytes(), base: content_start, diags, is_setter };
    let len = source.len();
    let mut rules = Vec::new();
    let mut i = 0;
    loop {
        i = p.skip_trivia(i, len);
        if i >= len {
            break;
        }
        match p.b[i] {
            b'@' => i = p.at_rule(i),
            b'}' => {
                p.diags.error(SYNTAX, p.sp(i, i + 1), "unexpected `}` without a matching `{`");
                i += 1;
            }
            _ => {
                let (next, rule) = p.rule(i);
                rules.extend(rule);
                i = next;
            }
        }
    }
    StyleSheet { rules }
}

/// Splits inline declarations (`a: b; c: d(e;f)`) into `(name, value)`, honouring parentheses
/// and quotes (as @svelters/style's `parseDeclarations`). Names are lower-cased.
pub fn split_declarations(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let (mut depth, mut quote, mut cur) = (0u32, None::<char>, String::new());
    let mut flush = |cur: &mut String| {
        if let Some(i) = cur.find(':') {
            let (name, value) = (cur[..i].trim(), cur[i + 1..].trim());
            if !name.is_empty() && !value.is_empty() {
                out.push((if name.starts_with("--") { name.to_string() } else { name.to_ascii_lowercase() }, value.to_string()));
            }
        }
        cur.clear();
    };
    for ch in text.chars() {
        match (quote, ch) {
            (Some(q), c) => {
                if c == q {
                    quote = None;
                }
                cur.push(c);
            }
            (None, '"' | '\'') => {
                quote = Some(ch);
                cur.push(ch);
            }
            (None, '(') => {
                depth += 1;
                cur.push(ch);
            }
            (None, ')') => {
                depth = depth.saturating_sub(1);
                cur.push(ch);
            }
            (None, ';') if depth == 0 => flush(&mut cur),
            (None, c) => cur.push(c),
        }
    }
    flush(&mut cur);
    out
}

/// The value tokens of one declaration value (a `style=` / `style:` setter value), or the first
/// problem.
pub fn parse_value(text: &str) -> Result<Vec<ValueToken>, String> {
    let mut diags = Diagnostics::default();
    let all = |_: &str| true;
    let mut p = Parser { src: text, b: text.as_bytes(), base: 0, diags: &mut diags, is_setter: &all };
    let tokens = p.value(0, text.len());
    match (tokens, diags.list.first()) {
        (Some(t), None) if !t.is_empty() => Ok(t.into_iter().map(|x| x.0).collect()),
        (_, Some(d)) => Err(d.message.clone()),
        _ => Err("missing value".into()),
    }
}

struct Parser<'a> {
    src: &'a str,
    b: &'a [u8],
    base: u32,
    diags: &'a mut Diagnostics,
    is_setter: &'a dyn Fn(&str) -> bool,
}

fn is_name_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c >= 0x80
}

fn is_name(c: u8) -> bool {
    is_name_start(c) || c.is_ascii_digit() || c == b'-'
}

impl Parser<'_> {
    fn sp(&self, s: usize, e: usize) -> Span {
        (self.base + s as u32, self.base + e as u32)
    }

    /// End of the character at `i` (for one-character spans).
    fn ch_end(&self, i: usize) -> usize {
        i + self.src.get(i..).and_then(|s| s.chars().next()).map_or(0, char::len_utf8)
    }

    fn ch(&self, i: usize) -> &str {
        &self.src[i..self.ch_end(i)]
    }

    /// Skips whitespace and comments.
    fn skip_trivia(&mut self, mut i: usize, end: usize) -> usize {
        while i < end {
            if self.b[i].is_ascii_whitespace() {
                i += 1;
            } else if self.b[i] == b'/' && i + 1 < end && self.b[i + 1] == b'*' {
                match self.src[i + 2..end].find("*/") {
                    Some(k) => i += k + 4,
                    None => {
                        self.diags.error(SYNTAX, self.sp(i, i + 2), "unterminated comment: expected `*/`");
                        return end;
                    }
                }
            } else {
                break;
            }
        }
        i
    }

    fn trim_end(&self, s: usize, mut e: usize) -> usize {
        while e > s && self.b[e - 1].is_ascii_whitespace() {
            e -= 1;
        }
        e
    }

    /// End of a CSS identifier starting at `i`, or `i` if there is none.
    fn ident_end(&self, i: usize, end: usize) -> usize {
        let mut j = i;
        if j < end && self.b[j] == b'-' {
            j += 1;
        }
        if j < end && (self.b[j] == b'-' || is_name_start(self.b[j])) {
            j += 1;
            while j < end && is_name(self.b[j]) {
                j += 1;
            }
            j
        } else {
            i
        }
    }

    /// If a string or comment starts at `i`, its end (clamped to `end`).
    fn skip_string_or_comment(&self, i: usize, end: usize) -> Option<usize> {
        match self.b[i] {
            q @ (b'"' | b'\'') => {
                let mut j = i + 1;
                while j < end {
                    match self.b[j] {
                        b'\\' => j += 2,
                        b'\n' => return Some(j),
                        c if c == q => return Some(j + 1),
                        _ => j += 1,
                    }
                }
                Some(end)
            }
            b'/' if i + 1 < end && self.b[i + 1] == b'*' => {
                Some(self.src[i + 2..end].find("*/").map_or(end, |k| i + k + 4))
            }
            _ => None,
        }
    }

    /// Finds the first byte of `stops` outside strings, comments and `()`/`[]` (`{`, `}` and `;`
    /// stop even inside brackets, so an unbalanced `(` cannot swallow the rest of the sheet).
    fn scan(&self, mut i: usize, end: usize, stops: &[u8]) -> (usize, Option<u8>) {
        let mut depth = 0u32;
        while i < end {
            if let Some(k) = self.skip_string_or_comment(i, end) {
                i = k;
                continue;
            }
            let c = self.b[i];
            if stops.contains(&c) && (depth == 0 || matches!(c, b'{' | b'}' | b';')) {
                return (i, Some(c));
            }
            match c {
                b'(' | b'[' => depth += 1,
                b')' | b']' => depth = depth.saturating_sub(1),
                _ => {}
            }
            i += 1;
        }
        (end, None)
    }

    /// `open` is at `{`; returns the position after the matching `}` (or the end of input).
    fn skip_block(&self, open: usize) -> usize {
        let mut depth = 0u32;
        let mut i = open;
        while i < self.b.len() {
            if let Some(k) = self.skip_string_or_comment(i, self.b.len()) {
                i = k;
                continue;
            }
            match self.b[i] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return i + 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        self.b.len()
    }

    /// `@name ...;` or `@name ... { ... }`: skipped (the X6 translator's), reported only when its
    /// block holds setter declarations.
    fn at_rule(&mut self, i: usize) -> usize {
        let name_end = self.ident_end(i + 1, self.b.len());
        let (stop_pos, stop) = self.scan(name_end, self.b.len(), b"{;}");
        let end = match stop {
            Some(b'{') => self.skip_block(stop_pos),
            Some(b';') => stop_pos + 1,
            _ => stop_pos,
        };
        if stop == Some(b'{')
            && let Some(prop) = self.setter_inside(stop_pos, end)
        {
            if name_end == i + 1 {
                self.diags.error(SYNTAX, self.sp(i, i + 1), "expected an at-rule name after `@`");
            } else {
                let name = &self.src[i..name_end];
                self.diags
                    .error(UNSUPPORTED_AT_RULE, self.sp(i, name_end), format!("setter property `{prop}` cannot be inside `{name}`: setter properties are written with setProp, not CSS"))
                    .help("put setter properties in a plain top-level rule; @media and @native are CSS-only");
            }
        }
        end
    }

    /// The first setter property declared anywhere in `[open, end)` (a block), if any.
    fn setter_inside(&self, open: usize, end: usize) -> Option<String> {
        let text = &self.src[open..end];
        for seg in text.split(['{', '}', ';']) {
            let t = seg.trim_start();
            if let Some(c) = t.find(':') {
                let name = t[..c].trim();
                if !name.is_empty() && name.bytes().all(|b| is_name(b)) && (self.is_setter)(name) {
                    return Some(name.to_string());
                }
            }
        }
        None
    }

    /// `selector-list { declarations }` starting at `i`.
    fn rule(&mut self, i: usize) -> (usize, Option<Rule>) {
        let (stop_pos, stop) = self.scan(i, self.b.len(), b"{;}");
        if stop != Some(b'{') {
            let e = self.trim_end(i, stop_pos);
            self.diags.error(SYNTAX, self.sp(i, e), "expected `{` after the selector");
            return (stop_pos + stop.is_some() as usize, None);
        }
        let mark = self.diags.list.len();
        let selectors = self.selector_list(i, stop_pos);
        let (end, declarations, setters) = self.block(stop_pos);
        if !setters {
            // a CSS-only rule: the X6 translator's to judge
            self.diags.list.truncate(mark);
        }
        let rule = (!selectors.is_empty()).then(|| Rule { span: self.sp(i, end), selectors, declarations });
        (end, rule)
    }

    fn selector_list(&mut self, s: usize, e: usize) -> Vec<ComplexSelector> {
        let mut out = Vec::new();
        let mut i = s;
        loop {
            let (comma, stop) = self.scan(i, e, b",");
            let start = self.skip_trivia(i, comma);
            if start >= comma {
                // Point at the comma next to the empty selector, or at the `{`.
                let at = if stop.is_some() { comma } else if i > s { i - 1 } else { e };
                self.diags.error(SYNTAX, self.sp(at, at + 1), "expected a selector");
            } else if let Some(sel) = self.complex(start, comma) {
                out.push(sel);
            }
            if stop.is_none() {
                return out;
            }
            i = comma + 1;
        }
    }

    fn complex(&mut self, s: usize, e: usize) -> Option<ComplexSelector> {
        let mut parts: Vec<(Combinator, Compound)> = Vec::new();
        let mut comb = Combinator::Start;
        let mut i = s;
        loop {
            let (next, compound) = self.compound(i, e)?;
            parts.push((comb, compound));
            let ws_end = self.skip_trivia(next, e);
            if ws_end >= e {
                break;
            }
            match self.b[ws_end] {
                b'>' => {
                    comb = Combinator::Child;
                    i = self.skip_trivia(ws_end + 1, e);
                    if i >= e {
                        self.diags.error(SYNTAX, self.sp(ws_end, ws_end + 1), "expected a selector after `>`");
                        return None;
                    }
                }
                c @ (b'+' | b'~') => {
                    let what = if c == b'+' { "next-sibling" } else { "subsequent-sibling" };
                    self.diags
                        .error(UNSUPPORTED_COMBINATOR, self.sp(ws_end, ws_end + 1), format!("the {what} combinator `{}` is not supported", c as char))
                        .help("only descendant (whitespace) and child (`>`) combinators are supported");
                    return None;
                }
                _ if ws_end > next => {
                    comb = Combinator::Descendant;
                    i = ws_end;
                }
                _ => {
                    self.unexpected(next, "selector");
                    return None;
                }
            }
        }
        let end = parts.last().map_or(s as u32, |(_, c)| c.span.1 - self.base) as usize;
        Some(ComplexSelector { span: self.sp(s, end), parts })
    }

    fn unexpected(&mut self, i: usize, what: &str) {
        let msg = format!("unexpected `{}` in {what}", self.ch(i));
        self.diags.error(SYNTAX, self.sp(i, self.ch_end(i)), msg);
    }

    fn unsupported_selector(&mut self, s: usize, e: usize, msg: String, help: &str) -> Option<(usize, Compound)> {
        self.diags.error(UNSUPPORTED_SELECTOR, self.sp(s, e), msg).help(help);
        None
    }

    /// `element? (.class | :state)*`
    fn compound(&mut self, s: usize, e: usize) -> Option<(usize, Compound)> {
        let mut i = s;
        let mut c = Compound { span: (0, 0), element: None, id: None, classes: Vec::new(), states: Vec::new() };
        if i < e && self.b[i].is_ascii_alphabetic() {
            while i < e && (self.b[i].is_ascii_alphanumeric() || self.b[i] == b'-') {
                i += 1;
            }
            c.element = Some(self.src[s..i].to_string());
        }
        const USE_CLASS: &str = "setter rules match element names, `.class` and `#id` only";
        while i < e {
            match self.b[i] {
                b'.' => {
                    let j = self.ident_end(i + 1, e);
                    if j == i + 1 {
                        self.diags.error(SYNTAX, self.sp(i, i + 1), "expected a class name after `.`");
                        return None;
                    }
                    c.classes.push(self.src[i + 1..j].to_string());
                    i = j;
                }
                b':' if i + 1 < e && self.b[i + 1] == b':' => {
                    let j = self.ident_end(i + 2, e);
                    let msg = format!("pseudo-elements (`{}`) are not supported", &self.src[i..j]);
                    return self.unsupported_selector(i, j, msg, "native widgets have no pseudo-elements; style the element itself");
                }
                b':' => {
                    let j = self.ident_end(i + 1, e);
                    if j == i + 1 {
                        self.diags.error(SYNTAX, self.sp(i, i + 1), "expected a state name after `:`");
                        return None;
                    }
                    if j < e && self.b[j] == b'(' {
                        let (close, stop) = self.scan(j + 1, e, b")");
                        let end = if stop.is_some() { close + 1 } else { e };
                        let name = &self.src[i + 1..j];
                        let (msg, help) = if name == "global" {
                            (
                                "`:global(...)` is not supported: native styles only apply to this component's elements".to_string(),
                                "move the rule into the component that renders the element",
                            )
                        } else {
                            (format!("functional pseudo-class `:{name}(...)` is not supported"), "use a plain state such as `:hover`, or a class")
                        };
                        return self.unsupported_selector(i, end, msg, help);
                    }
                    c.states.push(self.src[i + 1..j].to_string());
                    i = j;
                }
                b'#' => {
                    let j = self.ident_end(i + 1, e);
                    if j == i + 1 {
                        self.diags.error(SYNTAX, self.sp(i, i + 1), "expected an id after `#`");
                        return None;
                    }
                    c.id = Some(self.src[i + 1..j].to_string());
                    i = j;
                }
                b'*' => return self.unsupported_selector(i, i + 1, "the universal selector `*` is not supported".into(), USE_CLASS),
                b'[' => {
                    let (close, stop) = self.scan(i + 1, e, b"]");
                    let end = if stop.is_some() { close + 1 } else { e };
                    return self.unsupported_selector(i, end, "attribute selectors are not supported".into(), USE_CLASS);
                }
                b'&' => {
                    return self.unsupported_selector(i, i + 1, "the nesting selector `&` is not supported".into(), "write the full selector in a top-level rule")
                }
                _ => break,
            }
        }
        if i == s {
            if i < e {
                self.unexpected(i, "selector");
            } else {
                self.diags.error(SYNTAX, self.sp(s, e), "expected a selector");
            }
            return None;
        }
        c.span = self.sp(s, i);
        Some((i, c))
    }

    /// `open` is at `{`. Also returns whether the block declares a setter property (parsed or
    /// not): only then do the rule's selector problems matter.
    fn block(&mut self, open: usize) -> (usize, Vec<Declaration>, bool) {
        let len = self.b.len();
        let mut decls = Vec::new();
        let mut setters = false;
        let mut i = open + 1;
        loop {
            i = self.skip_trivia(i, len);
            if i >= len {
                self.diags.error(SYNTAX, self.sp(open, open + 1), "unclosed block: expected `}`");
                return (len, decls, true);
            }
            match self.b[i] {
                b'}' => return (i + 1, decls, setters),
                b';' => {
                    i += 1;
                    continue;
                }
                _ => {}
            }
            let (stop_pos, stop) = self.scan(i, len, b"{;}");
            if stop == Some(b'{') {
                let before = self.diags.list.len();
                if self.b[i] == b'@' {
                    self.at_rule(i);
                    // a reported at-rule keeps the rule's diagnostics
                    setters |= self.diags.list.len() > before;
                } else if let Some(prop) = self.setter_inside(stop_pos, self.skip_block(stop_pos)) {
                    setters = true;
                    let e = self.trim_end(i, stop_pos);
                    self.diags
                        .error(UNSUPPORTED_NESTING, self.sp(i, e), format!("setter property `{prop}` cannot be in a nested rule"))
                        .help("write the full selector as a separate top-level rule");
                }
                i = self.skip_block(stop_pos);
            } else {
                // problems of a CSS declaration are the X6 translator's
                let name_end = self.ident_end(i, stop_pos);
                let setter = (self.is_setter)(&self.src[i..name_end]);
                setters |= setter;
                let mark = self.diags.list.len();
                decls.extend(self.declaration(i, stop_pos));
                if !setter {
                    self.diags.list.truncate(mark);
                }
                i = stop_pos + (stop == Some(b';')) as usize;
            }
        }
    }

    /// `name: value` in `[s, e)` (without the `;`).
    fn declaration(&mut self, s: usize, e: usize) -> Option<Declaration> {
        if self.b[s] == b'@' {
            self.at_rule(s);
            return None;
        }
        let ne = self.ident_end(s, e);
        if ne == s {
            self.unexpected(s, "declaration: expected a property name");
            return None;
        }
        let name = self.src[s..ne].to_string();
        let colon = self.skip_trivia(ne, e);
        if colon >= e || self.b[colon] != b':' {
            let span = if colon >= e { (s, ne) } else { (colon, self.ch_end(colon)) };
            self.diags.error(SYNTAX, self.sp(span.0, span.1), format!("expected `:` after the property name `{name}`"));
            return None;
        }
        let (value, vs, ve) = if name.starts_with("--") {
            let mut vs = colon + 1;
            while vs < e && self.b[vs].is_ascii_whitespace() {
                vs += 1;
            }
            (Vec::new(), vs, self.trim_end(vs, e))
        } else {
            let tokens = self.value(colon + 1, e)?;
            let (Some(first), Some(last)) = (tokens.first(), tokens.last()) else {
                self.diags.error(SYNTAX, self.sp(s, colon + 1), format!("missing value for `{name}`"));
                return None;
            };
            let (vs, ve) = (first.1, last.2);
            (tokens.into_iter().map(|t| t.0).collect(), vs, ve)
        };
        Some(Declaration {
            span: self.sp(s, ve.max(colon + 1)),
            name,
            name_span: self.sp(s, ne),
            value,
            value_span: self.sp(vs, ve),
            raw_value: self.src[vs..ve].to_string(),
        })
    }

    /// Value tokens with their local spans; `None` (after a diagnostic) drops the declaration.
    fn value(&mut self, s: usize, e: usize) -> Option<Vec<(ValueToken, usize, usize)>> {
        let mut out = Vec::new();
        let mut i = self.skip_trivia(s, e);
        while i < e {
            let b = self.b[i];
            let next = |k: usize| self.b.get(i + k).copied().filter(|_| i + k < e).unwrap_or(0);
            let numeric = b.is_ascii_digit()
                || (b == b'.' && next(1).is_ascii_digit())
                || (matches!(b, b'+' | b'-') && (next(1).is_ascii_digit() || (next(1) == b'.' && next(2).is_ascii_digit())));
            let (tok, j) = if numeric {
                self.number(i, e)
            } else if b == b'"' || b == b'\'' {
                self.string(i, e)?
            } else if b == b'#' {
                let mut j = i + 1;
                while j < e && self.b[j].is_ascii_alphanumeric() {
                    j += 1;
                }
                let hex = &self.src[i + 1..j];
                if !matches!(hex.len(), 3 | 4 | 6 | 8) || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
                    let msg = format!("invalid color `{}`: expected 3, 4, 6 or 8 hex digits", &self.src[i..j]);
                    self.diags.error(SYNTAX, self.sp(i, j), msg);
                    return None;
                }
                (ValueToken::Color(self.src[i..j].to_string()), j)
            } else if b == b'!' {
                let w = self.skip_trivia(i + 1, e);
                let we = self.ident_end(w, e);
                if self.src[w..we].eq_ignore_ascii_case("important") {
                    self.diags
                        .error(UNSUPPORTED_IMPORTANT, self.sp(i, we), "`!important` is not supported")
                        .help("native styles apply by specificity and source order; use a more specific selector");
                } else {
                    self.unexpected(i, "value");
                }
                return None;
            } else {
                let j = self.ident_end(i, e);
                if j == i {
                    self.diags
                        .error(UNSUPPORTED_VALUE, self.sp(i, self.ch_end(i)), format!("unexpected `{}` in value", self.ch(i)))
                        .help("values are identifiers, numbers with an optional unit, strings, `#hex` colors or `name(...)` functions");
                    return None;
                }
                if j < e && self.b[j] == b'(' {
                    let (close, stop) = self.scan(j + 1, e, b")");
                    if stop.is_none() {
                        self.diags.error(SYNTAX, self.sp(i, j + 1), "unclosed `(`: expected `)`");
                        return None;
                    }
                    let raw = self.src[j + 1..close].trim().to_string();
                    (ValueToken::Function { name: self.src[i..j].to_string(), raw }, close + 1)
                } else {
                    (ValueToken::Ident(self.src[i..j].to_string()), j)
                }
            };
            out.push((tok, i, j));
            i = self.skip_trivia(j, e);
        }
        Some(out)
    }

    fn number(&self, i: usize, e: usize) -> (ValueToken, usize) {
        let digits = |mut j: usize| {
            while j < e && self.b[j].is_ascii_digit() {
                j += 1;
            }
            j
        };
        let mut j = i + matches!(self.b[i], b'+' | b'-') as usize;
        j = digits(j);
        if j + 1 < e && self.b[j] == b'.' && self.b[j + 1].is_ascii_digit() {
            j = digits(j + 1);
        }
        let value = self.src[i..j].parse::<f64>().unwrap_or(0.0);
        let unit_end = if j < e && self.b[j] == b'%' {
            j + 1
        } else {
            let mut k = j;
            while k < e && is_name_start(self.b[k]) {
                k += 1;
            }
            k
        };
        let unit = (unit_end > j).then(|| self.src[j..unit_end].to_string());
        (ValueToken::Number { value, unit }, unit_end)
    }

    fn string(&mut self, i: usize, e: usize) -> Option<(ValueToken, usize)> {
        let q = self.b[i];
        let mut out = String::new();
        let mut chars = self.src[i + 1..e].char_indices();
        while let Some((k, c)) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some((_, '\n')) | None => {}
                    Some((_, c)) => out.push(c),
                },
                '\n' => break,
                c if c as u32 == q as u32 => return Some((ValueToken::String(out), i + 1 + k + 1)),
                c => out.push(c),
            }
        }
        let end = self.src[i..e].find('\n').map_or(e, |k| i + k);
        self.diags.error(SYNTAX, self.sp(i, end), "unterminated string");
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::Diagnostic;

    /// Every declaration counts as a setter one, so every problem is reported.
    fn parse_at(src: &str, base: u32) -> (StyleSheet, Vec<Diagnostic>) {
        let mut d = Diagnostics::default();
        let sheet = parse_style(src, base, &mut d, &|_| true);
        (sheet, d.list)
    }

    /// Only `spacing` is a setter property: CSS-only rules are not reported.
    fn parse_mixed(src: &str) -> (StyleSheet, Vec<Diagnostic>) {
        let mut d = Diagnostics::default();
        let sheet = parse_style(src, 0, &mut d, &|n| n == "spacing");
        (sheet, d.list)
    }

    fn parse(src: &str) -> (StyleSheet, Vec<Diagnostic>) {
        parse_at(src, 0)
    }

    /// Span of the first occurrence of `needle`.
    fn at(src: &str, needle: &str) -> Span {
        let i = src.find(needle).unwrap_or_else(|| panic!("{needle:?} not in {src:?}")) as u32;
        (i, i + needle.len() as u32)
    }

    /// Asserts exactly one diagnostic with `code` covering `needle`.
    fn one_error(src: &str, code: &str, needle: &str) -> StyleSheet {
        let (sheet, diags) = parse(src);
        assert_eq!(diags.len(), 1, "{src:?}: {diags:#?}");
        assert_eq!(diags[0].code, code, "{src:?}: {}", diags[0].message);
        assert_eq!((diags[0].start, diags[0].end), at(src, needle), "{src:?}: {}", diags[0].message);
        sheet
    }

    fn num(value: f64, unit: Option<&str>) -> ValueToken {
        ValueToken::Number { value, unit: unit.map(str::to_string) }
    }

    fn sel(src: &str) -> ComplexSelector {
        let (sheet, diags) = parse(&format!("{src} {{}}"));
        assert!(diags.is_empty(), "{diags:#?}");
        sheet.rules[0].selectors[0].clone()
    }

    #[test]
    fn basic_rule() {
        let src = "box { orientation: vertical; spacing: 8dp }";
        let (sheet, diags) = parse(src);
        assert!(diags.is_empty());
        assert_eq!(sheet.rules.len(), 1);
        let r = &sheet.rules[0];
        assert_eq!(r.span, (0, src.len() as u32));
        assert_eq!(r.selectors[0].subject().element.as_deref(), Some("box"));
        assert_eq!(r.declarations.len(), 2);
        let d = &r.declarations[0];
        assert_eq!(d.name, "orientation");
        assert_eq!(d.name_span, at(src, "orientation"));
        assert_eq!(d.value, vec![ValueToken::Ident("vertical".into())]);
        assert_eq!(d.value_span, at(src, "vertical"));
        assert_eq!(d.span, at(src, "orientation: vertical"));
        assert_eq!(r.declarations[1].value, vec![num(8.0, Some("dp"))]);
        assert_eq!(r.declarations[1].raw_value, "8dp");
    }

    #[test]
    fn selectors_and_combinators() {
        let src = ".a > button.primary:hover:focused, window  label {}";
        let (sheet, diags) = parse(src);
        assert!(diags.is_empty(), "{diags:#?}");
        let sels = &sheet.rules[0].selectors;
        assert_eq!(sels.len(), 2);
        assert_eq!(sels[0].span, at(src, ".a > button.primary:hover:focused"));
        let (c0, a) = &sels[0].parts[0];
        assert_eq!((*c0, a.element.clone(), a.classes.clone()), (Combinator::Start, None, vec!["a".to_string()]));
        let (c1, b) = &sels[0].parts[1];
        assert_eq!(*c1, Combinator::Child);
        assert_eq!(b.element.as_deref(), Some("button"));
        assert_eq!(b.classes, vec!["primary"]);
        assert_eq!(b.states, vec!["hover", "focused"]);
        assert_eq!(b.span, at(src, "button.primary:hover:focused"));
        assert_eq!(sels[1].span, at(src, "window  label"));
        assert_eq!(sels[1].parts[1].0, Combinator::Descendant);
        assert_eq!(sels[1].subject().element.as_deref(), Some("label"));
        // Child combinator without spaces, and selector lists without spaces.
        let s = sel("a>b");
        assert_eq!(s.parts[1].0, Combinator::Child);
        let (sheet, _) = parse("a,b{}");
        assert_eq!(sheet.rules[0].selectors.len(), 2);
    }

    #[test]
    fn value_tokens() {
        let src = r#"x { a: 0 1.5 50% -2dp +3 .5em; b: "he said \"hi\"" 'x'; c: #fff #11223344; d: rgb( 1, 2, 3 ) solid; }"#;
        let (sheet, diags) = parse(src);
        assert!(diags.is_empty(), "{diags:#?}");
        let d = &sheet.rules[0].declarations;
        assert_eq!(
            d[0].value,
            vec![num(0.0, None), num(1.5, None), num(50.0, Some("%")), num(-2.0, Some("dp")), num(3.0, None), num(0.5, Some("em"))]
        );
        assert_eq!(d[0].raw_value, "0 1.5 50% -2dp +3 .5em");
        assert_eq!(d[1].value, vec![ValueToken::String("he said \"hi\"".into()), ValueToken::String("x".into())]);
        assert_eq!(d[2].value, vec![ValueToken::Color("#fff".into()), ValueToken::Color("#11223344".into())]);
        assert_eq!(
            d[3].value,
            vec![ValueToken::Function { name: "rgb".into(), raw: "1, 2, 3".into() }, ValueToken::Ident("solid".into())]
        );
        // Hyphenated identifiers are not numbers.
        let (sheet, _) = parse("x { a: -moz-thing }");
        assert_eq!(sheet.rules[0].declarations[0].value, vec![ValueToken::Ident("-moz-thing".into())]);
    }

    #[test]
    fn custom_properties_keep_raw_text() {
        let src = "x { --gap:  calc(1 + 2) , weird!stuff ; --empty:; y: 1 }";
        let (sheet, diags) = parse(src);
        assert!(diags.is_empty(), "{diags:#?}");
        let d = &sheet.rules[0].declarations;
        assert_eq!(d[0].name, "--gap");
        assert!(d[0].value.is_empty());
        assert_eq!(d[0].raw_value, "calc(1 + 2) , weird!stuff");
        assert_eq!(d[0].value_span, at(src, "calc(1 + 2) , weird!stuff"));
        assert_eq!(d[1].name, "--empty");
        assert_eq!(d[1].raw_value, "");
        assert_eq!(d[2].name, "y");
    }

    #[test]
    fn comments_everywhere() {
        let src = "/* a */ .a /* b */ > /* c */ b /* d */ { /* e */ x /* f */ : /* g */ 1dp /* h */ ; /* i */ } /* j */";
        let (sheet, diags) = parse(src);
        assert!(diags.is_empty(), "{diags:#?}");
        let r = &sheet.rules[0];
        assert_eq!(r.selectors[0].span, at(src, ".a /* b */ > /* c */ b"));
        assert_eq!(r.selectors[0].parts[1].0, Combinator::Child);
        let d = &r.declarations[0];
        assert_eq!(d.value, vec![num(1.0, Some("dp"))]);
        assert_eq!(d.raw_value, "1dp");
        // A `{` inside a comment or string does not open a block.
        let (sheet, diags) = parse("a /* { */ { b: \"}\" }");
        assert!(diags.is_empty(), "{diags:#?}");
        assert_eq!(sheet.rules[0].declarations[0].value, vec![ValueToken::String("}".into())]);
    }

    #[test]
    fn empty_and_trailing_semicolons() {
        let (sheet, diags) = parse("a {} b { ; x: 1;; y: 2; }");
        assert!(diags.is_empty(), "{diags:#?}");
        assert_eq!(sheet.rules.len(), 2);
        assert_eq!(sheet.rules[1].declarations.len(), 2);
        let (sheet, diags) = parse("");
        assert!(diags.is_empty() && sheet.rules.is_empty());
    }

    #[test]
    fn absolute_offsets() {
        let src = "\n  .a { spacing: 4dp; }\n  * { x: 1 }";
        let base = 100;
        let (sheet, diags) = parse_at(src, base);
        let shift = |(s, e): Span| (s + base, e + base);
        let r = &sheet.rules[0];
        assert_eq!(r.span, shift(at(src, ".a { spacing: 4dp; }")));
        assert_eq!(r.selectors[0].span, shift(at(src, ".a")));
        assert_eq!(r.selectors[0].parts[0].1.span, shift(at(src, ".a")));
        let d = &r.declarations[0];
        assert_eq!(d.name_span, shift(at(src, "spacing")));
        assert_eq!(d.value_span, shift(at(src, "4dp")));
        assert_eq!(d.span, shift(at(src, "spacing: 4dp")));
        assert_eq!((diags[0].start, diags[0].end), shift(at(src, "*")));
    }

    #[test]
    fn unsupported_selectors() {
        let cases = [
            ("* { x: 1 }", UNSUPPORTED_SELECTOR, "*"),
            ("a [type=\"x]\"] { x: 1 }", UNSUPPORTED_SELECTOR, "[type=\"x]\"]"),
            ("a::before { x: 1 }", UNSUPPORTED_SELECTOR, "::before"),
            ("a:not(.b, .c) { x: 1 }", UNSUPPORTED_SELECTOR, ":not(.b, .c)"),
            (":global(.x) { x: 1 }", UNSUPPORTED_SELECTOR, ":global(.x)"),
            ("& .x { x: 1 }", UNSUPPORTED_SELECTOR, "&"),
            ("a + b { x: 1 }", UNSUPPORTED_COMBINATOR, "+"),
            ("a~b { x: 1 }", UNSUPPORTED_COMBINATOR, "~"),
            ("a > { x: 1 }", SYNTAX, ">"),
            ("> a { x: 1 }", SYNTAX, ">"),
            ("a. { x: 1 }", SYNTAX, "."),
            ("a# { x: 1 }", SYNTAX, "#"),
            ("a: { x: 1 }", SYNTAX, ":"),
            ("a\"s\" { x: 1 }", SYNTAX, "\""),
        ];
        for (src, code, needle) in cases {
            let sheet = one_error(src, code, needle);
            assert!(sheet.rules.is_empty(), "{src:?}");
        }
        let sheet = one_error(", a { x: 1 }", SYNTAX, ",");
        assert_eq!(sheet.rules[0].selectors.len(), 1);
        let sheet = one_error("a, { x: 1 }", SYNTAX, ",");
        assert_eq!(sheet.rules[0].selectors.len(), 1);
        let (_, d) = parse(":global(.x) { x: 1 }");
        assert!(d[0].message.contains(":global"));
    }

    #[test]
    fn id_selectors() {
        let s = sel("box#main.a");
        assert_eq!(s.subject().id.as_deref(), Some("main"));
        assert_eq!(s.subject().element.as_deref(), Some("box"));
        assert_eq!(s.specificity(), (1, 1, 1));
    }

    #[test]
    fn css_only_rules_are_not_reported() {
        // selectors, values and at-rules the subset does not model are the X6 translator's
        let src = "a + b { color: red } .x::before { color: #ggg } @media (prefers-color-scheme: dark) { .y { color: red } } @keyframes k { from { opacity: 0 } } .ok { spacing: 4px; color: 1 / 2 }";
        let (sheet, diags) = parse_mixed(src);
        assert!(diags.is_empty(), "{diags:#?}");
        let ok = sheet.rules.iter().find(|r| r.selectors[0].subject().classes == ["ok"]).unwrap();
        assert_eq!(ok.declarations.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), ["spacing"]);
        // setter declarations in a rule make its selector problems count, and @media / nesting
        // holding one is an error
        let (_, diags) = parse_mixed("a + b { spacing: 4px; color: red } @media (x) { .y { spacing: 1 } } .z { .w { spacing: 2 } } .v { spacing: !important }");
        let codes: Vec<_> = diags.iter().map(|d| d.code).collect();
        assert_eq!(codes, [UNSUPPORTED_COMBINATOR, UNSUPPORTED_AT_RULE, UNSUPPORTED_NESTING, UNSUPPORTED_IMPORTANT], "{diags:#?}");
    }

    #[test]
    fn bad_selector_in_list_keeps_the_rest() {
        let src = "*, .ok { x: 1 }";
        let sheet = one_error(src, UNSUPPORTED_SELECTOR, "*");
        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(sheet.rules[0].selectors.len(), 1);
        assert_eq!(sheet.rules[0].selectors[0].subject().classes, vec!["ok"]);
        assert_eq!(sheet.rules[0].declarations.len(), 1);
    }

    #[test]
    fn recovery_continues_with_next_rule() {
        let src = "a::b { x: 1 } * { y: 2 } .ok { z: 3 }";
        let (sheet, diags) = parse(src);
        assert_eq!(diags.iter().map(|d| d.code).collect::<Vec<_>>(), [UNSUPPORTED_SELECTOR, UNSUPPORTED_SELECTOR]);
        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(sheet.rules[0].declarations[0].name, "z");
    }

    #[test]
    fn at_rules() {
        let src = "@media (min-width: 10px) { a { b: 1 } } @import 'x.css'; .ok { c: 2 }";
        let (sheet, diags) = parse(src);
        assert_eq!(diags.len(), 1, "{diags:#?}");
        assert_eq!(diags[0].code, UNSUPPORTED_AT_RULE);
        assert_eq!((diags[0].start, diags[0].end), at(src, "@media"));
        assert!(diags[0].message.contains("cannot be inside `@media`"));
        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(sheet.rules[0].declarations[0].name, "c");
        // Inside a block, with or without a block of its own.
        let src = "a { @apply foo; @media x { b: 1 } c: 2 }";
        let (sheet, diags) = parse(src);
        assert_eq!(diags.iter().map(|d| d.code).collect::<Vec<_>>(), [UNSUPPORTED_AT_RULE]);
        assert_eq!((diags[0].start, diags[0].end), at(src, "@media"));
        assert_eq!(sheet.rules[0].declarations.len(), 1);
    }

    #[test]
    fn nesting_is_rejected() {
        let src = "a { x: 1; .b { y: 2 } &:hover { z: 3 } w: 4 }";
        let (sheet, diags) = parse(src);
        assert_eq!(diags.len(), 2, "{diags:#?}");
        assert_eq!(diags[0].code, UNSUPPORTED_NESTING);
        assert_eq!((diags[0].start, diags[0].end), at(src, ".b"));
        assert_eq!((diags[1].start, diags[1].end), at(src, "&:hover"));
        let names: Vec<_> = sheet.rules[0].declarations.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["x", "w"]);
    }

    #[test]
    fn important_is_rejected() {
        let src = "a { x: 1dp !important; y: 2 }";
        let sheet = one_error(src, UNSUPPORTED_IMPORTANT, "!important");
        let names: Vec<_> = sheet.rules[0].declarations.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["y"]);
        one_error("a { x: 1 ! IMPORTANT }", UNSUPPORTED_IMPORTANT, "! IMPORTANT");
    }

    #[test]
    fn declaration_errors() {
        let cases = [
            ("a { x 1; y: 2 }", SYNTAX, "1"),
            ("a { x; y: 2 }", SYNTAX, "x"),
            ("a { x: ; y: 2 }", SYNTAX, "x:"),
            ("a { 1: 2; y: 2 }", SYNTAX, "1"),
            ("a { x: #ggg; y: 2 }", SYNTAX, "#ggg"),
            ("a { x: #12345; y: 2 }", SYNTAX, "#12345"),
            ("a { x: a, b; y: 2 }", UNSUPPORTED_VALUE, ","),
            ("a { x: 1 / 2; y: 2 }", UNSUPPORTED_VALUE, "/"),
            ("a { x: \"abc\n; y: 2 }", SYNTAX, "\"abc"),
            ("a { x: f(1; y: 2 }", SYNTAX, "f("),
        ];
        for (src, code, needle) in cases {
            let sheet = one_error(src, code, needle);
            let names: Vec<_> = sheet.rules[0].declarations.iter().map(|d| d.name.as_str()).collect();
            assert_eq!(names, ["y"], "{src:?}");
        }
    }

    #[test]
    fn structural_errors() {
        // Unclosed block keeps what was parsed.
        let sheet = one_error("a { x: 1", SYNTAX, "{");
        assert_eq!(sheet.rules[0].declarations.len(), 1);
        // Unterminated comment.
        let sheet = one_error("a { x: 1 } /* never closed", SYNTAX, "/*");
        assert_eq!(sheet.rules.len(), 1);
        // Stray `}` and missing `{`.
        let sheet = one_error("} a { x: 1 }", SYNTAX, "}");
        assert_eq!(sheet.rules.len(), 1);
        let sheet = one_error("a b; c { x: 1 }", SYNTAX, "a b");
        assert_eq!(sheet.rules.len(), 1);
        one_error("{ x: 1 }", SYNTAX, "{");
    }

    #[test]
    fn specificity_and_subject() {
        assert_eq!(sel("button").specificity(), (0, 0, 1));
        assert_eq!(sel(".a").specificity(), (0, 1, 0));
        assert_eq!(sel("button:hover").specificity(), (0, 1, 1));
        assert_eq!(sel("box > button.primary:hover").specificity(), (0, 2, 2));
        let mut ordered = [sel(".a .b"), sel("a"), sel("a b"), sel(".a"), sel("a.b"), sel("#x")];
        ordered.sort_by_key(|s| s.specificity());
        let spans: Vec<_> = ordered.iter().map(|s| s.specificity()).collect();
        assert_eq!(spans, [(0, 0, 1), (0, 0, 2), (0, 1, 0), (0, 1, 1), (0, 2, 0), (1, 0, 0)]);
        let s = sel("window .bar > label.big");
        assert_eq!(s.subject().element.as_deref(), Some("label"));
        assert_eq!(s.subject().classes, vec!["big"]);
        assert_eq!(s.parts[0].1.specificity(), (0, 0, 1));
    }

    #[test]
    fn serializes() {
        let (sheet, _) = parse(".a:hover { spacing: 8dp; color: #fff }");
        let json = serde_json::to_value(&sheet).unwrap();
        assert_eq!(json["rules"][0]["selectors"][0]["parts"][0][0], "start");
        assert_eq!(json["rules"][0]["declarations"][0]["value"][0]["Number"]["unit"], "dp");
    }

    #[test]
    fn unicode_spans() {
        let src = "a { x: \"é\" ü }";
        let (sheet, diags) = parse(src);
        assert!(diags.is_empty(), "{diags:#?}");
        let d = &sheet.rules[0].declarations[0];
        assert_eq!(d.value, vec![ValueToken::String("é".into()), ValueToken::Ident("ü".into())]);
        assert_eq!(d.value_span, at(src, "\"é\" ü"));
        one_error("a { x: 1 = }", UNSUPPORTED_VALUE, "=");
    }
}
