//! Port of acorn 8.19.0 `src/locutil.js`.
//!
//! SVELTERS: `Position`/`SourceLocation` are not stored on nodes (byte offsets only), so only
//! `getLineInfo` remains. Columns are counted in UTF-16 code units, like acorn.

use super::whitespace::next_line_break;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    /// UTF-16 column.
    pub column: u32,
}

// The `getLineInfo` function is mostly useful when the
// `locations` option is off (for performance reasons) and you
// want to find the line/column position for a given character
// offset. `input` should be the code string that the offset refers
// into.

pub fn get_line_info(input: &str, offset: usize) -> Position {
    let offset = offset.min(input.len());
    let mut line = 1;
    let mut cur = 0;
    loop {
        match next_line_break(input, cur, offset) {
            None => {
                let column = input.get(cur..offset).map(|s| s.encode_utf16().count()).unwrap_or(offset - cur);
                return Position { line, column: column as u32 };
            }
            Some(next) => {
                line += 1;
                cur = next;
            }
        }
    }
}
