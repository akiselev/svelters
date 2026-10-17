//! Port of acorn 8.19.0 `src/tokenize.js`.
//!
//! SVELTERS: positions are byte offsets. Where acorn reads a UTF-16 unit with `charCodeAt`
//! this port decodes the whole code point at that byte offset (`code_at`) and advances by its
//! UTF-8 length, which is equivalent because UTF-8 input never contains lone surrogates.
//! Past the end of input `code_at` returns `EOF_CODE` (acorn's `NaN`).

use super::identifier::{is_identifier_char, is_identifier_start};
use super::state::{PErr, PResult, Parser, TokVal};
use super::tokentype::{keyword_type, TokenType as T};
use super::util::{bigint_to_decimal, code_point_to_string, JsStr};
use super::whitespace::{is_new_line, is_non_ascii_whitespace};
use crate::estree::{Comment, CommentKind};

/// acorn's `NaN` from `charCodeAt` past the end of input.
pub const EOF_CODE: u32 = u32::MAX;

// ## Tokenizer

impl<'a> Parser<'a> {
    #[inline]
    pub fn len(&self) -> u32 {
        self.input.len() as u32
    }

    /// `input.charCodeAt(pos)` / `fullCharCodeAt(pos)`: the code point at byte offset `pos`.
    #[inline]
    pub fn code_at(&self, pos: u32) -> u32 {
        let b = self.input.as_bytes();
        match b.get(pos as usize) {
            None => EOF_CODE,
            Some(&c) if c < 0x80 => c as u32,
            Some(_) => match self.input.get(pos as usize..).and_then(|s| s.chars().next()) {
                Some(ch) => ch as u32,
                // Not a char boundary: cannot happen for positions the tokenizer produces.
                None => 0xFFFD,
            },
        }
    }

    /// UTF-8 length of the char at `pos` (1 past the end).
    #[inline]
    pub fn char_len_at(&self, pos: u32) -> u32 {
        let b = self.input.as_bytes();
        match b.get(pos as usize) {
            None => 1,
            Some(&c) if c < 0x80 => 1,
            Some(&c) if c >= 0xF0 => 4,
            Some(&c) if c >= 0xE0 => 3,
            Some(_) => 2,
        }
    }

    pub fn full_char_code_at_pos(&self) -> u32 {
        self.code_at(self.pos)
    }

    // Move to the next token

    pub fn next(&mut self) -> PResult<()> {
        self.next_ex(false)
    }

    pub fn next_ex(&mut self, ignore_escape_sequence_in_keyword: bool) -> PResult<()> {
        if !ignore_escape_sequence_in_keyword && self.contains_esc {
            if let Some(kw) = self.type_.keyword() {
                return self.raise_recoverable(self.start, format!("Escape sequence in keyword {}", kw));
            }
        }

        self.last_tok_end = self.end;
        self.last_tok_start = self.start;
        self.next_token()
    }

    // Read a single token, updating the parser object's token-related
    // properties.

    pub fn next_token(&mut self) -> PResult<()> {
        let cur_context = self.cur_context();
        if cur_context.map(|c| !c.preserve_space()).unwrap_or(true) {
            self.skip_space()?;
        }

        self.start = self.pos;
        if self.pos >= self.len() {
            self.finish_token(T::Eof, TokVal::None);
            return Ok(());
        }

        if cur_context.map(|c| c.has_override()).unwrap_or(false) {
            self.try_read_template_token()
        } else {
            let code = self.full_char_code_at_pos();
            self.read_token(code)
        }
    }

    pub fn read_token(&mut self, code: u32) -> PResult<()> {
        // Identifier or keyword. '\uXXXX' sequences are allowed in
        // identifiers, so '\' also dispatches to that.
        if is_identifier_start(code, self.options.ecma_version >= 6) || code == 92
        /* '\' */
        {
            return self.read_word();
        }

        self.get_token_from_code(code)
    }

    pub fn skip_block_comment(&mut self) -> PResult<()> {
        let start = self.pos;
        self.pos += 2;
        let end = match self.input[self.pos as usize..].find("*/") {
            Some(i) => self.pos + i as u32,
            None => return self.raise(self.pos - 2, "Unterminated comment"),
        };
        self.pos = end + 2;
        self.comments.push(Comment {
            kind: CommentKind::Block,
            value: self.input[start as usize + 2..end as usize].to_string(),
            start,
            end: self.pos,
        });
        Ok(())
    }

    pub fn skip_line_comment(&mut self, start_skip: u32) {
        let start = self.pos;
        self.pos += start_skip;
        while self.pos < self.len() {
            let ch = self.code_at(self.pos);
            if is_new_line(ch) {
                break;
            }
            self.pos += self.char_len_at(self.pos);
        }
        let value_start = (start + start_skip).min(self.pos);
        self.comments.push(Comment {
            kind: CommentKind::Line,
            value: self.input[value_start as usize..self.pos as usize].to_string(),
            start,
            end: self.pos,
        });
    }

    // Called at the start of the parse and after every token. Skips
    // whitespace and comments, and.

    pub fn skip_space(&mut self) -> PResult<()> {
        let b = self.input.as_bytes();
        while (self.pos as usize) < b.len() {
            let ch = b[self.pos as usize];
            match ch {
                32 => {
                    // ' '
                    self.pos += 1;
                }
                13 => {
                    if b.get(self.pos as usize + 1) == Some(&10) {
                        self.pos += 1;
                    }
                    self.pos += 1;
                }
                10 => {
                    self.pos += 1;
                }
                47 => {
                    // '/'
                    match b.get(self.pos as usize + 1) {
                        Some(42) => {
                            // '*'
                            self.skip_block_comment()?;
                        }
                        Some(47) => {
                            self.skip_line_comment(2);
                        }
                        _ => break,
                    }
                }
                _ => {
                    if ch > 8 && ch < 14 {
                        self.pos += 1;
                    } else if ch >= 0x80 {
                        let code = self.code_at(self.pos);
                        // 160 (nbsp), 8232, 8233 (line breaks), other non-ASCII whitespace
                        if code == 160 || code == 8232 || code == 8233 || code >= 5760 && is_non_ascii_whitespace(code) {
                            self.pos += self.char_len_at(self.pos);
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    // Called at the end of every token. Sets `end`, `val`, and
    // maintains `context` and `exprAllowed`, and skips the space after
    // the token, so that the next one's `start` will point at the
    // right position.

    pub fn finish_token(&mut self, type_: T, val: TokVal) {
        self.end = self.pos;
        let prev_type = self.type_;
        self.type_ = type_;
        self.value = val;

        self.update_context(prev_type);
    }

    // ### Token reading

    // This is the function that is called to fetch the next token. It
    // is somewhat obscure, because it works in character codes rather
    // than characters, and because operator parsing has been inlined
    // into it.
    //
    // All in the name of speed.
    //
    fn read_token_dot(&mut self) -> PResult<()> {
        let next = self.code_at(self.pos + 1);
        if (48..=57).contains(&next) {
            return self.read_number(true);
        }
        let next2 = self.code_at(self.pos + 2);
        if self.options.ecma_version >= 6 && next == 46 && next2 == 46 {
            // 46 = dot '.'
            self.pos += 3;
            self.finish_token(T::Ellipsis, TokVal::None);
        } else {
            self.pos += 1;
            self.finish_token(T::Dot, TokVal::None);
        }
        Ok(())
    }

    fn read_token_slash(&mut self) -> PResult<()> {
        // '/'
        let next = self.code_at(self.pos + 1);
        if self.expr_allowed {
            self.pos += 1;
            return self.read_regexp();
        }
        if next == 61 {
            return self.finish_op(T::Assign, 2);
        }
        self.finish_op(T::Slash, 1)
    }

    fn read_token_mult_modulo_exp(&mut self, code: u32) -> PResult<()> {
        // '%*'
        let mut next = self.code_at(self.pos + 1);
        let mut size = 1;
        let mut tokentype = if code == 42 { T::Star } else { T::Modulo };

        // exponentiation operator ** and **=
        if self.options.ecma_version >= 7 && code == 42 && next == 42 {
            size += 1;
            tokentype = T::Starstar;
            next = self.code_at(self.pos + 2);
        }

        if next == 61 {
            return self.finish_op(T::Assign, size + 1);
        }
        self.finish_op(tokentype, size)
    }

    fn read_token_pipe_amp(&mut self, code: u32) -> PResult<()> {
        // '|&'
        let next = self.code_at(self.pos + 1);
        if next == code {
            if self.options.ecma_version >= 12 {
                let next2 = self.code_at(self.pos + 2);
                if next2 == 61 {
                    return self.finish_op(T::Assign, 3);
                }
            }
            return self.finish_op(if code == 124 { T::LogicalOR } else { T::LogicalAND }, 2);
        }
        if next == 61 {
            return self.finish_op(T::Assign, 2);
        }
        self.finish_op(if code == 124 { T::BitwiseOR } else { T::BitwiseAND }, 1)
    }

    fn read_token_caret(&mut self) -> PResult<()> {
        // '^'
        let next = self.code_at(self.pos + 1);
        if next == 61 {
            return self.finish_op(T::Assign, 2);
        }
        self.finish_op(T::BitwiseXOR, 1)
    }

    fn read_token_plus_min(&mut self, code: u32) -> PResult<()> {
        // '+-'
        let next = self.code_at(self.pos + 1);
        if next == code {
            if next == 45
                && !self.in_module
                && self.code_at(self.pos + 2) == 62
                && (self.last_tok_end == 0
                    || super::whitespace::has_line_break(&self.input[self.last_tok_end as usize..self.pos as usize]))
            {
                // A `-->` line comment
                self.skip_line_comment(3);
                self.skip_space()?;
                return self.next_token();
            }
            return self.finish_op(T::IncDec, 2);
        }
        if next == 61 {
            return self.finish_op(T::Assign, 2);
        }
        self.finish_op(T::PlusMin, 1)
    }

    fn read_token_lt_gt(&mut self, code: u32) -> PResult<()> {
        // '<>'
        let next = self.code_at(self.pos + 1);
        let mut size = 1;
        if next == code {
            size = if code == 62 && self.code_at(self.pos + 2) == 62 { 3 } else { 2 };
            if self.code_at(self.pos + size) == 61 {
                return self.finish_op(T::Assign, size + 1);
            }
            return self.finish_op(T::BitShift, size);
        }
        if next == 33 && code == 60 && !self.in_module && self.code_at(self.pos + 2) == 45 && self.code_at(self.pos + 3) == 45
        {
            // `<!--`, an XML-style comment that should be interpreted as a line comment
            self.skip_line_comment(4);
            self.skip_space()?;
            return self.next_token();
        }
        if next == 61 {
            size = 2;
        }
        self.finish_op(T::Relational, size)
    }

    fn read_token_eq_excl(&mut self, code: u32) -> PResult<()> {
        // '=!'
        let next = self.code_at(self.pos + 1);
        if next == 61 {
            return self.finish_op(T::Equality, if self.code_at(self.pos + 2) == 61 { 3 } else { 2 });
        }
        if code == 61 && next == 62 && self.options.ecma_version >= 6 {
            // '=>'
            self.pos += 2;
            self.finish_token(T::Arrow, TokVal::None);
            return Ok(());
        }
        self.finish_op(if code == 61 { T::Eq } else { T::Prefix }, 1)
    }

    fn read_token_question(&mut self) -> PResult<()> {
        // '?'
        let ecma_version = self.options.ecma_version;
        if ecma_version >= 11 {
            let next = self.code_at(self.pos + 1);
            if next == 46 {
                let next2 = self.code_at(self.pos + 2);
                // `next2 < 48 || next2 > 57` is false for NaN
                if next2 != EOF_CODE && !(48..=57).contains(&next2) {
                    return self.finish_op(T::QuestionDot, 2);
                }
            }
            if next == 63 {
                if ecma_version >= 12 {
                    let next2 = self.code_at(self.pos + 2);
                    if next2 == 61 {
                        return self.finish_op(T::Assign, 3);
                    }
                }
                return self.finish_op(T::Coalesce, 2);
            }
        }
        self.finish_op(T::Question, 1)
    }

    fn read_token_number_sign(&mut self) -> PResult<()> {
        // '#'
        let ecma_version = self.options.ecma_version;
        let mut code = 35; // '#'
        if ecma_version >= 13 {
            self.pos += 1;
            code = self.full_char_code_at_pos();
            if is_identifier_start(code, true) || code == 92
            /* '\' */
            {
                let word = self.read_word1()?;
                self.finish_token(T::PrivateId, TokVal::Str(word));
                return Ok(());
            }
        }

        self.raise(self.pos, format!("Unexpected character '{}'", code_unit_display(code)))
    }

    pub fn get_token_from_code(&mut self, code: u32) -> PResult<()> {
        match code {
            // The interpretation of a dot depends on whether it is followed
            // by a digit or another two dots.
            46 => return self.read_token_dot(), // '.'

            // Punctuation tokens.
            40 => return self.punct(T::ParenL),
            41 => return self.punct(T::ParenR),
            59 => return self.punct(T::Semi),
            44 => return self.punct(T::Comma),
            91 => return self.punct(T::BracketL),
            93 => return self.punct(T::BracketR),
            123 => return self.punct(T::BraceL),
            125 => return self.punct(T::BraceR),
            58 => return self.punct(T::Colon),

            96 => {
                // '`'
                if self.options.ecma_version >= 6 {
                    return self.punct(T::BackQuote);
                }
            }

            48..=57 => {
                if code == 48 {
                    // '0'
                    let next = self.code_at(self.pos + 1);
                    if next == 120 || next == 88 {
                        return self.read_radix_number(16); // '0x', '0X' - hex number
                    }
                    if self.options.ecma_version >= 6 {
                        if next == 111 || next == 79 {
                            return self.read_radix_number(8); // '0o', '0O' - octal number
                        }
                        if next == 98 || next == 66 {
                            return self.read_radix_number(2); // '0b', '0B' - binary number
                        }
                    }
                }
                // Anything else beginning with a digit is an integer, octal
                // number, or float.
                return self.read_number(false);
            }

            // Quotes produce strings.
            34 | 39 => return self.read_string(code), // '"', "'"

            // Operators are parsed inline in tiny state machines. '=' (61) is
            // often referred to. `finishOp` simply skips the amount of
            // characters it is given as second argument, and returns a token
            // of the type given by its first argument.
            47 => return self.read_token_slash(), // '/'

            37 | 42 => return self.read_token_mult_modulo_exp(code), // '%*'

            124 | 38 => return self.read_token_pipe_amp(code), // '|&'

            94 => return self.read_token_caret(), // '^'

            43 | 45 => return self.read_token_plus_min(code), // '+-'

            60 | 62 => return self.read_token_lt_gt(code), // '<>'

            61 | 33 => return self.read_token_eq_excl(code), // '=!'

            63 => return self.read_token_question(), // '?'

            126 => return self.finish_op(T::Prefix, 1), // '~'

            35 => return self.read_token_number_sign(), // '#'
            _ => {}
        }

        self.raise(self.pos, format!("Unexpected character '{}'", code_unit_display(code)))
    }

    fn punct(&mut self, t: T) -> PResult<()> {
        self.pos += 1;
        self.finish_token(t, TokVal::None);
        Ok(())
    }

    /// SVELTERS: the value of operator tokens is their source text, read back from the input
    /// (`tok_text`) instead of being stored.
    pub fn finish_op(&mut self, type_: T, size: u32) -> PResult<()> {
        self.pos += size;
        self.finish_token(type_, TokVal::None);
        Ok(())
    }

    /// Source text of the current token.
    pub fn tok_text(&self) -> &'a str {
        &self.input[self.start as usize..self.end as usize]
    }

    pub fn read_regexp(&mut self) -> PResult<()> {
        let mut escaped = false;
        let mut in_class = false;
        let start = self.pos;
        loop {
            if self.pos >= self.len() {
                return self.raise(start, "Unterminated regular expression");
            }
            let ch = self.code_at(self.pos);
            if is_new_line(ch) {
                return self.raise(start, "Unterminated regular expression");
            }
            if !escaped {
                if ch == '[' as u32 {
                    in_class = true;
                } else if ch == ']' as u32 && in_class {
                    in_class = false;
                } else if ch == '/' as u32 && !in_class {
                    break;
                }
                escaped = ch == '\\' as u32;
            } else {
                escaped = false;
            }
            self.pos += self.char_len_at(self.pos);
        }
        let pattern = self.input[start as usize..self.pos as usize].to_string();
        self.pos += 1;
        let flags_start = self.pos;
        let flags = self.read_word1()?;
        if self.contains_esc {
            return self.unexpected(Some(flags_start));
        }

        // Validate pattern
        self.validate_regexp(start, &pattern, &flags)?;

        // Create Literal#value property value.
        self.finish_token(T::Regexp, TokVal::Regex { pattern, flags });
        Ok(())
    }

    /// `validateRegExpFlags(state)` + `validateRegExpPattern(state)` (ported in `regexp.rs`).
    fn validate_regexp(&self, start: u32, pattern: &str, flags: &str) -> PResult<()> {
        let ecma = self.options.ecma_version;
        if let Err(message) = super::regexp::validate_flags(flags, ecma) {
            return self.raise(start, message);
        }
        if let Err((message, offset)) = super::regexp::validate_pattern(pattern, flags, ecma) {
            // acorn's `RegExpValidationState.raise` reports at the pattern start (`offset` is relative to it).
            return self.raise_recoverable(start + offset, message);
        }
        Ok(())
    }

    // Read an integer in the given radix. Return null if zero digits
    // were read, the integer value otherwise. When `len` is given, this
    // will return `null` unless the integer has exactly `len` digits.

    pub fn read_int(&mut self, radix: u32, len: Option<i64>, maybe_legacy_octal_numeric_literal: bool) -> PResult<Option<f64>> {
        // `len` is used for character escape sequences. In that case, disallow separators.
        let allow_separators = self.options.ecma_version >= 12 && len.is_none();

        // `maybeLegacyOctalNumericLiteral` is true if it doesn't have prefix (0x,0o,0b)
        // and isn't fraction part nor exponent part. In that case, if the first digit
        // is zero then disallow separators.
        let is_legacy_octal_numeric_literal = maybe_legacy_octal_numeric_literal && self.code_at(self.pos) == 48;

        let start = self.pos;
        let mut total: f64 = 0.0;
        let mut last_code = 0;
        let e = len.unwrap_or(i64::MAX);
        let mut i: i64 = 0;
        while i < e {
            let code = self.code_at(self.pos);

            if allow_separators && code == 95 {
                if is_legacy_octal_numeric_literal {
                    return self
                        .raise_recoverable(self.pos, "Numeric separator is not allowed in legacy octal numeric literals");
                }
                if last_code == 95 {
                    return self.raise_recoverable(self.pos, "Numeric separator must be exactly one underscore");
                }
                if i == 0 {
                    return self.raise_recoverable(self.pos, "Numeric separator is not allowed at the first of digits");
                }
                last_code = code;
                i += 1;
                self.pos += 1;
                continue;
            }

            let val = if code == EOF_CODE {
                u32::MAX
            } else if code >= 97 {
                code - 97 + 10 // a
            } else if code >= 65 {
                code - 65 + 10 // A
            } else if (48..=57).contains(&code) {
                code - 48 // 0-9
            } else {
                u32::MAX
            };
            if val >= radix {
                break;
            }
            last_code = code;
            total = total * radix as f64 + val as f64;
            i += 1;
            self.pos += 1;
        }

        if allow_separators && last_code == 95 {
            return self.raise_recoverable(self.pos - 1, "Numeric separator is not allowed at the last of digits");
        }
        if self.pos == start || len.is_some() && (self.pos - start) as i64 != len.unwrap() {
            return Ok(None);
        }

        Ok(Some(total))
    }

    pub fn read_radix_number(&mut self, radix: u32) -> PResult<()> {
        let start = self.pos;
        self.pos += 2; // 0x
        let val = self.read_int(radix, None, false)?;
        let Some(val) = val else {
            return self.raise(self.start + 2, format!("Expected number in radix {}", radix));
        };
        if self.options.ecma_version >= 11 && self.code_at(self.pos) == 110 {
            let v = bigint_to_decimal(&self.input[start as usize..self.pos as usize]);
            self.pos += 1;
            self.finish_token(T::Num, TokVal::BigInt(v));
            return Ok(());
        } else if is_identifier_start(self.full_char_code_at_pos(), true) {
            return self.raise(self.pos, "Identifier directly after number");
        }
        self.finish_token(T::Num, TokVal::Num(val));
        Ok(())
    }

    // Read an integer, octal integer, or floating-point number.

    pub fn read_number(&mut self, starts_with_dot: bool) -> PResult<()> {
        let start = self.pos;
        if !starts_with_dot && self.read_int(10, None, true)?.is_none() {
            return self.raise(start, "Invalid number");
        }
        let mut octal = self.pos - start >= 2 && self.code_at(start) == 48;
        if octal && self.strict {
            return self.raise(start, "Invalid number");
        }
        let mut next = self.code_at(self.pos);
        if !octal && !starts_with_dot && self.options.ecma_version >= 11 && next == 110 {
            let v = bigint_to_decimal(&self.input[start as usize..self.pos as usize]);
            self.pos += 1;
            if is_identifier_start(self.full_char_code_at_pos(), true) {
                return self.raise(self.pos, "Identifier directly after number");
            }
            self.finish_token(T::Num, TokVal::BigInt(v));
            return Ok(());
        }
        if octal && self.input[start as usize..self.pos as usize].bytes().any(|b| b == b'8' || b == b'9') {
            octal = false;
        }
        if next == 46 && !octal {
            // '.'
            self.pos += 1;
            self.read_int(10, None, false)?;
            next = self.code_at(self.pos);
        }
        if (next == 69 || next == 101) && !octal {
            // 'eE'
            self.pos += 1;
            next = self.code_at(self.pos);
            if next == 43 || next == 45 {
                self.pos += 1; // '+-'
            }
            if self.read_int(10, None, false)?.is_none() {
                return self.raise(start, "Invalid number");
            }
        }
        if is_identifier_start(self.full_char_code_at_pos(), true) {
            return self.raise(self.pos, "Identifier directly after number");
        }

        let val = string_to_number(&self.input[start as usize..self.pos as usize], octal);
        self.finish_token(T::Num, TokVal::Num(val));
        Ok(())
    }

    // Read a string value, interpreting backslash-escapes.

    pub fn read_code_point(&mut self) -> PResult<u32> {
        let ch = self.code_at(self.pos);
        let code;

        if ch == 123 {
            // '{'
            if self.options.ecma_version < 6 {
                return self.unexpected(None);
            }
            self.pos += 1;
            let code_pos = self.pos;
            let close = self.input[self.pos as usize..].find('}').map(|i| i as i64).unwrap_or(-1 - self.pos as i64);
            code = self.read_hex_char(close)?;
            self.pos += 1;
            if code > 0x10FFFF as f64 {
                return self.invalid_string_token(code_pos, "Code point out of bounds");
            }
        } else {
            code = self.read_hex_char(4)?;
        }
        Ok(code as u32)
    }

    pub fn read_string(&mut self, quote: u32) -> PResult<()> {
        let mut out = JsStr::new();
        self.pos += 1;
        let mut chunk_start = self.pos;
        loop {
            if self.pos >= self.len() {
                return self.raise(self.start, "Unterminated string constant");
            }
            let ch = self.code_at(self.pos);
            if ch == quote {
                break;
            }
            if ch == 92 {
                // '\'
                out.push_str(&self.input[chunk_start as usize..self.pos as usize]);
                if let Some(c) = self.read_escaped_char(false)? {
                    out.push_code(c);
                }
                chunk_start = self.pos;
            } else if ch == 0x2028 || ch == 0x2029 {
                if self.options.ecma_version < 10 {
                    return self.raise(self.start, "Unterminated string constant");
                }
                self.pos += 3;
            } else {
                if is_new_line(ch) {
                    return self.raise(self.start, "Unterminated string constant");
                }
                self.pos += self.char_len_at(self.pos);
            }
        }
        out.push_str(&self.input[chunk_start as usize..self.pos as usize]);
        self.pos += 1;
        let (s, lone) = out.finish();
        self.finish_token(T::String, TokVal::Str(s));
        self.value_lone_surrogate = lone;
        Ok(())
    }

    // Reads template string tokens.

    pub fn try_read_template_token(&mut self) -> PResult<()> {
        self.in_template_element = true;
        match self.read_tmpl_token() {
            Ok(()) => {}
            Err(e) if matches!(*e, PErr::InvalidTemplateEscape) => {
                self.read_invalid_template_token()?;
            }
            Err(e) => return Err(e),
        }

        self.in_template_element = false;
        Ok(())
    }

    pub fn invalid_string_token<R>(&self, position: u32, message: &str) -> PResult<R> {
        if self.in_template_element && self.options.ecma_version >= 9 {
            Err(Box::new(PErr::InvalidTemplateEscape))
        } else {
            self.raise(position, message)
        }
    }

    pub fn read_tmpl_token(&mut self) -> PResult<()> {
        let mut out = JsStr::new();
        let mut chunk_start = self.pos;
        loop {
            if self.pos >= self.len() {
                return self.raise(self.start, "Unterminated template");
            }
            let ch = self.code_at(self.pos);
            if ch == 96 || ch == 36 && self.code_at(self.pos + 1) == 123 {
                // '`', '${'
                if self.pos == self.start && (self.type_ == T::Template || self.type_ == T::InvalidTemplate) {
                    if ch == 36 {
                        self.pos += 2;
                        self.finish_token(T::DollarBraceL, TokVal::None);
                    } else {
                        self.pos += 1;
                        self.finish_token(T::BackQuote, TokVal::None);
                    }
                    return Ok(());
                }
                out.push_str(&self.input[chunk_start as usize..self.pos as usize]);
                let (s, _) = out.finish();
                self.finish_token(T::Template, TokVal::Template(s));
                return Ok(());
            }
            if ch == 92 {
                // '\'
                out.push_str(&self.input[chunk_start as usize..self.pos as usize]);
                if let Some(c) = self.read_escaped_char(true)? {
                    out.push_code(c);
                }
                chunk_start = self.pos;
            } else if is_new_line(ch) {
                out.push_str(&self.input[chunk_start as usize..self.pos as usize]);
                self.pos += self.char_len_at(self.pos);
                match ch {
                    13 | 10 => {
                        if ch == 13 && self.code_at(self.pos) == 10 {
                            self.pos += 1;
                        }
                        out.push_str("\n");
                    }
                    _ => {
                        out.push_code(ch);
                    }
                }
                chunk_start = self.pos;
            } else {
                self.pos += self.char_len_at(self.pos);
            }
        }
    }

    // Reads a template token to search for the end, without validating any escape sequences
    pub fn read_invalid_template_token(&mut self) -> PResult<()> {
        let b = self.input.as_bytes();
        while (self.pos as usize) < b.len() {
            match b[self.pos as usize] {
                b'\\' => {
                    self.pos += 1;
                }
                b'$' if b.get(self.pos as usize + 1) == Some(&b'{') => {
                    let raw = self.input[self.start as usize..self.pos as usize].to_string();
                    self.finish_token(T::InvalidTemplate, TokVal::InvalidTemplate(raw));
                    return Ok(());
                }
                b'`' => {
                    let raw = self.input[self.start as usize..self.pos as usize].to_string();
                    self.finish_token(T::InvalidTemplate, TokVal::InvalidTemplate(raw));
                    return Ok(());
                }
                _ => {}
            }
            self.pos += 1;
        }
        self.raise(self.start, "Unterminated template")
    }

    // Used to read escaped characters. Returns None for line continuations (acorn's "").

    pub fn read_escaped_char(&mut self, in_template: bool) -> PResult<Option<u32>> {
        self.pos += 1;
        let mut ch = self.code_at(self.pos);
        self.pos += self.char_len_at(self.pos);
        match ch {
            110 => return Ok(Some('\n' as u32)), // 'n' -> '\n'
            114 => return Ok(Some('\r' as u32)), // 'r' -> '\r'
            120 => return Ok(Some(self.read_hex_char(2)? as u32)), // 'x'
            117 => return Ok(Some(self.read_code_point()?)), // 'u'
            116 => return Ok(Some('\t' as u32)), // 't' -> '\t'
            98 => return Ok(Some(8)),            // 'b' -> '\b'
            118 => return Ok(Some(0x0b)),        // 'v' -> '\u000b'
            102 => return Ok(Some(0x0c)),        // 'f' -> '\f'
            13 | 10 => {
                if ch == 13 && self.code_at(self.pos) == 10 {
                    self.pos += 1; // '\r\n'
                }
                return Ok(None);
            }
            56 | 57 => {
                if self.strict {
                    self.invalid_string_token(self.pos - 1, "Invalid escape sequence")?;
                }
                if in_template {
                    let code_pos = self.pos - 1;

                    self.invalid_string_token(code_pos, "Invalid escape sequence in template string")?;
                }
            }
            _ => {}
        }
        // default:
        if (48..=55).contains(&ch) {
            let start = (self.pos - 1) as usize;
            let b = self.input.as_bytes();
            let mut end = start;
            while end < b.len() && end < start + 3 && (b'0'..=b'7').contains(&b[end]) {
                end += 1;
            }
            let mut octal_str = &self.input[start..end];
            let mut octal = u32::from_str_radix(octal_str, 8).unwrap();
            if octal > 255 {
                octal_str = &octal_str[..octal_str.len() - 1];
                octal = u32::from_str_radix(octal_str, 8).unwrap();
            }
            self.pos += octal_str.len() as u32 - 1;
            ch = self.code_at(self.pos);
            if (octal_str != "0" || ch == 56 || ch == 57) && (self.strict || in_template) {
                self.invalid_string_token(
                    self.pos - 1 - octal_str.len() as u32,
                    if in_template { "Octal literal in template string" } else { "Octal literal in strict mode" },
                )?;
            }
            return Ok(Some(octal));
        }
        if is_new_line(ch) {
            // Unicode new line characters after \ get removed from output in both
            // template literals and strings
            return Ok(None);
        }
        if ch == EOF_CODE {
            // `String.fromCharCode(NaN)`
            return Ok(Some(0));
        }
        Ok(Some(ch))
    }

    // Used to read character escape sequences ('\x', '\u', '\U').

    pub fn read_hex_char(&mut self, len: i64) -> PResult<f64> {
        let code_pos = self.pos;
        let n = self.read_int(16, Some(len), false)?;
        match n {
            None => self.invalid_string_token(code_pos, "Bad character escape sequence"),
            Some(n) => Ok(n),
        }
    }

    // Read an identifier, and return it as a string. Sets `this.containsEsc`
    // to whether the word contained a '\u' escape.
    //
    // Incrementally adds only escaped chars, adding other chunks as-is
    // as a micro-optimization.

    pub fn read_word1(&mut self) -> PResult<String> {
        self.contains_esc = false;
        let mut word = String::new();
        let mut first = true;
        let mut chunk_start = self.pos;
        let astral = self.options.ecma_version >= 6;
        let len = self.len();
        let bytes = self.input.as_bytes();
        while self.pos < len {
            // Fast path for ASCII identifier chars.
            let b = bytes[self.pos as usize];
            if b < 0x80 {
                if b.is_ascii_alphanumeric() || b == b'_' || b == b'$' {
                    self.pos += 1;
                    first = false;
                    continue;
                }
                if b != b'\\' {
                    break;
                }
            }
            let ch = self.full_char_code_at_pos();
            if is_identifier_char(ch, astral) {
                self.pos += self.char_len_at(self.pos);
            } else if ch == 92 {
                // "\"
                self.contains_esc = true;
                word.push_str(&self.input[chunk_start as usize..self.pos as usize]);
                let esc_start = self.pos;
                self.pos += 1;
                if self.code_at(self.pos) != 117 {
                    // "u"
                    return self.invalid_string_token(self.pos, "Expecting Unicode escape sequence \\uXXXX");
                }
                self.pos += 1;
                let esc = self.read_code_point()?;
                let ok = if first { is_identifier_start(esc, astral) } else { is_identifier_char(esc, astral) };
                if !ok {
                    return self.invalid_string_token(esc_start, "Invalid Unicode escape");
                }
                word.push_str(&code_point_to_string(esc));
                chunk_start = self.pos;
            } else {
                break;
            }
            first = false;
        }
        word.push_str(&self.input[chunk_start as usize..self.pos as usize]);
        Ok(word)
    }

    // Read an identifier or keyword token. Will check for reserved
    // words when necessary.

    pub fn read_word(&mut self) -> PResult<()> {
        let word = self.read_word1()?;
        let mut type_ = T::Name;
        if self.is_keyword_word(&word) {
            type_ = keyword_type(&word).unwrap();
        }
        self.finish_token(type_, TokVal::Str(word));
        Ok(())
    }
}

/// `codePointToString(code)` for error messages; `NaN` reads as `\u0000` in JS.
fn code_unit_display(code: u32) -> String {
    if code == EOF_CODE {
        return "\u{0}".into();
    }
    code_point_to_string(code)
}

fn string_to_number(s: &str, is_legacy_octal_numeric_literal: bool) -> f64 {
    if is_legacy_octal_numeric_literal {
        // parseInt(str, 8)
        let mut total = 0.0;
        for b in s.bytes() {
            total = total * 8.0 + (b - b'0') as f64;
        }
        return total;
    }

    // `parseFloat(value)` stops parsing at the first numeric separator then returns a wrong value.
    let cleaned: String = s.chars().filter(|c| *c != '_').collect();
    cleaned.parse::<f64>().unwrap_or(f64::NAN)
}
