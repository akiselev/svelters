//! Port of acorn 8.19.0 `src/regexp.js` (+ `unicode-property-data.js`). Owner: agent B.

/// acorn `validateRegExpFlags`: Err(message) uses acorn's wording ("Invalid regular expression flag").
pub fn validate_flags(_flags: &str, _ecma_version: u32) -> Result<(), String> {
    todo!("agent B")
}

/// acorn `validateRegExpPattern`: Err((message, byte offset into `pattern`)) with acorn's wording,
/// e.g. "Invalid regular expression: /(/: Unterminated group".
pub fn validate_pattern(_pattern: &str, _flags: &str, _ecma_version: u32) -> Result<(), (String, u32)> {
    todo!("agent B")
}
