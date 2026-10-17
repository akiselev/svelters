//! Port of acorn 8.19.0 `src/whitespace.js`.
//!
//! SVELTERS: positions are byte offsets into UTF-8; the regexps (`lineBreak`, `skipWhiteSpace`,
//! `nonASCIIwhitespace`) are hand-written scanners with the same semantics.

/// `lineBreak.test(s)`: matches a whole line break (where CRLF is considered a single line break).
pub fn has_line_break(s: &str) -> bool {
    s.bytes().any(|b| b == b'\n' || b == b'\r') || s.contains('\u{2028}') || s.contains('\u{2029}')
}

pub fn is_new_line(code: u32) -> bool {
    code == 10 || code == 13 || code == 0x2028 || code == 0x2029
}

/// `nextLineBreak(code, from, end)`: byte position just after the next line break in
/// `[from, end)`, or None.
pub fn next_line_break(code: &str, from: usize, end: usize) -> Option<usize> {
    let b = code.as_bytes();
    let mut i = from;
    while i < end {
        let c = b[i];
        if c == b'\n' {
            return Some(i + 1);
        }
        if c == b'\r' {
            return Some(if i < end - 1 && b.get(i + 1) == Some(&b'\n') { i + 2 } else { i + 1 });
        }
        if c == 0xE2 && b.get(i + 1) == Some(&0x80) && matches!(b.get(i + 2), Some(0xA8) | Some(0xA9)) {
            return Some(i + 3);
        }
        i += 1;
    }
    None
}

/// `nonASCIIwhitespace = /[  -   　﻿]/`
pub fn is_non_ascii_whitespace(code: u32) -> bool {
    code == 0x1680 || (0x2000..=0x200a).contains(&code) || code == 0x202f || code == 0x205f || code == 0x3000 || code == 0xfeff
}

/// JS regexp `\s`.
pub fn is_js_space(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{2028}' | '\u{2029}')
        || is_non_ascii_whitespace(c as u32)
}

/// `skipWhiteSpace = /(?:\s|\/\/.*|\/\*[^]*?\*\/)*/g` applied at `pos`: returns the end of the match.
pub fn skip_white_space(input: &str, pos: usize) -> usize {
    let b = input.as_bytes();
    let mut p = pos;
    loop {
        if p >= b.len() {
            return p;
        }
        let c = b[p];
        if c < 0x80 {
            if matches!(c, b'\t' | b'\n' | 0x0b | 0x0c | b'\r' | b' ') {
                p += 1;
                continue;
            }
            if c == b'/' && b.get(p + 1) == Some(&b'/') {
                // `\/\/.*`: `.` stops at line terminators
                p += 2;
                while p < b.len() {
                    let ch = input[p..].chars().next().unwrap();
                    if is_new_line(ch as u32) {
                        break;
                    }
                    p += ch.len_utf8();
                }
                continue;
            }
            if c == b'/' && b.get(p + 1) == Some(&b'*') {
                match input[p + 2..].find("*/") {
                    Some(i) => {
                        p = p + 2 + i + 2;
                        continue;
                    }
                    None => return p,
                }
            }
            return p;
        }
        let ch = input[p..].chars().next().unwrap();
        if is_js_space(ch) {
            p += ch.len_utf8();
            continue;
        }
        return p;
    }
}
