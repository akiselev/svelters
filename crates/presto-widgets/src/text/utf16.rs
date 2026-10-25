//! UTF-16 offsets (architecture §11.3: TSF, macOS and UIA speak UTF-16 code units of the whole
//! document). Over quire-text's [`Text`] they are quire's own O(log n) conversions (request R1); the
//! `str_*` functions do the same over a `&str` (the IME document with its preedit, a preedit itself).

use quire_text::Text;

/// The UTF-16 offset of byte `b` (rounded down to a char boundary, clamped to the end).
pub fn byte_to_utf16(text: &Text, b: usize) -> usize {
    text.byte_to_utf16_offset(b)
}

/// The byte of UTF-16 offset `u` (clamped to the end; an offset inside a surrogate pair rounds down
/// to the pair's start).
pub fn utf16_to_byte(text: &Text, u: usize) -> usize {
    text.utf16_offset_to_byte(u)
}

/// The length of `text` in UTF-16 code units.
pub fn len_utf16(text: &Text) -> usize {
    text.len_utf16()
}

/// [`byte_to_utf16`] on a `&str` (`b` must be a char boundary of `s`, or its length).
pub fn str_byte_to_utf16(s: &str, b: usize) -> usize {
    let b = floor_boundary(s, b);
    let head = &s[..b];
    if head.is_ascii() { b } else { head.chars().map(char::len_utf16).sum() }
}

/// [`utf16_to_byte`] on a `&str`.
pub fn str_utf16_to_byte(s: &str, u: usize) -> usize {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if units + c.len_utf16() > u {
            return i;
        }
        units += c.len_utf16();
    }
    s.len()
}

/// The largest char boundary of `s` at or before `b`.
pub fn floor_boundary(s: &str, b: usize) -> usize {
    let mut b = b.min(s.len());
    while !s.is_char_boundary(b) {
        b -= 1;
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_offsets_cross_surrogate_pairs_and_chunks() {
        // "a😀é\nb": a(1) 😀(4 bytes, 2 units) é(2 bytes, 1 unit) \n b
        let t = Text::from("a😀é\nb");
        assert_eq!(byte_to_utf16(&t, 0), 0);
        assert_eq!(byte_to_utf16(&t, 1), 1);
        assert_eq!(byte_to_utf16(&t, 5), 3);
        assert_eq!(byte_to_utf16(&t, 7), 4);
        assert_eq!(byte_to_utf16(&t, 9), 6);
        // inside the emoji: rounds down to its start
        assert_eq!(byte_to_utf16(&t, 3), 1);
        assert_eq!(utf16_to_byte(&t, 1), 1);
        assert_eq!(utf16_to_byte(&t, 2), 1, "inside the pair rounds down");
        assert_eq!(utf16_to_byte(&t, 3), 5);
        assert_eq!(utf16_to_byte(&t, 99), t.len_bytes());
        assert_eq!(len_utf16(&t), 6);
        // a long text takes quire's rope backend: the chunk walk must agree with the string walk
        let long: String = "x😀é\n".repeat(4000);
        let t = Text::from(long.as_str());
        for b in [0, 1, 5, 7, 8, 9, 4000, 31_999, long.len()] {
            let b = floor_boundary(&long, b);
            let u = byte_to_utf16(&t, b);
            assert_eq!(u, str_byte_to_utf16(&long, b));
            assert_eq!(utf16_to_byte(&t, u), b);
        }
        assert_eq!(len_utf16(&t), long.encode_utf16().count());
    }
}
