//! Port of `phases/1-parse/utils/html.js` (Svelte 5.57.2 @ f3de3f2).
//!
//! Upstream builds one big regex, `&(#(?:[xX][a-fA-F\d]+|\d+)(?:;)?|<entity names>)` (for
//! attribute values, names without `;` get `\b(?!=)`), and calls `String.replace` with it. The
//! port runs the same alternation by hand: at each `&`, the numeric form first, then the entity
//! names in table order; on a match, scanning resumes after it.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::entities::ENTITIES;

const WINDOWS_1252: [u32; 32] = [
    8364, 129, 8218, 402, 8222, 8230, 8224, 8225, 710, 8240, 352, 8249, 338, 141, 381, 143, 144, 8216, 8217, 8220,
    8221, 8226, 8211, 8212, 732, 8482, 353, 8250, 339, 157, 382, 376,
];

/// Indices into `ENTITIES` grouped by first byte, each group in table order (two names with
/// different first bytes can never match at the same position, so this preserves the
/// alternation's priority).
fn entity_index() -> &'static HashMap<u8, Vec<usize>> {
    static INDEX: OnceLock<HashMap<u8, Vec<usize>>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut m: HashMap<u8, Vec<usize>> = HashMap::new();
        for (i, (name, _)) in ENTITIES.iter().enumerate() {
            m.entry(name.as_bytes()[0]).or_default().push(i);
        }
        m
    })
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// `reg_exp_entity(entity_name, is_attribute_value)` applied at `rest` (the text after `&`):
/// returns whether the alternative matches.
fn entity_matches(name: &str, rest: &[u8], is_attribute_value: bool) -> bool {
    if !rest.starts_with(name.as_bytes()) {
        return false;
    }
    // https://html.spec.whatwg.org/multipage/parsing.html#named-character-reference-state
    // doesn't decode the html entity which not ends with ; and next character is =, number or alphabet in attribute value.
    if is_attribute_value && !name.ends_with(';') {
        // `\b(?!=)`
        match rest.get(name.len()) {
            None => true,
            Some(&b) => !is_word_byte(b) && b != b'=',
        }
    } else {
        true
    }
}

/// `#(?:[xX][a-fA-F\d]+|\d+)(?:;)?`: returns the matched length.
fn numeric_match(rest: &[u8]) -> Option<usize> {
    if rest.first() != Some(&b'#') {
        return None;
    }
    let mut i = 1;
    if matches!(rest.get(1), Some(b'x' | b'X')) {
        let n = rest[2..].iter().take_while(|b| b.is_ascii_hexdigit()).count();
        if n > 0 {
            i = 2 + n;
        }
    }
    if i == 1 {
        let n = rest[1..].iter().take_while(|b| b.is_ascii_digit()).count();
        if n == 0 {
            return None;
        }
        i = 1 + n;
    }
    if rest.get(i) == Some(&b';') {
        i += 1;
    }
    Some(i)
}

/// `parseInt(digits, radix)` saturated (any value past the Unicode range maps to NUL anyway).
fn parse_int(digits: &str, radix: u32) -> u64 {
    let mut v: u64 = 0;
    for c in digits.chars() {
        let Some(d) = c.to_digit(radix) else { break };
        v = v.saturating_mul(radix as u64).saturating_add(d as u64);
    }
    v
}

pub fn decode_character_references(html: &str, is_attribute_value: bool) -> String {
    if !html.contains('&') {
        return html.to_string(); // fast path
    }

    let bytes = html.as_bytes();
    let index = entity_index();
    let mut out = String::with_capacity(html.len());
    let mut last = 0;
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'&' {
            i += 1;
            continue;
        }
        let rest = &bytes[i + 1..];

        // (entity, code)
        let mut found: Option<(usize, u64)> = None;
        if let Some(len) = numeric_match(rest) {
            let entity = &html[i + 1..i + 1 + len];
            let code = if entity.as_bytes()[1] == b'x' || entity.as_bytes()[1] == b'X' {
                parse_int(&entity[2..], 16)
            } else {
                parse_int(&entity[1..], 10)
            };
            found = Some((len, code));
        } else if let Some(candidates) = rest.first().and_then(|b| index.get(b)) {
            for &k in candidates {
                let (name, code) = ENTITIES[k];
                if entity_matches(name, rest, is_attribute_value) {
                    found = Some((name.len(), code as u64));
                    break;
                }
            }
        }

        match found {
            Some((len, code)) => {
                let match_end = i + 1 + len;
                if code != 0 {
                    out.push_str(&html[last..i]);
                    let c = validate_code(code, is_attribute_value);
                    out.push(char::from_u32(c).unwrap_or('\0'));
                    last = match_end;
                }
                i = match_end;
            }
            None => i += 1,
        }
    }

    out.push_str(&html[last..]);
    out
}

const NUL: u32 = 0;

// some code points are verboten. If we were inserting HTML, the browser would replace the illegal
// code points with alternatives in some cases - since we're bypassing that mechanism, we need
// to replace them ourselves
//
// Source: http://en.wikipedia.org/wiki/Character_encodings_in_HTML#Illegal_characters
// Also see: https://en.wikipedia.org/wiki/Plane_(Unicode)
// Also see: https://html.spec.whatwg.org/multipage/parsing.html#preprocessing-the-input-stream

fn validate_code(code: u64, is_attribute_value: bool) -> u32 {
    // line feed becomes generic whitespace, since it is collapsed along with the
    // surrounding whitespace anyway. In an attribute value it is significant, so it
    // is left alone there
    if code == 10 && !is_attribute_value {
        return 32;
    }

    // ASCII range. (Why someone would use HTML entities for ASCII characters I don't know, but...)
    if code < 128 {
        return code as u32;
    }

    // code points 128-159 are dealt with leniently by browsers, but they're incorrect. We need
    // to correct the mistake or we'll end up with missing € signs and so on
    if code <= 159 {
        return WINDOWS_1252[(code - 128) as usize];
    }

    // basic multilingual plane
    if code < 55296 {
        return code as u32;
    }

    // UTF-16 surrogate halves
    if code <= 57343 {
        return NUL;
    }

    // rest of the basic multilingual plane
    if code <= 65535 {
        return code as u32;
    }

    // supplementary multilingual plane 0x10000 - 0x1ffff
    if (65536..=131071).contains(&code) {
        return code as u32;
    }

    // supplementary ideographic plane 0x20000 - 0x2ffff
    if (131072..=196607).contains(&code) {
        return code as u32;
    }

    // supplementary special-purpose plane 0xe0000 - 0xe07f and 0xe0100 - 0xe01ef
    if (917504..=917631).contains(&code) || (917760..=917999).contains(&code) {
        return code as u32;
    }

    NUL
}
