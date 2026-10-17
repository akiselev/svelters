//! Port of acorn 8.19.0 `src/parseutil.js`.

use super::identifier::is_identifier_char;
use super::state::{PResult, Parser, TokVal};
use super::tokentype::TokenType as T;
use super::whitespace::{has_line_break, skip_white_space};
use crate::estree::Expression;

// ## Parser utilities

/// `literal = /^(?:'((?:\\[^]|[^'\\])*?)'|"((?:\\[^]|[^"\\])*?)")/`: returns (content, match length).
fn match_literal(s: &str) -> Option<(&str, usize)> {
    let b = s.as_bytes();
    let q = *b.first()?;
    if q != b'\'' && q != b'"' {
        return None;
    }
    let mut i = 1;
    while i < b.len() {
        let c = b[i];
        if c == q {
            return Some((&s[1..i], i + 1));
        }
        if c == b'\\' {
            i += 1;
            if i >= b.len() {
                return None;
            }
            // `\\[^]` consumes one UTF-16 unit; skip the whole char.
            let ch = s[i..].chars().next().unwrap();
            i += ch.len_utf8();
            continue;
        }
        i += 1;
    }
    None
}

impl<'a> Parser<'a> {
    pub fn strict_directive(&self, start: u32) -> bool {
        if self.options.ecma_version < 5 {
            return false;
        }
        let input = self.input;
        let mut start = start as usize;
        loop {
            // Try to find string literal.
            start = skip_white_space(input, start);
            let Some((content, len)) = match_literal(&input[start..]) else { return false };
            if content == "use strict" {
                let space_start = start + len;
                let end = skip_white_space(input, space_start);
                let next = input[end..].chars().next();
                let next1 = input[end..].chars().nth(1);
                return next == Some(';')
                    || next == Some('}')
                    || (has_line_break(&input[space_start..end])
                        && !(matches!(
                            next,
                            Some(
                                '(' | '`' | '.' | '[' | '+' | '-' | '/' | '*' | '%' | '<' | '>' | '=' | ',' | '?'
                                    | '^' | '&'
                            )
                        ) || next == Some('!') && next1 == Some('=')
                            || next == Some('i') && keyword_op_at(self, end)));
            }
            start += len;

            // Skip semicolon, if any.
            start = skip_white_space(input, start);
            if input.as_bytes().get(start) == Some(&b';') {
                start += 1;
            }
        }
    }

    // Predicate that tests whether the next token is of the given
    // type, and if yes, consumes it as a side effect.

    pub fn eat(&mut self, type_: T) -> PResult<bool> {
        if self.type_ == type_ {
            self.next()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // Tests whether parsed token is a contextual keyword.

    pub fn is_contextual(&self, name: &str) -> bool {
        self.type_ == T::Name && matches!(&self.value, TokVal::Str(s) if s == name) && !self.contains_esc
    }

    // Consumes contextual keyword if possible.

    pub fn eat_contextual(&mut self, name: &str) -> PResult<bool> {
        if !self.is_contextual(name) {
            return Ok(false);
        }
        self.next()?;
        Ok(true)
    }

    // Asserts that following token is given contextual keyword.

    pub fn expect_contextual(&mut self, name: &str) -> PResult<()> {
        if !self.eat_contextual(name)? {
            return self.unexpected(None);
        }
        Ok(())
    }

    // Test whether a semicolon can be inserted at the current position.

    pub fn can_insert_semicolon(&self) -> bool {
        self.type_ == T::Eof
            || self.type_ == T::BraceR
            || has_line_break(&self.input[self.last_tok_end as usize..self.start as usize])
    }

    pub fn insert_semicolon(&self) -> bool {
        self.can_insert_semicolon()
    }

    // Consume a semicolon, or, failing that, see if we are allowed to
    // pretend that there is a semicolon at this position.

    pub fn semicolon(&mut self) -> PResult<()> {
        if !self.eat(T::Semi)? && !self.insert_semicolon() {
            return self.unexpected(None);
        }
        Ok(())
    }

    pub fn after_trailing_comma(&mut self, tok_type: T, not_next: bool) -> PResult<bool> {
        if self.type_ == tok_type {
            if !not_next {
                self.next()?;
            }
            return Ok(true);
        }
        Ok(false)
    }

    // Expect a token of a given type. If found, consume it, otherwise,
    // raise an unexpected token error.

    pub fn expect(&mut self, type_: T) -> PResult<()> {
        if !self.eat(type_)? {
            return self.unexpected(None);
        }
        Ok(())
    }

    // Raise an unexpected token error.

    pub fn unexpected<R>(&self, pos: Option<u32>) -> PResult<R> {
        self.raise(pos.unwrap_or(self.start), "Unexpected token")
    }

    pub fn check_pattern_errors(&self, r: Option<&DestructuringErrors>, is_assign: bool) -> PResult<()> {
        let Some(r) = r else { return Ok(()) };
        if r.trailing_comma > -1 {
            return self.raise_recoverable(r.trailing_comma as u32, "Comma is not permitted after the rest element");
        }
        let parens = if is_assign { r.parenthesized_assign } else { r.parenthesized_bind };
        if parens > -1 {
            return self
                .raise_recoverable(parens as u32, if is_assign { "Assigning to rvalue" } else { "Parenthesized pattern" });
        }
        Ok(())
    }

    pub fn check_expression_errors(&self, r: Option<&DestructuringErrors>, and_throw: bool) -> PResult<bool> {
        let Some(r) = r else { return Ok(false) };
        let (shorthand_assign, double_proto) = (r.shorthand_assign, r.double_proto);
        if !and_throw {
            return Ok(shorthand_assign >= 0 || double_proto >= 0);
        }
        if shorthand_assign >= 0 {
            return self.raise(
                shorthand_assign as u32,
                "Shorthand property assignments are valid only in destructuring patterns",
            );
        }
        if double_proto >= 0 {
            return self.raise_recoverable(double_proto as u32, "Redefinition of __proto__ property");
        }
        Ok(false)
    }

    pub fn check_yield_await_in_default_params(&self) -> PResult<()> {
        if self.yield_pos != 0 && (self.await_pos == 0 || self.yield_pos < self.await_pos) {
            return self.raise(self.yield_pos, "Yield expression cannot be a default value");
        }
        if self.await_pos != 0 {
            return self.raise(self.await_pos, "Await expression cannot be a default value");
        }
        Ok(())
    }

    pub fn is_simple_assign_target(&self, expr: &Expression) -> bool {
        match expr {
            Expression::ParenthesizedExpression(p) => self.is_simple_assign_target(&p.expression),
            Expression::Identifier(_) | Expression::MemberExpression(_) => true,
            _ => false,
        }
    }
}

// Scan for `in` or `instanceof` at the given position, knowing that
// there's an `i` there.
fn keyword_op_at(parser: &Parser, pos: usize) -> bool {
    let input = parser.input;
    // `stop = Math.min(input.length, pos + 11)` in UTF-16 units; identifiers longer than
    // 10 units cannot match anyway.
    let mut end = pos + 1;
    let mut units = 1;
    while units < 11 && end < input.len() {
        let ch = input[end..].chars().next().unwrap();
        if !is_identifier_char(ch as u32, true) {
            break;
        }
        end += ch.len_utf8();
        units += ch.len_utf16();
    }
    let word = &input[pos..end];
    word == "in" || word == "instanceof"
}

#[derive(Clone, Copy, Debug)]
pub struct DestructuringErrors {
    pub shorthand_assign: i64,
    pub trailing_comma: i64,
    pub parenthesized_assign: i64,
    pub parenthesized_bind: i64,
    pub double_proto: i64,
}

impl DestructuringErrors {
    pub fn new() -> DestructuringErrors {
        DestructuringErrors {
            shorthand_assign: -1,
            trailing_comma: -1,
            parenthesized_assign: -1,
            parenthesized_bind: -1,
            double_proto: -1,
        }
    }
}

impl Default for DestructuringErrors {
    fn default() -> Self {
        Self::new()
    }
}
