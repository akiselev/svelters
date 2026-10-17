//! Port of `phases/1-parse/state/fragment.js` (Svelte 5.57.2 @ f3de3f2).

use crate::svelte::parse::{Parser, ParserState};

pub fn fragment(parser: &mut Parser) -> ParserState {
    if parser.match_str("<") {
        return ParserState::Element;
    }

    if parser.match_str("{") {
        return ParserState::Tag;
    }

    ParserState::Text
}
