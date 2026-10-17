//! Port of `phases/1-parse/read/expression.js` (Svelte 5.57.2 @ f3de3f2).

use crate::estree::{Expression, Identifier, MemberExpression, PropertyKey};
use crate::svelte::parse::acorn::{parse_expression_at, remove_parens};
use crate::svelte::parse::shared::utils::{is_identifier_char, is_identifier_start, is_reserved};
use crate::svelte::parse::utils::bracket::find_matching_bracket;
use crate::svelte::parse::{PResult, Parser};

pub fn get_loose_identifier(parser: &mut Parser, opening_token: Option<char>) -> Option<Expression> {
    // Find the next } and treat it as the end of the expression
    let end = find_matching_bracket(&parser.template, parser.index, opening_token.unwrap_or('{'));
    // `if (end)`: 0 is falsy
    if let Some(end) = end.filter(|e| *e != 0) {
        let start = parser.index;
        parser.index = end;
        // We don't know what the expression is and signal this by returning an empty identifier
        return Some(Expression::Identifier(Identifier { start: start as u32, end: end as u32, name: String::new() }));
    }
    None
}

pub fn read_expression(parser: &mut Parser, opening_token: Option<char>, disallow_loose: bool) -> PResult<Expression> {
    if let Some(simple) = read_simple_expression(parser) {
        return Ok(simple);
    }

    let result = parse_expression_at(&parser.template, &mut parser.root.comments, parser.index);
    match result {
        Ok(node) => {
            use crate::estree::Spanned;
            let mut index = node.span().end as usize;

            if let Some(last_comment) = parser.root.comments.last()
                && last_comment.end as usize > index
            {
                index = last_comment.end as usize;
            }

            parser.index = index;

            Ok(remove_parens(node))
        }
        Err(err) => {
            // If we are in an each loop we need the error to be thrown in cases like
            // `as { y = z }` so we still throw and handle the error there
            if parser.loose
                && !disallow_loose
                && let Some(expression) = get_loose_identifier(parser, opening_token)
            {
                return Ok(expression);
            }

            Err(err)
        }
    }
}

/// Most template expressions are an identifier or a `a.b.c` member chain followed by `}`.
/// Those are built directly for better parse performance, with the same shape acorn would produce; anything else goes to acorn
fn read_simple_expression(parser: &mut Parser) -> Option<Expression> {
    if !parser.has_lf_line_breaks_only() {
        return None;
    }

    let index = parser.index;

    parser.allow_whitespace();
    let start = parser.index;

    let template = &parser.template;
    let mut end = read_word(template, start);
    if end.is_none_or(|end| is_reserved(&template[start..end])) {
        parser.index = index;
        return None;
    }
    let mut end_ = end.unwrap();

    let mut node = identifier(template, start, end_);

    while template.as_bytes().get(end_) == Some(&b'.') {
        end = read_word(template, end_ + 1);
        let Some(property_end) = end else {
            parser.index = index;
            return None;
        };

        node = Expression::MemberExpression(MemberExpression {
            start: start as u32,
            end: property_end as u32,
            object: Box::new(node),
            property: PropertyKey::Expression(Box::new(identifier(template, end_ + 1, property_end))),
            computed: false,
            optional: false,
        });

        end_ = property_end;
    }

    parser.index = end_;
    parser.allow_whitespace();

    if !parser.match_str("}") {
        parser.index = index;
        return None;
    }

    parser.index = end_;
    Some(node)
}

/// @returns the end of the identifier starting at `start`, or -1 (`None`)
fn read_word(template: &str, start: usize) -> Option<usize> {
    if start >= template.len() {
        return None;
    }

    let code = template[start..].chars().next()?;
    if !is_identifier_start(code) {
        return None;
    }

    let mut end = start + code.len_utf8();

    while end < template.len() {
        let code = template[end..].chars().next().unwrap();
        if !is_identifier_char(code) {
            break;
        }
        end += code.len_utf8();
    }

    Some(end)
}

fn identifier(template: &str, start: usize, end: usize) -> Expression {
    Expression::Identifier(Identifier { start: start as u32, end: end as u32, name: template[start..end].to_string() })
}
