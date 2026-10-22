//! SVELTERS: replaces `phases/1-parse/read/style.js` (Svelte 5.57.2 @ f3de3f2).
//!
//! The top-level `<style>` block is kept raw (`ast::StyleBlock`: attributes + content span +
//! text); its CSS AST is rebuilt on demand by `crate::svelte::css` (Q3), whose parser is the port
//! of upstream's. In HTML syntax (`!native`) that parser runs here, as upstream's does: CSS syntax
//! errors are parse errors and the content ends where upstream's does.
//!
//! Differences from upstream's `read_style`: in native syntax the content ends at the first
//! `</style` and no CSS syntax errors are reported (native `<style>` blocks are parsed by the
//! native style subset parser, `crate::style`); `content.comment` (the HTML comment before the
//! block) is not kept (`crate::svelte::css` finds it again).

use crate::svelte::ast::{Attribute, StyleBlock, StyleContent};
use crate::svelte::parse::utils::whitespace::is_js_regex_space;
use crate::svelte::parse::{PResult, Parser};

/// `/\s*>/y`
fn regex_whitespace_then_gt(s: &str, at: usize) -> Option<usize> {
    let rest = s.get(at..)?;
    let ws: usize = rest.chars().take_while(|c| is_js_regex_space(*c)).map(char::len_utf8).sum();
    rest[ws..].starts_with('>').then_some(ws + 1)
}

pub fn read_style(parser: &mut Parser, start: usize, attributes: Vec<Attribute>) -> PResult<StyleBlock> {
    let content_start = parser.index;
    let content_end = if parser.native {
        parser.template[content_start..].find("</style").map_or(parser.template.len(), |i| content_start + i)
    } else {
        crate::svelte::css::parse::parse_stylesheet(&parser.template, start, content_start)?.content_end
    };
    parser.index = content_end;

    parser.eat_required("</style")?;
    parser.read(regex_whitespace_then_gt);

    Ok(StyleBlock {
        start: start as u32,
        end: parser.index as u32,
        attributes,
        content: StyleContent {
            start: content_start as u32,
            end: content_end as u32,
            styles: parser.template[content_start..content_end].to_string(),
        },
    })
}
