//! Port of `phases/1-parse/read/context.js` (Svelte 5.57.2 @ f3de3f2).

use crate::estree::{Expression, Pattern, Spanned};
use crate::svelte::errors as e;
use crate::svelte::parse::acorn::{parse_expression_at, remove_parens};
use crate::svelte::parse::utils::bracket::{DEFAULT_BRACKETS, match_bracket};
use crate::svelte::parse::utils::whitespace::is_js_regex_space;
use crate::svelte::parse::{PResult, Parser};

pub fn read_pattern(parser: &mut Parser) -> PResult<Pattern> {
    let start = parser.index;
    let mut i = parser.index;

    let id = parser.read_identifier()?;

    if !id.name.is_empty() {
        // SVELTERS: the `typeAnnotation` (TSTypeAnnotation) is not stored on the pattern;
        // only its effect on the parser position is kept (TypeScript is not supported).
        let _annotation = read_type_annotation(parser)?;

        return Ok(Pattern::Identifier(id));
    }

    let char = parser.byte_at(i);

    if char != Some(b'{') && char != Some(b'[') {
        return Err(e::expected_pattern(i));
    }

    i = match_bracket(parser, start, DEFAULT_BRACKETS)?;
    parser.index = i;

    // acorn never reads before `start`, so the template itself can serve as the prefix
    let source = format!("{} = 1", &parser.template[..i]);
    let expression = remove_parens(parse_expression_at(&source, &mut parser.root.comments, start)?);

    let Expression::AssignmentExpression(assignment) = expression else {
        // upstream reads `.left` of whatever acorn returned; `{...} = 1` / `[...] = 1` is always
        // an assignment, so this is unreachable for any input acorn accepts
        return Err(e::expected_pattern(start));
    };
    let mut expression = assignment.left;

    if let Some((_, end)) = read_type_annotation(parser)? {
        set_pattern_end(&mut expression, end as u32);
    }

    Ok(expression)
}

fn set_pattern_end(p: &mut Pattern, end: u32) {
    match p {
        Pattern::Identifier(n) => n.end = end,
        Pattern::ObjectPattern(n) => n.end = end,
        Pattern::ArrayPattern(n) => n.end = end,
        Pattern::RestElement(n) => n.end = end,
        Pattern::AssignmentPattern(n) => n.end = end,
        Pattern::MemberExpression(n) => n.end = end,
        Pattern::ParenthesizedExpression(n) => n.end = end,
    }
}

/// `/\?\s*:/g` replaced by `:`
fn replace_optional_colons(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(q) = rest.find('?') {
        out.push_str(&rest[..q]);
        let after = &rest[q + 1..];
        let ws: usize = after.chars().take_while(|c| is_js_regex_space(*c)).map(char::len_utf8).sum();
        if after[ws..].starts_with(':') {
            out.push(':');
            rest = &after[ws + 1..];
        } else {
            out.push('?');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// Returns the `TSTypeAnnotation`'s (start, end), if there is one.
fn read_type_annotation(parser: &mut Parser) -> PResult<Option<(usize, usize)>> {
    let start = parser.index;
    parser.allow_whitespace();

    if !parser.eat_opt(":") {
        parser.index = start;
        return Ok(None);
    }

    // we need to trick Acorn into parsing the type annotation
    let insert = "_ as ";
    // SVELTERS: upstream steps back 5 UTF-16 units; with byte offsets we step back 5 bytes
    // (snapped to a character boundary), identical whenever the preceding text is ASCII.
    let mut a = parser.index.saturating_sub(insert.len());
    while !parser.template.is_char_boundary(a) {
        a -= 1;
    }
    // If this is a type annotation for a function parameter, Acorn-TS will treat subsequent
    // parameters as part of a sequence expression instead, and will then error on optional
    // parameters (`?:`). Therefore replace that sequence with something that will not error.
    let template = format!("{}{}{}", &parser.template[..a], insert, replace_optional_colons(&parser.template[parser.index..]));
    let mut expression = remove_parens(parse_expression_at(&template, &mut parser.root.comments, a)?);

    // `foo: bar = baz` gets mangled — fix it
    if let Expression::AssignmentExpression(assignment) = &expression {
        let mut b = assignment.right.span().start as usize;
        while template.as_bytes()[b] != b'=' {
            b -= 1;
        }
        expression = remove_parens(parse_expression_at(&template[..b], &mut parser.root.comments, a)?);
    }

    // `array as item: string, index` becomes `string, index`, which is mistaken as a sequence expression - fix that
    if let Expression::SequenceExpression(mut seq) = expression {
        expression = seq.expressions.remove(0);
    }

    parser.index = expression.span().end as usize;
    Ok(Some((start, parser.index)))
}
