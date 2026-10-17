//! Port of `phases/1-parse/utils/whitespace.js` (Svelte 5.57.2 @ f3de3f2).

/// `cc` is a UTF-16 code unit upstream; every character it accepts is in the BMP, so passing a
/// code point is equivalent.
pub fn is_whitespace(cc: u32) -> bool {
    // fast path for common whitespace
    if cc == 32 || (9..=13).contains(&cc) {
        return true;
    }
    // rare whitespace —  ,  ,  - ,  ,  ,  ,  , 　,
    if cc < 160 {
        return false;
    }
    cc == 160
        || cc == 5760
        || (8192..=8202).contains(&cc)
        || cc == 8232
        || cc == 8233
        || cc == 8239
        || cc == 8287
        || cc == 12288
        || cc == 65279
}

/// JS regex `\s` (WhiteSpace + LineTerminator), the same set as [`is_whitespace`].
pub fn is_js_regex_space(c: char) -> bool {
    is_whitespace(c as u32)
}
