//! JavaScript generation: output writer with source maps and span edits.

pub mod component;
pub mod edit;
pub mod writer;

use crate::estree::{BinaryLeft, Expression, Pattern};

/// Whether `e` must be parenthesized as the body of an arrow (`() => e`): it starts with `{`
/// (an object literal, also as the leftmost operand: `{ a }.a`, `{ a } ?? b`), which would be
/// read as a block, or it is a sequence, whose commas would end the arrow inside a call.
pub fn arrow_body_needs_parens(e: &Expression) -> bool {
    matches!(e, Expression::SequenceExpression(_)) || starts_with_brace(e)
}

fn starts_with_brace(e: &Expression) -> bool {
    match e {
        Expression::ObjectExpression(_) => true,
        Expression::MemberExpression(m) => starts_with_brace(&m.object),
        Expression::CallExpression(c) => starts_with_brace(&c.callee),
        Expression::TaggedTemplateExpression(t) => starts_with_brace(&t.tag),
        Expression::ChainExpression(c) => starts_with_brace(&c.expression),
        Expression::BinaryExpression(b) => matches!(&b.left, BinaryLeft::Expression(l) if starts_with_brace(l)),
        Expression::LogicalExpression(l) => starts_with_brace(&l.left),
        Expression::ConditionalExpression(c) => starts_with_brace(&c.test),
        Expression::UpdateExpression(u) => !u.prefix && starts_with_brace(&u.argument),
        Expression::AssignmentExpression(a) => matches!(a.left, Pattern::ObjectPattern(_)),
        Expression::SequenceExpression(s) => s.expressions.first().is_some_and(starts_with_brace),
        _ => false,
    }
}

/// The JS text `body` as the body of an arrow: parenthesized when it starts with `{`, which
/// would otherwise be read as a block.
pub fn arrow_body(body: &str) -> std::borrow::Cow<'_, str> {
    if body.trim_start().starts_with('{') { format!("({body})").into() } else { body.into() }
}
