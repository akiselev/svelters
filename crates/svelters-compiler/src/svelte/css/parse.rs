//! Port of `phases/1-parse/read/style.js` (Svelte 5.57.2): Svelte's own CSS parser, character by
//! character, so the spans (and the raw `prelude` / `value` strings) are exactly upstream's.
//!
//! SVELTERS: positions are byte offsets into the component source; a JS string index is a UTF-16
//! unit, so the loops step by `char`. `cssparser` is not used here: Svelte's grammar is not CSS
//! Syntax Level 3 (it reads raw values, keeps `<!-- -->`, rejects identifiers CSS accepts) and
//! byte-identical output needs its exact spans. CSS comments are skipped, not collected.

use super::ast::*;
use super::errors as e;
use crate::svelte::errors::CompileError;
use crate::svelte::parse::utils::whitespace::is_whitespace;

pub type PResult<T> = Result<T, CompileError>;

fn is_ws(c: char) -> bool {
    is_whitespace(c as u32)
}

/// JS `String.prototype.trim` (the same white space set as `\s`).
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(is_ws)
}

pub struct CssParser<'s> {
    pub template: &'s str,
    pub index: usize,
    next_rule: usize,
    next_complex: usize,
    next_relative: usize,
}

/// Parses the `<style>` content starting at `content_start` (the byte after `<style ...>`), up to
/// `</style` or the end of the source; returns the stylesheet and the content end.
pub fn parse_stylesheet(template: &str, start: usize, content_start: usize) -> PResult<StyleSheet> {
    let mut p = CssParser { template, index: content_start, next_rule: 0, next_complex: 0, next_relative: 0 };
    let children = p.read_body(|p| p.match_str("</style") || p.index >= p.template.len())?;
    let content_end = p.index;
    p.eat("</style", true)?;
    // `parser.read(/\s*>/y)`
    let save = p.index;
    p.allow_whitespace();
    if p.match_str(">") {
        p.index += 1;
    } else {
        p.index = save;
    }
    Ok(StyleSheet { start, end: p.index, children, content_start, content_end, counts: (p.next_rule, p.next_complex, p.next_relative) })
}

impl<'s> CssParser<'s> {
    fn rest(&self) -> &'s str {
        &self.template[self.index..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn char_at(&self, i: usize) -> Option<char> {
        self.template.get(i..).and_then(|s| s.chars().next())
    }

    fn match_str(&self, s: &str) -> bool {
        self.rest().starts_with(s)
    }

    fn eat(&mut self, s: &str, required: bool) -> PResult<bool> {
        if self.match_str(s) {
            self.index += s.len();
            return Ok(true);
        }
        if required {
            return Err(e::expected_token(self.index, s));
        }
        Ok(false)
    }

    fn allow_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if !is_ws(c) {
                break;
            }
            self.index += c.len_utf8();
        }
    }

    /// `read_until(delimiter)`
    fn read_until(&mut self, delimiter: &str) -> PResult<&'s str> {
        if self.index >= self.template.len() {
            return Err(e::unexpected_eof(self.template.len()));
        }
        let start = self.index;
        match self.template[start..].find(delimiter) {
            Some(i) => self.index = start + i,
            None => self.index = self.template.len(),
        }
        Ok(&self.template[start..self.index])
    }

    fn read_body(&mut self, finished: impl Fn(&CssParser) -> bool) -> PResult<Vec<Node>> {
        let mut children = Vec::new();
        loop {
            self.allow_comment_or_whitespace()?;
            if finished(self) {
                break;
            }
            if self.match_str("@") {
                children.push(Node::Atrule(self.read_at_rule()?));
            } else {
                children.push(Node::Rule(self.read_rule()?));
            }
        }
        Ok(children)
    }

    fn read_at_rule(&mut self) -> PResult<Atrule> {
        let start = self.index;
        self.eat("@", true)?;
        let name = self.read_identifier()?;
        let prelude = self.read_value()?;
        let block = if self.match_str("{") {
            Some(self.read_block()?)
        } else {
            self.eat(";", true)?;
            None
        };
        Ok(Atrule { start, end: self.index, name, prelude, block })
    }

    fn read_rule(&mut self) -> PResult<Rule> {
        let start = self.index;
        let id = self.next_rule;
        self.next_rule += 1;
        let prelude = self.read_selector_list(false)?;
        let block = self.read_block()?;
        Ok(Rule { id, start, end: self.index, prelude, block })
    }

    fn read_selector_list(&mut self, inside_pseudo_class: bool) -> PResult<SelectorList> {
        let mut children = Vec::new();
        self.allow_comment_or_whitespace()?;
        let start = self.index;
        while self.index < self.template.len() {
            children.push(self.read_selector(inside_pseudo_class)?);
            let end = self.index;
            self.allow_comment_or_whitespace()?;
            if if inside_pseudo_class { self.match_str(")") } else { self.match_str("{") } {
                return Ok(SelectorList { start, end, children });
            }
            self.eat(",", true)?;
            self.allow_comment_or_whitespace()?;
        }
        Err(e::unexpected_eof(self.template.len()))
    }

    fn new_relative(&mut self, combinator: Option<Combinator>, start: usize) -> RelativeSelector {
        let id = self.next_relative;
        self.next_relative += 1;
        RelativeSelector { id, start, end: 0, combinator, selectors: Vec::new() }
    }

    fn read_selector(&mut self, inside_pseudo_class: bool) -> PResult<ComplexSelector> {
        let list_start = self.index;
        let complex_id = self.next_complex;
        self.next_complex += 1;
        let mut children = Vec::new();
        let mut relative = self.new_relative(None, self.index);

        while self.index < self.template.len() {
            let start = self.index;

            if self.eat("&", false)? {
                relative.selectors.push(SimpleSelector::new(SimpleKind::NestingSelector, start, self.index, "&".into()));
            } else if self.eat("*", false)? {
                let mut name = "*".to_string();
                let mut namespace = None;
                if self.eat("|", false)? {
                    namespace = Some(name);
                    name = if self.eat("*", false)? { "*".into() } else { self.read_identifier()? };
                }
                let mut s = SimpleSelector::new(SimpleKind::TypeSelector, start, self.index, name);
                s.namespace = namespace;
                relative.selectors.push(s);
            } else if self.eat("#", false)? {
                let name = self.read_identifier()?;
                relative.selectors.push(SimpleSelector::new(SimpleKind::IdSelector, start, self.index, name));
            } else if self.eat(".", false)? {
                let name = self.read_identifier()?;
                relative.selectors.push(SimpleSelector::new(SimpleKind::ClassSelector, start, self.index, name));
            } else if self.eat("::", false)? {
                let name = self.read_identifier()?;
                let mut args = None;
                if self.eat("(", false)? {
                    args = Some(self.read_selector_list(true)?);
                    self.eat(")", true)?;
                }
                let mut s = SimpleSelector::new(SimpleKind::PseudoElementSelector, start, self.index, name);
                s.args = args;
                relative.selectors.push(s);
            } else if self.eat(":", false)? {
                let name = self.read_identifier()?;
                let mut args = None;
                if self.eat("(", false)? {
                    args = Some(self.read_selector_list(true)?);
                    self.eat(")", true)?;
                }
                let mut s = SimpleSelector::new(SimpleKind::PseudoClassSelector, start, self.index, name);
                s.args = args;
                relative.selectors.push(s);
            } else if self.eat("[", false)? {
                self.allow_whitespace();
                let name = self.read_identifier()?;
                self.allow_whitespace();
                let mut value = None;
                let matcher = self.read_matcher();
                if matcher.is_some() {
                    self.allow_whitespace();
                    value = Some(self.read_attribute_value()?);
                }
                self.allow_whitespace();
                let flags = self.read_flags();
                self.allow_whitespace();
                self.eat("]", true)?;
                let mut s = SimpleSelector::new(SimpleKind::AttributeSelector, start, self.index, name);
                s.matcher = matcher;
                s.value = value;
                s.flags = flags;
                relative.selectors.push(s);
            } else if let Some(len) = inside_pseudo_class.then(|| match_nth_of(self.rest())).flatten() {
                // nth of matcher must come before combinator matcher to prevent collision else the '+' in '+2n-1' would be parsed as a combinator
                let value = self.rest()[..len].to_string();
                self.index += len;
                relative.selectors.push(SimpleSelector::new(SimpleKind::Nth, start, self.index, value));
            } else if let Some(len) = match_percentage(self.rest()) {
                let value = self.rest()[..len].to_string();
                self.index += len;
                relative.selectors.push(SimpleSelector::new(SimpleKind::Percentage, start, self.index, value));
            } else if match_combinator(self.rest()).is_none() {
                let mut name = self.read_identifier()?;
                let mut namespace = None;
                if self.eat("|", false)? {
                    namespace = Some(name);
                    name = if self.eat("*", false)? { "*".into() } else { self.read_identifier()? };
                }
                let mut s = SimpleSelector::new(SimpleKind::TypeSelector, start, self.index, name);
                s.namespace = namespace;
                relative.selectors.push(s);
            }

            let index = self.index;
            self.allow_comment_or_whitespace_no_capture()?;

            if self.match_str(",") || if inside_pseudo_class { self.match_str(")") } else { self.match_str("{") } {
                // rewind, so we know whether to continue building the selector list
                self.index = index;
                relative.end = index;
                children.push(relative);
                return Ok(ComplexSelector { id: complex_id, start: list_start, end: index, children });
            }

            self.index = index;
            if let Some(combinator) = self.read_combinator() {
                if !relative.selectors.is_empty() {
                    relative.end = index;
                    children.push(relative);
                }
                // ...and start a new one
                let cstart = combinator.start;
                relative = self.new_relative(Some(combinator), cstart);
                self.allow_whitespace();
                if self.match_str(",") || if inside_pseudo_class { self.match_str(")") } else { self.match_str("{") } {
                    return Err(e::css_selector_invalid(self.index));
                }
            }
        }
        Err(e::unexpected_eof(self.template.len()))
    }

    fn read_combinator(&mut self) -> Option<Combinator> {
        let start = self.index;
        self.allow_whitespace();
        let index = self.index;
        if let Some(len) = match_combinator(self.rest()) {
            let name = self.rest()[..len].to_string();
            self.index += len;
            let end = self.index;
            self.allow_whitespace();
            return Some(Combinator { start: index, end, name });
        }
        if self.index != start {
            return Some(Combinator { start, end: self.index, name: " ".into() });
        }
        None
    }

    fn read_block(&mut self) -> PResult<Block> {
        let start = self.index;
        self.eat("{", true)?;
        let mut children = Vec::new();
        while self.index < self.template.len() {
            self.allow_comment_or_whitespace()?;
            if self.match_str("}") {
                break;
            }
            children.push(self.read_block_item()?);
        }
        self.eat("}", true)?;
        Ok(Block { start, end: self.index, children })
    }

    fn read_block_item(&mut self) -> PResult<Node> {
        if self.match_str("@") {
            return Ok(Node::Atrule(self.read_at_rule()?));
        }
        // read ahead to understand whether we're dealing with a declaration or a nested rule.
        let start = self.index;
        self.read_value_impl()?;
        let c = self.peek();
        self.index = start;
        if c == Some('{') {
            Ok(Node::Rule(self.read_rule()?))
        } else {
            Ok(Node::Declaration(self.read_declaration()?))
        }
    }

    fn read_declaration(&mut self) -> PResult<Declaration> {
        let start = self.index;
        // `read_until_regex(/[\s:]/)`
        if self.index >= self.template.len() {
            return Err(e::unexpected_eof(self.template.len()));
        }
        let pstart = self.index;
        let plen = self.rest().find(|c: char| is_ws(c) || c == ':').unwrap_or(self.rest().len());
        self.index += plen;
        let property = self.template[pstart..self.index].to_string();
        self.allow_whitespace();
        self.eat(":", false)?;
        let index = self.index;
        self.allow_whitespace();
        let value = self.read_value()?;
        if value.is_empty() && !property.starts_with("--") {
            return Err(e::css_empty_declaration((start, index)));
        }
        let end = self.index;
        if !self.match_str("}") {
            self.eat(";", true)?;
        }
        Ok(Declaration { start, end, property, value })
    }

    fn read_value(&mut self) -> PResult<String> {
        self.read_value_impl()
    }

    fn read_value_impl(&mut self) -> PResult<String> {
        let mut value = String::new();
        let mut escaped = false;
        let mut in_url = false;
        let mut quote_mark: Option<char> = None;

        while let Some(c) = self.peek() {
            if escaped {
                value.push('\\');
                value.push(c);
                escaped = false;
                self.index += c.len_utf8();
                continue;
            } else if c == '\\' {
                escaped = true;
                self.index += 1;
                continue;
            } else if Some(c) == quote_mark {
                quote_mark = None;
            } else if c == ')' {
                in_url = false;
            } else if quote_mark.is_none() && (c == '"' || c == '\'') {
                quote_mark = Some(c);
            } else if c == '(' && value.ends_with("url") {
                in_url = true;
            } else if (c == ';' || c == '{' || c == '}') && !in_url && quote_mark.is_none() {
                return Ok(js_trim(&value).to_string());
            } else if c == '/' && !in_url && quote_mark.is_none() && self.char_at(self.index + 1) == Some('*') {
                self.read_comment()?;
                continue;
            }
            value.push(c);
            self.index += c.len_utf8();
        }
        Err(e::unexpected_eof(self.template.len()))
    }

    fn read_attribute_value(&mut self) -> PResult<String> {
        let mut value = String::new();
        let mut escaped = false;
        let quote_mark = if self.eat("\"", false)? {
            Some('"')
        } else if self.eat("'", false)? {
            Some('\'')
        } else {
            None
        };
        while let Some(c) = self.peek() {
            if escaped {
                value.push('\\');
                value.push(c);
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if match quote_mark {
                Some(q) => c == q,
                None => is_ws(c) || c == ']',
            } {
                if let Some(q) = quote_mark {
                    let mut buf = [0u8; 4];
                    self.eat(q.encode_utf8(&mut buf), true)?;
                }
                return Ok(js_trim(&value).to_string());
            } else {
                value.push(c);
            }
            self.index += c.len_utf8();
        }
        Err(e::unexpected_eof(self.template.len()))
    }

    /// `REGEX_MATCHER = /[~^$*|]?=/y`
    fn read_matcher(&mut self) -> Option<String> {
        let r = self.rest().as_bytes();
        let len = match r {
            [b'~' | b'^' | b'$' | b'*' | b'|', b'=', ..] => 2,
            [b'=', ..] => 1,
            _ => return None,
        };
        let m = self.rest()[..len].to_string();
        self.index += len;
        Some(m)
    }

    /// `REGEX_ATTRIBUTE_FLAGS = /[a-zA-Z]+/y`
    fn read_flags(&mut self) -> Option<String> {
        let len = self.rest().bytes().take_while(|b| b.is_ascii_alphabetic()).count();
        if len == 0 {
            return None;
        }
        let m = self.rest()[..len].to_string();
        self.index += len;
        Some(m)
    }

    /// <https://www.w3.org/TR/css-syntax-3/#ident-token-diagram>
    fn read_identifier(&mut self) -> PResult<String> {
        let start = self.index;
        let mut identifier = String::new();

        // `REGEX_LEADING_HYPHEN_OR_DIGIT = /-?\d/y`
        let b = self.rest().as_bytes();
        if matches!(b, [b'0'..=b'9', ..] | [b'-', b'0'..=b'9', ..]) {
            return Err(e::css_expected_identifier(start));
        }

        while let Some(c) = self.peek() {
            if c == '\\' {
                if let Some(len) = match_unicode_sequence(self.rest()) {
                    let seq = &self.rest()[1..len];
                    let hex: String = seq.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
                    let cp = u32::from_str_radix(&hex, 16).unwrap_or(0);
                    if cp > 0x10FFFF {
                        // upstream: `String.fromCodePoint` throws a RangeError (not a CompileError)
                        return Err(e::css_expected_identifier(start));
                    }
                    let ch = char::from_u32(cp).unwrap_or('\u{FFFD}');
                    if ch == '\\' {
                        identifier.push_str("\\\\");
                    } else {
                        identifier.push(ch);
                    }
                    self.index += len;
                } else {
                    identifier.push('\\');
                    match self.char_at(self.index + 1) {
                        Some(n) => {
                            identifier.push(n);
                            self.index += 1 + n.len_utf8();
                        }
                        None => {
                            // `'\\' + undefined`
                            identifier.push_str("undefined");
                            self.index += 2;
                        }
                    }
                }
            } else if c as u32 >= 160 || c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                identifier.push(c);
                self.index += c.len_utf8();
            } else {
                break;
            }
        }

        if identifier.is_empty() {
            return Err(e::css_expected_identifier(start));
        }
        Ok(identifier)
    }

    fn allow_comment_or_whitespace(&mut self) -> PResult<()> {
        self.allow_comment_or_whitespace_no_capture()
    }

    fn allow_comment_or_whitespace_no_capture(&mut self) -> PResult<()> {
        self.allow_whitespace();
        while self.match_str("/*") || self.match_str("<!--") {
            if self.match_str("/*") {
                self.read_comment()?;
            }
            if self.eat("<!--", false)? {
                self.read_until("-->")?;
                self.eat("-->", true)?;
            }
            self.allow_whitespace();
        }
        Ok(())
    }

    fn read_comment(&mut self) -> PResult<()> {
        self.eat("/*", true)?;
        self.read_until("*/")?;
        self.eat("*/", true)?;
        Ok(())
    }
}

/// `REGEX_COMBINATOR = /(\+|~|>|\|\|)/y`
fn match_combinator(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    match b {
        [b'+', ..] | [b'~', ..] | [b'>', ..] => Some(1),
        [b'|', b'|', ..] => Some(2),
        _ => None,
    }
}

fn digits(b: &[u8], i: usize) -> usize {
    b[i..].iter().take_while(|c| c.is_ascii_digit()).count()
}

/// `REGEX_PERCENTAGE = /\d+(\.\d+)?%/y`
fn match_percentage(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let n = digits(b, 0);
    if n == 0 {
        return None;
    }
    let mut i = n;
    if b.get(i) == Some(&b'.') {
        let f = digits(b, i + 1);
        if f > 0 {
            i += 1 + f;
        }
    }
    (b.get(i) == Some(&b'%')).then_some(i + 1)
}

/// `REGEX_UNICODE_SEQUENCE = /\\[0-9a-fA-F]{1,6}(\r\n|\s)?/y`
fn match_unicode_sequence(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    if b.first() != Some(&b'\\') {
        return None;
    }
    let n = b[1..].iter().take(6).take_while(|c| c.is_ascii_hexdigit()).count();
    if n == 0 {
        return None;
    }
    let mut i = 1 + n;
    if s[i..].starts_with("\r\n") {
        i += 2;
    } else if let Some(c) = s[i..].chars().next() {
        if is_ws(c) {
            i += c.len_utf8();
        }
    }
    Some(i)
}

fn ws_len(s: &str, i: usize) -> usize {
    s[i..].chars().take_while(|c| is_ws(*c)).map(char::len_utf8).sum()
}

/// `REGEX_NTH_OF`:
/// `/(even|odd|\+?(\d+|\d*n(\s*[+-]\s*\d+)?)|-\d*n(\s*\+\s*\d+))((?=\s*[,)])|\s+of(\s+|(?=[.#[*:&])))/y`
///
/// The quantifiers are such that only the longest run of each can be followed by the next
/// token, so trying the alternatives in order (and the optional group with, then without) is the
/// regex's backtracking.
fn match_nth_of(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let tail = |i: usize| -> Option<usize> {
        // (?=\s*[,)])
        let w = ws_len(s, i);
        if matches!(b.get(i + w), Some(b',' | b')')) {
            return Some(i);
        }
        // \s+of(\s+|(?=[.#[*:&]))
        if w > 0 && s[i + w..].starts_with("of") {
            let j = i + w + 2;
            let w2 = ws_len(s, j);
            if w2 > 0 {
                return Some(j + w2);
            }
            if matches!(b.get(j), Some(b'.' | b'#' | b'[' | b'*' | b':' | b'&')) {
                return Some(j);
            }
        }
        None
    };
    // `\s*[+-]\s*\d+` (or `\s*\+\s*\d+` when `plus_only`) at i
    let signed_tail = |i: usize, plus_only: bool| -> Option<usize> {
        let mut j = i + ws_len(s, i);
        match b.get(j) {
            Some(b'+') => {}
            Some(b'-') if !plus_only => {}
            _ => return None,
        }
        j += 1;
        j += ws_len(s, j);
        let d = digits(b, j);
        (d > 0).then_some(j + d)
    };

    let mut candidates: Vec<usize> = Vec::new();
    if s.starts_with("even") {
        candidates.push(4);
    }
    if s.starts_with("odd") {
        candidates.push(3);
    }
    {
        let p = usize::from(b.first() == Some(&b'+'));
        let d = digits(b, p);
        if d > 0 {
            candidates.push(p + d);
        }
        if b.get(p + d) == Some(&b'n') {
            let n_end = p + d + 1;
            if let Some(e) = signed_tail(n_end, false) {
                candidates.push(e);
            }
            candidates.push(n_end);
        }
    }
    if b.first() == Some(&b'-') {
        let d = digits(b, 1);
        if b.get(1 + d) == Some(&b'n') {
            if let Some(e) = signed_tail(2 + d, true) {
                candidates.push(e);
            }
        }
    }
    candidates.into_iter().find_map(tail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nth_of() {
        assert_eq!(match_nth_of("2n+1)"), Some(4));
        assert_eq!(match_nth_of("2n + 1 of .a)"), Some(10));
        assert_eq!(match_nth_of("odd)"), Some(3));
        assert_eq!(match_nth_of("-n+3)"), Some(4));
        assert_eq!(match_nth_of("-n)"), None);
        assert_eq!(match_nth_of("3 of.a)"), Some(4));
        assert_eq!(match_nth_of("n)"), Some(1));
        assert_eq!(match_nth_of(".a)"), None);
    }
}
