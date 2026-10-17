//! acorn-typescript 1.0.13: the type grammar (`tsParseType` and everything it calls), type
//! parameters/arguments, type members, modifiers.

use super::err;
use super::{token_is_identifier, token_is_keyword_or_identifier, token_is_ts_type_operator, ts_is_access_modifier};
use crate::acorn::expression::{ForInit, Operand};
use crate::acorn::state::{PResult, Parser, TokVal};
use crate::acorn::tokentype::TokenType as T;
use crate::estree::*;

/// `tsIsListTerminator` kinds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    EnumMembers,
    TypeMembers,
    HeritageClauseElement,
    TupleElementTypes,
    TypeParametersOrArguments,
}

/// The `parseModifiers` argument of `tsParseTypeParameter(s)`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TypeParamModifiers {
    /// `tsParseNoneModifiers` (the default)
    None,
    /// `tsParseConstModifier`
    Const,
    /// `tsParseInOutModifiers`
    InOut,
    /// `tsParseClassTypeParameterModifiers`
    Class,
}

/// The `modified` object of `tsParseModifiers` (the node the modifiers are set on).
#[derive(Clone, Copy, Debug, Default)]
pub struct Modified {
    pub accessibility: Option<&'static str>,
    pub declare: bool,
    pub is_abstract: bool,
    pub readonly: bool,
    pub is_override: bool,
    pub is_static: bool,
    pub accessor: bool,
    pub is_in: bool,
    pub is_out: bool,
    pub is_const: bool,
}

impl Modified {
    /// `modified[modifier]` (truthiness) / `hasOwnProperty(modifier)`.
    pub fn get(&self, m: &str) -> bool {
        match m {
            "accessibility" => self.accessibility.is_some(),
            "declare" => self.declare,
            "abstract" => self.is_abstract,
            "readonly" => self.readonly,
            "override" => self.is_override,
            "static" => self.is_static,
            "accessor" => self.accessor,
            "in" => self.is_in,
            "out" => self.is_out,
            "const" => self.is_const,
            _ => false,
        }
    }

    fn set(&mut self, m: &str) {
        match m {
            "declare" => self.declare = true,
            "abstract" => self.is_abstract = true,
            "readonly" => self.readonly = true,
            "override" => self.is_override = true,
            "static" => self.is_static = true,
            "accessor" => self.accessor = true,
            "in" => self.is_in = true,
            "out" => self.is_out = true,
            "const" => self.is_const = true,
            _ => {}
        }
    }

    /// `tsHasSomeModifiers(member, modifiers)`
    pub fn has_some(&self, modifiers: &[&'static str]) -> bool {
        modifiers.iter().any(|m| if ts_is_access_modifier(m) { self.accessibility == Some(*m) } else { self.get(m) })
    }
}

/// The `errorTemplate` of `tsParseModifiers`. The plugin passes the template *function* to
/// `raise`, so the message is the function's source text.
#[derive(Clone, Copy)]
pub enum ModifierError {
    OnTypeMember,
    OnTypeParameter,
    OnTypeParameterPositions,
}

impl ModifierError {
    fn source_text(self) -> &'static str {
        match self {
            ModifierError::OnTypeMember => "({ modifier }) => `'${modifier}' modifier cannot appear on a type member.`",
            ModifierError::OnTypeParameter => {
                "({ modifier }) => `'${modifier}' modifier cannot appear on a type parameter.`"
            }
            ModifierError::OnTypeParameterPositions => {
                "({ modifier }) => `'${modifier}' modifier can only appear on a type parameter of a class, interface or type alias.`"
            }
        }
    }
}

/// `keywordTypeFromName(value)`
fn keyword_type_from_name(value: &str) -> Option<fn(TSKeyword) -> TSType> {
    Some(match value {
        "any" => TSType::TSAnyKeyword,
        "boolean" => TSType::TSBooleanKeyword,
        "bigint" => TSType::TSBigIntKeyword,
        "never" => TSType::TSNeverKeyword,
        "number" => TSType::TSNumberKeyword,
        "object" => TSType::TSObjectKeyword,
        "string" => TSType::TSStringKeyword,
        "symbol" => TSType::TSSymbolKeyword,
        "undefined" => TSType::TSUndefinedKeyword,
        "unknown" => TSType::TSUnknownKeyword,
        _ => return None,
    })
}

/// A signature filled by `tsFillSignature`.
pub struct Signature {
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub parameters: Vec<Pattern>,
    pub type_annotation: Option<TSTypeAnnotation>,
}

fn pattern_type_name(p: &Pattern) -> &'static str {
    match p {
        Pattern::Identifier(_) => "Identifier",
        Pattern::ObjectPattern(_) => "ObjectPattern",
        Pattern::ArrayPattern(_) => "ArrayPattern",
        Pattern::RestElement(_) => "RestElement",
        Pattern::AssignmentPattern(_) => "AssignmentPattern",
        Pattern::MemberExpression(_) => "MemberExpression",
        Pattern::ParenthesizedExpression(_) => "ParenthesizedExpression",
        Pattern::TSParameterProperty(_) => "TSParameterProperty",
        Pattern::TSAsExpression(_) => "TSAsExpression",
        Pattern::TSSatisfiesExpression(_) => "TSSatisfiesExpression",
        Pattern::TSTypeAssertion(_) => "TSTypeAssertion",
        Pattern::TSNonNullExpression(_) => "TSNonNullExpression",
    }
}

fn keyword(start: u32, end: u32) -> TSKeyword {
    TSKeyword { start, end }
}

impl<'a> Parser<'a> {
    // --- context helpers

    /// `tsInType(cb)`: runs `cb` in a type context. This should be called one token *before*
    /// the first type token, so that the call to `next()` is run in type context.
    pub(crate) fn ts_in_type<R>(&mut self, cb: impl FnOnce(&mut Self) -> PResult<R>) -> PResult<R> {
        let old_in_type = self.tss().in_type;
        self.tsm().in_type = true;
        let r = cb(self);
        self.tsm().in_type = old_in_type;
        r
    }

    /// `tsInNoContext(cb)`
    pub(crate) fn ts_in_no_context<R>(&mut self, cb: impl FnOnce(&mut Self) -> PResult<R>) -> PResult<R> {
        let first = self.context.first().copied();
        let old_context = std::mem::replace(&mut self.context, first.into_iter().collect());
        let r = cb(self);
        self.context = old_context;
        r
    }

    pub(crate) fn ts_in_disallow_conditional_types_context<R>(
        &mut self,
        cb: impl FnOnce(&mut Self) -> PResult<R>,
    ) -> PResult<R> {
        let old = self.tss().in_disallow_conditional_types_context;
        self.tsm().in_disallow_conditional_types_context = true;
        let r = cb(self);
        self.tsm().in_disallow_conditional_types_context = old;
        r
    }

    pub(crate) fn ts_in_allow_conditional_types_context<R>(
        &mut self,
        cb: impl FnOnce(&mut Self) -> PResult<R>,
    ) -> PResult<R> {
        let old = self.tss().in_disallow_conditional_types_context;
        self.tsm().in_disallow_conditional_types_context = false;
        let r = cb(self);
        self.tsm().in_disallow_conditional_types_context = old;
        r
    }

    pub(crate) fn ts_in_ambient_context<R>(&mut self, cb: impl FnOnce(&mut Self) -> PResult<R>) -> PResult<R> {
        let old = self.tss().is_ambient_context;
        self.tsm().is_ambient_context = true;
        let r = cb(self);
        self.tsm().is_ambient_context = old;
        r
    }

    // --- lists

    fn ts_is_list_terminator(&self, kind: ListKind) -> bool {
        match kind {
            ListKind::EnumMembers | ListKind::TypeMembers => self.type_ == T::BraceR,
            ListKind::HeritageClauseElement => self.type_ == T::BraceL,
            ListKind::TupleElementTypes => self.type_ == T::BracketR,
            ListKind::TypeParametersOrArguments => self.ts_match_right_relational(),
        }
    }

    /// `tsParseDelimitedList(kind, parseElement, refTrailingCommaPos)` (expectSuccess)
    pub(crate) fn ts_parse_delimited_list<N>(
        &mut self,
        kind: ListKind,
        mut parse_element: impl FnMut(&mut Self) -> PResult<N>,
        ref_trailing_comma_pos: Option<&mut i64>,
    ) -> PResult<Vec<N>> {
        let mut result = Vec::new();
        let mut trailing_comma_pos: i64 = -1;
        loop {
            if self.ts_is_list_terminator(kind) {
                break;
            }
            trailing_comma_pos = -1;
            let element = parse_element(self)?;
            result.push(element);
            if self.eat(T::Comma)? {
                trailing_comma_pos = self.last_tok_start as i64;
                continue;
            }
            if self.ts_is_list_terminator(kind) {
                break;
            }
            // expectSuccess
            self.expect(T::Comma)?;
            return self.unexpected(None);
        }
        if let Some(r) = ref_trailing_comma_pos {
            *r = trailing_comma_pos;
        }
        Ok(result)
    }

    /// `tsParseBracketedList(kind, parseElement, bracket, skipFirstToken, refTrailingCommaPos)`
    fn ts_parse_bracketed_list<N>(
        &mut self,
        kind: ListKind,
        parse_element: impl FnMut(&mut Self) -> PResult<N>,
        bracket: bool,
        skip_first_token: bool,
        ref_trailing_comma_pos: Option<&mut i64>,
    ) -> PResult<Vec<N>> {
        if !skip_first_token {
            if bracket {
                self.expect(T::BracketL)?;
            } else {
                self.expect(T::Relational)?;
            }
        }
        let result = self.ts_parse_delimited_list(kind, parse_element, ref_trailing_comma_pos)?;
        if bracket {
            self.expect(T::BracketR)?;
        } else {
            self.expect(T::Relational)?;
        }
        Ok(result)
    }

    /// `tsParseList(kind, parseElement)`
    fn ts_parse_list<N>(&mut self, kind: ListKind, mut parse_element: impl FnMut(&mut Self) -> PResult<N>) -> PResult<Vec<N>> {
        let mut result = Vec::new();
        while !self.ts_is_list_terminator(kind) {
            result.push(parse_element(self)?);
        }
        Ok(result)
    }

    // --- names

    /// `tsParseEntityName(allowReservedWords = true)`
    pub(crate) fn ts_parse_entity_name(&mut self, allow_reserved_words: bool) -> PResult<TSEntityName> {
        let mut entity = TSEntityName::Identifier(self.parse_ident(allow_reserved_words)?);
        while self.eat(T::Dot)? {
            let start = entity.span().start;
            let right = self.parse_ident(allow_reserved_words)?;
            entity = TSEntityName::TSQualifiedName(Box::new(TSQualifiedName {
                start,
                end: self.last_tok_end,
                left: entity,
                right,
            }));
        }
        Ok(entity)
    }

    /// `tsParseTypeParameterName()`
    fn ts_parse_type_parameter_name(&mut self) -> PResult<String> {
        let type_name = self.parse_ident(false)?;
        Ok(type_name.name)
    }

    // --- annotations

    /// `tsParseTypeAnnotation(eatColon = true, t = this.startNode())`
    pub(crate) fn ts_parse_type_annotation(&mut self, eat_colon: bool, t_start: Option<u32>) -> PResult<TSTypeAnnotation> {
        let start = t_start.unwrap_or(self.start);
        let type_annotation = self.ts_in_type(|p| {
            if eat_colon {
                p.expect(T::Colon)?;
            }
            p.ts_parse_type()
        })?;
        Ok(TSTypeAnnotation { start, end: self.last_tok_end, type_annotation: Box::new(type_annotation) })
    }

    /// `tsTryParseTypeAnnotation()`
    pub(crate) fn ts_try_parse_type_annotation(&mut self) -> PResult<Option<TSTypeAnnotation>> {
        if self.type_ == T::Colon { Ok(Some(self.ts_parse_type_annotation(true, None)?)) } else { Ok(None) }
    }

    /// `tsTryParseTypeOrTypePredicateAnnotation()`
    pub(crate) fn ts_try_parse_type_or_type_predicate_annotation(&mut self) -> PResult<Option<TSTypeAnnotation>> {
        if self.type_ == T::Colon {
            Ok(Some(self.ts_parse_type_or_type_predicate_annotation(T::Colon)?))
        } else {
            Ok(None)
        }
    }

    /// `tsTryParseType()`
    fn ts_try_parse_type(&mut self) -> PResult<Option<TSType>> {
        self.ts_eat_then_parse_type(T::Colon)
    }

    /// `tsEatThenParseType(token)`
    fn ts_eat_then_parse_type(&mut self, token: T) -> PResult<Option<TSType>> {
        if self.type_ != token { Ok(None) } else { Ok(Some(self.ts_next_then_parse_type()?)) }
    }

    /// `tsExpectThenParseType(token)`
    fn ts_expect_then_parse_type(&mut self, token: T) -> PResult<TSType> {
        self.ts_in_type(|p| {
            p.expect(token)?;
            p.ts_parse_type()
        })
    }

    /// `tsNextThenParseType()`
    pub(crate) fn ts_next_then_parse_type(&mut self) -> PResult<TSType> {
        self.ts_in_type(|p| {
            p.next()?;
            p.ts_parse_type()
        })
    }

    /// `tsParseThisTypeNode()`
    fn ts_parse_this_type_node(&mut self) -> PResult<TSKeyword> {
        let start = self.start;
        self.next()?;
        Ok(keyword(start, self.last_tok_end))
    }

    /// `tsParseThisTypePredicate(lhs)`
    fn ts_parse_this_type_predicate(&mut self, lhs: TSKeyword) -> PResult<TSTypePredicate> {
        self.next()?;
        let start = lhs.start;
        let type_annotation = self.ts_parse_type_annotation(false, None)?;
        Ok(TSTypePredicate {
            start,
            end: self.last_tok_end,
            parameter_name: TSTypePredicateName::TSThisType(lhs),
            type_annotation: Some(type_annotation),
            asserts: false,
        })
    }

    /// `tsParseThisTypeOrThisTypePredicate()`
    fn ts_parse_this_type_or_this_type_predicate(&mut self) -> PResult<TSType> {
        let this_keyword = self.ts_parse_this_type_node()?;
        if self.is_contextual("is") && !self.has_preceding_line_break() {
            Ok(TSType::TSTypePredicate(self.ts_parse_this_type_predicate(this_keyword)?))
        } else {
            Ok(TSType::TSThisType(this_keyword))
        }
    }

    /// `tsParseTypePredicatePrefix()`
    fn ts_parse_type_predicate_prefix(&mut self) -> PResult<Option<Identifier>> {
        let id = self.parse_ident(false)?;
        if self.is_contextual("is") && !self.has_preceding_line_break() {
            self.next()?;
            return Ok(Some(id));
        }
        Ok(None)
    }

    /// `tsParseTypePredicateAsserts()`
    fn ts_parse_type_predicate_asserts(&mut self) -> PResult<Option<bool>> {
        if self.type_ != T::TsAsserts {
            return Ok(None);
        }
        let contains_esc = self.contains_esc;
        self.next()?;
        if !token_is_identifier(self.type_) && self.type_ != T::This {
            return Ok(None);
        }
        if contains_esc {
            return self.raise(self.last_tok_start, "Escape sequence in keyword asserts");
        }
        Ok(Some(true))
    }

    /// `tsParseTypeOrTypePredicateAnnotation(returnToken)`
    pub(crate) fn ts_parse_type_or_type_predicate_annotation(&mut self, return_token: T) -> PResult<TSTypeAnnotation> {
        self.ts_in_type(|p| {
            let t_start = p.start;
            p.expect(return_token)?;
            let node_start = p.start;
            let type_predicate_variable = if p.ts_is_identifier() {
                p.ts_try_parse(|p| p.ts_parse_type_predicate_prefix())?
            } else {
                None
            };
            if let Some(var) = type_predicate_variable {
                let type2 = p.ts_parse_type_annotation(false, None)?;
                let pred = TSTypePredicate {
                    start: node_start,
                    end: p.last_tok_end,
                    parameter_name: TSTypePredicateName::Identifier(var),
                    type_annotation: Some(type2),
                    asserts: false,
                };
                return Ok(TSTypeAnnotation {
                    start: t_start,
                    end: p.last_tok_end,
                    type_annotation: Box::new(TSType::TSTypePredicate(pred)),
                });
            }
            let asserts = p.ts_try_parse(|p| p.ts_parse_type_predicate_asserts())?.is_some();
            if asserts && p.type_ == T::This {
                let this_type_predicate = p.ts_parse_this_type_or_this_type_predicate()?;
                let pred = match this_type_predicate {
                    TSType::TSThisType(k) => TSTypePredicate {
                        start: node_start,
                        end: p.last_tok_end,
                        parameter_name: TSTypePredicateName::TSThisType(k),
                        type_annotation: None,
                        asserts: true,
                    },
                    TSType::TSTypePredicate(mut pred) => {
                        // resetStartLocationFromNode(thisTypePredicate, node)
                        pred.start = node_start;
                        pred.asserts = true;
                        pred
                    }
                    _ => unreachable!(),
                };
                return Ok(TSTypeAnnotation {
                    start: t_start,
                    end: p.last_tok_end,
                    type_annotation: Box::new(TSType::TSTypePredicate(pred)),
                });
            }
            let asserted_type_predicate_variable = if asserts && p.ts_is_identifier() {
                p.ts_try_parse(|p| p.ts_parse_type_predicate_prefix())?
            } else {
                None
            };
            let Some(var) = asserted_type_predicate_variable else {
                if !asserts {
                    return p.ts_parse_type_annotation(false, Some(t_start));
                }
                let parameter_name = p.parse_ident(false)?;
                let pred = TSTypePredicate {
                    start: node_start,
                    end: p.last_tok_end,
                    parameter_name: TSTypePredicateName::Identifier(parameter_name),
                    type_annotation: None,
                    asserts,
                };
                return Ok(TSTypeAnnotation {
                    start: t_start,
                    end: p.last_tok_end,
                    type_annotation: Box::new(TSType::TSTypePredicate(pred)),
                });
            };
            let type_ = p.ts_parse_type_annotation(false, None)?;
            let pred = TSTypePredicate {
                start: node_start,
                end: p.last_tok_end,
                parameter_name: TSTypePredicateName::Identifier(var),
                type_annotation: Some(type_),
                asserts,
            };
            Ok(TSTypeAnnotation {
                start: t_start,
                end: p.last_tok_end,
                type_annotation: Box::new(TSType::TSTypePredicate(pred)),
            })
        })
    }

    // --- signatures

    /// `tsParseBindingListForSignature()`
    fn ts_parse_binding_list_for_signature(&mut self) -> PResult<Vec<Pattern>> {
        let list = self.parse_binding_list(T::ParenR, true, true, None)?;
        let mut out = Vec::with_capacity(list.len());
        for pattern in list {
            // SVELTERS: a hole (`(a,,b)`) makes the plugin throw a TypeError.
            let Some(pattern) = pattern else { return self.unexpected(None) };
            if !matches!(
                pattern,
                Pattern::Identifier(_) | Pattern::RestElement(_) | Pattern::ObjectPattern(_) | Pattern::ArrayPattern(_)
            ) {
                return self.raise(pattern.span().start, err::UNSUPPORTED_SIGNATURE_PARAMETER_KIND(pattern_type_name(&pattern)));
            }
            out.push(pattern);
        }
        Ok(out)
    }

    /// `tsFillSignature(returnToken, signature)`.
    /// Note: In TypeScript implementation we must provide `yieldContext` and `awaitContext`,
    /// but here it's always false, because this is only used for types.
    fn ts_fill_signature(&mut self, return_token: T) -> PResult<Signature> {
        let return_token_required = return_token == T::Arrow;
        let type_parameters = self.ts_try_parse_type_parameters(TypeParamModifiers::None)?;
        self.expect(T::ParenL)?;
        let parameters = self.ts_parse_binding_list_for_signature()?;
        let type_annotation = if return_token_required || self.type_ == return_token {
            Some(self.ts_parse_type_or_type_predicate_annotation(return_token)?)
        } else {
            None
        };
        Ok(Signature { type_parameters, parameters, type_annotation })
    }

    /// `tsTryNextParseConstantContext()`
    pub(crate) fn ts_try_next_parse_constant_context(&mut self) -> PResult<Option<TSType>> {
        if self.lookahead()?.type_ != T::Const {
            return Ok(None);
        }
        self.next()?;
        let type_reference = self.ts_parse_type_reference()?;
        if type_reference.type_arguments.is_some() {
            return self.raise(type_reference.type_name.span().start, err::CANNOT_FIND_NAME("const"));
        }
        Ok(Some(TSType::TSTypeReference(type_reference)))
    }

    /// `tsParseFunctionOrConstructorType(type, abstract)`
    fn ts_parse_function_or_constructor_type(&mut self, constructor: bool, is_abstract: bool) -> PResult<TSType> {
        let start = self.start;
        let mut abstract_ = None;
        if constructor {
            abstract_ = Some(is_abstract);
            if is_abstract {
                self.next()?;
            }
            self.next()?;
        }
        let sig = self.ts_in_allow_conditional_types_context(|p| p.ts_fill_signature(T::Arrow))?;
        let node = TSFunctionType {
            start,
            end: self.last_tok_end,
            is_abstract: abstract_,
            type_parameters: sig.type_parameters,
            parameters: sig.parameters,
            type_annotation: sig.type_annotation,
        };
        Ok(if constructor { TSType::TSConstructorType(node) } else { TSType::TSFunctionType(node) })
    }

    /// `tsParseUnionOrIntersectionType(kind, parseConstituentType, operator)`
    fn ts_parse_union_or_intersection_type(
        &mut self,
        union: bool,
        parse_constituent_type: fn(&mut Self) -> PResult<TSType>,
        operator: T,
    ) -> PResult<TSType> {
        let start = self.start;
        let has_leading_operator = self.eat(operator)?;
        let mut types = Vec::new();
        loop {
            types.push(parse_constituent_type(self)?);
            if !self.eat(operator)? {
                break;
            }
        }
        if types.len() == 1 && !has_leading_operator {
            return Ok(types.pop().unwrap());
        }
        let node = TSUnionType { start, end: self.last_tok_end, types };
        Ok(if union { TSType::TSUnionType(node) } else { TSType::TSIntersectionType(node) })
    }

    /// `tsParseTypeOperator()`
    fn ts_parse_type_operator(&mut self) -> PResult<TSType> {
        let start = self.start;
        let operator = self.value_str().to_string();
        self.next()?;
        let type_annotation = self.ts_parse_type_operator_or_higher()?;
        if operator == "readonly" {
            // tsCheckTypeAnnotationForReadOnly(node)
            match &type_annotation {
                TSType::TSTupleType(_) | TSType::TSArrayType(_) => {}
                _ => return self.raise(start, err::UNEXPECTED_READONLY),
            }
        }
        Ok(TSType::TSTypeOperator(TSTypeOperator {
            start,
            end: self.last_tok_end,
            operator,
            type_annotation: Box::new(type_annotation),
        }))
    }

    /// `tsParseConstraintForInferType()`
    fn ts_parse_constraint_for_infer_type(&mut self) -> PResult<Option<TSType>> {
        if self.eat(T::Extends)? {
            let constraint = self.ts_in_disallow_conditional_types_context(|p| p.ts_parse_type())?;
            if self.tss().in_disallow_conditional_types_context || self.type_ != T::Question {
                return Ok(Some(constraint));
            }
        }
        Ok(None)
    }

    /// `tsParseInferType()`
    fn ts_parse_infer_type(&mut self) -> PResult<TSType> {
        let start = self.start;
        self.expect_contextual("infer")?;
        let tp_start = self.start;
        let name = self.ts_parse_type_parameter_name()?;
        let constraint = self.ts_try_parse(|p| p.ts_parse_constraint_for_infer_type())?;
        let type_parameter = TSTypeParameter {
            start: tp_start,
            end: self.last_tok_end,
            name,
            is_in: false,
            is_out: false,
            is_const: false,
            constraint: constraint.map(Box::new),
            default: None,
        };
        Ok(TSType::TSInferType(TSInferType { start, end: self.last_tok_end, type_parameter }))
    }

    /// `parseExprAtom()` on a literal token, as a `Literal`.
    fn ts_parse_literal_atom(&mut self) -> PResult<Literal> {
        match self.parse_expr_atom(None, ForInit::No, false)? {
            Expression::Literal(l) => Ok(l),
            other => self.unexpected(Some(other.span().start)),
        }
    }

    /// `tsParseLiteralTypeNode()`
    fn ts_parse_literal_type_node(&mut self) -> PResult<TSType> {
        let start = self.start;
        let literal = match self.type_ {
            // we don't need bigint type here
            T::Num | T::String | T::True | T::False => self.ts_parse_literal_atom()?,
            _ => return self.unexpected(None),
        };
        Ok(TSType::TSLiteralType(TSLiteralType {
            start,
            end: self.last_tok_end,
            literal: TSLiteralTypeLiteral::Literal(literal),
        }))
    }

    /// `tsParseImportType()`
    fn ts_parse_import_type(&mut self) -> PResult<TSImportType> {
        let start = self.start;
        self.expect(T::Import)?;
        self.expect(T::ParenL)?;
        if self.type_ != T::String {
            return self.raise(self.start, err::UNSUPPORTED_IMPORT_TYPE_ARGUMENT);
        }
        let argument = self.parse_expr_atom(None, ForInit::No, false)?;
        self.expect(T::ParenR)?;
        let qualifier = if self.eat(T::Dot)? { Some(self.ts_parse_entity_name(true)?) } else { None };
        let type_arguments = if self.ts_match_left_relational() { Some(self.ts_parse_type_arguments(false)?) } else { None };
        Ok(TSImportType { start, end: self.last_tok_end, argument: Box::new(argument), qualifier, type_arguments })
    }

    /// `tsParseTypeQuery()`
    fn ts_parse_type_query(&mut self) -> PResult<TSType> {
        let start = self.start;
        self.expect(T::Typeof)?;
        let expr_name = if self.type_ == T::Import {
            TSTypeQueryName::TSImportType(self.ts_parse_import_type()?)
        } else {
            match self.ts_parse_entity_name(true)? {
                TSEntityName::Identifier(i) => TSTypeQueryName::Identifier(i),
                TSEntityName::TSQualifiedName(q) => TSTypeQueryName::TSQualifiedName(q),
            }
        };
        let type_arguments = if !self.has_preceding_line_break() && self.ts_match_left_relational() {
            Some(self.ts_parse_type_arguments(false)?)
        } else {
            None
        };
        Ok(TSType::TSTypeQuery(TSTypeQuery { start, end: self.last_tok_end, expr_name, type_arguments }))
    }

    /// `tsParseMappedTypeParameter()`
    fn ts_parse_mapped_type_parameter(&mut self) -> PResult<TSTypeParameter> {
        let start = self.start;
        let name = self.ts_parse_type_parameter_name()?;
        let constraint = self.ts_expect_then_parse_type(T::In)?;
        Ok(TSTypeParameter {
            start,
            end: self.last_tok_end,
            name,
            is_in: false,
            is_out: false,
            is_const: false,
            constraint: Some(Box::new(constraint)),
            default: None,
        })
    }

    fn plus_min_sign(&self) -> TsModifierSign {
        if self.tok_text() == "-" { TsModifierSign::Minus } else { TsModifierSign::Plus }
    }

    /// `tsParseMappedType()`
    fn ts_parse_mapped_type(&mut self) -> PResult<TSType> {
        let start = self.start;
        self.expect(T::BraceL)?;
        let mut readonly = None;
        if self.type_ == T::PlusMin {
            readonly = Some(self.plus_min_sign());
            self.next()?;
            self.expect_contextual("readonly")?;
        } else if self.eat_contextual("readonly")? {
            readonly = Some(TsModifierSign::True);
        }
        self.expect(T::BracketL)?;
        let type_parameter = self.ts_parse_mapped_type_parameter()?;
        let name_type = if self.eat_contextual("as")? { Some(Box::new(self.ts_parse_type()?)) } else { None };
        self.expect(T::BracketR)?;
        let mut optional = None;
        if self.type_ == T::PlusMin {
            optional = Some(self.plus_min_sign());
            self.next()?;
            self.expect(T::Question)?;
        } else if self.eat(T::Question)? {
            optional = Some(TsModifierSign::True);
        }
        let type_annotation = self.ts_try_parse_type()?.map(Box::new);
        self.semicolon()?;
        self.expect(T::BraceR)?;
        Ok(TSType::TSMappedType(TSMappedType {
            start,
            end: self.last_tok_end,
            readonly,
            type_parameter,
            name_type,
            optional,
            type_annotation,
        }))
    }

    /// `tsParseTypeLiteral()`
    fn ts_parse_type_literal(&mut self) -> PResult<TSType> {
        let start = self.start;
        let members = self.ts_parse_object_type_members()?;
        Ok(TSType::TSTypeLiteral(TSTypeLiteral { start, end: self.last_tok_end, members }))
    }

    /// `tsIsTupleElementLabel()`
    fn ts_is_tuple_element_label(&mut self) -> PResult<bool> {
        if !token_is_keyword_or_identifier(self.type_) {
            return Ok(false);
        }
        let next_token = self.lookahead()?;
        if next_token.type_ == T::Colon {
            return Ok(true);
        }
        if next_token.type_ != T::Question {
            return Ok(false);
        }
        Ok(self.lookahead_n(2)?.type_ == T::Colon)
    }

    /// `tsParseTupleElementType()`
    fn ts_parse_tuple_element_type(&mut self) -> PResult<TSType> {
        let start_pos = self.start;
        let rest = self.eat(T::Ellipsis)?;
        let mut type_;
        if self.ts_is_tuple_element_label()? {
            let labeled_start = self.start;
            let label = self.parse_ident(true)?;
            let optional = self.eat(T::Question)?;
            self.expect(T::Colon)?;
            let element_type = self.ts_parse_type()?;
            type_ = TSType::TSNamedTupleMember(TSNamedTupleMember {
                start: labeled_start,
                end: self.last_tok_end,
                label,
                optional,
                element_type: Box::new(element_type),
            });
        } else {
            type_ = self.ts_parse_type()?;
            let optional = self.eat(T::Question)?;
            if self.eat(T::Colon)? {
                return self.raise(type_.span().start, err::INVALID_TUPLE_MEMBER_LABEL);
            }
            if optional {
                let start = type_.span().start;
                type_ = TSType::TSOptionalType(TSWrappedType {
                    start,
                    end: self.last_tok_end,
                    type_annotation: Box::new(type_),
                });
            }
        }
        if rest {
            type_ = TSType::TSRestType(TSWrappedType {
                start: start_pos,
                end: self.last_tok_end,
                type_annotation: Box::new(type_),
            });
        }
        Ok(type_)
    }

    /// `tsParseTupleType()`
    fn ts_parse_tuple_type(&mut self) -> PResult<TSType> {
        let start = self.start;
        let element_types = self.ts_parse_bracketed_list(
            ListKind::TupleElementTypes,
            |p| p.ts_parse_tuple_element_type(),
            /* bracket */ true,
            /* skipFirstToken */ false,
            None,
        )?;
        let mut seen_optional_element = false;
        for element_node in &element_types {
            let named_optional = matches!(element_node, TSType::TSNamedTupleMember(m) if m.optional);
            if seen_optional_element
                && !matches!(element_node, TSType::TSRestType(_) | TSType::TSOptionalType(_))
                && !named_optional
            {
                return self.raise(element_node.span().start, err::OPTIONAL_TYPE_BEFORE_REQUIRED);
            }
            seen_optional_element = seen_optional_element || named_optional || matches!(element_node, TSType::TSOptionalType(_));
        }
        Ok(TSType::TSTupleType(TSTupleType { start, end: self.last_tok_end, element_types }))
    }

    /// `tsParseTemplateLiteralType()` (`parseTemplate` with `inType`: the expressions are types)
    fn ts_parse_template_literal_type(&mut self) -> PResult<TSType> {
        let start = self.start;
        // parseTemplate({ isTagged: false })
        let t_start = self.start;
        self.next()?;
        let mut expressions = Vec::new();
        let mut cur_elt = self.parse_template_element(false)?;
        let mut tail = cur_elt.tail;
        let mut quasis = vec![cur_elt];
        while !tail {
            if self.type_ == T::Eof {
                return self.raise(self.pos, "Unterminated template literal");
            }
            self.expect(T::DollarBraceL)?;
            expressions.push(self.ts_parse_type()?);
            self.expect(T::BraceR)?;
            cur_elt = self.parse_template_element(false)?;
            tail = cur_elt.tail;
            quasis.push(cur_elt);
        }
        self.next()?;
        let literal = TSTemplateLiteralType { start: t_start, end: self.last_tok_end, expressions, quasis };
        Ok(TSType::TSLiteralType(TSLiteralType {
            start,
            end: self.last_tok_end,
            literal: TSLiteralTypeLiteral::TemplateLiteral(literal),
        }))
    }

    /// `tsParseTypeReference()`
    fn ts_parse_type_reference(&mut self) -> PResult<TSTypeReference> {
        let start = self.start;
        let type_name = self.ts_parse_entity_name(true)?;
        let type_arguments = if !self.has_preceding_line_break() && self.ts_match_left_relational() {
            Some(self.ts_parse_type_arguments(false)?)
        } else {
            None
        };
        Ok(TSTypeReference { start, end: self.last_tok_end, type_name, type_arguments })
    }

    /// `tsParseParenthesizedType()`
    fn ts_parse_parenthesized_type(&mut self) -> PResult<TSType> {
        let start = self.start;
        self.expect(T::ParenL)?;
        let type_annotation = self.ts_parse_type()?;
        self.expect(T::ParenR)?;
        Ok(TSType::TSParenthesizedType(TSWrappedType {
            start,
            end: self.last_tok_end,
            type_annotation: Box::new(type_annotation),
        }))
    }

    /// `tsParseNonArrayType()`
    fn ts_parse_non_array_type(&mut self) -> PResult<TSType> {
        match self.type_ {
            // we don't need bigint type here
            T::String | T::Num | T::True | T::False => return self.ts_parse_literal_type_node(),
            T::PlusMin => {
                if self.tok_text() == "-" {
                    let start = self.start;
                    let next_token = self.lookahead()?;
                    if next_token.type_ != T::Num {
                        return self.unexpected(None);
                    }
                    let literal = match self.parse_maybe_unary(None, false, false, ForInit::No)? {
                        Operand::Expr(Expression::UnaryExpression(u)) => TSLiteralTypeLiteral::UnaryExpression(u),
                        Operand::Expr(Expression::Literal(l)) => TSLiteralTypeLiteral::Literal(l),
                        Operand::Expr(other) => return self.unexpected(Some(other.span().start)),
                        Operand::Private(p) => return self.unexpected(Some(p.start)),
                    };
                    return Ok(TSType::TSLiteralType(TSLiteralType { start, end: self.last_tok_end, literal }));
                }
            }
            T::This => return self.ts_parse_this_type_or_this_type_predicate(),
            T::Typeof => return self.ts_parse_type_query(),
            T::Import => return Ok(TSType::TSImportType(self.ts_parse_import_type()?)),
            T::BraceL => {
                return if self.ts_look_ahead(|p| p.ts_is_start_of_mapped_type())? {
                    self.ts_parse_mapped_type()
                } else {
                    self.ts_parse_type_literal()
                };
            }
            T::BracketL => return self.ts_parse_tuple_type(),
            T::ParenL => return self.ts_parse_parenthesized_type(),
            // parse template string here
            T::BackQuote | T::DollarBraceL => return self.ts_parse_template_literal_type(),
            type_ => {
                if token_is_keyword_or_identifier(type_) || type_ == T::Void || type_ == T::Null {
                    let node_type: Option<fn(TSKeyword) -> TSType> = if type_ == T::Void {
                        Some(TSType::TSVoidKeyword)
                    } else if type_ == T::Null {
                        Some(TSType::TSNullKeyword)
                    } else {
                        keyword_type_from_name(self.value_str())
                    };
                    if let Some(node_type) = node_type {
                        if self.lookahead_char_code() != 46 {
                            let start = self.start;
                            self.next()?;
                            return Ok(node_type(keyword(start, self.last_tok_end)));
                        }
                    }
                    return Ok(TSType::TSTypeReference(self.ts_parse_type_reference()?));
                }
            }
        }
        self.unexpected(None)
    }

    /// `tsParseArrayTypeOrHigher()`
    fn ts_parse_array_type_or_higher(&mut self) -> PResult<TSType> {
        let mut type_ = self.ts_parse_non_array_type()?;
        while !self.has_preceding_line_break() && self.eat(T::BracketL)? {
            let start = type_.span().start;
            if self.type_ == T::BracketR {
                self.expect(T::BracketR)?;
                type_ = TSType::TSArrayType(TSArrayType { start, end: self.last_tok_end, element_type: Box::new(type_) });
            } else {
                let index_type = self.ts_parse_type()?;
                self.expect(T::BracketR)?;
                type_ = TSType::TSIndexedAccessType(TSIndexedAccessType {
                    start,
                    end: self.last_tok_end,
                    object_type: Box::new(type_),
                    index_type: Box::new(index_type),
                });
            }
        }
        Ok(type_)
    }

    /// `tsParseTypeOperatorOrHigher()`
    fn ts_parse_type_operator_or_higher(&mut self) -> PResult<TSType> {
        let is_type_operator = token_is_ts_type_operator(self.type_) && !self.contains_esc;
        if is_type_operator {
            self.ts_parse_type_operator()
        } else if self.is_contextual("infer") {
            self.ts_parse_infer_type()
        } else {
            self.ts_in_allow_conditional_types_context(|p| p.ts_parse_array_type_or_higher())
        }
    }

    /// `tsParseIntersectionTypeOrHigher()`
    fn ts_parse_intersection_type_or_higher(&mut self) -> PResult<TSType> {
        self.ts_parse_union_or_intersection_type(false, Self::ts_parse_type_operator_or_higher, T::BitwiseAND)
    }

    /// `tsParseUnionTypeOrHigher()`
    fn ts_parse_union_type_or_higher(&mut self) -> PResult<TSType> {
        self.ts_parse_union_or_intersection_type(true, Self::ts_parse_intersection_type_or_higher, T::BitwiseOR)
    }

    /// `isAbstractConstructorSignature()`
    fn is_abstract_constructor_signature(&mut self) -> PResult<bool> {
        Ok(self.ts_is_contextual(T::TsAbstract) && self.lookahead()?.type_ == T::New)
    }

    /// `tsParseNonConditionalType()`
    fn ts_parse_non_conditional_type(&mut self) -> PResult<TSType> {
        if self.ts_is_start_of_function_type()? {
            return self.ts_parse_function_or_constructor_type(false, false);
        }
        if self.type_ == T::New {
            return self.ts_parse_function_or_constructor_type(true, false);
        } else if self.is_abstract_constructor_signature()? {
            return self.ts_parse_function_or_constructor_type(true, /* abstract */ true);
        }
        self.ts_parse_union_type_or_higher()
    }

    /// `tsParseType()`: be sure to be in a type context before calling this, using `tsInType`.
    pub(crate) fn ts_parse_type(&mut self) -> PResult<TSType> {
        self.enter_depth()?;
        let r = self.ts_parse_type_inner();
        self.depth -= 1;
        r
    }

    fn ts_parse_type_inner(&mut self) -> PResult<TSType> {
        debug_assert!(self.in_type());
        let type_ = self.ts_parse_non_conditional_type()?;
        if self.tss().in_disallow_conditional_types_context || self.has_preceding_line_break() || !self.eat(T::Extends)? {
            return Ok(type_);
        }
        let start = type_.span().start;
        let extends_type = self.ts_in_disallow_conditional_types_context(|p| p.ts_parse_non_conditional_type())?;
        self.expect(T::Question)?;
        let true_type = self.ts_in_allow_conditional_types_context(|p| p.ts_parse_type())?;
        self.expect(T::Colon)?;
        let false_type = self.ts_in_allow_conditional_types_context(|p| p.ts_parse_type())?;
        Ok(TSType::TSConditionalType(TSConditionalType {
            start,
            end: self.last_tok_end,
            check_type: Box::new(type_),
            extends_type: Box::new(extends_type),
            true_type: Box::new(true_type),
            false_type: Box::new(false_type),
        }))
    }

    // --- lookahead predicates

    /// `tsIsStartOfMappedType()`
    fn ts_is_start_of_mapped_type(&mut self) -> PResult<bool> {
        self.next()?;
        if self.eat(T::PlusMin)? {
            return Ok(self.ts_is_contextual(T::TsReadonly));
        }
        if self.ts_is_contextual(T::TsReadonly) {
            self.next()?;
        }
        if self.type_ != T::BracketL {
            return Ok(false);
        }
        self.next()?;
        if !self.ts_is_identifier() {
            return Ok(false);
        }
        self.next()?;
        Ok(self.type_ == T::In)
    }

    /// `tsSkipParameterStart()`
    fn ts_skip_parameter_start(&mut self) -> PResult<bool> {
        if token_is_identifier(self.type_) || self.type_ == T::This {
            self.next()?;
            return Ok(true);
        }
        if self.type_ == T::BraceL {
            return Ok(self.parse_obj(true, None).is_ok());
        }
        if self.type_ == T::BracketL {
            self.next()?;
            return Ok(self.parse_binding_list(T::BracketR, true, true, None).is_ok());
        }
        Ok(false)
    }

    /// `tsIsUnambiguouslyStartOfFunctionType()`
    fn ts_is_unambiguously_start_of_function_type(&mut self) -> PResult<bool> {
        self.next()?;
        if self.type_ == T::ParenR || self.type_ == T::Ellipsis {
            return Ok(true);
        }
        if self.ts_skip_parameter_start()? {
            if matches!(self.type_, T::Colon | T::Comma | T::Question | T::Eq) {
                return Ok(true);
            }
            if self.type_ == T::ParenR {
                self.next()?;
                if self.type_ == T::Arrow {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// `tsIsStartOfFunctionType()`
    fn ts_is_start_of_function_type(&mut self) -> PResult<bool> {
        if self.ts_match_left_relational() {
            return Ok(true);
        }
        Ok(self.type_ == T::ParenL && self.ts_look_ahead(|p| p.ts_is_unambiguously_start_of_function_type())?)
    }

    /// `tsIsUnambiguouslyIndexSignature()`
    fn ts_is_unambiguously_index_signature(&mut self) -> PResult<bool> {
        self.next()?;
        if token_is_identifier(self.type_) {
            self.next()?;
            return Ok(self.type_ == T::Colon);
        }
        Ok(false)
    }

    /// `tsTryParseIndexSignature(node)`: `start` is the node's start.
    pub(crate) fn ts_try_parse_index_signature(&mut self, start: u32) -> PResult<Option<TSIndexSignature>> {
        if !(self.type_ == T::BracketL && self.ts_look_ahead(|p| p.ts_is_unambiguously_index_signature())?) {
            return Ok(None);
        }
        self.expect(T::BracketL)?;
        let mut id = self.parse_ident(false)?;
        let annotation = self.ts_parse_type_annotation(true, None)?;
        id.ts = Some(Box::new(TsPatternExtra { type_annotation: Some(annotation), ..TsPatternExtra::default() }));
        id.end = self.last_tok_end;
        self.expect(T::BracketR)?;
        let type_annotation = self.ts_try_parse_type_annotation()?;
        self.ts_parse_type_member_semicolon()?;
        Ok(Some(TSIndexSignature {
            start,
            end: self.last_tok_end,
            readonly: false,
            is_static: false,
            parameters: vec![id],
            type_annotation,
            decorators: None,
        }))
    }

    // --- modifiers

    /// `tsTokenCanFollowModifier()`
    pub(crate) fn ts_token_can_follow_modifier(&self) -> bool {
        (matches!(self.type_, T::BracketL | T::BraceL | T::Star | T::Ellipsis | T::PrivateId)
            || self.is_literal_property_name())
            && !self.has_preceding_line_break()
    }

    /// `tsNextTokenCanFollowModifier()`
    fn ts_next_token_can_follow_modifier(&mut self) -> PResult<Option<bool>> {
        self.next_ex(true)?;
        Ok(if self.ts_token_can_follow_modifier() { Some(true) } else { None })
    }

    /// `tsIsStartOfStaticBlocks()`
    pub(crate) fn ts_is_start_of_static_blocks(&self) -> bool {
        self.is_contextual("static") && self.lookahead_char_code() == 123
    }

    /// `tsParseModifier(allowedModifiers, stopOnStartOfClassStaticBlock)`
    fn ts_parse_modifier(&mut self, allowed_modifiers: &[&'static str], stop_on_start_of_class_static_block: bool) -> PResult<Option<&'static str>> {
        let Some(modifier) = (match &self.value {
            TokVal::Str(s) => allowed_modifiers.iter().find(|m| **m == s.as_str()).copied(),
            _ => None,
        }) else {
            return Ok(None);
        };
        if !self.contains_esc {
            if stop_on_start_of_class_static_block && self.ts_is_start_of_static_blocks() {
                return Ok(None);
            }
            if self.ts_try_parse(|p| p.ts_next_token_can_follow_modifier())?.is_some() {
                return Ok(Some(modifier));
            }
        }
        Ok(None)
    }

    /// `tsParseModifiers({ modified, allowedModifiers, disallowedModifiers,
    /// stopOnStartOfClassStaticBlock, errorTemplate })`: returns `modifiedMap`.
    pub(crate) fn ts_parse_modifiers(
        &mut self,
        modified: &mut Modified,
        allowed_modifiers: &[&'static str],
        disallowed_modifiers: &[&'static str],
        stop_on_start_of_class_static_block: bool,
        error_template: ModifierError,
    ) -> PResult<Modified> {
        let mut modified_map = Modified::default();
        let all: Vec<&'static str> = allowed_modifiers.iter().chain(disallowed_modifiers.iter()).copied().collect();
        loop {
            let start_loc = self.start;
            let Some(modifier) = self.ts_parse_modifier(&all, stop_on_start_of_class_static_block)? else { break };
            let enforce_order = |p: &Self, modified: &Modified, before: &str, after: &str| -> PResult<()> {
                if modifier == before && modified.get(after) {
                    return p.raise(p.column_of(start_loc), err::INVALID_MODIFIERS_ORDER(before, after));
                }
                Ok(())
            };
            let incompatible = |p: &Self, modified: &Modified, mod1: &str, mod2: &str| -> PResult<()> {
                if modified.get(mod1) && modifier == mod2 || modified.get(mod2) && modifier == mod1 {
                    return p.raise(p.column_of(start_loc), err::INCOMPATIBLE_MODIFIERS(mod1, mod2));
                }
                Ok(())
            };
            if ts_is_access_modifier(modifier) {
                if modified.accessibility.is_some() {
                    return self.raise(self.start, err::DUPLICATE_ACCESSIBILITY_MODIFIER);
                } else {
                    enforce_order(self, modified, modifier, "override")?;
                    enforce_order(self, modified, modifier, "static")?;
                    enforce_order(self, modified, modifier, "readonly")?;
                    enforce_order(self, modified, modifier, "accessor")?;
                    modified_map.accessibility = Some(modifier);
                    modified.accessibility = Some(modifier);
                }
            } else if modifier == "in" || modifier == "out" {
                if modified.get(modifier) {
                    return self.raise(self.start, err::DUPLICATE_MODIFIER(modifier));
                } else {
                    enforce_order(self, modified, "in", "out")?;
                    modified_map.set(modifier);
                    modified.set(modifier);
                }
            } else if modifier == "accessor" {
                if modified.get(modifier) {
                    return self.raise(self.start, err::DUPLICATE_MODIFIER(modifier));
                } else {
                    incompatible(self, modified, "accessor", "readonly")?;
                    incompatible(self, modified, "accessor", "override")?;
                    modified_map.set(modifier);
                    modified.set(modifier);
                }
            } else if modifier == "const" {
                if modified.get(modifier) {
                    return self.raise(self.start, err::DUPLICATE_MODIFIER(modifier));
                } else {
                    modified_map.set(modifier);
                    modified.set(modifier);
                }
            } else if modified.get(modifier) {
                return self.raise(self.start, err::DUPLICATE_MODIFIER(modifier));
            } else {
                enforce_order(self, modified, "static", "readonly")?;
                enforce_order(self, modified, "static", "override")?;
                enforce_order(self, modified, "override", "readonly")?;
                enforce_order(self, modified, "abstract", "override")?;
                incompatible(self, modified, "declare", "override")?;
                incompatible(self, modified, "static", "abstract")?;
                modified_map.set(modifier);
                modified.set(modifier);
            }
            if disallowed_modifiers.contains(&modifier) {
                return self.raise(self.start, error_template.source_text());
            }
        }
        Ok(modified_map)
    }

    fn ts_parse_type_param_modifiers(&mut self, which: TypeParamModifiers, modified: &mut Modified) -> PResult<()> {
        const DISALLOWED: [&str; 7] = ["public", "private", "protected", "readonly", "declare", "abstract", "override"];
        match which {
            TypeParamModifiers::None => {
                self.ts_parse_modifiers(modified, &[], &["in", "out"], false, ModifierError::OnTypeParameterPositions)?;
            }
            TypeParamModifiers::Const => {
                self.ts_parse_modifiers(
                    modified,
                    &["const"],
                    // for better error recovery
                    &["in", "out"],
                    false,
                    ModifierError::OnTypeParameterPositions,
                )?;
            }
            TypeParamModifiers::InOut => {
                self.ts_parse_modifiers(modified, &["in", "out"], &DISALLOWED, false, ModifierError::OnTypeParameter)?;
            }
            TypeParamModifiers::Class => {
                self.ts_parse_modifiers(modified, &["const", "in", "out"], &DISALLOWED, false, ModifierError::OnTypeParameter)?;
            }
        }
        Ok(())
    }

    // --- type parameters / arguments

    /// `tsParseTypeParameter(parseModifiers)`
    fn ts_parse_type_parameter(&mut self, parse_modifiers: TypeParamModifiers) -> PResult<TSTypeParameter> {
        let start = self.start;
        let mut modified = Modified::default();
        self.ts_parse_type_param_modifiers(parse_modifiers, &mut modified)?;
        let name = self.ts_parse_type_parameter_name()?;
        let constraint = self.ts_eat_then_parse_type(T::Extends)?.map(Box::new);
        let default = self.ts_eat_then_parse_type(T::Eq)?.map(Box::new);
        Ok(TSTypeParameter {
            start,
            end: self.last_tok_end,
            name,
            is_in: modified.is_in,
            is_out: modified.is_out,
            is_const: modified.is_const,
            constraint,
            default,
        })
    }

    /// `tsParseTypeParameters(parseModifiers)`
    pub(crate) fn ts_parse_type_parameters(&mut self, parse_modifiers: TypeParamModifiers) -> PResult<TSTypeParameterDeclaration> {
        let start = self.start;
        if self.ts_match_left_relational() {
            self.next()?;
        } else {
            return self.unexpected(None);
        }
        let mut ref_trailing_comma_pos: i64 = -1;
        let params = self.ts_parse_bracketed_list(
            ListKind::TypeParametersOrArguments,
            |p| p.ts_parse_type_parameter(parse_modifiers),
            /* bracket */ false,
            /* skipFirstToken */ true,
            Some(&mut ref_trailing_comma_pos),
        )?;
        if params.is_empty() {
            return self.raise(self.start, err::EMPTY_TYPE_PARAMETERS);
        }
        let extra = if ref_trailing_comma_pos != -1 {
            Some(TsTrailingComma { trailing_comma: ref_trailing_comma_pos as u32 })
        } else {
            None
        };
        Ok(TSTypeParameterDeclaration { start, end: self.last_tok_end, params, extra })
    }

    /// `tsTryParseTypeParameters(parseModifiers)`
    pub(crate) fn ts_try_parse_type_parameters(&mut self, parse_modifiers: TypeParamModifiers) -> PResult<Option<TSTypeParameterDeclaration>> {
        if self.ts_match_left_relational() { Ok(Some(self.ts_parse_type_parameters(parse_modifiers)?)) } else { Ok(None) }
    }

    /// `tsParseTypeArguments(inExpression = false)`
    pub(crate) fn ts_parse_type_arguments(&mut self, in_expression: bool) -> PResult<TSTypeParameterInstantiation> {
        let start = self.start;
        let params = self.ts_in_type(|p| {
            // Temporarily remove a JSX parsing context, which makes us scan different tokens.
            p.ts_in_no_context(|p| {
                p.expect(T::Relational)?;
                p.ts_parse_delimited_list(ListKind::TypeParametersOrArguments, |p| p.ts_parse_type(), None)
            })
        })?;
        if params.is_empty() {
            return self.raise(self.start, err::EMPTY_TYPE_ARGUMENTS);
        }
        // (`this.curContext() !== tsTokContexts.tc_oTag` always holds without JSX)
        if in_expression {
            self.re_scan_lt_gt()?;
        }
        if !self.ts_match_right_relational() {
            return self.unexpected(None);
        }
        self.expr_allowed = false;
        self.next()?;
        Ok(TSTypeParameterInstantiation { start, end: self.last_tok_end, params })
    }

    /// `tsParseTypeArgumentsInExpression()`: used when parsing type arguments from ES
    /// productions, where the first token has been created without state.inType. Thus we need
    /// to rescan the lt token.
    pub(crate) fn ts_parse_type_arguments_in_expression(&mut self) -> PResult<Option<TSTypeParameterInstantiation>> {
        if self.re_scan_lt()? != T::Relational {
            return Ok(None);
        }
        Ok(Some(self.ts_parse_type_arguments(true)?))
    }

    /// `tsParseHeritageClause(token)`
    pub(crate) fn ts_parse_heritage_clause(&mut self, token: &str) -> PResult<Vec<TSExpressionWithTypeArguments>> {
        let original_start = self.start;
        let delimited_list = self.ts_parse_delimited_list(
            ListKind::HeritageClauseElement,
            |p| {
                let start = p.start;
                let expression = p.ts_parse_entity_name(true)?;
                let type_parameters = if p.ts_match_left_relational() { Some(p.ts_parse_type_arguments(false)?) } else { None };
                Ok(TSExpressionWithTypeArguments { start, end: p.last_tok_end, expression, type_parameters })
            },
            None,
        )?;
        if delimited_list.is_empty() {
            return self.raise(original_start, err::EMPTY_HERITAGE_CLAUSE_TYPE(token));
        }
        Ok(delimited_list)
    }

    // --- type members

    /// `tsParseTypeMemberSemicolon()`
    pub(crate) fn ts_parse_type_member_semicolon(&mut self) -> PResult<()> {
        if !self.eat(T::Comma)? && !self.is_line_terminator()? {
            self.expect(T::Semi)?;
        }
        Ok(())
    }

    /// `tsParseSignatureMember(kind, node)`
    fn ts_parse_signature_member(&mut self, construct: bool, start: u32) -> PResult<TSTypeElement> {
        let sig = self.ts_fill_signature(T::Colon)?;
        self.ts_parse_type_member_semicolon()?;
        let node = TSSignatureDeclaration {
            start,
            end: self.last_tok_end,
            type_parameters: sig.type_parameters,
            parameters: sig.parameters,
            type_annotation: sig.type_annotation,
        };
        Ok(if construct {
            TSTypeElement::TSConstructSignatureDeclaration(node)
        } else {
            TSTypeElement::TSCallSignatureDeclaration(node)
        })
    }

    /// `tsParsePropertyOrMethodSignature(node, readonly)`
    fn ts_parse_property_or_method_signature(
        &mut self,
        start: u32,
        computed: bool,
        key: PropertyKey,
        kind: Option<&'static str>,
        readonly: bool,
    ) -> PResult<TSTypeElement> {
        let optional = self.eat(T::Question)?;
        if self.type_ == T::ParenL || self.ts_match_left_relational() {
            if readonly {
                return self.raise(start, err::READONLY_FOR_METHOD_SIGNATURE);
            }
            if kind.is_some() && self.ts_match_left_relational() {
                return self.raise(self.start, err::ACCESOR_CANNOT_HAVE_TYPE_PARAMETERS);
            }
            let sig = self.ts_fill_signature(T::Colon)?;
            self.ts_parse_type_member_semicolon()?;
            let is_this_param = |p: &Pattern| matches!(p, Pattern::Identifier(i) if i.name == "this");
            if kind == Some("get") {
                if !sig.parameters.is_empty() {
                    return self.raise(self.start, "A 'get' accesor must not have any formal parameters.");
                }
            } else if kind == Some("set") {
                if sig.parameters.len() != 1 {
                    return self.raise(self.start, "A 'get' accesor must not have any formal parameters.");
                } else {
                    let first_parameter = &sig.parameters[0];
                    if is_this_param(first_parameter) {
                        return self.raise(self.start, err::ACCESOR_CANNOT_DECLARE_THIS_PARAMETER);
                    }
                    if let Pattern::Identifier(i) = first_parameter {
                        if i.ts.as_ref().is_some_and(|t| t.optional) {
                            return self.raise(self.start, err::SET_ACCESOR_CANNOT_HAVE_OPTIONAL_PARAMETER);
                        }
                    }
                    if matches!(first_parameter, Pattern::RestElement(_)) {
                        return self.raise(self.start, err::SET_ACCESOR_CANNOT_HAVE_REST_PARAMETER);
                    }
                }
                if let Some(t) = &sig.type_annotation {
                    return self.raise(t.start, err::SET_ACCESOR_CANNOT_HAVE_RETURN_TYPE);
                }
            }
            return Ok(TSTypeElement::TSMethodSignature(TSMethodSignature {
                start,
                end: self.last_tok_end,
                readonly,
                computed,
                key,
                kind: kind.unwrap_or("method"),
                optional,
                type_parameters: sig.type_parameters,
                parameters: sig.parameters,
                type_annotation: sig.type_annotation,
            }));
        }
        let type_annotation = self.ts_try_parse_type_annotation()?;
        self.ts_parse_type_member_semicolon()?;
        Ok(TSTypeElement::TSPropertySignature(TSPropertySignature {
            start,
            end: self.last_tok_end,
            readonly,
            computed,
            key,
            kind,
            optional,
            type_annotation,
        }))
    }

    /// `tsParseTypeMember()`
    fn ts_parse_type_member(&mut self) -> PResult<TSTypeElement> {
        let start = self.start;
        if self.type_ == T::ParenL || self.ts_match_left_relational() {
            return self.ts_parse_signature_member(false, start);
        }
        if self.type_ == T::New {
            let id_start = self.start;
            self.next()?;
            if self.type_ == T::ParenL || self.ts_match_left_relational() {
                return self.ts_parse_signature_member(true, start);
            } else {
                let key = Identifier { start: id_start, end: self.last_tok_end, name: "new".into(), ts: None };
                return self.ts_parse_property_or_method_signature(
                    start,
                    false,
                    PropertyKey::Expression(Box::new(Expression::Identifier(key))),
                    None,
                    false,
                );
            }
        }
        let mut modified = Modified::default();
        self.ts_parse_modifiers(
            &mut modified,
            &["readonly"],
            &["declare", "abstract", "private", "protected", "public", "static", "override"],
            false,
            ModifierError::OnTypeMember,
        )?;
        if let Some(mut idx) = self.ts_try_parse_index_signature(start)? {
            idx.readonly = modified.readonly;
            return Ok(TSTypeElement::TSIndexSignature(idx));
        }
        let (mut computed, mut key) = self.parse_property_name()?;
        let mut kind = None;
        if !computed && self.ts_token_can_follow_modifier() {
            if let PropertyKey::Expression(e) = &key {
                if let Expression::Identifier(i) = &**e {
                    if i.name == "get" || i.name == "set" {
                        kind = Some(if i.name == "get" { "get" } else { "set" });
                        (computed, key) = self.parse_property_name()?;
                    }
                }
            }
        }
        self.ts_parse_property_or_method_signature(start, computed, key, kind, modified.readonly)
    }

    /// `tsParseObjectTypeMembers()`
    fn ts_parse_object_type_members(&mut self) -> PResult<Vec<TSTypeElement>> {
        self.expect(T::BraceL)?;
        let members = self.ts_parse_list(ListKind::TypeMembers, |p| p.ts_parse_type_member())?;
        self.expect(T::BraceR)?;
        Ok(members)
    }

    /// `tsParseInterfaceBody()`: parse interface body, ensuring the closing brace is read outside
    /// of type context so that decorators following the interface are properly tokenized.
    pub(crate) fn ts_parse_interface_body(&mut self) -> PResult<Vec<TSTypeElement>> {
        self.expect(T::BraceL)?;
        let old_in_type = self.tss().in_type;
        self.tsm().in_type = true;
        let members = self.ts_parse_list(ListKind::TypeMembers, |p| p.ts_parse_type_member());
        self.tsm().in_type = old_in_type;
        let members = members?;
        self.expect(T::BraceR)?;
        Ok(members)
    }
}
