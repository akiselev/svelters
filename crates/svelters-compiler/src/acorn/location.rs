//! Port of acorn 8.19.0 `src/location.js`.

use super::locutil::get_line_info;
use super::state::{PErr, PResult, Parser};
use super::ParseError;

impl<'a> Parser<'a> {
    // This function is used to raise exceptions on parse errors. It
    // takes an offset integer (into the current `input`) to indicate
    // the location of the error, attaches the position to the end
    // of the error message, and then raises a `SyntaxError` with that
    // message.

    pub fn raise<T>(&self, pos: u32, message: impl Into<String>) -> PResult<T> {
        Err(self.error_at(pos, message))
    }

    pub fn error_at(&self, pos: u32, message: impl Into<String>) -> Box<PErr> {
        let mut message = message.into();
        let loc = get_line_info(self.input, pos as usize);
        message.push_str(&format!(" ({}:{})", loc.line, loc.column));
        Box::new(PErr::Syntax(ParseError { message, pos, raised_at: self.pos }))
    }

    pub fn raise_recoverable<T>(&self, pos: u32, message: impl Into<String>) -> PResult<T> {
        self.raise(pos, message)
    }
}
