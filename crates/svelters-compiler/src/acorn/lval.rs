//! Port of acorn 8.19.0 `src/lval.js`.
//!
//! SVELTERS: acorn converts expression nodes into patterns in place (`node.type =
//! "ObjectPattern"`). Here `to_assignable` consumes a typed `Expression` and returns a typed
//! `Pattern`; nodes that are already patterns go through `to_assignable_pattern`. A
//! parenthesized target whose inner expression converts to a destructuring pattern cannot be
//! represented, so the error acorn's following `checkLVal*` call would raise is raised right
//! away.

use std::collections::HashSet;

use super::parseutil::DestructuringErrors;
use super::scopeflags::*;
use super::state::{PResult, Parser};
use super::tokentype::TokenType as T;
use crate::estree::*;

/// An element of an arrow parameter list before `toAssignableList`.
pub enum ParamItem {
    Expr(Expression),
    Spread(SpreadElement),
    Rest(RestElement),
}

fn pattern_to_expression(p: Pattern) -> Result<Expression, Pattern> {
    match p {
        Pattern::Identifier(i) => Ok(Expression::Identifier(i)),
        Pattern::MemberExpression(m) => Ok(Expression::MemberExpression(*m)),
        Pattern::ParenthesizedExpression(p) => Ok(Expression::ParenthesizedExpression(p)),
        other => Err(other),
    }
}

impl<'a> Parser<'a> {
    // Convert existing expression atom to assignable pattern
    // if possible.

    pub fn to_assignable(
        &mut self,
        node: Expression,
        is_binding: bool,
        ref_destructuring_errors: Option<&mut DestructuringErrors>,
    ) -> PResult<Pattern> {
        if self.options.ecma_version < 6 {
            if let Some(r) = ref_destructuring_errors {
                self.check_pattern_errors(Some(r), true)?;
            }
            // SVELTERS: acorn leaves the node unchanged; `checkLValPattern` then rejects anything
            // that is not a simple target.
            return match node {
                Expression::Identifier(i) => Ok(Pattern::Identifier(i)),
                Expression::MemberExpression(m) => Ok(Pattern::MemberExpression(Box::new(m))),
                Expression::ParenthesizedExpression(p) => Ok(Pattern::ParenthesizedExpression(p)),
                other => self.raise(other.span().start, if is_binding { "Binding rvalue" } else { "Assigning to rvalue" }),
            };
        }
        match node {
            Expression::Identifier(i) => {
                if self.in_async() && i.name == "await" {
                    return self.raise(i.start, "Cannot use 'await' as identifier inside an async function");
                }
                Ok(Pattern::Identifier(i))
            }

            Expression::ObjectExpression(o) => {
                if let Some(r) = ref_destructuring_errors {
                    self.check_pattern_errors(Some(r), true)?;
                }
                let mut properties = Vec::with_capacity(o.properties.len());
                for prop in o.properties {
                    let member = match prop {
                        ObjectMember::Property(p) => {
                            // AssignmentProperty has type === "Property"
                            if p.kind != PropertyKind::Init {
                                let key_start = property_key_start(&p.key);
                                return self.raise(key_start, "Object pattern can't contain getter or setter");
                            }
                            let value = self.to_assignable(p.value, is_binding, None)?;
                            ObjectPatternMember::Property(PatternProperty {
                                start: p.start,
                                end: p.end,
                                method: p.method,
                                shorthand: p.shorthand,
                                computed: p.computed,
                                key: p.key,
                                value,
                                kind: p.kind,
                            })
                        }
                        ObjectMember::SpreadElement(s) => {
                            let rest = self.to_assignable_spread(s, is_binding)?;
                            // Early error:
                            //   AssignmentRestProperty[Yield, Await] :
                            //     `...` DestructuringAssignmentTarget[Yield, Await]
                            //
                            //   It is a Syntax Error if |DestructuringAssignmentTarget| is an |ArrayLiteral| or an |ObjectLiteral|.
                            if matches!(*rest.argument, Pattern::ArrayPattern(_) | Pattern::ObjectPattern(_)) {
                                return self.raise(rest.argument.span().start, "Unexpected token");
                            }
                            ObjectPatternMember::RestElement(rest)
                        }
                    };
                    properties.push(member);
                }
                Ok(Pattern::ObjectPattern(ObjectPattern { start: o.start, end: o.end, properties, ts: None }))
            }

            Expression::ArrayExpression(a) => {
                if let Some(r) = ref_destructuring_errors {
                    self.check_pattern_errors(Some(r), true)?;
                }
                let elements = self.to_assignable_list(a.elements, is_binding)?;
                Ok(Pattern::ArrayPattern(ArrayPattern { start: a.start, end: a.end, elements, ts: None }))
            }

            Expression::AssignmentExpression(a) => {
                if a.operator != AssignmentOperator::Assign {
                    return self.raise(a.left.span().end, "Only '=' operator can be used for specifying default value.");
                }
                let left = self.to_assignable_pattern(a.left, is_binding)?;
                Ok(Pattern::AssignmentPattern(AssignmentPattern {
                    ts: None,
                    start: a.start,
                    end: a.end,
                    left: Box::new(left),
                    right: a.right,
                }))
            }

            Expression::ParenthesizedExpression(p) => {
                let (start, end) = (p.start, p.end);
                let inner = self.to_assignable(*p.expression, is_binding, ref_destructuring_errors)?;
                match pattern_to_expression(inner) {
                    Ok(e) => Ok(Pattern::ParenthesizedExpression(ParenthesizedExpression {
                        start,
                        end,
                        expression: Box::new(e),
                    })),
                    Err(inner) => {
                        // SVELTERS: what `checkLValSimple` raises for this node.
                        if is_binding {
                            self.raise_recoverable(start, "Binding parenthesized expression")
                        } else {
                            self.raise(inner.span().start, "Assigning to rvalue")
                        }
                    }
                }
            }

            Expression::ChainExpression(c) => {
                self.raise_recoverable(c.start, "Optional chaining cannot appear in left-hand side")
            }

            Expression::MemberExpression(m) if !is_binding => Ok(Pattern::MemberExpression(Box::new(m))),

            other => self.raise(other.span().start, "Assigning to rvalue"),
        }
    }

    /// `toAssignable` on a node that already is a pattern (acorn's `ObjectPattern`,
    /// `ArrayPattern`, `AssignmentPattern`, `RestElement` cases are no-ops).
    pub fn to_assignable_pattern(&mut self, node: Pattern, is_binding: bool) -> PResult<Pattern> {
        if self.options.ecma_version < 6 {
            return Ok(node);
        }
        match node {
            Pattern::Identifier(i) => self.to_assignable(Expression::Identifier(i), is_binding, None),
            Pattern::MemberExpression(m) => {
                if is_binding {
                    return self.raise(m.start, "Assigning to rvalue");
                }
                Ok(Pattern::MemberExpression(m))
            }
            Pattern::ParenthesizedExpression(p) => {
                self.to_assignable(Expression::ParenthesizedExpression(p), is_binding, None)
            }
            other => Ok(other),
        }
    }

    /// The `SpreadElement` case of `toAssignable`.
    pub fn to_assignable_spread(&mut self, node: SpreadElement, is_binding: bool) -> PResult<RestElement> {
        let argument = self.to_assignable(node.argument, is_binding, None)?;
        if let Pattern::AssignmentPattern(a) = &argument {
            return self.raise(a.start, "Rest elements cannot have a default value");
        }
        Ok(RestElement { start: node.start, end: node.end, argument: Box::new(argument), ts: None })
    }

    // Convert list of expression atoms to binding list.

    pub fn to_assignable_list(
        &mut self,
        expr_list: Vec<Option<ArrayElement>>,
        is_binding: bool,
    ) -> PResult<Vec<Option<Pattern>>> {
        let mut out = Vec::with_capacity(expr_list.len());
        for elt in expr_list {
            out.push(match elt {
                None => None,
                Some(ArrayElement::Expression(e)) => Some(self.to_assignable(e, is_binding, None)?),
                Some(ArrayElement::Spread(SpreadElementNode::SpreadElement(s))) => {
                    Some(Pattern::RestElement(self.to_assignable_spread(s, is_binding)?))
                }
            });
        }
        if let Some(Some(last)) = out.last() {
            self.check_es6_binding_rest(last, is_binding)?;
        }
        Ok(out)
    }

    /// `toAssignableList` for arrow function parameters.
    pub fn to_assignable_params(&mut self, expr_list: Vec<ParamItem>, is_binding: bool) -> PResult<Vec<Pattern>> {
        let mut out = Vec::with_capacity(expr_list.len());
        for elt in expr_list {
            out.push(match elt {
                ParamItem::Expr(e) => self.to_assignable(e, is_binding, None)?,
                ParamItem::Spread(s) => Pattern::RestElement(self.to_assignable_spread(s, is_binding)?),
                ParamItem::Rest(r) => Pattern::RestElement(r),
            });
        }
        if let Some(last) = out.last() {
            self.check_es6_binding_rest(last, is_binding)?;
        }
        Ok(out)
    }

    fn check_es6_binding_rest(&self, last: &Pattern, is_binding: bool) -> PResult<()> {
        if self.options.ecma_version == 6 && is_binding {
            if let Pattern::RestElement(r) = last {
                if !matches!(*r.argument, Pattern::Identifier(_)) {
                    return self.unexpected(Some(r.argument.span().start));
                }
            }
        }
        Ok(())
    }

    // Parses spread element.

    pub fn parse_spread(&mut self, ref_destructuring_errors: Option<&mut DestructuringErrors>) -> PResult<SpreadElement> {
        let start = self.start;
        self.next()?;
        let argument = self.parse_maybe_assign(super::expression::ForInit::No, ref_destructuring_errors)?;
        Ok(SpreadElement { start, end: self.last_tok_end, argument, ts: None })
    }

    pub fn parse_rest_binding(&mut self) -> PResult<RestElement> {
        let start = self.start;
        self.next()?;

        // RestElement inside of a function parameter must be an identifier
        if self.options.ecma_version == 6 && self.type_ != T::Name {
            return self.unexpected(None);
        }

        let argument = self.parse_binding_atom()?;

        Ok(RestElement { start, end: self.last_tok_end, argument: Box::new(argument), ts: None })
    }

    // Parses lvalue (assignable) atom.

    pub fn parse_binding_atom(&mut self) -> PResult<Pattern> {
        self.enter_depth()?;
        let r = self.parse_binding_atom_inner();
        self.depth -= 1;
        r
    }

    fn parse_binding_atom_inner(&mut self) -> PResult<Pattern> {
        if self.options.ecma_version >= 6 {
            match self.type_ {
                T::BracketL => {
                    let start = self.start;
                    self.next()?;
                    let elements = self.parse_binding_list(T::BracketR, true, true)?;
                    return Ok(Pattern::ArrayPattern(ArrayPattern { start, end: self.last_tok_end, elements, ts: None }));
                }

                T::BraceL => return self.parse_obj_pattern(),
                _ => {}
            }
        }
        Ok(Pattern::Identifier(self.parse_ident(false)?))
    }

    pub fn parse_binding_list(
        &mut self,
        close: T,
        allow_empty: bool,
        allow_trailing_comma: bool,
    ) -> PResult<Vec<Option<Pattern>>> {
        let mut elts = Vec::new();
        let mut first = true;
        while !self.eat(close)? {
            if first {
                first = false;
            } else {
                self.expect(T::Comma)?;
            }
            if allow_empty && self.type_ == T::Comma {
                elts.push(None);
            } else if allow_trailing_comma && self.after_trailing_comma(close, false)? {
                break;
            } else if self.type_ == T::Ellipsis {
                let rest = self.parse_rest_binding()?;
                elts.push(Some(Pattern::RestElement(rest)));
                if self.type_ == T::Comma {
                    return self.raise_recoverable(self.start, "Comma is not permitted after the rest element");
                }
                self.expect(close)?;
                break;
            } else {
                elts.push(Some(self.parse_assignable_list_item()?));
            }
        }
        Ok(elts)
    }

    pub fn parse_assignable_list_item(&mut self) -> PResult<Pattern> {
        self.parse_maybe_default(self.start, None)
    }

    // Parses assignment pattern around given atom if possible.

    pub fn parse_maybe_default(&mut self, start_pos: u32, left: Option<Pattern>) -> PResult<Pattern> {
        let left = match left {
            Some(l) => l,
            None => self.parse_binding_atom()?,
        };
        if self.options.ecma_version < 6 || !self.eat(T::Eq)? {
            return Ok(left);
        }
        let right = self.parse_maybe_assign(super::expression::ForInit::No, None)?;
        Ok(Pattern::AssignmentPattern(AssignmentPattern {
            ts: None,
            start: start_pos,
            end: self.last_tok_end,
            left: Box::new(left),
            right: Box::new(right),
        }))
    }

    // The following three functions all verify that a node is an lvalue —
    // something that can be bound, or assigned to. In order to do so, they perform
    // a variety of checks:
    //
    // - Check that none of the bound/assigned-to identifiers are reserved words.
    // - Record name declarations for bindings in the appropriate scope.
    // - Check duplicate argument names, if checkClashes is set.
    //
    // If a complex binding pattern is encountered (e.g., object and array
    // destructuring), the entire pattern is recursively checked.
    //
    // There are three versions of checkLVal*() appropriate for different
    // circumstances:
    //
    // - checkLValSimple() shall be used if the syntactic construct supports
    //   nothing other than identifiers and member expressions. Parenthesized
    //   expressions are also correctly handled. This is generally appropriate for
    //   constructs for which the spec says
    //
    //   > It is a Syntax Error if AssignmentTargetType of [the production] is not
    //   > simple.
    //
    //   It is also appropriate for checking if an identifier is valid and not
    //   defined elsewhere, like import declarations or function/class identifiers.
    //
    //   Examples where this is used include:
    //     a += …;
    //     import a from '…';
    //   where a is the node to be checked.
    //
    // - checkLValPattern() shall be used if the syntactic construct supports
    //   anything checkLValSimple() supports, as well as object and array
    //   destructuring patterns. This is generally appropriate for constructs for
    //   which the spec says
    //
    //   > It is a Syntax Error if [the production] is neither an ObjectLiteral nor
    //   > an ArrayLiteral and AssignmentTargetType of [the production] is not
    //   > simple.
    //
    //   Examples where this is used include:
    //     (a = …);
    //     const a = …;
    //     try { … } catch (a) { … }
    //   where a is the node to be checked.
    //
    // - checkLValInnerPattern() shall be used if the syntactic construct supports
    //   anything checkLValPattern() supports, as well as default assignment
    //   patterns, rest elements, and other constructs that may appear within an
    //   object or array destructuring pattern.
    //
    //   As a special case, function parameters also use checkLValInnerPattern(),
    //   as they also support defaults and rest constructs.
    //
    // These functions deliberately support both assignment and binding constructs,
    // as the logic for both is exceedingly similar. If the node is the target of
    // an assignment, then bindingType should be set to BIND_NONE. Otherwise, it
    // should be set to the appropriate BIND_* constant, like BIND_VAR or
    // BIND_LEXICAL.
    //
    // If the function is called with a non-BIND_NONE bindingType, then
    // additionally a checkClashes object may be specified to allow checking for
    // duplicate argument names. checkClashes is ignored if the provided construct
    // is an assignment (i.e., bindingType is BIND_NONE).

    /// The `Identifier` case of `checkLValSimple`.
    pub fn check_lval_identifier(
        &mut self,
        expr: &Identifier,
        binding_type: u32,
        check_clashes: Option<&mut HashSet<String>>,
    ) -> PResult<()> {
        let is_bind = binding_type != BIND_NONE;
        if self.strict && self.reserved_words_strict_bind(&expr.name) {
            return self.raise_recoverable(
                expr.start,
                format!("{}{} in strict mode", if is_bind { "Binding " } else { "Assigning to " }, expr.name),
            );
        }
        if is_bind {
            if binding_type == BIND_LEXICAL && expr.name == "let" {
                return self.raise_recoverable(expr.start, "let is disallowed as a lexically bound name");
            }
            if let Some(clashes) = check_clashes {
                if clashes.contains(&expr.name) {
                    return self.raise_recoverable(expr.start, "Argument name clash");
                }
                clashes.insert(expr.name.clone());
            }
            if binding_type != BIND_OUTSIDE {
                self.declare_name(&expr.name, binding_type, expr.start)?;
            }
        }
        Ok(())
    }

    pub fn check_lval_simple(
        &mut self,
        expr: &Pattern,
        binding_type: u32,
        check_clashes: Option<&mut HashSet<String>>,
    ) -> PResult<()> {
        let is_bind = binding_type != BIND_NONE;

        match expr {
            Pattern::Identifier(i) => self.check_lval_identifier(i, binding_type, check_clashes),

            Pattern::MemberExpression(m) => {
                if is_bind {
                    return self.raise_recoverable(m.start, "Binding member expression");
                }
                Ok(())
            }

            Pattern::ParenthesizedExpression(p) => {
                if is_bind {
                    return self.raise_recoverable(p.start, "Binding parenthesized expression");
                }
                self.check_lval_simple_expr(&p.expression, binding_type, check_clashes)
            }

            other => {
                self.raise(other.span().start, if is_bind { "Binding rvalue" } else { "Assigning to rvalue" })
            }
        }
    }

    /// `checkLValSimple` on a node that is still an expression (update expressions, compound
    /// assignment targets, parenthesized targets).
    pub fn check_lval_simple_expr(
        &mut self,
        expr: &Expression,
        binding_type: u32,
        check_clashes: Option<&mut HashSet<String>>,
    ) -> PResult<()> {
        let is_bind = binding_type != BIND_NONE;

        match expr {
            Expression::Identifier(i) => self.check_lval_identifier(i, binding_type, check_clashes),

            Expression::ChainExpression(c) => {
                self.raise_recoverable(c.start, "Optional chaining cannot appear in left-hand side")
            }

            Expression::MemberExpression(m) => {
                if is_bind {
                    return self.raise_recoverable(m.start, "Binding member expression");
                }
                Ok(())
            }

            Expression::ParenthesizedExpression(p) => {
                if is_bind {
                    return self.raise_recoverable(p.start, "Binding parenthesized expression");
                }
                self.check_lval_simple_expr(&p.expression, binding_type, check_clashes)
            }

            other => {
                self.raise(other.span().start, if is_bind { "Binding rvalue" } else { "Assigning to rvalue" })
            }
        }
    }

    pub fn check_lval_pattern(
        &mut self,
        expr: &Pattern,
        binding_type: u32,
        mut check_clashes: Option<&mut HashSet<String>>,
    ) -> PResult<()> {
        match expr {
            Pattern::ObjectPattern(o) => {
                for prop in &o.properties {
                    match prop {
                        ObjectPatternMember::Property(p) => {
                            // AssignmentProperty has type === "Property"
                            self.check_lval_inner_pattern(&p.value, binding_type, check_clashes.as_deref_mut())?
                        }
                        ObjectPatternMember::RestElement(r) => {
                            self.check_lval_pattern(&r.argument, binding_type, check_clashes.as_deref_mut())?
                        }
                    }
                }
                Ok(())
            }

            Pattern::ArrayPattern(a) => {
                for elem in a.elements.iter().flatten() {
                    self.check_lval_inner_pattern(elem, binding_type, check_clashes.as_deref_mut())?;
                }
                Ok(())
            }

            _ => self.check_lval_simple(expr, binding_type, check_clashes),
        }
    }

    pub fn check_lval_inner_pattern(
        &mut self,
        expr: &Pattern,
        binding_type: u32,
        check_clashes: Option<&mut HashSet<String>>,
    ) -> PResult<()> {
        match expr {
            Pattern::AssignmentPattern(a) => self.check_lval_pattern(&a.left, binding_type, check_clashes),

            Pattern::RestElement(r) => self.check_lval_pattern(&r.argument, binding_type, check_clashes),

            _ => self.check_lval_pattern(expr, binding_type, check_clashes),
        }
    }
}

pub fn property_key_start(key: &PropertyKey) -> u32 {
    match key {
        PropertyKey::Private(PrivateIdentifierNode::PrivateIdentifier(p)) => p.start,
        PropertyKey::Expression(e) => e.span().start,
    }
}
