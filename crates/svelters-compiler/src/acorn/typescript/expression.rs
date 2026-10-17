//! acorn-typescript 1.0.13: overrides of acorn's `expression.js` methods.

use super::err;
use super::lval::{expression_ts_mut, set_expression_end};
use super::types::TypeParamModifiers;
use super::{token_is_identifier, token_is_keyword_or_identifier, TryParse};
use crate::acorn::expression::{FnBody, ForInit, Operand, PropResult};
use crate::acorn::lval::ParamItem;
use crate::acorn::parseutil::DestructuringErrors;
use crate::acorn::scopeflags::*;
use crate::acorn::state::{PResult, Parser, TokVal};
use crate::acorn::tokentype::TokenType as T;
use crate::estree::*;

/// Where a function body without braces may end the function (`tsConfig` of
/// `parseFunctionBody`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Bodiless {
    /// `isFunctionDeclaration`: `TSDeclareFunction`
    FunctionDeclaration,
    /// `isClassMethod`: `TSDeclareMethod`
    ClassMethod,
}

fn function_flags(is_async: bool, generator: bool) -> u32 {
    crate::acorn::scopeflags::function_flags(is_async, generator)
}

fn expr_list_to_params(expr_list: Vec<Option<ArrayElement>>) -> Vec<ParamItem> {
    expr_list
        .into_iter()
        .flatten()
        .map(|e| match e {
            ArrayElement::Expression(e) => ParamItem::Expr(e),
            ArrayElement::Spread(SpreadElementNode::SpreadElement(s)) => ParamItem::Spread(s),
        })
        .collect()
}

/// An expression in assignment-target position whose `toAssignable` was skipped (compound
/// assignment): the same node as a `Pattern`.
#[allow(clippy::result_large_err)]
fn simple_target_pattern(e: Expression) -> Result<Pattern, Expression> {
    match e {
        Expression::Identifier(i) => Ok(Pattern::Identifier(i)),
        Expression::MemberExpression(m) => Ok(Pattern::MemberExpression(Box::new(m))),
        Expression::ParenthesizedExpression(p) => Ok(Pattern::ParenthesizedExpression(p)),
        Expression::TSAsExpression(n) => wrap(n.start, n.end, *n.expression, Some(n.type_annotation), Pattern::TSAsExpression),
        Expression::TSSatisfiesExpression(n) => {
            wrap(n.start, n.end, *n.expression, Some(n.type_annotation), Pattern::TSSatisfiesExpression)
        }
        Expression::TSTypeAssertion(n) => wrap(n.start, n.end, *n.expression, Some(n.type_annotation), Pattern::TSTypeAssertion),
        Expression::TSNonNullExpression(n) => wrap(n.start, n.end, *n.expression, None, Pattern::TSNonNullExpression),
        other => Err(other),
    }
}

#[allow(clippy::result_large_err)]
fn wrap(
    start: u32,
    end: u32,
    inner: Expression,
    type_annotation: Option<Box<TSType>>,
    f: fn(TSPatternWrapper) -> Pattern,
) -> Result<Pattern, Expression> {
    match simple_target_pattern(inner) {
        Ok(p) => Ok(f(TSPatternWrapper { start, end, expression: Box::new(p), type_annotation })),
        Err(e) => Err(e),
    }
}

/// `checkKeyName(node, name)` on a property key.
pub fn key_is(computed: bool, key: &PropertyKey, name: &str) -> bool {
    !computed
        && match key {
            PropertyKey::Expression(e) => match &**e {
                Expression::Identifier(i) => i.name == name,
                Expression::Literal(Literal { value: LiteralValue::String(s), .. }) => s == name,
                _ => false,
            },
            _ => false,
        }
}

impl<'a> Parser<'a> {
    // --- parseIdentNode

    /// `parseIdentNode()`: TypeScript keywords (and keywords) are identifiers; `None` falls
    /// back to acorn's.
    pub fn parse_ident_node_ts(&mut self) -> Option<(u32, String)> {
        let consumes_keyword_context = (self.type_ == T::Class || self.type_ == T::Function)
            && (self.last_tok_end != self.last_tok_start + 1
                || self.input.as_bytes().get(self.last_tok_start as usize) != Some(&b'.'));
        if token_is_keyword_or_identifier(self.type_) && !consumes_keyword_context {
            let name = match std::mem::take(&mut self.value) {
                TokVal::Str(s) => s,
                _ => self.type_.keyword().unwrap_or("").to_string(),
            };
            return Some((self.start, name));
        }
        None
    }

    // --- unary / binary

    /// `tsParseTypeAssertion()` (called by `parseMaybeUnary` on `<`)
    pub(crate) fn ts_parse_type_assertion(&mut self) -> PResult<Expression> {
        let result = self.try_parse(|p| {
            let start = p.start;
            let const_ = p.ts_try_next_parse_constant_context()?;
            let type_annotation = match const_ {
                Some(c) => c,
                None => p.ts_next_then_parse_type()?,
            };
            p.expect(T::Relational)?;
            let expression = match p.parse_maybe_unary(None, false, false, ForInit::No)? {
                Operand::Expr(e) => e,
                Operand::Private(pi) => return p.unexpected(Some(pi.start)),
            };
            Ok(Some(Expression::TSTypeAssertion(TSTypeAssertion {
                start,
                end: p.last_tok_end,
                type_annotation: Box::new(type_annotation),
                expression: Box::new(expression),
            })))
        })?;
        match result {
            TryParse::Ok(n) => Ok(n),
            _ => {
                // The plugin returns `this.tsParseTypeParameters(this.tsParseConstModifier)`, a
                // TSTypeParameterDeclaration, as the expression. SVELTERS: that node cannot be an
                // `Expression`; parse it and report the token after it.
                self.ts_parse_type_parameters(TypeParamModifiers::Const)?;
                self.unexpected(None)
            }
        }
    }

    /// The `parseExprOp` override: `as` / `satisfies` at the current position.
    pub(crate) fn ts_at_as_expression(&self, min_prec: i32) -> bool {
        T::In.binop().unwrap() > min_prec
            && !self.has_preceding_line_break()
            && (self.is_contextual("as") || self.is_contextual("satisfies"))
    }

    pub(crate) fn ts_parse_as_expression(&mut self, left: Expression, left_start_pos: u32) -> PResult<Expression> {
        let satisfies = self.is_contextual("satisfies");
        let const_ = self.ts_try_next_parse_constant_context()?;
        let type_annotation = match const_ {
            Some(c) => c,
            None => self.ts_next_then_parse_type()?,
        };
        let node = TSAsExpression {
            start: left_start_pos,
            end: self.last_tok_end,
            expression: Box::new(left),
            type_annotation: Box::new(type_annotation),
        };
        self.re_scan_lt_gt()?;
        Ok(if satisfies { Expression::TSSatisfiesExpression(node) } else { Expression::TSAsExpression(node) })
    }

    // --- assignment, conditional

    /// `parseMaybeAssign(forInit, refExpressionErrors, afterLeftParse)`; `after_left_parse`:
    /// `parseParenItem` is the `afterLeftParse` callback.
    pub(crate) fn parse_maybe_assign_ts(
        &mut self,
        for_init: ForInit,
        mut ref_expression_errors: Option<&mut DestructuringErrors>,
        after_left_parse: bool,
    ) -> PResult<Expression> {
        if !self.ts_match_left_relational() {
            return self.parse_maybe_assign_origin(for_init, ref_expression_errors, after_left_parse);
        }
        let arrow = self.try_parse(|p| {
            let type_parameters = p.ts_parse_type_parameters(TypeParamModifiers::Const)?;
            let expr = p.parse_maybe_assign_origin(for_init, ref_expression_errors.as_deref_mut(), after_left_parse)?;
            let Expression::ArrowFunctionExpression(mut expr) = expr else { return Ok(None) };
            if !type_parameters.params.is_empty() {
                expr.start = type_parameters.start;
            }
            expr.ts.get_or_insert_with(Box::default).type_parameters = Some(type_parameters);
            Ok(Some(Expression::ArrowFunctionExpression(expr)))
        })?;
        let arrow_thrown = match arrow {
            TryParse::Ok(n) => return Ok(n),
            TryParse::Aborted(_) => None,
            TryParse::Thrown(e, events) => Some((e, events)),
        };
        let type_cast = self.try_parse(|p| {
            p.parse_maybe_assign_origin(for_init, ref_expression_errors, after_left_parse).map(Some)
        })?;
        let type_cast_thrown = match type_cast {
            TryParse::Ok(n) => return Ok(n),
            TryParse::Aborted(_) => None,
            TryParse::Thrown(e, events) => Some((e, events)),
        };
        if let Some((e, events)) = arrow_thrown {
            self.select_try_parse_result(events);
            return Err(e);
        }
        if let Some((e, events)) = type_cast_thrown {
            self.select_try_parse_result(events);
            return Err(e);
        }
        unreachable!("tryParse results")
    }

    /// `parseMaybeAssignOrigin(forInit, refDestructuringErrors, afterLeftParse)`
    fn parse_maybe_assign_origin(
        &mut self,
        for_init: ForInit,
        ref_destructuring_errors: Option<&mut DestructuringErrors>,
        after_left_parse: bool,
    ) -> PResult<Expression> {
        if self.is_contextual("yield") {
            if self.in_generator() {
                return self.parse_yield(for_init);
            } else {
                self.expr_allowed = false;
            }
        }
        let mut own_destructuring_errors = false;
        let mut old_paren_assign = -1;
        let mut old_trailing_comma = -1;
        let mut old_double_proto = -1;
        let mut own = DestructuringErrors::new();
        let r: &mut DestructuringErrors = match ref_destructuring_errors {
            Some(r) => {
                old_paren_assign = r.parenthesized_assign;
                old_trailing_comma = r.trailing_comma;
                old_double_proto = r.double_proto;
                r.parenthesized_assign = -1;
                r.trailing_comma = -1;
                r
            }
            None => {
                own_destructuring_errors = true;
                &mut own
            }
        };
        let start_pos = self.start;
        if self.type_ == T::ParenL || token_is_identifier(self.type_) {
            self.potential_arrow_at = self.start as i64;
            self.potential_arrow_in_for_await = for_init == ForInit::Await;
        }
        let mut left = self.parse_maybe_conditional(for_init, Some(&mut *r))?;
        if after_left_parse {
            left = self.parse_paren_item(left)?;
        }
        if self.type_.is_assign() {
            let operator = AssignmentOperator::from_str(self.tok_text()).unwrap();
            let is_eq = self.type_ == T::Eq;
            let left_pat: Result<Pattern, Expression> =
                if is_eq { Ok(self.to_assignable_ts(left, true, Some(&mut *r), true)?) } else { Err(left) };
            if !own_destructuring_errors {
                r.parenthesized_assign = -1;
                r.trailing_comma = -1;
                r.double_proto = -1;
            }
            let left_start = match &left_pat {
                Ok(p) => p.span().start,
                Err(e) => e.span().start,
            } as i64;
            if r.shorthand_assign >= left_start {
                r.shorthand_assign = -1;
            }
            let maybe_in_arrow_parameters = self.tss().maybe_in_arrow_parameters;
            let left = match left_pat {
                Ok(p) => {
                    if !maybe_in_arrow_parameters {
                        self.check_lval_pattern(&p, BIND_NONE, None)?;
                    }
                    p
                }
                Err(e) => {
                    if !maybe_in_arrow_parameters {
                        self.check_lval_simple_expr(&e, BIND_NONE, None)?;
                    }
                    match simple_target_pattern(e) {
                        Ok(p) => p,
                        // SVELTERS: inside possible arrow parameters the plugin skips the check
                        // and keeps any expression as the target.
                        Err(other) => return self.raise(other.span().start, "Assigning to rvalue"),
                    }
                }
            };
            self.next()?;
            let right = self.parse_maybe_assign(for_init, None)?;
            if old_double_proto > -1 {
                r.double_proto = old_double_proto;
            }
            return Ok(Expression::AssignmentExpression(AssignmentExpression {
                start: start_pos,
                end: self.last_tok_end,
                operator,
                left,
                right: Box::new(right),
            }));
        } else if own_destructuring_errors {
            self.check_expression_errors(Some(r), true)?;
        }
        if old_paren_assign > -1 {
            r.parenthesized_assign = old_paren_assign;
        }
        if old_trailing_comma > -1 {
            r.trailing_comma = old_trailing_comma;
        }
        Ok(left)
    }

    /// `parseConditional(expr, startPos, startLoc, forInit, refDestructuringErrors)`
    fn parse_conditional(&mut self, expr: Expression, start_pos: u32, for_init: ForInit) -> PResult<Expression> {
        if self.eat(T::Question)? {
            let consequent = self.parse_maybe_assign(ForInit::No, None)?;
            self.expect(T::Colon)?;
            let alternate = self.parse_maybe_assign(for_init, None)?;
            return Ok(Expression::ConditionalExpression(ConditionalExpression {
                start: start_pos,
                end: self.last_tok_end,
                test: Box::new(expr),
                consequent: Box::new(consequent),
                alternate: Box::new(alternate),
            }));
        }
        Ok(expr)
    }

    /// `parseMaybeConditional(forInit, refDestructuringErrors)`
    pub fn parse_maybe_conditional_ts(
        &mut self,
        for_init: ForInit,
        mut ref_destructuring_errors: Option<&mut DestructuringErrors>,
    ) -> PResult<Expression> {
        let start_pos = self.start;
        let expr = self.parse_expr_ops(for_init, ref_destructuring_errors.as_deref_mut())?;
        if self.check_expression_errors(ref_destructuring_errors.as_deref(), false)? {
            return Ok(expr);
        }
        if !self.tss().maybe_in_arrow_parameters || self.type_ != T::Question {
            return self.parse_conditional(expr, start_pos, for_init);
        }
        let backup = expr.clone();
        match self.try_parse(|p| p.parse_conditional(expr, start_pos, for_init).map(Some))? {
            TryParse::Ok(n) => Ok(n),
            // `setOptionalParametersError` records a location acorn never reads
            _ => Ok(backup),
        }
    }

    /// `parseParenItem(node)`: `?` and a type annotation after a parenthesized item.
    pub(crate) fn parse_paren_item(&mut self, mut node: Expression) -> PResult<Expression> {
        let start_pos = self.start;
        if self.eat(T::Question)? {
            match &mut node {
                Expression::MemberExpression(m) => m.optional = true,
                Expression::CallExpression(c) => c.optional = true,
                other => {
                    if let Some(ts) = expression_ts_mut(other) {
                        ts.optional = true;
                    }
                }
            }
            set_expression_end(&mut node, self.last_tok_end);
        }
        if self.type_ == T::Colon {
            let type_annotation = self.ts_parse_type_annotation(true, None)?;
            return Ok(Expression::TSTypeCastExpression(TSTypeCastExpression {
                start: start_pos,
                end: self.last_tok_end,
                expression: Box::new(node),
                type_annotation,
            }));
        }
        Ok(node)
    }

    /// `parseParenItem` on the rest element of a parenthesized list. SVELTERS: the cast is
    /// applied right away (`typeCastToParameter` would do the same once it is an arrow
    /// parameter; otherwise the rest element is an error anyway).
    fn parse_paren_item_rest(&mut self, mut node: RestElement) -> PResult<RestElement> {
        if self.eat(T::Question)? {
            node.ts.get_or_insert_with(Box::default).optional = true;
            node.end = self.last_tok_end;
        }
        if self.type_ == T::Colon {
            let type_annotation = self.ts_parse_type_annotation(true, None)?;
            node.end = type_annotation.end;
            node.ts.get_or_insert_with(Box::default).type_annotation = Some(type_annotation);
        }
        Ok(node)
    }

    /// `parseYield(forInit)`
    pub fn parse_yield_ts(&mut self, for_init: ForInit) -> PResult<Expression> {
        if self.yield_pos == 0 {
            self.yield_pos = self.start;
        }
        let start = self.start;
        self.next()?;
        let starts_type_script_expression = self.ts_match_left_relational();
        let has_argument = self.type_ != T::Semi
            && !self.can_insert_semicolon()
            && (self.type_ == T::Star || self.type_.starts_expr() || starts_type_script_expression);
        if !has_argument {
            return Ok(Expression::YieldExpression(YieldExpression { start, end: self.last_tok_end, delegate: false, argument: None }));
        }
        let delegate = self.eat(T::Star)?;
        let argument = self.parse_maybe_assign(for_init, None)?;
        Ok(Expression::YieldExpression(YieldExpression {
            start,
            end: self.last_tok_end,
            delegate,
            argument: Some(Box::new(argument)),
        }))
    }

    // --- arrows

    /// `parseArrowExpression(node, params, isAsync, forInit)`; `extra` holds the node's
    /// `typeParameters`/`returnType` set by the caller.
    pub(crate) fn parse_arrow_expression_ts(
        &mut self,
        start: u32,
        params: Vec<ParamItem>,
        is_async: bool,
        for_init: ForInit,
        mut extra: TsFunctionExtra,
    ) -> PResult<Expression> {
        if self.type_ == T::Colon {
            extra.return_type = Some(self.ts_parse_type_annotation(true, None)?);
        }
        let old_yield_pos = self.yield_pos;
        let old_await_pos = self.await_pos;
        let old_await_ident_pos = self.await_ident_pos;
        self.enter_scope(function_flags(is_async, false) | SCOPE_ARROW);
        let old_maybe_in_arrow_parameters = self.tss().maybe_in_arrow_parameters;
        let is_async_node = self.options.ecma_version >= 8 && is_async;
        self.yield_pos = 0;
        self.await_pos = 0;
        self.await_ident_pos = 0;
        self.tsm().maybe_in_arrow_parameters = true;
        let params = self.to_assignable_params(params, true)?;
        self.tsm().maybe_in_arrow_parameters = false;
        let (return_type, body) = self.parse_function_body_ts(start, &params, None, true, false, for_init, None)?;
        if return_type.is_some() {
            extra.return_type = return_type;
        }
        self.yield_pos = old_yield_pos;
        self.await_pos = old_await_pos;
        self.await_ident_pos = old_await_ident_pos;
        self.tsm().maybe_in_arrow_parameters = old_maybe_in_arrow_parameters;
        let (expression, body) = match body.expect("arrow body") {
            FnBody::Expr(e) => (true, ArrowBody::Expression(Box::new(e))),
            FnBody::Block(b) => (false, ArrowBody::Block(BlockStatementNode::BlockStatement(b))),
        };
        let ts = if extra == TsFunctionExtra::default() { None } else { Some(Box::new(extra)) };
        Ok(Expression::ArrowFunctionExpression(ArrowFunctionExpression {
            start,
            end: self.last_tok_end,
            id: None,
            expression,
            generator: false,
            is_async: is_async_node,
            params,
            body,
            ts,
        }))
    }

    /// `shouldParseArrow(exprList)`
    fn should_parse_arrow_ts(&mut self, expr_list: &[ParamItem]) -> PResult<bool> {
        let should_parse_arrow_res = if self.type_ == T::Colon {
            expr_list.iter().all(|e| self.is_assignable_item(e, true))
        } else {
            !self.can_insert_semicolon()
        };
        if should_parse_arrow_res {
            if self.type_ == T::Colon {
                let result = self.try_parse(|p| {
                    let return_type = p.ts_parse_type_or_type_predicate_annotation(T::Colon)?;
                    if p.can_insert_semicolon() || p.type_ != T::Arrow {
                        return Ok(None);
                    }
                    Ok(Some(return_type))
                })?;
                match result {
                    TryParse::Aborted(_) => {
                        self.tsm().should_parse_arrow_return_type = None;
                        return Ok(false);
                    }
                    TryParse::Ok(n) => self.tsm().should_parse_arrow_return_type = Some(n),
                    TryParse::Thrown(..) => {}
                }
            }
            if self.type_ != T::Arrow {
                self.tsm().should_parse_arrow_return_type = None;
                return Ok(false);
            }
            return Ok(true);
        }
        self.tsm().should_parse_arrow_return_type = None;
        Ok(should_parse_arrow_res)
    }

    /// `parseParenArrowList(startPos, startLoc, exprList, forInit)`
    fn parse_paren_arrow_list_ts(&mut self, start_pos: u32, expr_list: Vec<ParamItem>, for_init: ForInit) -> PResult<Expression> {
        let return_type = self.tsm().should_parse_arrow_return_type.take();
        let extra = TsFunctionExtra { return_type, ..TsFunctionExtra::default() };
        self.parse_arrow_expression_ts(start_pos, expr_list, false, for_init, extra)
    }

    /// `parseParenAndDistinguishExpression(canBeArrow, forInit)`
    pub fn parse_paren_and_distinguish_expression_ts(&mut self, can_be_arrow: bool, for_init: ForInit) -> PResult<Expression> {
        let start_pos = self.start;
        let val;
        let allow_trailing_comma = self.options.ecma_version >= 8;
        if self.options.ecma_version >= 6 {
            let old_maybe_in_arrow_parameters = self.tss().maybe_in_arrow_parameters;
            self.tsm().maybe_in_arrow_parameters = true;
            self.next()?;
            let inner_start_pos = self.start;
            let mut expr_list: Vec<ParamItem> = Vec::new();
            let mut first = true;
            let mut last_is_comma = false;
            let mut ref_destructuring_errors = DestructuringErrors::new();
            let old_yield_pos = self.yield_pos;
            let old_await_pos = self.await_pos;
            let mut spread_start: Option<u32> = None;
            self.yield_pos = 0;
            self.await_pos = 0;
            while self.type_ != T::ParenR {
                if first {
                    first = false;
                } else {
                    self.expect(T::Comma)?;
                }
                if allow_trailing_comma && self.after_trailing_comma(T::ParenR, true)? {
                    last_is_comma = true;
                    break;
                } else if self.type_ == T::Ellipsis {
                    spread_start = Some(self.start);
                    let rest = self.parse_rest_binding()?;
                    expr_list.push(ParamItem::Rest(self.parse_paren_item_rest(rest)?));
                    if self.type_ == T::Comma {
                        self.raise_rest_comma(self.start)?;
                    }
                    break;
                } else {
                    expr_list.push(ParamItem::Expr(self.parse_maybe_assign_ts(
                        ForInit::No,
                        Some(&mut ref_destructuring_errors),
                        true,
                    )?));
                }
            }
            let inner_end_pos = self.last_tok_end;
            self.expect(T::ParenR)?;
            self.tsm().maybe_in_arrow_parameters = old_maybe_in_arrow_parameters;
            if can_be_arrow && self.should_parse_arrow_ts(&expr_list)? && self.eat(T::Arrow)? {
                self.check_pattern_errors(Some(&ref_destructuring_errors), false)?;
                self.check_yield_await_in_default_params()?;
                self.yield_pos = old_yield_pos;
                self.await_pos = old_await_pos;
                return self.parse_paren_arrow_list_ts(start_pos, expr_list, for_init);
            }
            if expr_list.is_empty() || last_is_comma {
                return self.unexpected(Some(self.last_tok_start));
            }
            if let Some(s) = spread_start {
                return self.unexpected(Some(s));
            }
            self.check_expression_errors(Some(&ref_destructuring_errors), true)?;
            if old_yield_pos != 0 {
                self.yield_pos = old_yield_pos;
            }
            if old_await_pos != 0 {
                self.await_pos = old_await_pos;
            }
            let mut exprs: Vec<Expression> = expr_list
                .into_iter()
                .map(|e| match e {
                    ParamItem::Expr(e) => e,
                    _ => unreachable!(),
                })
                .collect();
            if exprs.len() > 1 {
                val = Expression::SequenceExpression(SequenceExpression {
                    start: inner_start_pos,
                    end: inner_end_pos,
                    expressions: exprs,
                });
            } else {
                val = exprs.pop().unwrap();
            }
        } else {
            val = self.parse_paren_expression()?;
        }
        if self.options.preserve_parens {
            Ok(Expression::ParenthesizedExpression(ParenthesizedExpression {
                start: start_pos,
                end: self.last_tok_end,
                expression: Box::new(val),
            }))
        } else {
            Ok(val)
        }
    }

    /// `shouldParseAsyncArrow()`
    fn should_parse_async_arrow_ts(&mut self) -> PResult<bool> {
        if self.type_ == T::Colon {
            let result = self.try_parse(|p| {
                let return_type = p.ts_parse_type_or_type_predicate_annotation(T::Colon)?;
                if p.can_insert_semicolon() || p.type_ != T::Arrow {
                    return Ok(None);
                }
                Ok(Some(return_type))
            })?;
            match result {
                TryParse::Aborted(_) => {
                    self.tsm().should_parse_async_arrow_return_type = None;
                    Ok(false)
                }
                TryParse::Ok(n) => {
                    self.tsm().should_parse_async_arrow_return_type = Some(n);
                    Ok(!self.can_insert_semicolon() && self.eat(T::Arrow)?)
                }
                TryParse::Thrown(..) => Ok(false),
            }
        } else {
            Ok(!self.can_insert_semicolon() && self.eat(T::Arrow)?)
        }
    }

    /// `parseSubscriptAsyncArrow(startPos, startLoc, exprList, forInit)`
    fn parse_subscript_async_arrow_ts(&mut self, start_pos: u32, expr_list: Vec<Option<ArrayElement>>, for_init: ForInit) -> PResult<Expression> {
        let return_type = self.tsm().should_parse_async_arrow_return_type.take();
        let extra = TsFunctionExtra { return_type, ..TsFunctionExtra::default() };
        self.parse_arrow_expression_ts(start_pos, expr_list_to_params(expr_list), true, for_init, extra)
    }

    /// `parseExprList(close, allowTrailingComma, allowEmpty, refDestructuringErrors)`
    pub fn parse_expr_list_ts(
        &mut self,
        close: T,
        allow_trailing_comma: bool,
        allow_empty: bool,
        mut ref_destructuring_errors: Option<&mut DestructuringErrors>,
    ) -> PResult<Vec<Option<ArrayElement>>> {
        let mut elts = Vec::new();
        let mut first = true;
        while !self.eat(close)? {
            if !first {
                self.expect(T::Comma)?;
                if allow_trailing_comma && self.after_trailing_comma(close, false)? {
                    break;
                }
            } else {
                first = false;
            }
            let elt = if allow_empty && self.type_ == T::Comma {
                None
            } else if self.type_ == T::Ellipsis {
                let mut s = self.parse_spread(ref_destructuring_errors.as_deref_mut())?;
                if self.tss().maybe_in_arrow_parameters && self.type_ == T::Colon {
                    let t = self.ts_parse_type_annotation(true, None)?;
                    s.ts.get_or_insert_with(Box::default).type_annotation = Some(t);
                }
                if let Some(r) = ref_destructuring_errors.as_deref_mut() {
                    if self.type_ == T::Comma && r.trailing_comma < 0 {
                        r.trailing_comma = self.start as i64;
                    }
                }
                Some(ArrayElement::Spread(SpreadElementNode::SpreadElement(s)))
            } else {
                Some(ArrayElement::Expression(self.parse_maybe_assign_ts(
                    ForInit::No,
                    ref_destructuring_errors.as_deref_mut(),
                    true,
                )?))
            };
            elts.push(elt);
        }
        Ok(elts)
    }

    // --- subscripts

    /// `atPossibleAsyncArrow(base)`
    fn at_possible_async_arrow(&self, base: &Expression) -> bool {
        matches!(base, Expression::Identifier(i) if i.name == "async")
            && self.last_tok_end == base.span().end
            && !self.can_insert_semicolon()
            && base.span().end - base.span().start == 5
            && base.span().start as i64 == self.potential_arrow_at
    }

    /// `tsTryParseGenericAsyncArrowFunction(startPos, startLoc, forInit)`
    fn ts_try_parse_generic_async_arrow_function(&mut self, start_pos: u32, for_init: ForInit) -> PResult<Option<Expression>> {
        if !self.ts_match_left_relational() {
            return Ok(None);
        }
        let old_maybe_in_arrow_parameters = self.tss().maybe_in_arrow_parameters;
        self.tsm().maybe_in_arrow_parameters = true;
        let res = self.ts_try_parse_and_catch(|p| {
            let type_parameters = p.ts_parse_type_parameters(TypeParamModifiers::Const)?;
            // super.parseFunctionParams(node)
            let params = p.parse_function_params()?;
            let return_type = p.ts_try_parse_type_or_type_predicate_annotation()?;
            p.expect(T::Arrow)?;
            Ok(Some((type_parameters, params, return_type)))
        });
        self.tsm().maybe_in_arrow_parameters = old_maybe_in_arrow_parameters;
        let Some((type_parameters, params, return_type)) = res? else { return Ok(None) };
        let extra = TsFunctionExtra { type_parameters: Some(type_parameters), return_type, declare: false };
        let params = params.into_iter().map(ParamItem::Pat).collect();
        Ok(Some(self.parse_arrow_expression_ts(start_pos, params, /* async */ true, for_init, extra)?))
    }

    /// `parseTaggedTemplateExpression(base, startPos, startLoc, optionalChainMember)`
    fn parse_tagged_template_expression(&mut self, base: Expression, start_pos: u32, optional_chain_member: bool) -> PResult<TaggedTemplateExpression> {
        let quasi = self.parse_template(true)?;
        if optional_chain_member {
            return self.raise(start_pos, "Tagged Template Literals are not allowed in optionalChain.");
        }
        Ok(TaggedTemplateExpression {
            start: start_pos,
            end: self.last_tok_end,
            tag: Box::new(base),
            quasi: TemplateLiteralNode::TemplateLiteral(quasi),
            ts: None,
        })
    }

    /// `parseSubscript(base, startPos, startLoc, noCalls, maybeAsyncArrow, optionalChained, forInit)`
    #[allow(clippy::too_many_arguments)]
    pub fn parse_subscript_ts(
        &mut self,
        mut base: Expression,
        start_pos: u32,
        no_calls: bool,
        maybe_async_arrow: bool,
        optional_chained: bool,
        for_init: ForInit,
    ) -> PResult<Expression> {
        let mut optional_chained_ = optional_chained;
        if !self.has_preceding_line_break() && self.type_ == T::Prefix && self.tok_text() == "!" {
            // NODE: replace bang
            self.expr_allowed = false;
            self.next()?;
            return Ok(Expression::TSNonNullExpression(TSNonNullExpression {
                start: start_pos,
                end: self.last_tok_end,
                expression: Box::new(base),
            }));
        }
        let mut is_optional_call = false;
        if self.type_ == T::QuestionDot && self.lookahead_char_code() == 60 {
            if no_calls {
                return Ok(base);
            }
            match &mut base {
                Expression::MemberExpression(m) => m.optional = true,
                Expression::CallExpression(c) => c.optional = true,
                other => {
                    if let Some(ts) = expression_ts_mut(other) {
                        ts.optional = true;
                    }
                }
            }
            optional_chained_ = true;
            is_optional_call = true;
            self.next()?;
        }
        if self.ts_match_left_relational() || self.type_ == T::BitShift {
            let mut missing_paren_error_loc = None;
            let base_ref = &base;
            let result = self.ts_try_parse_and_catch(|p| {
                if !no_calls && p.at_possible_async_arrow(base_ref) {
                    if let Some(async_arrow_fn) = p.ts_try_parse_generic_async_arrow_function(start_pos, for_init)? {
                        return Ok(Some(async_arrow_fn));
                    }
                }
                let Some(type_arguments) = p.ts_parse_type_arguments_in_expression()? else {
                    return Ok(Some(base_ref.clone()));
                };
                if is_optional_call && p.type_ != T::ParenL {
                    missing_paren_error_loc = Some(p.start);
                    return Ok(Some(base_ref.clone()));
                }
                if p.type_ == T::InvalidTemplate || p.type_ == T::BackQuote {
                    let mut result2 = p.parse_tagged_template_expression(base_ref.clone(), start_pos, optional_chained_)?;
                    result2.ts = Some(Box::new(TsTypeArgumentsExtra { type_arguments: Some(type_arguments), omit_optional: false }));
                    return Ok(Some(Expression::TaggedTemplateExpression(result2)));
                }
                if !no_calls && p.eat(T::ParenL)? {
                    let mut ref_destructuring_errors = DestructuringErrors::new();
                    let arguments = p.parse_expr_list(
                        T::ParenR,
                        p.options.ecma_version >= 8,
                        false,
                        Some(&mut ref_destructuring_errors),
                    )?;
                    // tsCheckForInvalidTypeCasts(node2.arguments)
                    for a in arguments.iter().flatten() {
                        if let ArrayElement::Expression(Expression::TSTypeCastExpression(tc)) = a {
                            return p.raise(tc.type_annotation.start, err::UNEXPECTED_TYPE_ANNOTATION);
                        }
                    }
                    p.check_expression_errors(Some(&ref_destructuring_errors), true)?;
                    return Ok(Some(Expression::CallExpression(CallExpression {
                        start: start_pos,
                        end: p.last_tok_end,
                        callee: Box::new(base_ref.clone()),
                        arguments: arguments.into_iter().flatten().collect(),
                        optional: is_optional_call,
                        ts: Some(Box::new(TsTypeArgumentsExtra {
                            type_arguments: Some(type_arguments),
                            omit_optional: !optional_chained_,
                        })),
                    })));
                }
                let token_type = p.type_;
                if
                // a<b>>c is not (a<b>)>c, but a<(b>>c)
                p.ts_match_right_relational()
                    // a<b>>>c is not (a<b>)>>c, but a<(b>>>c)
                    || token_type == T::BitShift
                    // a<b>c is (a<b)>c
                    || token_type != T::ParenL && token_type.starts_expr() && !p.has_preceding_line_break()
                {
                    return Ok(None);
                }
                Ok(Some(Expression::TSInstantiationExpression(TSInstantiationExpression {
                    start: start_pos,
                    end: p.last_tok_end,
                    expression: Box::new(base_ref.clone()),
                    type_arguments,
                })))
            })?;
            if missing_paren_error_loc.is_some() {
                return self.unexpected(missing_paren_error_loc);
            }
            if let Some(result) = result {
                if matches!(result, Expression::TSInstantiationExpression(_))
                    && (self.type_ == T::Dot || self.type_ == T::QuestionDot && self.lookahead_char_code() != 40)
                {
                    return self.raise(self.start, err::INVALID_PROPERTY_ACCESS_AFTER_INSTANTIATION_EXPRESSION);
                }
                return Ok(result);
            }
        }
        let optional_supported = self.options.ecma_version >= 11;
        let optional = optional_supported && self.eat(T::QuestionDot)?;
        if no_calls && optional {
            return self.raise(self.last_tok_start, "Optional chaining cannot appear in the callee of new expressions");
        }
        let computed = self.eat(T::BracketL)?;
        if computed || optional && self.type_ != T::ParenL && self.type_ != T::BackQuote || self.eat(T::Dot)? {
            let property = if computed {
                let p = self.parse_expression(ForInit::No, None)?;
                self.expect(T::BracketR)?;
                PropertyKey::Expression(Box::new(p))
            } else if self.type_ == T::PrivateId && !matches!(base, Expression::Super(_)) {
                PropertyKey::Private(PrivateIdentifierNode::PrivateIdentifier(self.parse_private_ident()?))
            } else {
                let liberal = self.options.allow_reserved != crate::acorn::options::AllowReserved::Never;
                PropertyKey::Expression(Box::new(Expression::Identifier(self.parse_ident(liberal)?)))
            };
            base = Expression::MemberExpression(MemberExpression {
                start: start_pos,
                end: self.last_tok_end,
                object: Box::new(base),
                property,
                computed,
                optional: optional_supported && optional,
            });
        } else if !no_calls && self.eat(T::ParenL)? {
            let old_maybe_in_arrow_parameters = self.tss().maybe_in_arrow_parameters;
            self.tsm().maybe_in_arrow_parameters = true;
            let mut ref_destructuring_errors = DestructuringErrors::new();
            let old_yield_pos = self.yield_pos;
            let old_await_pos = self.await_pos;
            let old_await_ident_pos = self.await_ident_pos;
            self.yield_pos = 0;
            self.await_pos = 0;
            self.await_ident_pos = 0;
            let expr_list =
                self.parse_expr_list(T::ParenR, self.options.ecma_version >= 8, false, Some(&mut ref_destructuring_errors))?;
            if maybe_async_arrow && !optional && self.should_parse_async_arrow_ts()? {
                self.check_pattern_errors(Some(&ref_destructuring_errors), false)?;
                self.check_yield_await_in_default_params()?;
                if self.await_ident_pos > 0 {
                    return self.raise(self.await_ident_pos, "Cannot use 'await' as identifier inside an async function");
                }
                self.yield_pos = old_yield_pos;
                self.await_pos = old_await_pos;
                self.await_ident_pos = old_await_ident_pos;
                base = self.parse_subscript_async_arrow_ts(start_pos, expr_list, for_init)?;
            } else {
                self.check_expression_errors(Some(&ref_destructuring_errors), true)?;
                if old_yield_pos != 0 {
                    self.yield_pos = old_yield_pos;
                }
                if old_await_pos != 0 {
                    self.await_pos = old_await_pos;
                }
                if old_await_ident_pos != 0 {
                    self.await_ident_pos = old_await_ident_pos;
                }
                base = Expression::CallExpression(CallExpression {
                    start: start_pos,
                    end: self.last_tok_end,
                    callee: Box::new(base),
                    arguments: expr_list.into_iter().flatten().collect(),
                    optional: optional_supported && optional,
                    ts: None,
                });
            }
            self.tsm().maybe_in_arrow_parameters = old_maybe_in_arrow_parameters;
        } else if self.type_ == T::BackQuote {
            if optional || optional_chained_ {
                return self.raise(self.start, "Optional chaining cannot appear in the tag of tagged template expressions");
            }
            let quasi = self.parse_template(true)?;
            base = Expression::TaggedTemplateExpression(TaggedTemplateExpression {
                start: start_pos,
                end: self.last_tok_end,
                tag: Box::new(base),
                quasi: TemplateLiteralNode::TemplateLiteral(quasi),
                ts: None,
            });
        }
        Ok(base)
    }

    // --- object members, methods

    /// `parseGetterSetter(prop)`
    pub fn parse_getter_setter_ts(&mut self, kind_name: &str) -> PResult<(bool, PropertyKey, Function, PropertyKind)> {
        let kind = if kind_name == "get" { PropertyKind::Get } else { PropertyKind::Set };
        let (computed, key) = self.parse_property_name()?;
        let type_parameters = self.ts_try_parse_type_parameters(TypeParamModifiers::Const)?;
        let mut value = self.parse_method(false, false, false)?;
        if let Some(tp) = type_parameters {
            value.ts.get_or_insert_with(Box::default).type_parameters = Some(tp);
        }
        let mut param_count = if kind == PropertyKind::Get { 0 } else { 1 };
        let has_context_param = matches!(value.params.first(), Some(Pattern::Identifier(i)) if i.name == "this");
        if has_context_param {
            param_count += 1;
        }
        if value.params.len() != param_count {
            let start = value.start;
            if kind == PropertyKind::Get {
                return self.raise_recoverable(start, "getter should have no params");
            } else {
                return self.raise_recoverable(start, "setter should have exactly one param");
            }
        } else if kind == PropertyKind::Set {
            if let Pattern::RestElement(r) = &value.params[0] {
                return self.raise_recoverable(r.start, "Setter cannot use rest params");
            }
        }
        Ok((computed, key, value, kind))
    }

    /// The `parsePropertyValue` override: a generic method `m<T>() {}`.
    pub fn parse_property_value_ts(
        &mut self,
        start: u32,
        computed: bool,
        key: PropertyKey,
        is_pattern: bool,
        is_generator: bool,
        is_async: bool,
    ) -> PResult<PropResult> {
        if is_pattern {
            return self.unexpected(None);
        }
        let type_parameters = self.ts_try_parse_type_parameters(TypeParamModifiers::Const)?;
        let mut value = self.parse_method(is_generator, is_async, false)?;
        if let Some(tp) = type_parameters {
            value.ts.get_or_insert_with(Box::default).type_parameters = Some(tp);
        }
        Ok(PropResult::Property(Property {
            start,
            end: self.last_tok_end,
            method: true,
            shorthand: false,
            computed,
            key,
            value: Expression::FunctionExpression(value),
            kind: PropertyKind::Init,
        }))
    }

    /// `parseMethod(isGenerator, isAsync, allowDirectSuper, inClassScope, method)`;
    /// `abstract_method`: the method's key text when `method.abstract`.
    pub(crate) fn parse_method_ts(
        &mut self,
        is_generator: bool,
        is_async: bool,
        allow_direct_super: bool,
        in_class_scope: bool,
        abstract_method: Option<(u32, String)>,
    ) -> PResult<FunctionNode> {
        let start = self.start;
        let old_yield_pos = self.yield_pos;
        let old_await_pos = self.await_pos;
        let old_await_ident_pos = self.await_ident_pos;
        let generator = self.options.ecma_version >= 6 && is_generator;
        let is_async_node = self.options.ecma_version >= 8 && is_async;
        self.yield_pos = 0;
        self.await_pos = 0;
        self.await_ident_pos = 0;
        self.enter_scope(
            function_flags(is_async, generator) | SCOPE_SUPER | if allow_direct_super { SCOPE_DIRECT_SUPER } else { 0 },
        );
        self.expect(T::ParenL)?;
        let params = self.parse_class_function_params()?;
        self.check_yield_await_in_default_params()?;
        let bodiless = if in_class_scope { Some(Bodiless::ClassMethod) } else { None };
        let (return_type, body) = self.parse_function_body_ts(start, &params, None, false, true, ForInit::No, bodiless)?;
        self.yield_pos = old_yield_pos;
        self.await_pos = old_await_pos;
        self.await_ident_pos = old_await_ident_pos;
        let ts_extra = TsFunctionExtra { return_type, ..TsFunctionExtra::default() };
        let Some(body) = body else {
            return Ok(FunctionNode::TSDeclareMethod(TSDeclareFunction {
                start,
                end: self.last_tok_end,
                id: None,
                expression: false,
                generator,
                is_async: is_async_node,
                params,
                ts: ts_extra,
            }));
        };
        if let Some((method_start, method_name)) = abstract_method {
            return self.raise(method_start, err::ABSTRACT_METHOD_HAS_IMPLEMENTATION(&method_name));
        }
        Ok(FunctionNode::FunctionExpression(Function {
            start,
            end: self.last_tok_end,
            id: None,
            expression: false,
            generator,
            is_async: is_async_node,
            params,
            body: BlockStatementNode::BlockStatement(body.into_block()),
            ts: if ts_extra == TsFunctionExtra::default() { None } else { Some(Box::new(ts_extra)) },
        }))
    }

    /// `parseClassFunctionParams()`
    fn parse_class_function_params(&mut self) -> PResult<Vec<Pattern>> {
        // `params.typeParameters = typeParameters` sets a property on the array: dropped.
        let _type_parameters = self.ts_try_parse_type_parameters(TypeParamModifiers::None)?;
        Ok(self.parse_binding_list(T::ParenR, false, self.options.ecma_version >= 8, Some(true))?.into_iter().flatten().collect())
    }

    // --- new, import()

    /// `parseNew()`
    pub fn parse_new_ts(&mut self) -> PResult<Expression> {
        if self.contains_esc {
            return self.raise_recoverable(self.start, "Escape sequence in keyword new");
        }
        let start = self.start;
        let meta = self.parse_ident(true)?;
        if self.options.ecma_version >= 6 && self.eat(T::Dot)? {
            let contains_esc = self.contains_esc;
            let property = self.parse_ident(true)?;
            if property.name != "target" {
                return self.raise_recoverable(property.start, "The only valid meta property for new is 'new.target'");
            }
            if contains_esc {
                return self.raise_recoverable(start, "'new.target' must not contain escaped characters");
            }
            if !self.allow_new_dot_target() {
                return self.raise_recoverable(start, "'new.target' can only be used in functions and class static block");
            }
            return Ok(Expression::MetaProperty(MetaProperty { start, end: self.last_tok_end, meta, property }));
        }
        let start_pos = self.start;
        let is_import = self.type_ == T::Import;
        let atom = self.parse_expr_atom(None, ForInit::No, false)?;
        let mut callee = self.parse_subscripts(atom, start_pos, true, ForInit::No)?;
        if is_import && matches!(callee, Expression::ImportExpression(_)) {
            return self.raise(start_pos, "Cannot use new with import()");
        }
        let mut ts = None;
        if let Expression::TSInstantiationExpression(_) = callee {
            let Expression::TSInstantiationExpression(i) = callee else { unreachable!() };
            ts = Some(Box::new(TsTypeArgumentsExtra { type_arguments: Some(i.type_arguments), omit_optional: false }));
            callee = *i.expression;
        }
        let arguments = if self.eat(T::ParenL)? {
            self.parse_expr_list(T::ParenR, self.options.ecma_version >= 8, false, None)?.into_iter().flatten().collect()
        } else {
            Vec::new()
        };
        Ok(Expression::NewExpression(NewExpression { start, end: self.last_tok_end, callee: Box::new(callee), arguments, ts }))
    }

    /// `parseDynamicImport(node)`
    pub fn parse_dynamic_import_ts(&mut self, start: u32) -> PResult<Expression> {
        self.next()?;
        let source = self.parse_maybe_assign(ForInit::No, None)?;
        let mut arguments = None;
        if self.eat(T::Comma)? {
            let expr = self.parse_expression(ForInit::No, None)?;
            arguments = Some(vec![expr]);
        }
        if !self.eat(T::ParenR)? {
            let error_pos = self.start;
            if self.eat(T::Comma)? && self.eat(T::ParenR)? {
                return self.raise_recoverable(error_pos, "Trailing comma is not allowed in import()");
            } else {
                return self.unexpected(Some(error_pos));
            }
        }
        Ok(Expression::ImportExpression(ImportExpression {
            start,
            end: self.last_tok_end,
            source: Box::new(source),
            options: None,
            ts: Some(Box::new(TsImportExpressionExtra { arguments })),
        }))
    }

    /// The `@` case of `parseExprAtom`: decorators before a class expression.
    pub fn parse_expr_atom_decorators(&mut self) -> PResult<Expression> {
        self.parse_decorators(false)?;
        self.parse_expr_atom(None, ForInit::No, false)
    }
}
