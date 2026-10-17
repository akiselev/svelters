//! Parser-related parts of `packages/svelte/src/utils.js` (Svelte 5.57.2 @ f3de3f2), plus the
//! character classes the parser's regexes use.

const VOID_ELEMENT_NAMES: [&str; 16] = [
    "area", "base", "br", "col", "command", "embed", "hr", "img", "input", "keygen", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Returns `true` if `name` is of a void element
pub fn is_void(name: &str) -> bool {
    VOID_ELEMENT_NAMES.contains(&name) || js_to_lower(name) == "!doctype"
}

/// `String.prototype.toLowerCase` (only compared against ASCII here; Rust's full lowercase
/// mapping agrees with JS's on every input that can produce `!doctype`).
fn js_to_lower(s: &str) -> String {
    s.to_lowercase()
}

const RESERVED_WORDS: [&str; 48] = [
    "arguments",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "eval",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

/// Returns `true` if `word` is a reserved JavaScript keyword
pub fn is_reserved(word: &str) -> bool {
    RESERVED_WORDS.contains(&word)
}

/// `/^[a-zA-Z][a-zA-Z0-9]*(-[a-zA-Z0-9.\-_·À-ÖØ-öø-ͽͿ-῿‌-‍‿-⁀⁰-↏Ⰰ-⿯、-퟿豈-﷏ﷰ-�\u{10000}-\u{EFFFF}]*)?$/u`
pub fn regex_valid_tag_name(name: &str) -> bool {
    let mut chars = name.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    let rest = chars.as_str();
    let head_len = rest.bytes().take_while(|b| b.is_ascii_alphanumeric()).count();
    let tail = &rest[head_len..];
    if tail.is_empty() {
        return true;
    }
    let Some(tail) = tail.strip_prefix('-') else { return false };
    tail.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(c, '.' | '-' | '_' | '\u{B7}')
            || ('\u{C0}'..='\u{D6}').contains(&c)
            || ('\u{D8}'..='\u{F6}').contains(&c)
            || ('\u{F8}'..='\u{37D}').contains(&c)
            || ('\u{37F}'..='\u{1FFF}').contains(&c)
            || ('\u{200C}'..='\u{200D}').contains(&c)
            || ('\u{203F}'..='\u{2040}').contains(&c)
            || ('\u{2070}'..='\u{218F}').contains(&c)
            || ('\u{2C00}'..='\u{2FEF}').contains(&c)
            || ('\u{3001}'..='\u{D7FF}').contains(&c)
            || ('\u{F900}'..='\u{FDCF}').contains(&c)
            || ('\u{FDF0}'..='\u{FFFD}').contains(&c)
            || ('\u{10000}'..='\u{EFFFF}').contains(&c)
    })
}

/// acorn `isIdentifierStart(code, true)`. ASCII is answered here (as acorn's own fast path
/// does), everything else is delegated to the acorn port.
pub fn is_identifier_start(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphabetic() || c == '$' || c == '_';
    }
    crate::acorn::is_identifier_start(c)
}

/// acorn `isIdentifierChar(code, true)`; see [`is_identifier_start`].
pub fn is_identifier_char(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphanumeric() || c == '$' || c == '_';
    }
    crate::acorn::is_identifier_char(c)
}

/// `\p{ID_Start}`: acorn's identifier start minus the two extra characters it allows.
pub fn is_id_start(c: char) -> bool {
    c != '$' && c != '_' && is_identifier_start(c)
}

/// `[$‌‍\p{ID_Continue}]`: exactly acorn's identifier char set.
pub fn is_id_continue_or_dollar(c: char) -> bool {
    is_identifier_char(c)
}

/// `\p{Lu}`: Rust's `is_uppercase` is the `Uppercase` property, i.e. `Lu` plus
/// `Other_Uppercase`; remove the latter.
pub fn is_lu(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_uppercase();
    }
    let other_uppercase = matches!(c as u32, 0x2160..=0x216F | 0x24B6..=0x24CF | 0x1F130..=0x1F149 | 0x1F150..=0x1F169 | 0x1F170..=0x1F189);
    c.is_uppercase() && !other_uppercase
}
