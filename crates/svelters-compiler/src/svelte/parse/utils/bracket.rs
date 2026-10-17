//! Port of `phases/1-parse/utils/bracket.js` (Svelte 5.57.2 @ f3de3f2).
//!
//! `Infinity` results are `None`; all delimiters are ASCII so scanning bytes is equivalent to
//! scanning UTF-16 units.

use crate::svelte::errors as e;
use crate::svelte::parse::{PResult, Parser};

fn find_byte_from(s: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if from > s.len() {
        return None;
    }
    s[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

/// @returns The index of the end of this string expression, or `Infinity` if not found.
fn find_string_end(string: &[u8], search_start_index: usize, string_start_char: u8) -> Option<usize> {
    let string_to_search = if string_start_char == b'`' {
        string
    } else {
        // we could slice at the search start index, but this way the index remains valid
        match find_byte_from(string, search_start_index, b"\n") {
            Some(nl) => &string[..nl],
            None => string,
        }
    };

    find_unescaped_char(string_to_search, search_start_index, string_start_char)
}

/// @returns The index of the end of this regex expression, or `Infinity` if not found.
fn find_regex_end(string: &[u8], search_start_index: usize) -> Option<usize> {
    let slash = find_unescaped_char(string, search_start_index, b'/');
    let eol = find_unescaped_char(string, search_start_index, b'\n');
    match (slash, eol) {
        (Some(s), Some(e)) if s < e => Some(s),
        (Some(s), None) => Some(s),
        _ => None,
    }
}

/// @returns The index of the first unescaped instance of `char`, or `Infinity` if not found.
fn find_unescaped_char(string: &[u8], search_start_index: usize, char: u8) -> Option<usize> {
    let mut i = search_start_index;
    loop {
        let found_index = find_byte_from(string, i, &[char])?;
        if count_leading_backslashes(string, found_index as isize - 1).is_multiple_of(2) {
            return Some(found_index);
        }
        i = found_index + 1;
    }
}

/// Count consecutive leading backslashes before `search_start_index`.
fn count_leading_backslashes(string: &[u8], search_start_index: isize) -> usize {
    let mut i = search_start_index;
    let mut count = 0;
    while i >= 0 && string.get(i as usize) == Some(&b'\\') {
        count += 1;
        i -= 1;
    }
    count
}

fn default_close(open: u8) -> u8 {
    match open {
        b'{' => b'}',
        b'(' => b')',
        b'[' => b']',
        // `default_brackets[open]` is undefined for anything else
        _ => 0,
    }
}

/// Finds the corresponding closing bracket, ignoring brackets found inside comments, strings, or regex expressions.
/// @returns The index of the closing bracket, or undefined if not found.
pub fn find_matching_bracket(template: &str, index: usize, open: char) -> Option<usize> {
    let template = template.as_bytes();
    let open = open as u8;
    let close = default_close(open);
    let mut brackets = 1;
    let mut i = index;
    // `Infinity` positions end the loop (`i < template.length` fails)
    while brackets > 0 && i < template.len() {
        let char = template[i];
        match char {
            b'\'' | b'"' | b'`' => {
                i = find_string_end(template, i + 1, char)? + 1;
                continue;
            }
            b'/' => {
                let Some(&next_char) = template.get(i + 1) else {
                    // `/` is the last character; advance past it so we don't loop forever
                    i += 1;
                    continue;
                };
                if next_char == b'/' {
                    i = find_byte_from(template, i + 1, b"\n")? + 1;
                    continue;
                }
                if next_char == b'*' {
                    i = find_byte_from(template, i + 1, b"*/")? + 2;
                    continue;
                }
                match find_regex_end(template, i + 1) {
                    None => i += 1,
                    Some(end) => i = end + 1,
                }
                continue;
            }
            _ => {
                if char == open {
                    brackets += 1;
                } else if char == close {
                    brackets -= 1;
                }
                if brackets == 0 {
                    return Some(i);
                }
                i += 1;
            }
        }
    }
    None
}

/// `brackets` of `match_bracket`: (open, close) pairs.
pub type Brackets = &'static [(char, char)];

pub const DEFAULT_BRACKETS: Brackets = &[('{', '}'), ('(', ')'), ('[', ']')];

pub fn match_bracket(parser: &Parser, start: usize, brackets: Brackets) -> PResult<usize> {
    let mut bracket_stack: Vec<char> = Vec::new();

    let mut i = start;

    while i < parser.template.len() {
        let char = parser.char_at(i).unwrap();
        i += char.len_utf8();

        if char == '\'' || char == '"' || char == '`' {
            i = match_quote(parser, i, char)?;
            continue;
        }

        if brackets.iter().any(|(o, _)| *o == char) {
            bracket_stack.push(char);
        } else if brackets.iter().any(|(_, c)| *c == char) {
            let popped = bracket_stack.pop();
            let expected = popped.and_then(|p| brackets.iter().find(|(o, _)| *o == p)).map(|(_, c)| *c);

            if Some(char) != expected {
                // `Expected token undefined` when nothing was open
                let expected = expected.map_or_else(|| "undefined".to_string(), |c| c.to_string());
                return Err(e::expected_token(i - 1, &expected));
            }

            if bracket_stack.is_empty() {
                return Ok(i);
            }
        }
    }

    Err(e::unexpected_eof(parser.template.len()))
}

fn match_quote(parser: &Parser, start: usize, quote: char) -> PResult<usize> {
    let mut is_escaped = false;
    let mut i = start;

    while i < parser.template.len() {
        let char = parser.char_at(i).unwrap();
        i += char.len_utf8();

        if is_escaped {
            is_escaped = false;
            continue;
        }

        if char == quote {
            return Ok(i);
        }

        if char == '\\' {
            is_escaped = true;
        }

        if quote == '`' && char == '$' && parser.byte_at(i) == Some(b'{') {
            i = match_bracket(parser, i, DEFAULT_BRACKETS)?;
        }
    }

    Err(e::unterminated_string_constant(start))
}
