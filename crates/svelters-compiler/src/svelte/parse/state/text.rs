//! Port of `phases/1-parse/state/text.js` (Svelte 5.57.2 @ f3de3f2).

use crate::svelte::ast::{FragmentNode, Text};
use crate::svelte::parse::utils::html::decode_character_references;
use crate::svelte::parse::{Item, Parser};

pub fn text(parser: &mut Parser) {
    let start = parser.index;

    // `<` and `{` are ASCII: stepping bytes is equivalent to stepping UTF-16 units
    while parser.index < parser.template.len() && !parser.match_str("<") && !parser.match_str("{") {
        parser.index += 1;
    }

    let data = parser.template[start..parser.index].to_string();

    parser.append(Item::Leaf(FragmentNode::Text(Text {
        start: start as u32,
        end: parser.index as u32,
        data: decode_character_references(&data, false),
        raw: data,
    })));
}
