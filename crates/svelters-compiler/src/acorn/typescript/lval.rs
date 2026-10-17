//! acorn-typescript 1.0.13: overrides of acorn's `lval.js` methods (binding lists, parameter
//! properties, `toAssignable`, `isAssignable`).

use super::err;
use super::types::{ModifierError, Modified};
use crate::acorn::lval::ParamItem;
use crate::acorn::parseutil::DestructuringErrors;
use crate::acorn::state::{PResult, Parser};
use crate::acorn::tokentype::TokenType as T;
use crate::estree::*;

/// The acorn-typescript properties of a binding pattern, if it can carry them.
pub fn pattern_ts_mut(p: &mut Pattern) -> Option<&mut TsPatternExtra> {
    let ts = match p {
        Pattern::Identifier(n) => &mut n.ts,
        Pattern::ObjectPattern(n) => &mut n.ts,
        Pattern::ArrayPattern(n) => &mut n.ts,
        Pattern::RestElement(n) => &mut n.ts,
        Pattern::AssignmentPattern(n) => &mut n.ts,
        _ => return None,
    };
    Some(ts.get_or_insert_with(Box::default))
}

pub fn pattern_ts(p: &Pattern) -> Option<&TsPatternExtra> {
    match p {
        Pattern::Identifier(n) => n.ts.as_deref(),
        Pattern::ObjectPattern(n) => n.ts.as_deref(),
        Pattern::ArrayPattern(n) => n.ts.as_deref(),
        Pattern::RestElement(n) => n.ts.as_deref(),
        Pattern::AssignmentPattern(n) => n.ts.as_deref(),
        _ => None,
    }
}

/// `resetEndLocation(node, end)` on a pattern.
pub fn set_pattern_end(p: &mut Pattern, end: u32) {
    match p {
        Pattern::Identifier(n) => n.end = end,
        Pattern::ObjectPattern(n) => n.end = end,
        Pattern::ArrayPattern(n) => n.end = end,
        Pattern::RestElement(n) => n.end = end,
        Pattern::AssignmentPattern(n) => n.end = end,
        Pattern::MemberExpression(n) => n.end = end,
        Pattern::ParenthesizedExpression(n) => n.end = end,
        Pattern::TSParameterProperty(n) => n.end = end,
        Pattern::TSAsExpression(n)
        | Pattern::TSSatisfiesExpression(n)
        | Pattern::TSTypeAssertion(n)
        | Pattern::TSNonNullExpression(n) => n.end = end,
    }
}

/// The acorn-typescript properties of an expression that may become a binding pattern
/// (`parseParenItem` / `typeCastToParameter` set them before `toAssignable`).
pub fn expression_ts_mut(e: &mut Expression) -> Option<&mut TsPatternExtra> {
    let ts = match e {
        Expression::Identifier(n) => &mut n.ts,
        Expression::ObjectExpression(n) => &mut n.ts,
        Expression::ArrayExpression(n) => &mut n.ts,
        _ => return None,
    };
    Some(ts.get_or_insert_with(Box::default))
}

/// `resetEndLocation(node, end)` on an expression (the node kinds `typeCastToParameter` and
/// `parseParenItem` can reach).
pub fn set_expression_end(e: &mut Expression, end: u32) {
    match e {
        Expression::Identifier(n) => n.end = end,
        Expression::ObjectExpression(n) => n.end = end,
        Expression::ArrayExpression(n) => n.end = end,
        Expression::AssignmentExpression(n) => n.end = end,
        Expression::MemberExpression(n) => n.end = end,
        Expression::ParenthesizedExpression(n) => n.end = end,
        Expression::TSAsExpression(n) | Expression::TSSatisfiesExpression(n) => n.end = end,
        Expression::TSTypeAssertion(n) => n.end = end,
        Expression::TSNonNullExpression(n) => n.end = end,
        Expression::TSTypeCastExpression(n) => n.end = end,
        _ => {}
    }
}

/// A pattern that is one of the TypeScript wrappers, as the expression it was parsed from
/// (to sit inside a `ParenthesizedExpression` pattern, like acorn's paren handling).
fn ts_pattern_to_expression(p: Pattern) -> Result<Expression, Pattern> {
    match p {
        Pattern::Identifier(i) => Ok(Expression::Identifier(i)),
        Pattern::MemberExpression(m) => Ok(Expression::MemberExpression(*m)),
        Pattern::ParenthesizedExpression(p) => Ok(Expression::ParenthesizedExpression(p)),
        Pattern::TSAsExpression(w) | Pattern::TSSatisfiesExpression(w) | Pattern::TSTypeAssertion(w) | Pattern::TSNonNullExpression(w)
            if !matches!(*w.expression, Pattern::ObjectPattern(_) | Pattern::ArrayPattern(_)) =>
        {
            // unreachable in practice; keep the wrapper's inner target
            ts_pattern_to_expression(*w.expression)
        }
        other => Err(other),
    }
}

impl<'a> Parser<'a> {
    /// `parseMaybeDefault(startPos, startLoc, left)`
    pub fn parse_maybe_default_ts(&mut self, start_pos: u32, left: Option<Pattern>) -> PResult<Pattern> {
        let node = self.parse_maybe_default_base(start_pos, left)?;
        if let Pattern::AssignmentPattern(a) = &node {
            if let Some(ta) = a.ts.as_ref().and_then(|t| t.type_annotation.as_ref()) {
                if a.right.span().start < ta.start {
                    return self.raise(ta.start, err::TYPE_ANNOTATION_AFTER_ASSIGN);
                }
            }
        }
        Ok(node)
    }

    /// `typeCastToParameter(node)`: the cast expression, carrying the annotation.
    pub(crate) fn type_cast_to_parameter(&mut self, node: TSTypeCastExpression) -> Expression {
        let mut expression = *node.expression;
        let end = node.type_annotation.end;
        if let Some(ts) = expression_ts_mut(&mut expression) {
            ts.type_annotation = Some(node.type_annotation);
        }
        set_expression_end(&mut expression, end);
        expression
    }

    /// `toAssignableList` pre-pass: `typeCastToParameter` on every TSTypeCastExpression.
    pub(crate) fn ts_unwrap_type_casts(&mut self, list: &mut [ParamItem]) {
        for item in list.iter_mut() {
            if let ParamItem::Expr(Expression::TSTypeCastExpression(_)) = item {
                let ParamItem::Expr(Expression::TSTypeCastExpression(tc)) =
                    std::mem::replace(item, ParamItem::Expr(Expression::ThisExpression(ThisExpression { start: 0, end: 0 })))
                else {
                    unreachable!()
                };
                *item = ParamItem::Expr(self.type_cast_to_parameter(tc));
            }
        }
    }

    pub(crate) fn ts_unwrap_type_casts_in_elements(&mut self, list: &mut [Option<ArrayElement>]) {
        for item in list.iter_mut() {
            if let Some(ArrayElement::Expression(e @ Expression::TSTypeCastExpression(_))) = item {
                let Expression::TSTypeCastExpression(tc) =
                    std::mem::replace(e, Expression::ThisExpression(ThisExpression { start: 0, end: 0 }))
                else {
                    unreachable!()
                };
                *e = self.type_cast_to_parameter(tc);
            }
        }
    }

    /// `parseAssignableListItem(allowModifiers)`
    pub fn parse_assignable_list_item_ts(&mut self, allow_modifiers: Option<bool>) -> PResult<Pattern> {
        let mut decorators = Vec::new();
        while self.type_ == T::At {
            decorators.push(self.parse_decorator()?);
        }
        let start_pos = self.start;
        let mut accessibility = None;
        let mut readonly = false;
        let mut is_override = false;
        if let Some(allow) = allow_modifiers {
            let mut modified = Modified::default();
            self.ts_parse_modifiers(
                &mut modified,
                &["public", "private", "protected", "override", "readonly"],
                &[],
                false,
                ModifierError::OnTypeMember,
            )?;
            accessibility = modified.accessibility;
            is_override = modified.is_override;
            readonly = modified.readonly;
            if !allow && (accessibility.is_some() || readonly || is_override) {
                return self.raise(self.column_of(start_pos), err::UNEXPECTED_PARAMETER_MODIFIER);
            }
        }
        let mut left = self.parse_maybe_default(start_pos, None)?;
        self.parse_binding_list_item_ts(&mut left)?;
        let left_start = left.span().start;
        let mut elt = self.parse_maybe_default(left_start, Some(left))?;
        if !decorators.is_empty() {
            if let Some(ts) = pattern_ts_mut(&mut elt) {
                ts.decorators = Some(decorators);
            }
        }
        if accessibility.is_some() || readonly || is_override {
            if !matches!(elt, Pattern::Identifier(_) | Pattern::AssignmentPattern(_)) {
                return self.raise(start_pos, err::UNSUPPORTED_PARAMETER_PROPERTY_KIND);
            }
            return Ok(Pattern::TSParameterProperty(TSParameterProperty {
                start: start_pos,
                end: self.last_tok_end,
                accessibility,
                readonly,
                is_override,
                parameter: Box::new(elt),
            }));
        }
        Ok(elt)
    }

    /// `parseBindingListItem(param)`: allow type annotations inside of a parameter list.
    pub fn parse_binding_list_item_ts(&mut self, param: &mut Pattern) -> PResult<()> {
        if self.eat(T::Question)? {
            if !matches!(param, Pattern::Identifier(_)) && !self.is_ambient_context() && !self.in_type() {
                return self.raise(param.span().start, err::PATTERN_IS_OPTIONAL);
            }
            if let Some(ts) = pattern_ts_mut(param) {
                ts.optional = true;
            }
        }
        if let Some(type_) = self.ts_try_parse_type_annotation()? {
            if let Some(ts) = pattern_ts_mut(param) {
                ts.type_annotation = Some(type_);
            }
        }
        set_pattern_end(param, self.last_tok_end);
        Ok(())
    }

    /// `parseBindingAtom()`: `this` is a valid binding (for `this` parameters).
    pub fn parse_binding_atom_ts(&mut self) -> PResult<Option<Pattern>> {
        if self.type_ == T::This {
            return Ok(Some(Pattern::Identifier(self.parse_ident(/* liberal */ true)?)));
        }
        Ok(None)
    }

    /// `isAssignable(node, isBinding)`
    pub(crate) fn is_assignable_expr(&self, node: &Expression, is_binding: bool) -> bool {
        match node {
            Expression::TSTypeCastExpression(n) => self.is_assignable_expr(&n.expression, is_binding),
            Expression::Identifier(_) => true,
            Expression::ObjectExpression(o) => {
                let last = o.properties.len().wrapping_sub(1);
                o.properties.iter().enumerate().all(|(i, prop)| match prop {
                    ObjectMember::SpreadElement(s) => i == last && self.is_assignable_expr(&s.argument, false),
                    ObjectMember::Property(p) => self.is_assignable_expr(&p.value, false),
                })
            }
            Expression::ArrayExpression(a) => a.elements.iter().all(|e| match e {
                None => true,
                Some(ArrayElement::Expression(e)) => self.is_assignable_expr(e, false),
                Some(ArrayElement::Spread(SpreadElementNode::SpreadElement(s))) => self.is_assignable_expr(&s.argument, false),
            }),
            Expression::AssignmentExpression(a) => a.operator == AssignmentOperator::Assign,
            Expression::ParenthesizedExpression(p) => self.is_assignable_expr(&p.expression, is_binding),
            Expression::MemberExpression(_) => !is_binding,
            _ => false,
        }
    }

    pub(crate) fn is_assignable_item(&self, node: &ParamItem, is_binding: bool) -> bool {
        match node {
            ParamItem::Expr(e) => self.is_assignable_expr(e, is_binding),
            ParamItem::Spread(s) => self.is_assignable_expr(&s.argument, is_binding),
            ParamItem::Rest(_) | ParamItem::Pat(_) => true,
        }
    }

    /// `toAssignable(node, isBinding, refDestructuringErrors, preserveTypeScriptWrapper)`
    pub fn to_assignable_ts(
        &mut self,
        node: Expression,
        is_binding: bool,
        ref_destructuring_errors: Option<&mut DestructuringErrors>,
        preserve_type_script_wrapper: bool,
    ) -> PResult<Pattern> {
        match node {
            Expression::ParenthesizedExpression(p) => {
                self.to_assignable_parenthesized_expression(p, is_binding, ref_destructuring_errors, preserve_type_script_wrapper)
            }
            Expression::TSAsExpression(_)
            | Expression::TSSatisfiesExpression(_)
            | Expression::TSNonNullExpression(_)
            | Expression::TSTypeAssertion(_) => {
                if !is_binding {
                    return self.raise(node.span().start, err::UNEXPECTED_TYPE_CAST_IN_PARAMETER);
                }
                let (start, end, inner, type_annotation, kind) = match node {
                    Expression::TSAsExpression(n) => (n.start, n.end, n.expression, Some(n.type_annotation), 0),
                    Expression::TSSatisfiesExpression(n) => (n.start, n.end, n.expression, Some(n.type_annotation), 1),
                    Expression::TSTypeAssertion(n) => (n.start, n.end, n.expression, Some(n.type_annotation), 2),
                    Expression::TSNonNullExpression(n) => (n.start, n.end, n.expression, None, 3),
                    _ => unreachable!(),
                };
                let expression =
                    self.to_assignable_ts(*inner, is_binding, ref_destructuring_errors, preserve_type_script_wrapper)?;
                // SVELTERS: without `preserveTypeScriptWrapper` the plugin returns the inner
                // pattern, but every caller that reaches this with `isBinding` (acorn's
                // `toAssignableList`, the `Property` / `AssignmentExpression` / `SpreadElement`
                // cases, `toAssignableList` of arrow parameters) discards the return value: the
                // wrapper stays in the tree with its expression converted in place.
                let _ = preserve_type_script_wrapper;
                let w = TSPatternWrapper { start, end, expression: Box::new(expression), type_annotation };
                Ok(match kind {
                    0 => Pattern::TSAsExpression(w),
                    1 => Pattern::TSSatisfiesExpression(w),
                    2 => Pattern::TSTypeAssertion(w),
                    _ => Pattern::TSNonNullExpression(w),
                })
            }
            Expression::MemberExpression(m) => Ok(Pattern::MemberExpression(Box::new(m))),
            Expression::TSTypeCastExpression(tc) => {
                // SVELTERS: the plugin returns the cast expression itself (`typeCastToParameter`);
                // here it is converted to the pattern it stands for.
                let annotation = tc.type_annotation.clone();
                let end = annotation.end;
                let expr = *tc.expression;
                let mut pattern = self.to_assignable_base(expr, is_binding, None)?;
                if let Some(ts) = pattern_ts_mut(&mut pattern) {
                    ts.type_annotation = Some(annotation);
                }
                set_pattern_end(&mut pattern, end);
                Ok(pattern)
            }
            other => self.to_assignable_base(other, is_binding, ref_destructuring_errors),
        }
    }

    /// `toAssignableParenthesizedExpression(node, isBinding, refDestructuringErrors, preserve)`
    fn to_assignable_parenthesized_expression(
        &mut self,
        node: ParenthesizedExpression,
        is_binding: bool,
        ref_destructuring_errors: Option<&mut DestructuringErrors>,
        preserve_type_script_wrapper: bool,
    ) -> PResult<Pattern> {
        match &*node.expression {
            Expression::TSAsExpression(_)
            | Expression::TSSatisfiesExpression(_)
            | Expression::TSNonNullExpression(_)
            | Expression::TSTypeAssertion(_)
            | Expression::ParenthesizedExpression(_) => {
                let (start, end) = (node.start, node.end);
                let expression = self.to_assignable_ts(
                    *node.expression,
                    is_binding,
                    ref_destructuring_errors,
                    preserve_type_script_wrapper,
                )?;
                if preserve_type_script_wrapper {
                    return match ts_pattern_to_expression(expression) {
                        Ok(e) => Ok(Pattern::ParenthesizedExpression(ParenthesizedExpression {
                            start,
                            end,
                            expression: Box::new(e),
                        })),
                        Err(p) => self.raise(p.span().start, "Assigning to rvalue"),
                    };
                }
                Ok(expression)
            }
            _ => self.to_assignable_base(Expression::ParenthesizedExpression(node), is_binding, ref_destructuring_errors),
        }
    }

    /// `checkLValInnerPattern` for `TSParameterProperty`: checks its parameter.
    pub(crate) fn ts_parameter_property_inner<'p>(&self, expr: &'p Pattern) -> Option<&'p Pattern> {
        match expr {
            Pattern::TSParameterProperty(p) => Some(&p.parameter),
            _ => None,
        }
    }
}
