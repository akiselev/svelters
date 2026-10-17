//! Attribute helpers of `compiler/utils/ast.js` (Svelte 5.57.2 @ f3de3f2).

use crate::estree::Expression;
use crate::svelte::ast::{Attribute, AttributeValue, ExpressionTagNode, TextOrExpressionTag};

/// Returns true if the attribute contains a single static text node.
pub fn is_text_attribute(attribute: &Attribute) -> bool {
    matches!(&attribute.value, AttributeValue::Sequence(v) if v.len() == 1 && matches!(v[0], TextOrExpressionTag::Text(_)))
}

/// Returns true if the attribute contains a single expression node.
/// In Svelte 5, this also includes a single expression node wrapped in an array.
pub fn is_expression_attribute(attribute: &Attribute) -> bool {
    match &attribute.value {
        AttributeValue::Expression(_) => true,
        AttributeValue::Sequence(v) => v.len() == 1 && matches!(v[0], TextOrExpressionTag::ExpressionTag(_)),
        AttributeValue::True(_) => false,
    }
}

/// Returns the single attribute expression node (call only when `is_expression_attribute`).
pub fn get_attribute_expression(attribute: Attribute) -> Expression {
    match attribute.value {
        AttributeValue::Expression(ExpressionTagNode::ExpressionTag(t)) => t.expression,
        AttributeValue::Sequence(mut v) => match v.remove(0) {
            TextOrExpressionTag::ExpressionTag(t) => t.expression,
            TextOrExpressionTag::Text(_) => panic!("get_attribute_expression on a text attribute"),
        },
        AttributeValue::True(_) => panic!("get_attribute_expression on a boolean attribute"),
    }
}
