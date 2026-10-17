//! Port of acorn 8.19.0 `src/expression.js`.
//!
//! A recursive descent parser operates by defining functions for all
//! syntactic elements, and recursively calling those, each function
//! advancing the input stream and returning an AST node. Precedence
//! of constructs (for example, the fact that `!x[1]` means `!(x[1])`
//! instead of `(!x)[1]` is handled by the fact that the parser
//! function that parses unary prefix operators is called first, and
//! in turn calls the function that parses `[]` subscripts — that
//! way, it'll receive the node for `x[1]` already parsed, and wraps
//! *that* in the unary operator node.
//!
//! Acorn uses an [operator precedence parser][opp] to handle binary
//! operator precedence, because it is much more compact than using
//! the technique outlined above, which uses different, nesting
//! functions to specify precedence, for all of the ten binary
//! precedence levels that JavaScript defines.
//!
//! [opp]: http://en.wikipedia.org/wiki/Operator-precedence_parser
//!
//! SVELTERS: typed results. `parseMaybeUnary`/`parseExprOp` can yield a `PrivateIdentifier`
//! (`#x in obj`), modelled by `Operand`. The shorthand-with-default property of an object
//! literal (`{a = 1}`, valid only once converted to a pattern) is held as an
//! `AssignmentExpression` until `toAssignable` turns it into the `AssignmentPattern` acorn builds.
//! The tail recursion of `parseExprOp` is a loop (same result, bounded stack).

use std::collections::{HashMap, HashSet};

use super::lval::ParamItem;
use super::options::{AcornSourceType, AllowReserved};
use super::parseutil::DestructuringErrors;
use super::scopeflags::*;
use super::state::{PResult, Parser, TokVal};
use super::tokencontext::TokContext;
use super::tokentype::TokenType as T;
use super::util::js_number_to_string;
use super::whitespace::has_line_break;
use crate::estree::*;

/// acorn's `forInit` argument: `false`, `true` or `"await"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForInit {
    No,
    Yes,
    Await,
}

impl ForInit {
    pub fn truthy(self) -> bool {
        self != ForInit::No
    }
}

/// Result of `parseMaybeUnary` / `parseExprOp`.
pub enum Operand {
    Expr(Expression),
    Private(PrivateIdentifier),
}

/// `propHash` of `parseObj`.
#[derive(Default)]
pub struct PropHash {
    proto: bool,
    names: HashMap<String, [bool; 3]>,
}

/// A parsed object member (`parseProperty` result).
pub enum PropResult {
    Property(Property),
    Spread(SpreadElement),
    PatternProperty(PatternProperty),
    Rest(RestElement),
}

pub enum PropValue {
    Expr(Expression),
    Pat(Pattern),
}

/// Function body (`node.body`) as built by `parseFunctionBody`.
pub enum FnBody {
    Block(BlockStatement),
    Expr(Expression),
}

impl FnBody {
    pub fn into_block(self) -> BlockStatement {
        match self {
            FnBody::Block(b) => b,
            FnBody::Expr(_) => unreachable!("function body is a block"),
        }
    }
}

/// SVELTERS: nesting limit standing in for acorn's `catchStackOverflow` (counted per
/// `parseStatement` / `parseMaybeAssign` / `parseMaybeUnary` level, i.e. ~500 nested parens).
/// Measured stack use is up to ~3.3 KiB per level in release and ~16 KiB in debug builds, so
/// the limit fits an 8 MiB main-thread stack in release; parse deeply nested untrusted input on
/// a thread with a larger stack.
const MAX_DEPTH: u32 = 1000;

fn is_arrow_at(e: &Expression, start: u32) -> bool {
    matches!(e, Expression::ArrowFunctionExpression(a) if a.start == start)
}

pub fn expr_as_operand_expr(e: Operand) -> Option<Expression> {
    match e {
        Operand::Expr(e) => Some(e),
        Operand::Private(_) => None,
    }
}

impl<'a> Parser<'a> {
    pub fn enter_depth(&mut self) -> PResult<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return self.raise(self.start, "Not enough stack space to parse input");
        }
        Ok(())
    }

    // Check if property name clashes with already added.
    // Object/class getters and setters are not allowed to clash —
    // either with each other or with an init property — and in
    // strict mode, init properties are also not allowed to be repeated.

    pub fn check_prop_clash(
        &mut self,
        prop: &PropResult,
        prop_hash: &mut PropHash,
        ref_destructuring_errors: Option<&mut DestructuringErrors>,
    ) -> PResult<()> {
        let p = match prop {
            PropResult::Property(p) => p,
            PropResult::Spread(_) => {
                if self.options.ecma_version >= 9 {
                    return Ok(());
                }
                return Ok(());
            }
            _ => return Ok(()),
        };
        if self.options.ecma_version >= 6 && (p.computed || p.method || p.shorthand) {
            return Ok(());
        }
        let PropertyKey::Expression(key) = &p.key else { return Ok(()) };
        let (name, key_start) = match &**key {
            Expression::Identifier(i) => (i.name.clone(), i.start),
            Expression::Literal(l) => (
                match &l.value {
                    LiteralValue::String(s) => s.clone(),
                    LiteralValue::Number(n) => js_number_to_string(*n),
                    LiteralValue::Boolean(b) => b.to_string(),
                    LiteralValue::Null => "null".into(),
                    LiteralValue::BigInt => l.bigint.clone().unwrap_or_default(),
                    LiteralValue::RegExp => l.raw.clone(),
                },
                l.start,
            ),
            _ => return Ok(()),
        };
        let kind = p.kind;
        if self.options.ecma_version >= 6 {
            if name == "__proto__" && kind == PropertyKind::Init {
                if prop_hash.proto {
                    if let Some(r) = ref_destructuring_errors {
                        if r.double_proto < 0 {
                            r.double_proto = key_start as i64;
                        }
                    } else {
                        return self.raise_recoverable(key_start, "Redefinition of __proto__ property");
                    }
                }
                prop_hash.proto = true;
            }
            return Ok(());
        }
        let idx = match kind {
            PropertyKind::Init => 0,
            PropertyKind::Get => 1,
            PropertyKind::Set => 2,
        };
        let strict = self.strict;
        if let Some(other) = prop_hash.names.get(&name) {
            let redefinition = if kind == PropertyKind::Init {
                strict && other[0] || other[1] || other[2]
            } else {
                other[0] || other[idx]
            };
            if redefinition {
                return self.raise_recoverable(key_start, "Redefinition of property");
            }
        }
        prop_hash.names.entry(name).or_insert([false; 3])[idx] = true;
        Ok(())
    }

    // ### Expression parsing

    // These nest, from the most general expression type at the top to
    // 'atomic', nondivisible expression types at the bottom. Most of
    // the functions will simply let the function(s) below them parse,
    // and, *if* the syntactic construct they handle is present, wrap
    // the AST node that the inner parser gave them in another node.

    // Parse a full expression. The optional arguments are used to
    // forbid the `in` operator (in for loops initalization expressions)
    // and provide reference for storing '=' operator inside shorthand
    // property assignment in contexts where both object expression
    // and object pattern might appear (so it's possible to raise
    // delayed syntax error at correct position).

    pub fn parse_expression(
        &mut self,
        for_init: ForInit,
        mut ref_destructuring_errors: Option<&mut DestructuringErrors>,
    ) -> PResult<Expression> {
        let start_pos = self.start;
        let expr = self.parse_maybe_assign(for_init, ref_destructuring_errors.as_deref_mut())?;
        if self.type_ == T::Comma {
            let mut expressions = vec![expr];
            while self.eat(T::Comma)? {
                expressions.push(self.parse_maybe_assign(for_init, ref_destructuring_errors.as_deref_mut())?);
            }
            return Ok(Expression::SequenceExpression(SequenceExpression {
                start: start_pos,
                end: self.last_tok_end,
                expressions,
            }));
        }
        Ok(expr)
    }

    // Parse an assignment expression. This includes applications of
    // operators like `+=`.

    pub fn parse_maybe_assign(
        &mut self,
        for_init: ForInit,
        ref_destructuring_errors: Option<&mut DestructuringErrors>,
    ) -> PResult<Expression> {
        self.enter_depth()?;
        let r = self.parse_maybe_assign_inner(for_init, ref_destructuring_errors);
        self.depth -= 1;
        r
    }

    fn parse_maybe_assign_inner(
        &mut self,
        for_init: ForInit,
        ref_destructuring_errors: Option<&mut DestructuringErrors>,
    ) -> PResult<Expression> {
        if self.is_contextual("yield") {
            if self.in_generator() {
                return self.parse_yield(for_init);
            }
            // The tokenizer will assume an expression is allowed after
            // `yield`, but this isn't that kind of yield
            self.expr_allowed = false;
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
        if self.type_ == T::ParenL || self.type_ == T::Name {
            self.potential_arrow_at = self.start as i64;
            self.potential_arrow_in_for_await = for_init == ForInit::Await;
        }
        let left = self.parse_maybe_conditional(for_init, Some(&mut *r))?;
        if self.type_.is_assign() {
            let operator = AssignmentOperator::from_str(self.tok_text()).unwrap();
            let is_eq = self.type_ == T::Eq;
            let left_start = left.span().start;
            let left_pat = if is_eq {
                let left = self.to_assignable(left, false, Some(&mut *r))?;
                Ok(left)
            } else {
                Err(left)
            };
            if !own_destructuring_errors {
                r.parenthesized_assign = -1;
                r.trailing_comma = -1;
                // reset these when detected inside this expression
                if r.shorthand_assign >= left_start as i64 {
                    r.shorthand_assign = -1;
                }
                if r.double_proto >= left_start as i64 {
                    r.double_proto = -1;
                }
            }
            let left = match left_pat {
                Ok(p) => {
                    self.check_lval_pattern(&p, BIND_NONE, None)?;
                    p
                }
                Err(e) => {
                    self.check_lval_simple_expr(&e, BIND_NONE, None)?;
                    match e {
                        Expression::Identifier(i) => Pattern::Identifier(i),
                        Expression::MemberExpression(m) => Pattern::MemberExpression(Box::new(m)),
                        Expression::ParenthesizedExpression(p) => Pattern::ParenthesizedExpression(p),
                        other => return self.raise(other.span().start, "Assigning to rvalue"),
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

    // Parse a ternary conditional (`?:`) operator.

    pub fn parse_maybe_conditional(
        &mut self,
        for_init: ForInit,
        mut ref_destructuring_errors: Option<&mut DestructuringErrors>,
    ) -> PResult<Expression> {
        let start_pos = self.start;
        let expr = self.parse_expr_ops(for_init, ref_destructuring_errors.as_deref_mut())?;
        if self.check_expression_errors(ref_destructuring_errors.as_deref(), false)? {
            return Ok(expr);
        }
        if !is_arrow_at(&expr, start_pos) && self.eat(T::Question)? {
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

    // Start the precedence parser.

    pub fn parse_expr_ops(
        &mut self,
        for_init: ForInit,
        mut ref_destructuring_errors: Option<&mut DestructuringErrors>,
    ) -> PResult<Expression> {
        let start_pos = self.start;
        let expr = self.parse_maybe_unary(ref_destructuring_errors.as_deref_mut(), false, false, for_init)?;
        if self.check_expression_errors(ref_destructuring_errors.as_deref(), false)? {
            return match expr {
                Operand::Expr(e) => Ok(e),
                // SVELTERS: a bare `#x` here is always followed by `in`, which the caller rejects.
                Operand::Private(_) => self.unexpected(None),
            };
        }
        if let Operand::Expr(e) = &expr {
            if is_arrow_at(e, start_pos) {
                return Ok(expr_as_operand_expr(expr).unwrap());
            }
        }
        match self.parse_expr_op(expr, start_pos, -1, for_init)? {
            Operand::Expr(e) => Ok(e),
            Operand::Private(_) => self.unexpected(None),
        }
    }

    // Parse binary operators with the operator precedence parsing
    // algorithm. `left` is the left-hand side of the operator.
    // `minPrec` provides context that allows the function to stop and
    // defer further parser to one of its callers when it encounters an
    // operator that has a lower precedence than the set it is parsing.

    pub fn parse_expr_op(
        &mut self,
        mut left: Operand,
        left_start_pos: u32,
        min_prec: i32,
        for_init: ForInit,
    ) -> PResult<Operand> {
        loop {
            let Some(mut prec) = self.type_.binop() else { return Ok(left) };
            if !(!for_init.truthy() || self.type_ != T::In) {
                return Ok(left);
            }
            if prec <= min_prec {
                return Ok(left);
            }
            let logical = self.type_ == T::LogicalOR || self.type_ == T::LogicalAND;
            let coalesce = self.type_ == T::Coalesce;
            if coalesce {
                // Handle the precedence of `tt.coalesce` as equal to the range of logical expressions.
                // In other words, `node.right` shouldn't contain logical expressions in order to check the mixed error.
                prec = T::LogicalAND.binop().unwrap();
            }
            let op = self.tok_text();
            self.next()?;
            let start_pos = self.start;
            let unary = self.parse_maybe_unary(None, false, false, for_init)?;
            let right = self.parse_expr_op(unary, start_pos, prec, for_init)?;
            let node = self.build_binary(left_start_pos, left, right, op, logical || coalesce)?;
            if (logical && self.type_ == T::Coalesce)
                || (coalesce && (self.type_ == T::LogicalOR || self.type_ == T::LogicalAND))
            {
                return self.raise_recoverable(
                    self.start,
                    "Logical expressions and coalesce expressions cannot be mixed. Wrap either by parentheses",
                );
            }
            left = Operand::Expr(node);
        }
    }

    pub fn build_binary(
        &mut self,
        start_pos: u32,
        left: Operand,
        right: Operand,
        op: &str,
        logical: bool,
    ) -> PResult<Expression> {
        let right = match right {
            Operand::Private(p) => {
                return self.raise(p.start, "Private identifier can only be left side of binary expression")
            }
            Operand::Expr(e) => e,
        };
        let end = self.last_tok_end;
        if logical {
            let left = match left {
                Operand::Expr(e) => e,
                Operand::Private(p) => return self.unexpected(Some(p.start)),
            };
            return Ok(Expression::LogicalExpression(LogicalExpression {
                start: start_pos,
                end,
                left: Box::new(left),
                operator: LogicalOperator::from_str(op).unwrap(),
                right: Box::new(right),
            }));
        }
        let left = match left {
            Operand::Expr(e) => BinaryLeft::Expression(Box::new(e)),
            Operand::Private(p) => BinaryLeft::Private(PrivateIdentifierNode::PrivateIdentifier(p)),
        };
        Ok(Expression::BinaryExpression(BinaryExpression {
            start: start_pos,
            end,
            left,
            operator: BinaryOperator::from_str(op).unwrap(),
            right: Box::new(right),
        }))
    }

    // Parse unary operators, both prefix and postfix.

    pub fn parse_maybe_unary(
        &mut self,
        ref_destructuring_errors: Option<&mut DestructuringErrors>,
        saw_unary: bool,
        inc_dec: bool,
        for_init: ForInit,
    ) -> PResult<Operand> {
        self.enter_depth()?;
        let r = self.parse_maybe_unary_inner(ref_destructuring_errors, saw_unary, inc_dec, for_init);
        self.depth -= 1;
        r
    }

    fn parse_maybe_unary_inner(
        &mut self,
        mut ref_destructuring_errors: Option<&mut DestructuringErrors>,
        mut saw_unary: bool,
        inc_dec: bool,
        for_init: ForInit,
    ) -> PResult<Operand> {
        let start_pos = self.start;
        let expr: Operand;
        if self.is_contextual("await") && self.can_await() {
            expr = Operand::Expr(self.parse_await(for_init)?);
            saw_unary = true;
        } else if self.type_.prefix() {
            let start = self.start;
            let update = self.type_ == T::IncDec;
            let op = self.tok_text();
            self.next()?;
            let argument = match self.parse_maybe_unary(None, true, update, for_init)? {
                Operand::Expr(e) => e,
                Operand::Private(p) => return self.unexpected(Some(p.start)),
            };
            self.check_expression_errors(ref_destructuring_errors.as_deref(), true)?;
            if update {
                self.check_lval_simple_expr(&argument, BIND_NONE, None)?;
            } else if self.strict && op == "delete" && is_local_variable_access(&argument) {
                return self.raise_recoverable(start, "Deleting local variable in strict mode");
            } else if op == "delete" && is_private_field_access(&argument) {
                return self.raise_recoverable(start, "Private fields can not be deleted");
            } else {
                saw_unary = true;
            }
            let end = self.last_tok_end;
            expr = Operand::Expr(if update {
                Expression::UpdateExpression(UpdateExpression {
                    start,
                    end,
                    operator: UpdateOperator::from_str(op).unwrap(),
                    prefix: true,
                    argument: Box::new(argument),
                })
            } else {
                Expression::UnaryExpression(UnaryExpression {
                    start,
                    end,
                    operator: UnaryOperator::from_str(op).unwrap(),
                    prefix: true,
                    argument: Box::new(argument),
                })
            });
        } else if !saw_unary && self.type_ == T::PrivateId {
            if (for_init.truthy() || self.private_name_stack.is_empty()) && self.options.check_private_fields {
                return self.unexpected(None);
            }
            expr = Operand::Private(self.parse_private_ident()?);
            // only could be private fields in 'in', such as #x in obj
            if self.type_ != T::In {
                return self.unexpected(None);
            }
        } else {
            let mut e = self.parse_expr_subscripts(ref_destructuring_errors.as_deref_mut(), for_init)?;
            if self.check_expression_errors(ref_destructuring_errors.as_deref(), false)? {
                return Ok(Operand::Expr(e));
            }
            while self.type_.postfix() && !self.can_insert_semicolon() {
                let operator = UpdateOperator::from_str(self.tok_text()).unwrap();
                self.check_lval_simple_expr(&e, BIND_NONE, None)?;
                self.next()?;
                e = Expression::UpdateExpression(UpdateExpression {
                    start: start_pos,
                    end: self.last_tok_end,
                    operator,
                    prefix: false,
                    argument: Box::new(e),
                });
            }
            expr = Operand::Expr(e);
        }

        let is_arrow = matches!(&expr, Operand::Expr(e) if is_arrow_at(e, start_pos));
        if !inc_dec && !is_arrow && self.eat(T::Starstar)? {
            if saw_unary {
                self.unexpected(Some(self.last_tok_start))
            } else {
                let right = self.parse_maybe_unary(None, false, false, for_init)?;
                Ok(Operand::Expr(self.build_binary(start_pos, expr, right, "**", false)?))
            }
        } else {
            Ok(expr)
        }
    }

    // Parse call, dot, and `[]`-subscript expressions.

    pub fn parse_expr_subscripts(
        &mut self,
        mut ref_destructuring_errors: Option<&mut DestructuringErrors>,
        for_init: ForInit,
    ) -> PResult<Expression> {
        let start_pos = self.start;
        let mut old_double_proto = -1;
        let mut old_shorthand_assign = -1;
        if let Some(r) = ref_destructuring_errors.as_deref_mut() {
            old_double_proto = r.double_proto;
            old_shorthand_assign = r.shorthand_assign;
            r.double_proto = -1;
            r.shorthand_assign = -1;
        }
        let expr = self.parse_expr_atom(ref_destructuring_errors.as_deref_mut(), for_init, false)?;
        if matches!(expr, Expression::ArrowFunctionExpression(_))
            && &self.input[self.last_tok_start as usize..self.last_tok_end as usize] != ")"
        {
            return Ok(expr);
        }
        let expr_end = expr.span().end;
        let result = self.parse_subscripts(expr, start_pos, false, for_init)?;
        if let Some(r) = ref_destructuring_errors {
            let span = result.span();
            if span.end > expr_end {
                self.check_expression_errors(Some(r), true)?;
                if r.parenthesized_assign >= span.start as i64 {
                    r.parenthesized_assign = -1;
                }
                if r.parenthesized_bind >= span.start as i64 {
                    r.parenthesized_bind = -1;
                }
                if r.trailing_comma >= span.start as i64 {
                    r.trailing_comma = -1;
                }
            }
            if old_double_proto > -1 {
                r.double_proto = old_double_proto;
            }
            if old_shorthand_assign > -1 {
                r.shorthand_assign = old_shorthand_assign;
            }
        }
        Ok(result)
    }

    pub fn parse_subscripts(
        &mut self,
        mut base: Expression,
        start_pos: u32,
        no_calls: bool,
        for_init: ForInit,
    ) -> PResult<Expression> {
        let mut maybe_async_arrow = self.options.ecma_version >= 8
            && matches!(&base, Expression::Identifier(i) if i.name == "async")
            && self.last_tok_end == base.span().end
            && !self.can_insert_semicolon()
            && base.span().end - base.span().start == 5
            && self.potential_arrow_at == base.span().start as i64;
        let mut optional_chained = false;

        loop {
            let base_end = base.span().end;
            let element =
                self.parse_subscript(base, start_pos, no_calls, maybe_async_arrow, optional_chained, for_init)?;

            let optional = match &element {
                Expression::MemberExpression(m) => m.optional,
                Expression::CallExpression(c) => c.optional,
                _ => false,
            };
            if optional {
                optional_chained = true;
            }
            if element.span().end == base_end || matches!(element, Expression::ArrowFunctionExpression(_)) {
                if optional_chained {
                    return Ok(Expression::ChainExpression(ChainExpression {
                        start: start_pos,
                        end: self.last_tok_end,
                        expression: Box::new(element),
                    }));
                }
                return Ok(element);
            }

            base = element;
            maybe_async_arrow = false;
        }
    }

    pub fn should_parse_async_arrow(&mut self) -> PResult<bool> {
        Ok(!self.can_insert_semicolon() && self.eat(T::Arrow)?)
    }

    pub fn parse_subscript_async_arrow(
        &mut self,
        start_pos: u32,
        expr_list: Vec<Option<ArrayElement>>,
        for_init: ForInit,
    ) -> PResult<Expression> {
        let params = expr_list
            .into_iter()
            .flatten()
            .map(|e| match e {
                ArrayElement::Expression(e) => ParamItem::Expr(e),
                ArrayElement::Spread(SpreadElementNode::SpreadElement(s)) => ParamItem::Spread(s),
            })
            .collect();
        self.parse_arrow_expression(start_pos, params, true, for_init)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn parse_subscript(
        &mut self,
        base: Expression,
        start_pos: u32,
        no_calls: bool,
        maybe_async_arrow: bool,
        optional_chained: bool,
        for_init: ForInit,
    ) -> PResult<Expression> {
        let optional_supported = self.options.ecma_version >= 11;
        let optional = optional_supported && self.eat(T::QuestionDot)?;
        if no_calls && optional {
            return self.raise(self.last_tok_start, "Optional chaining cannot appear in the callee of new expressions");
        }

        let computed = self.eat(T::BracketL)?;
        if computed || (optional && self.type_ != T::ParenL && self.type_ != T::BackQuote) || self.eat(T::Dot)? {
            let property = if computed {
                let p = self.parse_expression(ForInit::No, None)?;
                self.expect(T::BracketR)?;
                PropertyKey::Expression(Box::new(p))
            } else if self.type_ == T::PrivateId && !matches!(base, Expression::Super(_)) {
                PropertyKey::Private(PrivateIdentifierNode::PrivateIdentifier(self.parse_private_ident()?))
            } else {
                let liberal = self.options.allow_reserved != AllowReserved::Never;
                PropertyKey::Expression(Box::new(Expression::Identifier(self.parse_ident(liberal)?)))
            };
            Ok(Expression::MemberExpression(MemberExpression {
                start: start_pos,
                end: self.last_tok_end,
                object: Box::new(base),
                property,
                computed,
                optional,
            }))
        } else if !no_calls && self.eat(T::ParenL)? {
            let mut ref_destructuring_errors = DestructuringErrors::new();
            let old_yield_pos = self.yield_pos;
            let old_await_pos = self.await_pos;
            let old_await_ident_pos = self.await_ident_pos;
            self.yield_pos = 0;
            self.await_pos = 0;
            self.await_ident_pos = 0;
            let expr_list = self.parse_expr_list(
                T::ParenR,
                self.options.ecma_version >= 8,
                false,
                Some(&mut ref_destructuring_errors),
            )?;
            if maybe_async_arrow && !optional && self.should_parse_async_arrow()? {
                self.check_pattern_errors(Some(&ref_destructuring_errors), false)?;
                self.check_yield_await_in_default_params()?;
                if self.await_ident_pos > 0 {
                    return self
                        .raise(self.await_ident_pos, "Cannot use 'await' as identifier inside an async function");
                }
                self.yield_pos = old_yield_pos;
                self.await_pos = old_await_pos;
                self.await_ident_pos = old_await_ident_pos;
                return self.parse_subscript_async_arrow(start_pos, expr_list, for_init);
            }
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
            Ok(Expression::CallExpression(CallExpression {
                ts: None,
                start: start_pos,
                end: self.last_tok_end,
                callee: Box::new(base),
                arguments: expr_list.into_iter().flatten().collect(),
                optional,
            }))
        } else if self.type_ == T::BackQuote {
            if optional || optional_chained {
                return self
                    .raise(self.start, "Optional chaining cannot appear in the tag of tagged template expressions");
            }
            let quasi = self.parse_template(true)?;
            Ok(Expression::TaggedTemplateExpression(TaggedTemplateExpression {
                ts: None,
                start: start_pos,
                end: self.last_tok_end,
                tag: Box::new(base),
                quasi: TemplateLiteralNode::TemplateLiteral(quasi),
            }))
        } else {
            Ok(base)
        }
    }

    // Parse an atomic expression — either a single token that is an
    // expression, an expression started by a keyword like `function` or
    // `new`, or an expression wrapped in punctuation like `()`, `[]`,
    // or `{}`.

    pub fn parse_expr_atom(
        &mut self,
        ref_destructuring_errors: Option<&mut DestructuringErrors>,
        for_init: ForInit,
        for_new: bool,
    ) -> PResult<Expression> {
        // If a division operator appears in an expression position, the
        // tokenizer got confused, and we force it to read a regexp instead.
        if self.type_ == T::Slash {
            self.read_regexp()?;
        }

        let can_be_arrow = self.potential_arrow_at == self.start as i64;
        match self.type_ {
            T::Super => {
                if !self.allow_super() {
                    return self.raise(self.start, "'super' keyword outside a method");
                }
                let start = self.start;
                self.next()?;
                if self.type_ == T::ParenL && !self.allow_direct_super() {
                    return self.raise(start, "super() call outside constructor of a subclass");
                }
                // The `super` keyword can appear at below:
                // SuperProperty:
                //     super [ Expression ]
                //     super . IdentifierName
                // SuperCall:
                //     super ( Arguments )
                if self.type_ != T::Dot && self.type_ != T::BracketL && self.type_ != T::ParenL {
                    return self.unexpected(None);
                }
                Ok(Expression::Super(Super { start, end: self.last_tok_end }))
            }

            T::This => {
                let start = self.start;
                self.next()?;
                Ok(Expression::ThisExpression(ThisExpression { start, end: self.last_tok_end }))
            }

            T::Name => {
                let start_pos = self.start;
                let contains_esc = self.contains_esc;
                let id = self.parse_ident(false)?;
                if self.options.ecma_version >= 8
                    && !contains_esc
                    && id.name == "async"
                    && !self.can_insert_semicolon()
                    && self.eat(T::Function)?
                {
                    self.override_context(TokContext::FExpr);
                    let f = self.parse_function(start_pos, 0, false, true, for_init)?;
                    return Ok(Expression::FunctionExpression(f));
                }
                if can_be_arrow && !self.can_insert_semicolon() {
                    if self.eat(T::Arrow)? {
                        return self.parse_arrow_expression(
                            start_pos,
                            vec![ParamItem::Expr(Expression::Identifier(id))],
                            false,
                            for_init,
                        );
                    }
                    if self.options.ecma_version >= 8
                        && id.name == "async"
                        && self.type_ == T::Name
                        && !contains_esc
                        && (!self.potential_arrow_in_for_await
                            || !matches!(&self.value, TokVal::Str(s) if s == "of")
                            || self.contains_esc)
                    {
                        let id = self.parse_ident(false)?;
                        if self.can_insert_semicolon() || !self.eat(T::Arrow)? {
                            return self.unexpected(None);
                        }
                        return self.parse_arrow_expression(
                            start_pos,
                            vec![ParamItem::Expr(Expression::Identifier(id))],
                            true,
                            for_init,
                        );
                    }
                }
                Ok(Expression::Identifier(id))
            }

            T::Regexp => {
                let (pattern, flags) = match std::mem::take(&mut self.value) {
                    TokVal::Regex { pattern, flags } => (pattern, flags),
                    _ => unreachable!(),
                };
                let mut node = self.parse_literal(LiteralValue::RegExp, None)?;
                node.regex = Some(RegExpLiteral { pattern, flags });
                Ok(Expression::Literal(node))
            }

            T::Num | T::String => Ok(Expression::Literal(self.parse_literal_token()?)),

            T::Null | T::True | T::False => {
                let start = self.start;
                let value = if self.type_ == T::Null {
                    LiteralValue::Null
                } else {
                    LiteralValue::Boolean(self.type_ == T::True)
                };
                let raw = self.type_.keyword().unwrap().to_string();
                self.next()?;
                Ok(Expression::Literal(Literal { start, end: self.last_tok_end, value, raw, regex: None, bigint: None }))
            }

            T::ParenL => {
                let start = self.start;
                let expr = self.parse_paren_and_distinguish_expression(can_be_arrow, for_init)?;
                if let Some(r) = ref_destructuring_errors {
                    if r.parenthesized_assign < 0 && !self.is_simple_assign_target(&expr) {
                        r.parenthesized_assign = start as i64;
                    }
                    if r.parenthesized_bind < 0 {
                        r.parenthesized_bind = start as i64;
                    }
                }
                Ok(expr)
            }

            T::BracketL => {
                let start = self.start;
                self.next()?;
                let elements = self.parse_expr_list(T::BracketR, true, true, ref_destructuring_errors)?;
                Ok(Expression::ArrayExpression(ArrayExpression { start, end: self.last_tok_end, elements, ts: None }))
            }

            T::BraceL => {
                self.override_context(TokContext::BExpr);
                Ok(Expression::ObjectExpression(self.parse_obj_expr(ref_destructuring_errors)?))
            }

            T::Function => {
                let start = self.start;
                self.next()?;
                Ok(Expression::FunctionExpression(self.parse_function(start, 0, false, false, ForInit::No)?))
            }

            T::Class => {
                let start = self.start;
                Ok(Expression::ClassExpression(self.parse_class(start, super::statement::IsStatement::No)?))
            }

            T::New => self.parse_new(),

            T::BackQuote => Ok(Expression::TemplateLiteral(self.parse_template(false)?)),

            T::Import => {
                if self.options.ecma_version >= 11 {
                    self.parse_expr_import(for_new)
                } else {
                    self.unexpected(None)
                }
            }

            _ => self.parse_expr_atom_default(),
        }
    }

    pub fn parse_expr_atom_default(&mut self) -> PResult<Expression> {
        self.unexpected(None)
    }

    pub fn parse_expr_import(&mut self, for_new: bool) -> PResult<Expression> {
        let start = self.start;

        // Consume `import` as an identifier for `import.meta`.
        // Because `this.parseIdent(true)` doesn't check escape sequences, it needs the check of `this.containsEsc`.
        if self.contains_esc {
            return self.raise_recoverable(self.start, "Escape sequence in keyword import");
        }
        self.next()?;

        if self.type_ == T::ParenL && !for_new {
            self.parse_dynamic_import(start)
        } else if self.type_ == T::Dot {
            let meta = Identifier { start, end: self.last_tok_end, name: "import".into(), ts: None };
            self.parse_import_meta(start, meta)
        } else {
            self.unexpected(None)
        }
    }

    pub fn parse_dynamic_import(&mut self, start: u32) -> PResult<Expression> {
        self.next()?; // skip `(`

        // Parse node.source.
        let source = self.parse_maybe_assign(ForInit::No, None)?;
        let mut options = None;

        if self.options.ecma_version >= 16 {
            if !self.eat(T::ParenR)? {
                self.expect(T::Comma)?;
                if !self.after_trailing_comma(T::ParenR, false)? {
                    options = Some(Box::new(self.parse_maybe_assign(ForInit::No, None)?));
                    if !self.eat(T::ParenR)? {
                        self.expect(T::Comma)?;
                        if !self.after_trailing_comma(T::ParenR, false)? {
                            return self.unexpected(None);
                        }
                    }
                }
            }
        } else {
            // Verify ending.
            if !self.eat(T::ParenR)? {
                let error_pos = self.start;
                if self.eat(T::Comma)? && self.eat(T::ParenR)? {
                    return self.raise_recoverable(error_pos, "Trailing comma is not allowed in import()");
                } else {
                    return self.unexpected(Some(error_pos));
                }
            }
        }

        Ok(Expression::ImportExpression(ImportExpression {
            ts: None,
            start,
            end: self.last_tok_end,
            source: Box::new(source),
            options,
        }))
    }

    pub fn parse_import_meta(&mut self, start: u32, meta: Identifier) -> PResult<Expression> {
        self.next()?; // skip `.`

        let contains_esc = self.contains_esc;
        let property = self.parse_ident(true)?;

        if property.name != "meta" {
            return self
                .raise_recoverable(property.start, "The only valid meta property for import is 'import.meta'");
        }
        if contains_esc {
            return self.raise_recoverable(start, "'import.meta' must not contain escaped characters");
        }
        if self.options.source_type != AcornSourceType::Module && !self.options.allow_import_export_everywhere {
            return self.raise_recoverable(start, "Cannot use 'import.meta' outside a module");
        }

        Ok(Expression::MetaProperty(MetaProperty { start, end: self.last_tok_end, meta, property }))
    }

    /// `parseLiteral(this.value)` for a num/string token.
    pub fn parse_literal_token(&mut self) -> PResult<Literal> {
        let (value, bigint) = match std::mem::take(&mut self.value) {
            TokVal::Str(s) => (LiteralValue::String(s), None),
            TokVal::Num(n) => (LiteralValue::Number(n), None),
            TokVal::BigInt(b) => (LiteralValue::BigInt, Some(b)),
            _ => (LiteralValue::Null, None),
        };
        self.parse_literal(value, bigint)
    }

    pub fn parse_literal(&mut self, value: LiteralValue, bigint_value: Option<String>) -> PResult<Literal> {
        let start = self.start;
        let raw = self.input[self.start as usize..self.end as usize].to_string();
        let bigint = if raw.as_bytes().last() == Some(&b'n') {
            Some(match bigint_value {
                Some(v) => v,
                None => raw[..raw.len() - 1].replace('_', ""),
            })
        } else {
            None
        };
        self.next()?;
        Ok(Literal { start, end: self.last_tok_end, value, raw, regex: None, bigint })
    }

    pub fn parse_paren_expression(&mut self) -> PResult<Expression> {
        self.expect(T::ParenL)?;
        let val = self.parse_expression(ForInit::No, None)?;
        self.expect(T::ParenR)?;
        Ok(val)
    }

    pub fn should_parse_arrow(&self) -> bool {
        !self.can_insert_semicolon()
    }

    pub fn parse_paren_and_distinguish_expression(
        &mut self,
        can_be_arrow: bool,
        for_init: ForInit,
    ) -> PResult<Expression> {
        let start_pos = self.start;
        let val;
        let allow_trailing_comma = self.options.ecma_version >= 8;
        if self.options.ecma_version >= 6 {
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
            // Do not save awaitIdentPos to allow checking awaits nested in parameters
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
                    expr_list.push(ParamItem::Rest(self.parse_rest_binding()?));
                    if self.type_ == T::Comma {
                        return self.raise_recoverable(self.start, "Comma is not permitted after the rest element");
                    }
                    break;
                } else {
                    expr_list.push(ParamItem::Expr(
                        self.parse_maybe_assign(ForInit::No, Some(&mut ref_destructuring_errors))?,
                    ));
                }
            }
            let inner_end_pos = self.last_tok_end;
            self.expect(T::ParenR)?;

            if can_be_arrow && self.should_parse_arrow() && self.eat(T::Arrow)? {
                self.check_pattern_errors(Some(&ref_destructuring_errors), false)?;
                self.check_yield_await_in_default_params()?;
                self.yield_pos = old_yield_pos;
                self.await_pos = old_await_pos;
                return self.parse_paren_arrow_list(start_pos, expr_list, for_init);
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

    pub fn parse_paren_arrow_list(
        &mut self,
        start_pos: u32,
        expr_list: Vec<ParamItem>,
        for_init: ForInit,
    ) -> PResult<Expression> {
        self.parse_arrow_expression(start_pos, expr_list, false, for_init)
    }

    // New's precedence is slightly tricky. It must allow its argument to
    // be a `[]` or dot subscript expression, but not a call — at least,
    // not without wrapping it in parentheses. Thus, it uses the noCalls
    // argument to parseSubscripts to prevent it from consuming the
    // argument list.

    pub fn parse_new(&mut self) -> PResult<Expression> {
        if self.contains_esc {
            return self.raise_recoverable(self.start, "Escape sequence in keyword new");
        }
        let start = self.start;
        self.next()?;
        if self.options.ecma_version >= 6 && self.type_ == T::Dot {
            let meta = Identifier { start, end: self.last_tok_end, name: "new".into(), ts: None };
            self.next()?;
            let contains_esc = self.contains_esc;
            let property = self.parse_ident(true)?;
            if property.name != "target" {
                return self.raise_recoverable(property.start, "The only valid meta property for new is 'new.target'");
            }
            if contains_esc {
                return self.raise_recoverable(start, "'new.target' must not contain escaped characters");
            }
            if !self.allow_new_dot_target() {
                return self
                    .raise_recoverable(start, "'new.target' can only be used in functions and class static block");
            }
            return Ok(Expression::MetaProperty(MetaProperty { start, end: self.last_tok_end, meta, property }));
        }
        let start_pos = self.start;
        let atom = self.parse_expr_atom(None, ForInit::No, true)?;
        let callee = self.parse_subscripts(atom, start_pos, true, ForInit::No)?;
        if matches!(callee, Expression::Super(_)) {
            return self.raise_recoverable(start_pos, "Invalid use of 'super'");
        }
        let arguments = if self.eat(T::ParenL)? {
            self.parse_expr_list(T::ParenR, self.options.ecma_version >= 8, false, None)?.into_iter().flatten().collect()
        } else {
            Vec::new()
        };
        Ok(Expression::NewExpression(NewExpression { start, end: self.last_tok_end, callee: Box::new(callee), arguments, ts: None }))
    }

    // Parse template expression.

    pub fn parse_template_element(&mut self, is_tagged: bool) -> PResult<TemplateElement> {
        let start = self.start;
        let value = if self.type_ == T::InvalidTemplate {
            if !is_tagged {
                return self.raise_recoverable(self.start, "Bad escape sequence in untagged template literal");
            }
            let raw = match &self.value {
                TokVal::InvalidTemplate(s) => normalize_crlf(s),
                _ => String::new(),
            };
            TemplateValue { raw, cooked: None }
        } else {
            let cooked = match std::mem::take(&mut self.value) {
                TokVal::Template(s) => s,
                _ => String::new(),
            };
            TemplateValue { raw: normalize_crlf(&self.input[self.start as usize..self.end as usize]), cooked: Some(cooked) }
        };
        self.next()?;
        let tail = self.type_ == T::BackQuote;
        Ok(TemplateElement { start, end: self.last_tok_end, value, tail })
    }

    pub fn parse_template(&mut self, is_tagged: bool) -> PResult<TemplateLiteral> {
        let start = self.start;
        self.next()?;
        let mut expressions = Vec::new();
        let mut cur_elt = self.parse_template_element(is_tagged)?;
        let mut tail = cur_elt.tail;
        let mut quasis = vec![cur_elt];
        while !tail {
            if self.type_ == T::Eof {
                return self.raise(self.pos, "Unterminated template literal");
            }
            self.expect(T::DollarBraceL)?;
            expressions.push(self.parse_expression(ForInit::No, None)?);
            self.expect(T::BraceR)?;
            cur_elt = self.parse_template_element(is_tagged)?;
            tail = cur_elt.tail;
            quasis.push(cur_elt);
        }
        self.next()?;
        Ok(TemplateLiteral { start, end: self.last_tok_end, expressions, quasis })
    }

    pub fn is_async_prop(&self, computed: bool, key: &PropertyKey) -> bool {
        !computed
            && matches!(key, PropertyKey::Expression(e) if matches!(&**e, Expression::Identifier(i) if i.name == "async"))
            && (matches!(self.type_, T::Name | T::Num | T::String | T::BracketL)
                || self.type_.is_keyword()
                || (self.options.ecma_version >= 9 && self.type_ == T::Star))
            && !has_line_break(&self.input[self.last_tok_end as usize..self.start as usize])
    }

    // Parse an object literal or binding pattern.

    pub fn parse_obj_expr(
        &mut self,
        ref_destructuring_errors: Option<&mut DestructuringErrors>,
    ) -> PResult<ObjectExpression> {
        let (start, end, props) = self.parse_obj(false, ref_destructuring_errors)?;
        let properties = props
            .into_iter()
            .map(|p| match p {
                PropResult::Property(p) => ObjectMember::Property(p),
                PropResult::Spread(s) => ObjectMember::SpreadElement(s),
                _ => unreachable!(),
            })
            .collect();
        Ok(ObjectExpression { start, end, properties, ts: None })
    }

    pub fn parse_obj_pattern(&mut self) -> PResult<Pattern> {
        let (start, end, props) = self.parse_obj(true, None)?;
        let properties = props
            .into_iter()
            .map(|p| match p {
                PropResult::PatternProperty(p) => ObjectPatternMember::Property(p),
                PropResult::Rest(r) => ObjectPatternMember::RestElement(r),
                _ => unreachable!(),
            })
            .collect();
        Ok(Pattern::ObjectPattern(ObjectPattern { start, end, properties, ts: None }))
    }

    pub fn parse_obj(
        &mut self,
        is_pattern: bool,
        mut ref_destructuring_errors: Option<&mut DestructuringErrors>,
    ) -> PResult<(u32, u32, Vec<PropResult>)> {
        let start = self.start;
        let mut first = true;
        let mut prop_hash = PropHash::default();
        let mut properties = Vec::new();
        self.next()?;
        while !self.eat(T::BraceR)? {
            if !first {
                self.expect(T::Comma)?;
                if self.options.ecma_version >= 5 && self.after_trailing_comma(T::BraceR, false)? {
                    break;
                }
            } else {
                first = false;
            }

            let prop = self.parse_property(is_pattern, ref_destructuring_errors.as_deref_mut())?;
            if !is_pattern {
                self.check_prop_clash(&prop, &mut prop_hash, ref_destructuring_errors.as_deref_mut())?;
            }
            properties.push(prop);
        }
        Ok((start, self.last_tok_end, properties))
    }

    pub fn parse_property(
        &mut self,
        is_pattern: bool,
        ref_destructuring_errors: Option<&mut DestructuringErrors>,
    ) -> PResult<PropResult> {
        let start = self.start;
        let mut is_generator = false;
        let mut start_pos: Option<u32> = None;
        if self.options.ecma_version >= 9 && self.eat(T::Ellipsis)? {
            if is_pattern {
                let argument = self.parse_ident(false)?;
                if self.type_ == T::Comma {
                    return self.raise_recoverable(self.start, "Comma is not permitted after the rest element");
                }
                return Ok(PropResult::Rest(RestElement {
                    ts: None,
                    start,
                    end: self.last_tok_end,
                    argument: Box::new(Pattern::Identifier(argument)),
                }));
            }
            // Parse argument.
            let has_ref = ref_destructuring_errors.is_some();
            let mut r = ref_destructuring_errors;
            let argument = self.parse_maybe_assign(ForInit::No, r.as_deref_mut())?;
            // To disallow trailing comma via `this.toAssignable()`.
            if self.type_ == T::Comma && has_ref {
                let r = r.unwrap();
                if r.trailing_comma < 0 {
                    r.trailing_comma = self.start as i64;
                }
            }
            // Finish
            return Ok(PropResult::Spread(SpreadElement { start, end: self.last_tok_end, argument, ts: None }));
        }
        if self.options.ecma_version >= 6 {
            if is_pattern || ref_destructuring_errors.is_some() {
                start_pos = Some(self.start);
            }
            if !is_pattern {
                is_generator = self.eat(T::Star)?;
            }
        }
        let contains_esc = self.contains_esc;
        let (mut computed, mut key) = self.parse_property_name()?;
        let is_async;
        if !is_pattern
            && !contains_esc
            && self.options.ecma_version >= 8
            && !is_generator
            && self.is_async_prop(computed, &key)
        {
            is_async = true;
            is_generator = self.options.ecma_version >= 9 && self.eat(T::Star)?;
            (computed, key) = self.parse_property_name()?;
        } else {
            is_async = false;
        }
        self.parse_property_value(
            start,
            computed,
            key,
            is_pattern,
            is_generator,
            is_async,
            start_pos,
            ref_destructuring_errors,
            contains_esc,
        )
    }

    /// `parseGetterSetter`: returns the new key, computed flag, value and kind.
    pub fn parse_getter_setter(&mut self, kind_name: &str) -> PResult<(bool, PropertyKey, Function, PropertyKind)> {
        let kind = if kind_name == "get" { PropertyKind::Get } else { PropertyKind::Set };
        let (computed, key) = self.parse_property_name()?;
        let value = self.parse_method(false, false, false)?;
        let param_count = if kind == PropertyKind::Get { 0 } else { 1 };
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

    #[allow(clippy::too_many_arguments)]
    pub fn parse_property_value(
        &mut self,
        start: u32,
        mut computed: bool,
        mut key: PropertyKey,
        is_pattern: bool,
        is_generator: bool,
        is_async: bool,
        start_pos: Option<u32>,
        ref_destructuring_errors: Option<&mut DestructuringErrors>,
        contains_esc: bool,
    ) -> PResult<PropResult> {
        if (is_generator || is_async) && self.type_ == T::Colon {
            return self.unexpected(None);
        }

        let mut method = false;
        let mut shorthand = false;
        let value: PropValue;
        let kind: PropertyKind;
        let key_ident_name = match &key {
            PropertyKey::Expression(e) => match &**e {
                Expression::Identifier(i) => Some(i.name.clone()),
                _ => None,
            },
            _ => None,
        };

        if self.eat(T::Colon)? {
            value = if is_pattern {
                PropValue::Pat(self.parse_maybe_default(self.start, None)?)
            } else {
                PropValue::Expr(self.parse_maybe_assign(ForInit::No, ref_destructuring_errors)?)
            };
            kind = PropertyKind::Init;
        } else if self.options.ecma_version >= 6 && self.type_ == T::ParenL {
            if is_pattern {
                return self.unexpected(None);
            }
            method = true;
            value = PropValue::Expr(Expression::FunctionExpression(self.parse_method(is_generator, is_async, false)?));
            kind = PropertyKind::Init;
        } else if !is_pattern
            && !contains_esc
            && self.options.ecma_version >= 5
            && !computed
            && matches!(key_ident_name.as_deref(), Some("get") | Some("set"))
            && (self.type_ != T::Comma && self.type_ != T::BraceR && self.type_ != T::Eq)
        {
            if is_generator || is_async {
                return self.unexpected(None);
            }
            let (c, k, v, kd) = self.parse_getter_setter(key_ident_name.as_deref().unwrap())?;
            computed = c;
            key = k;
            value = PropValue::Expr(Expression::FunctionExpression(v));
            kind = kd;
        } else if self.options.ecma_version >= 6 && !computed && key_ident_name.is_some() {
            if is_generator || is_async {
                return self.unexpected(None);
            }
            let key_ident = match &key {
                PropertyKey::Expression(e) => match &**e {
                    Expression::Identifier(i) => i.clone(),
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            };
            self.check_unreserved(key_ident.start, key_ident.end, &key_ident.name)?;
            if key_ident.name == "await" && self.await_ident_pos == 0 {
                self.await_ident_pos = start_pos.unwrap_or(0);
            }
            if is_pattern {
                value = PropValue::Pat(
                    self.parse_maybe_default(start_pos.unwrap_or(start), Some(Pattern::Identifier(key_ident)))?,
                );
            } else if self.type_ == T::Eq && ref_destructuring_errors.is_some() {
                let r = ref_destructuring_errors.unwrap();
                if r.shorthand_assign < 0 {
                    r.shorthand_assign = self.start as i64;
                }
                // `parseMaybeDefault(startPos, startLoc, this.copyNode(prop.key))`, held as an
                // AssignmentExpression until `toAssignable` (see module docs).
                self.expect(T::Eq)?;
                let right = self.parse_maybe_assign(ForInit::No, None)?;
                value = PropValue::Expr(Expression::AssignmentExpression(AssignmentExpression {
                    start: start_pos.unwrap_or(start),
                    end: self.last_tok_end,
                    operator: AssignmentOperator::Assign,
                    left: Pattern::Identifier(key_ident),
                    right: Box::new(right),
                }));
            } else {
                value = PropValue::Expr(Expression::Identifier(key_ident));
            }
            kind = PropertyKind::Init;
            shorthand = true;
        } else {
            return self.unexpected(None);
        }

        let end = self.last_tok_end;
        Ok(match value {
            PropValue::Expr(value) => {
                PropResult::Property(Property { start, end, method, shorthand, computed, key, value, kind })
            }
            PropValue::Pat(value) => {
                PropResult::PatternProperty(PatternProperty { start, end, method, shorthand, computed, key, value, kind })
            }
        })
    }

    pub fn parse_property_name(&mut self) -> PResult<(bool, PropertyKey)> {
        if self.options.ecma_version >= 6 && self.eat(T::BracketL)? {
            let key = self.parse_maybe_assign(ForInit::No, None)?;
            self.expect(T::BracketR)?;
            return Ok((true, PropertyKey::Expression(Box::new(key))));
        }
        let key = if self.type_ == T::Num || self.type_ == T::String {
            self.parse_expr_atom(None, ForInit::No, false)?
        } else {
            let liberal = self.options.allow_reserved != AllowReserved::Never;
            Expression::Identifier(self.parse_ident(liberal)?)
        };
        Ok((false, PropertyKey::Expression(Box::new(key))))
    }

    // Parse object or class method.

    pub fn parse_method(&mut self, is_generator: bool, is_async: bool, allow_direct_super: bool) -> PResult<Function> {
        let start = self.start;
        let old_yield_pos = self.yield_pos;
        let old_await_pos = self.await_pos;
        let old_await_ident_pos = self.await_ident_pos;

        // initFunction
        let generator = self.options.ecma_version >= 6 && is_generator;
        let is_async_node = self.options.ecma_version >= 8 && is_async;

        self.yield_pos = 0;
        self.await_pos = 0;
        self.await_ident_pos = 0;
        self.enter_scope(
            function_flags(is_async, generator) | SCOPE_SUPER | if allow_direct_super { SCOPE_DIRECT_SUPER } else { 0 },
        );

        self.expect(T::ParenL)?;
        let params: Vec<Pattern> =
            self.parse_binding_list(T::ParenR, false, self.options.ecma_version >= 8)?.into_iter().flatten().collect();
        self.check_yield_await_in_default_params()?;
        let body = self.parse_function_body(start, &params, None, false, true, ForInit::No)?.into_block();

        self.yield_pos = old_yield_pos;
        self.await_pos = old_await_pos;
        self.await_ident_pos = old_await_ident_pos;
        Ok(Function {
            ts: None,
            start,
            end: self.last_tok_end,
            id: None,
            expression: false,
            generator,
            is_async: is_async_node,
            params,
            body: BlockStatementNode::BlockStatement(body),
        })
    }

    // Parse arrow function expression with given parameters.

    pub fn parse_arrow_expression(
        &mut self,
        start: u32,
        params: Vec<ParamItem>,
        is_async: bool,
        for_init: ForInit,
    ) -> PResult<Expression> {
        let old_yield_pos = self.yield_pos;
        let old_await_pos = self.await_pos;
        let old_await_ident_pos = self.await_ident_pos;

        self.enter_scope(function_flags(is_async, false) | SCOPE_ARROW);
        let is_async_node = self.options.ecma_version >= 8 && is_async;

        self.yield_pos = 0;
        self.await_pos = 0;
        self.await_ident_pos = 0;

        let params = self.to_assignable_params(params, true)?;
        let body = self.parse_function_body(start, &params, None, true, false, for_init)?;

        self.yield_pos = old_yield_pos;
        self.await_pos = old_await_pos;
        self.await_ident_pos = old_await_ident_pos;
        let (expression, body) = match body {
            FnBody::Expr(e) => (true, ArrowBody::Expression(Box::new(e))),
            FnBody::Block(b) => (false, ArrowBody::Block(BlockStatementNode::BlockStatement(b))),
        };
        Ok(Expression::ArrowFunctionExpression(ArrowFunctionExpression {
            ts: None,
            start,
            end: self.last_tok_end,
            id: None,
            expression,
            generator: false,
            is_async: is_async_node,
            params,
            body,
        }))
    }

    // Parse function body and check parameters.

    pub fn parse_function_body(
        &mut self,
        node_start: u32,
        params: &[Pattern],
        id: Option<&Identifier>,
        is_arrow_function: bool,
        is_method: bool,
        for_init: ForInit,
    ) -> PResult<FnBody> {
        let is_expression = is_arrow_function && self.type_ != T::BraceL;
        let old_strict = self.strict;
        let mut use_strict = false;

        let body;
        if is_expression {
            body = FnBody::Expr(self.parse_maybe_assign(for_init, None)?);
            self.check_params(params, false)?;
        } else {
            let non_simple = self.options.ecma_version >= 7 && !self.is_simple_param_list(params);
            if !old_strict || non_simple {
                use_strict = self.strict_directive(self.end);
                // If this is a strict mode function, verify that argument names
                // are not repeated, and it does not try to bind the words `eval`
                // or `arguments`.
                if use_strict && non_simple {
                    return self.raise_recoverable(
                        node_start,
                        "Illegal 'use strict' directive in function with non-simple parameter list",
                    );
                }
            }
            // Start a new scope with regard to labels and the `inFunction`
            // flag (restore them to their old value afterwards).
            let old_labels = std::mem::take(&mut self.labels);
            if use_strict {
                self.strict = true;
            }

            // Add the params to varDeclaredNames to ensure that an error is thrown
            // if a let/const declaration in the function clashes with one of the params.
            let allow_duplicates = !old_strict
                && !use_strict
                && !is_arrow_function
                && !is_method
                && self.is_simple_param_list(params);
            self.check_params(params, allow_duplicates)?;
            // Ensure the function name isn't a forbidden identifier in strict mode, e.g. 'eval'
            if self.strict {
                if let Some(id) = id {
                    self.check_lval_identifier(id, BIND_OUTSIDE, None)?;
                }
            }
            let mut block = self.parse_block(false, None, use_strict && !old_strict)?;
            self.adapt_directive_prologue(&mut block.body);
            self.labels = old_labels;
            body = FnBody::Block(block);
        }
        self.exit_scope();
        Ok(body)
    }

    pub fn is_simple_param_list(&self, params: &[Pattern]) -> bool {
        params.iter().all(|p| matches!(p, Pattern::Identifier(_)))
    }

    // Checks function params for various disallowed patterns such as using "eval"
    // or "arguments" and duplicate parameters.

    pub fn check_params(&mut self, params: &[Pattern], allow_duplicates: bool) -> PResult<()> {
        let mut name_hash: HashSet<String> = HashSet::new();
        for param in params {
            self.check_lval_inner_pattern(param, BIND_VAR, if allow_duplicates { None } else { Some(&mut name_hash) })?;
        }
        Ok(())
    }

    // Parses a comma-separated list of expressions, and returns them as
    // an array. `close` is the token type that ends the list, and
    // `allowEmpty` can be turned on to allow subsequent commas with
    // nothing in between them to be parsed as `null` (which is needed
    // for array literals).

    pub fn parse_expr_list(
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
                let s = self.parse_spread(ref_destructuring_errors.as_deref_mut())?;
                if let Some(r) = ref_destructuring_errors.as_deref_mut() {
                    if self.type_ == T::Comma && r.trailing_comma < 0 {
                        r.trailing_comma = self.start as i64;
                    }
                }
                Some(ArrayElement::Spread(SpreadElementNode::SpreadElement(s)))
            } else {
                Some(ArrayElement::Expression(self.parse_maybe_assign(ForInit::No, ref_destructuring_errors.as_deref_mut())?))
            };
            elts.push(elt);
        }
        Ok(elts)
    }

    pub fn check_unreserved(&self, start: u32, end: u32, name: &str) -> PResult<()> {
        if self.in_generator() && name == "yield" {
            return self.raise_recoverable(start, "Cannot use 'yield' as identifier inside a generator");
        }
        if self.in_async() && name == "await" {
            return self.raise_recoverable(start, "Cannot use 'await' as identifier inside an async function");
        }
        if (self.current_this_scope().flags & SCOPE_VAR) == 0 && name == "arguments" {
            return self.raise_recoverable(start, "Cannot use 'arguments' in class field initializer");
        }
        if self.in_class_static_block() && (name == "arguments" || name == "await") {
            return self.raise(start, format!("Cannot use {} in class static initialization block", name));
        }
        if self.is_keyword_word(name) {
            return self.raise(start, format!("Unexpected keyword '{}'", name));
        }
        if self.options.ecma_version < 6 && self.input[start as usize..end as usize].contains('\\') {
            return Ok(());
        }
        let reserved = if self.strict { self.reserved_words_strict(name) } else { self.reserved_words(name) };
        if reserved {
            if !self.in_async() && name == "await" {
                return self.raise_recoverable(start, "Cannot use keyword 'await' outside an async function");
            }
            return self.raise_recoverable(start, format!("The keyword '{}' is reserved", name));
        }
        Ok(())
    }

    // Parse the next token as an identifier. If `liberal` is true (used
    // when parsing properties), it will also convert keywords into
    // identifiers.

    pub fn parse_ident(&mut self, liberal: bool) -> PResult<Identifier> {
        let (start, name) = self.parse_ident_node()?;
        self.next_ex(liberal)?;
        let node = Identifier { start, end: self.last_tok_end, name, ts: None };
        if !liberal {
            self.check_unreserved(node.start, node.end, &node.name)?;
            if node.name == "await" && self.await_ident_pos == 0 {
                self.await_ident_pos = node.start;
            }
        }
        Ok(node)
    }

    pub fn parse_ident_node(&mut self) -> PResult<(u32, String)> {
        let start = self.start;
        let name;
        if self.type_ == T::Name {
            name = match std::mem::take(&mut self.value) {
                TokVal::Str(s) => s,
                _ => String::new(),
            };
        } else if let Some(kw) = self.type_.keyword() {
            name = kw.to_string();

            // To fix https://github.com/acornjs/acorn/issues/575
            // `class` and `function` keywords push new context into this.context.
            // But there is no chance to pop the context if the keyword is consumed as an identifier such as a property name.
            // If the previous token is a dot, this does not apply because the context-managing code already ignored the keyword
            if (name == "class" || name == "function")
                && (self.last_tok_end != self.last_tok_start + 1
                    || self.input.as_bytes().get(self.last_tok_start as usize) != Some(&b'.'))
            {
                self.context.pop();
            }
            self.type_ = T::Name;
        } else {
            return self.unexpected(None);
        }
        Ok((start, name))
    }

    pub fn parse_private_ident(&mut self) -> PResult<PrivateIdentifier> {
        let start = self.start;
        let name = if self.type_ == T::PrivateId {
            match std::mem::take(&mut self.value) {
                TokVal::Str(s) => s,
                _ => String::new(),
            }
        } else {
            return self.unexpected(None);
        };
        self.next()?;
        let node = PrivateIdentifier { start, end: self.last_tok_end, name };

        // For validating existence
        if self.options.check_private_fields {
            match self.private_name_stack.last_mut() {
                None => {
                    return self
                        .raise(node.start, format!("Private field '#{}' must be declared in an enclosing class", node.name))
                }
                Some(top) => top.used.push((node.name.clone(), node.start)),
            }
        }

        Ok(node)
    }

    // Parses yield expression inside generator.

    pub fn parse_yield(&mut self, for_init: ForInit) -> PResult<Expression> {
        if self.yield_pos == 0 {
            self.yield_pos = self.start;
        }

        let start = self.start;
        self.next()?;
        let delegate;
        let argument;
        if self.type_ == T::Semi
            || self.can_insert_semicolon()
            || (self.type_ != T::Star && !self.type_.starts_expr())
        {
            delegate = false;
            argument = None;
        } else {
            delegate = self.eat(T::Star)?;
            argument = Some(Box::new(self.parse_maybe_assign(for_init, None)?));
        }
        Ok(Expression::YieldExpression(YieldExpression { start, end: self.last_tok_end, delegate, argument }))
    }

    pub fn parse_await(&mut self, for_init: ForInit) -> PResult<Expression> {
        if self.await_pos == 0 {
            self.await_pos = self.start;
        }

        let start = self.start;
        self.next()?;
        let argument = match self.parse_maybe_unary(None, true, false, for_init)? {
            Operand::Expr(e) => e,
            Operand::Private(p) => return self.unexpected(Some(p.start)),
        };
        Ok(Expression::AwaitExpression(AwaitExpression { start, end: self.last_tok_end, argument: Box::new(argument) }))
    }
}

fn is_local_variable_access(node: &Expression) -> bool {
    match node {
        Expression::Identifier(_) => true,
        Expression::ParenthesizedExpression(p) => is_local_variable_access(&p.expression),
        _ => false,
    }
}

fn is_private_field_access(node: &Expression) -> bool {
    match node {
        Expression::MemberExpression(m) => matches!(m.property, PropertyKey::Private(_)),
        Expression::ChainExpression(c) => is_private_field_access(&c.expression),
        Expression::ParenthesizedExpression(p) => is_private_field_access(&p.expression),
        _ => false,
    }
}

/// `.replace(/\r\n?/g, "\n")`
fn normalize_crlf(s: &str) -> String {
    if !s.contains('\r') {
        return s.to_string();
    }
    s.replace("\r\n", "\n").replace('\r', "\n")
}
