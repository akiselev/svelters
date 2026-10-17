//! acorn-typescript 1.0.13: overrides of acorn's `statement.js` methods, the TypeScript
//! declaration parsers (`tsParse*Declaration`), decorators (src/extentions/decorators.ts) and
//! import attributes (src/extentions/import-assertions.ts).

use super::expression::{key_is, Bodiless};
use super::lval::{pattern_ts, pattern_ts_mut, set_pattern_end};
use super::types::{ListKind, ModifierError, Modified, TypeParamModifiers};
use super::{err, token_is_identifier, token_is_keyword_or_identifier, token_is_ts_declaration_start, LookaheadState};
use super::{BIND_FLAGS_TS_EXPORT_ONLY, BIND_TS_CONST_ENUM, BIND_TS_ENUM, BIND_TS_INTERFACE, BIND_TS_NAMESPACE, BIND_TS_TYPE};
use super::{TS_SCOPE_OTHER, TS_SCOPE_TS_MODULE};
use crate::acorn::expression::{FnBody, ForInit};
use crate::acorn::scopeflags::*;
use crate::acorn::state::{PResult, Parser, TokVal};
use crate::acorn::statement::{Exports, IsStatement};
use crate::acorn::tokentype::TokenType as T;
use crate::estree::*;
use std::collections::HashSet;

const FUNC_STATEMENT: u32 = 1;
const FUNC_HANGING_STATEMENT: u32 = 2;
const FUNC_NULLABLE_ID: u32 = 4;

/// `parseFunction` result: a function, or a `TSDeclareFunction` (no body).
pub enum FnOrDeclare {
    Function(Function),
    Declare(TSDeclareFunction),
}

impl FnOrDeclare {
    pub fn into_statement(self) -> Statement {
        match self {
            FnOrDeclare::Function(f) => Statement::FunctionDeclaration(f),
            FnOrDeclare::Declare(d) => Statement::TSDeclareFunction(d),
        }
    }
}

fn var_kind(s: &str) -> VariableKind {
    match s {
        "let" => VariableKind::Let,
        "const" => VariableKind::Const,
        _ => VariableKind::Var,
    }
}

/// `isPossiblyLiteralEnum(expression)`
fn is_possibly_literal_enum(expression: &Expression) -> bool {
    let Expression::MemberExpression(m) = expression else { return false };
    if m.computed {
        let PropertyKey::Expression(p) = &m.property else { return false };
        if !matches!(&**p, Expression::TemplateLiteral(t) if t.expressions.is_empty()) {
            return false;
        }
    }
    is_uncomputed_member_expression_chain(&m.object)
}

/// `isAmbientNumericUnaryExpression(expression)`
fn is_ambient_numeric_unary_expression(expression: &Expression) -> bool {
    matches!(expression, Expression::UnaryExpression(u)
        if (u.operator == UnaryOperator::Minus || u.operator == UnaryOperator::Plus)
            && matches!(&*u.argument, Expression::Literal(Literal { value: LiteralValue::Number(_) | LiteralValue::BigInt, .. })))
}

/// `isUncomputedMemberExpressionChain(expression)`
fn is_uncomputed_member_expression_chain(expression: &Expression) -> bool {
    match expression {
        Expression::Identifier(_) => true,
        Expression::MemberExpression(m) => !m.computed && is_uncomputed_member_expression_chain(&m.object),
        _ => false,
    }
}

/// `declaration.declare = true` on whatever declaration node it is.
pub fn set_declare(s: &mut Statement) {
    match s {
        Statement::VariableDeclaration(d) => d.ts.get_or_insert_with(Box::default).declare = true,
        Statement::ClassDeclaration(c) => c.ts.get_or_insert_with(Box::default).declare = true,
        Statement::FunctionDeclaration(f) => f.ts.get_or_insert_with(Box::default).declare = true,
        Statement::TSDeclareFunction(f) => f.ts.declare = true,
        Statement::TSEnumDeclaration(n) => n.declare = true,
        Statement::TSModuleDeclaration(n) => n.declare = true,
        Statement::TSInterfaceDeclaration(n) => n.declare = true,
        Statement::TSTypeAliasDeclaration(n) => n.declare = true,
        _ => {}
    }
}

/// `resetStartLocation(declaration, start)`
fn set_statement_start(s: &mut Statement, start: u32) {
    match s {
        Statement::VariableDeclaration(n) => n.start = start,
        Statement::ClassDeclaration(n) => n.start = start,
        Statement::FunctionDeclaration(n) => n.start = start,
        Statement::TSDeclareFunction(n) => n.start = start,
        Statement::TSEnumDeclaration(n) => n.start = start,
        Statement::TSModuleDeclaration(n) => n.start = start,
        Statement::TSInterfaceDeclaration(n) => n.start = start,
        Statement::TSTypeAliasDeclaration(n) => n.start = start,
        Statement::ExpressionStatement(n) => n.start = start,
        _ => {}
    }
}

fn module_name_of(n: &ModuleExportName) -> (String, u32) {
    match n {
        ModuleExportName::Identifier(i) => (i.name.clone(), i.start),
        ModuleExportName::Literal(l) => (
            match &l.value {
                LiteralValue::String(s) => s.clone(),
                _ => l.raw.clone(),
            },
            l.start,
        ),
    }
}

impl<'a> Parser<'a> {
    // --- functions

    /// `parseFunction(node, statement, allowExpressionBody, isAsync, forInit)`; `declare`: the
    /// node's `declare` set by `tsTryParseDeclare`.
    pub(crate) fn parse_function_ts(
        &mut self,
        start: u32,
        statement: u32,
        _allow_expression_body: bool,
        is_async: bool,
        for_init: ForInit,
        declare: bool,
    ) -> PResult<FnOrDeclare> {
        // initFunction
        let mut generator = false;
        if self.options.ecma_version >= 9 || self.options.ecma_version >= 6 && !is_async {
            if self.type_ == T::Star && (statement & FUNC_HANGING_STATEMENT) != 0 {
                return self.unexpected(None);
            }
            generator = self.eat(T::Star)?;
        }
        let is_async_node = self.options.ecma_version >= 8 && is_async;
        let mut id = None;
        if statement & FUNC_STATEMENT != 0 {
            id = if (statement & FUNC_NULLABLE_ID) != 0 && self.type_ != T::Name { None } else { Some(self.parse_ident(false)?) };
        }
        let old_yield_pos = self.yield_pos;
        let old_await_pos = self.await_pos;
        let old_await_ident_pos = self.await_ident_pos;
        let old_maybe_in_arrow_parameters = self.tss().maybe_in_arrow_parameters;
        self.tsm().maybe_in_arrow_parameters = false;
        self.yield_pos = 0;
        self.await_pos = 0;
        self.await_ident_pos = 0;
        self.enter_scope(function_flags(is_async_node, generator));
        if statement & FUNC_STATEMENT == 0 {
            id = if self.type_ == T::Name { Some(self.parse_ident(false)?) } else { None };
        }
        // parseFunctionParams(node)
        let type_parameters = self.ts_try_parse_type_parameters(TypeParamModifiers::Const)?;
        let params = self.parse_function_params()?;
        let is_declaration = statement & FUNC_STATEMENT != 0;
        let bodiless = if is_declaration { Some(Bodiless::FunctionDeclaration) } else { None };
        let (return_type, body) = self.parse_function_body_ts(start, &params, id.as_ref(), false, false, for_init, bodiless)?;
        if is_declaration && self.is_ambient_context() {
            if let Some(id) = &id {
                if self.current_scope().flags & SCOPE_TOP != 0 {
                    self.declare_name(&id.name, BIND_FLAGS_TS_EXPORT_ONLY, id.start)?;
                }
            }
        }
        self.yield_pos = old_yield_pos;
        self.await_pos = old_await_pos;
        self.await_ident_pos = old_await_ident_pos;
        if statement & FUNC_STATEMENT != 0 && statement & FUNC_HANGING_STATEMENT == 0 {
            if let Some(id) = &id {
                let binding = if body.is_some() {
                    if self.strict || generator || is_async_node {
                        if self.treat_functions_as_var() {
                            BIND_VAR
                        } else {
                            BIND_LEXICAL
                        }
                    } else {
                        BIND_FUNCTION
                    }
                } else {
                    BIND_NONE
                };
                let id = id.clone();
                self.check_lval_identifier(&id, binding, None)?;
            }
        }
        self.tsm().maybe_in_arrow_parameters = old_maybe_in_arrow_parameters;
        let extra = TsFunctionExtra { type_parameters, return_type, declare };
        let end = self.last_tok_end;
        Ok(match body {
            None => FnOrDeclare::Declare(TSDeclareFunction {
                start,
                end,
                id,
                expression: false,
                generator,
                is_async: is_async_node,
                params,
                ts: extra,
            }),
            Some(body) => FnOrDeclare::Function(Function {
                start,
                end,
                id,
                expression: false,
                generator,
                is_async: is_async_node,
                params,
                body: BlockStatementNode::BlockStatement(body.into_block()),
                ts: if extra == TsFunctionExtra::default() { None } else { Some(Box::new(extra)) },
            }),
        })
    }

    /// `parseFunctionBody(node, isArrowFunction, isMethod, forInit, tsConfig)`: the return
    /// type, and the body (`None`: a bodiless `TSDeclareFunction` / `TSDeclareMethod`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn parse_function_body_ts(
        &mut self,
        node_start: u32,
        params: &[Pattern],
        id: Option<&Identifier>,
        is_arrow_function: bool,
        is_method: bool,
        for_init: ForInit,
        bodiless: Option<Bodiless>,
    ) -> PResult<(Option<TSTypeAnnotation>, Option<FnBody>)> {
        let return_type =
            if self.type_ == T::Colon { Some(self.ts_parse_type_or_type_predicate_annotation(T::Colon)?) } else { None };
        if bodiless.is_some() && self.type_ != T::BraceL && self.is_line_terminator()? {
            self.exit_scope();
            return Ok((return_type, None));
        }
        if bodiless == Some(Bodiless::FunctionDeclaration) && self.is_ambient_context() {
            return self.raise(node_start, err::DECLARE_FUNCTION_HAS_IMPLEMENTATION);
        }
        let body = self.parse_function_body(node_start, params, id, is_arrow_function, is_method, for_init)?;
        Ok((return_type, Some(body)))
    }

    /// acorn's `parseFunctionStatement` reaching the TypeScript `parseFunction`.
    pub(crate) fn parse_function_statement_ts(
        &mut self,
        start: u32,
        is_async: bool,
        declaration_position: bool,
        declare: bool,
    ) -> PResult<Statement> {
        self.next()?;
        let f = self.parse_function_ts(
            start,
            FUNC_STATEMENT | if declaration_position { 0 } else { FUNC_HANGING_STATEMENT },
            false,
            is_async,
            ForInit::No,
            declare,
        )?;
        Ok(f.into_statement())
    }

    // --- statements

    /// The `parseStatement` override (before acorn's): decorators, `const enum`, `enum`,
    /// `interface`.
    pub(crate) fn parse_statement_ts_prefix(&mut self) -> PResult<Option<Statement>> {
        if self.type_ == T::At {
            self.parse_decorators(true)?;
        }
        if self.type_ == T::Const && self.is_lookahead_contextual("enum") {
            let start = self.start;
            self.expect(T::Const)?;
            return Ok(Some(self.ts_parse_enum_declaration(start, true, false)?));
        }
        if self.ts_is_contextual(T::TsEnum) {
            let start = self.start;
            return Ok(Some(self.ts_parse_enum_declaration(start, false, false)?));
        }
        if self.ts_is_contextual(T::TsInterface) {
            let start = self.start;
            if let Some(result) = self.ts_parse_interface_declaration(start, false, false)? {
                return Ok(Some(result));
            }
        }
        Ok(None)
    }

    /// `parseVarStatement(node, kind, allowMissingInitializer)`
    pub fn parse_var_statement_ts(
        &mut self,
        start: u32,
        kind: VariableKind,
        allow_missing_initializer: bool,
        declare: bool,
    ) -> PResult<Statement> {
        let is_ambient_context = self.is_ambient_context();
        self.next()?;
        let declarations = self.parse_var(false, kind, allow_missing_initializer || is_ambient_context)?;
        self.semicolon()?;
        let declaration = VariableDeclaration {
            start,
            end: self.last_tok_end,
            declarations,
            kind,
            ts: if declare { Some(Box::new(TsDeclareExtra { declare: true })) } else { None },
        };
        if is_ambient_context {
            for d in &declaration.declarations {
                let Some(init) = &d.init else { continue };
                let id_has_type = pattern_ts(&d.id).is_some_and(|t| t.type_annotation.is_some());
                if kind != VariableKind::Const || id_has_type {
                    return self.raise(init.span().start, err::INITIALIZER_NOT_ALLOWED_IN_AMBIENT_CONTEXT);
                } else if !matches!(init, Expression::Literal(_))
                    && !matches!(init, Expression::TemplateLiteral(t) if t.expressions.is_empty())
                    && !is_ambient_numeric_unary_expression(init)
                    && !is_possibly_literal_enum(init)
                {
                    return self.raise(
                        init.span().start,
                        err::CONST_INITIALIZER_MUST_BE_STRING_OR_NUMERIC_LITERAL_OR_LITERAL_ENUM_REFERENCE,
                    );
                }
            }
        }
        Ok(Statement::VariableDeclaration(declaration))
    }

    /// `parseVarId(decl, kind)`: the id and the declarator's `definite`.
    pub fn parse_var_id_ts(&mut self, kind: VariableKind) -> PResult<(Pattern, bool)> {
        let mut id = self.parse_var_id(kind)?;
        let mut definite = false;
        if matches!(id, Pattern::Identifier(_))
            && !self.has_preceding_line_break()
            && self.type_ == T::Prefix
            && self.tok_text() == "!"
        {
            self.next()?;
            definite = true;
        }
        if let Some(type_) = self.ts_try_parse_type_annotation()? {
            if let Some(ts) = pattern_ts_mut(&mut id) {
                ts.type_annotation = Some(type_);
            }
            set_pattern_end(&mut id, self.last_tok_end);
        }
        Ok((id, definite))
    }

    /// `parseCatchClauseParam()`
    pub fn parse_catch_clause_param_ts(&mut self) -> PResult<Pattern> {
        let mut param = self.parse_binding_atom()?;
        let simple = matches!(param, Pattern::Identifier(_));
        self.enter_scope(if simple { SCOPE_SIMPLE_CATCH } else { 0 });
        self.check_lval_pattern(&param, if simple { BIND_SIMPLE_CATCH } else { BIND_LEXICAL }, None)?;
        if let Some(type_) = self.ts_try_parse_type_annotation()? {
            if let Some(ts) = pattern_ts_mut(&mut param) {
                ts.type_annotation = Some(type_);
            }
            set_pattern_end(&mut param, self.last_tok_end);
        }
        self.expect(T::ParenR)?;
        Ok(param)
    }

    /// The `parseExpressionStatement` override: `expr` is an Identifier
    /// (`tsParseExpressionStatement`).
    pub(crate) fn ts_parse_expression_statement(&mut self, start: u32, expr: &Identifier) -> PResult<Option<Statement>> {
        match expr.name.as_str() {
            "declare" => {
                if let Some(mut declaration) = self.ts_try_parse_declare(start)? {
                    set_declare(&mut declaration);
                    return Ok(Some(declaration));
                }
            }
            "global" => {
                if self.type_ == T::BraceL {
                    self.enter_scope(TS_SCOPE_TS_MODULE);
                    let body = self.ts_parse_module_block()?;
                    self.exit_scope_base();
                    return Ok(Some(Statement::TSModuleDeclaration(TSModuleDeclaration {
                        start,
                        end: self.last_tok_end,
                        declare: false,
                        global: true,
                        id: ModuleExportName::Identifier(expr.clone()),
                        body: Some(TSModuleBody::TSModuleBlock(body)),
                    })));
                }
            }
            _ => return self.ts_parse_declaration(start, &expr.name.clone(), false),
        }
        Ok(None)
    }

    /// `tsTryParseDeclare(nany)`
    fn ts_try_parse_declare(&mut self, start: u32) -> PResult<Option<Statement>> {
        if self.is_line_terminator()? {
            return Ok(None);
        }
        let mut starttype = self.type_;
        let mut kind = None;
        if self.is_contextual("let") {
            starttype = T::Var;
            kind = Some("let");
        }
        self.ts_in_ambient_context(|p| {
            if starttype == T::Function {
                return Ok(Some(p.parse_function_statement_ts(start, false, true, true)?));
            }
            if starttype == T::Class {
                return Ok(Some(Statement::ClassDeclaration(p.parse_class_ts(start, IsStatement::Yes, false, true)?)));
            }
            if starttype == T::TsEnum {
                return Ok(Some(p.ts_parse_enum_declaration(start, false, true)?));
            }
            if starttype == T::TsGlobal {
                return Ok(Some(p.ts_parse_ambient_external_module_declaration(start)?));
            }
            if starttype == T::Const || starttype == T::Var {
                if p.type_ != T::Const || !p.is_lookahead_contextual("enum") {
                    let k = kind.unwrap_or_else(|| if p.type_ == T::Const { "const" } else { "var" });
                    return Ok(Some(p.parse_var_statement_ts(start, var_kind(k), true, true)?));
                }
                p.expect(T::Const)?;
                return Ok(Some(p.ts_parse_enum_declaration(start, true, true)?));
            }
            if starttype == T::TsInterface {
                if let Some(result) = p.ts_parse_interface_declaration(start, true, false)? {
                    return Ok(Some(result));
                }
            }
            if token_is_identifier(starttype) {
                let value = p.value_str().to_string();
                return p.ts_parse_declaration(start, &value, /* next */ true);
            }
            Ok(None)
        })
    }

    /// `tsCheckLineTerminator(next)`
    fn ts_check_line_terminator(&mut self, next: bool) -> PResult<bool> {
        if next {
            if self.has_following_line_break() {
                return Ok(false);
            }
            self.next()?;
            return Ok(true);
        }
        Ok(!self.is_line_terminator()?)
    }

    /// `tsParseDeclaration(node, value, next)`: common to tsTryParseDeclare,
    /// tsTryParseExportDeclaration, and tsParseExpressionStatement.
    fn ts_parse_declaration(&mut self, start: u32, value: &str, next: bool) -> PResult<Option<Statement>> {
        match value {
            "abstract" => {
                if self.ts_check_line_terminator(next)? && (self.type_ == T::Class || token_is_identifier(self.type_)) {
                    return self.ts_parse_abstract_declaration(start);
                }
            }
            "module" => {
                if self.ts_check_line_terminator(next)? {
                    if self.type_ == T::String {
                        return Ok(Some(self.ts_parse_ambient_external_module_declaration(start)?));
                    } else if token_is_identifier(self.type_) {
                        return Ok(Some(Statement::TSModuleDeclaration(self.ts_parse_module_or_namespace_declaration(start, false)?)));
                    }
                }
            }
            "namespace" => {
                if self.ts_check_line_terminator(next)? && token_is_identifier(self.type_) {
                    return Ok(Some(Statement::TSModuleDeclaration(self.ts_parse_module_or_namespace_declaration(start, false)?)));
                }
            }
            "type" => {
                if self.ts_check_line_terminator(next)? && token_is_identifier(self.type_) {
                    return Ok(Some(self.ts_parse_type_alias_declaration(start)?));
                }
            }
            _ => {}
        }
        Ok(None)
    }

    /// `tsParseAbstractDeclaration(node)`
    fn ts_parse_abstract_declaration(&mut self, start: u32) -> PResult<Option<Statement>> {
        if self.type_ == T::Class {
            return Ok(Some(Statement::ClassDeclaration(self.parse_class_ts(start, IsStatement::Yes, true, false)?)));
        } else if self.ts_is_contextual(T::TsInterface) {
            if !self.has_following_line_break() {
                return self.ts_parse_interface_declaration(start, false, true);
            }
        } else {
            return self.unexpected(Some(start));
        }
        Ok(None)
    }

    /// `tsParseEnumMember()`
    fn ts_parse_enum_member(&mut self) -> PResult<TSEnumMember> {
        let start = self.start;
        let id = if self.type_ == T::String {
            ModuleExportName::Literal(self.parse_literal_token()?)
        } else {
            ModuleExportName::Identifier(self.parse_ident(/* liberal */ true)?)
        };
        let initializer = if self.eat(T::Eq)? { Some(self.parse_maybe_assign(ForInit::No, None)?) } else { None };
        Ok(TSEnumMember { start, end: self.last_tok_end, id, initializer })
    }

    /// `tsParseEnumDeclaration(node, properties)`
    fn ts_parse_enum_declaration(&mut self, start: u32, is_const: bool, declare: bool) -> PResult<Statement> {
        self.expect_contextual("enum")?;
        let id = self.parse_ident(false)?;
        let binding_type = if is_const { BIND_TS_CONST_ENUM } else { BIND_TS_ENUM };
        self.check_lval_identifier(&id, binding_type, None)?;
        self.expect(T::BraceL)?;
        let members = self.ts_parse_delimited_list(ListKind::EnumMembers, |p| p.ts_parse_enum_member(), None)?;
        self.expect(T::BraceR)?;
        Ok(Statement::TSEnumDeclaration(TSEnumDeclaration { start, end: self.last_tok_end, is_const, declare, id, members }))
    }

    /// `tsParseModuleBlock()`
    fn ts_parse_module_block(&mut self) -> PResult<TSModuleBlock> {
        let start = self.start;
        self.enter_scope(TS_SCOPE_OTHER);
        self.expect(T::BraceL)?;
        let mut body = Vec::new();
        while self.type_ != T::BraceR {
            let stmt = self.parse_statement(None, true, None)?;
            body.push(stmt);
        }
        self.next()?;
        self.exit_scope_base();
        Ok(TSModuleBlock { start, end: self.last_tok_end, body })
    }

    /// `tsParseAmbientExternalModuleDeclaration(node)`
    fn ts_parse_ambient_external_module_declaration(&mut self, start: u32) -> PResult<Statement> {
        let mut global = false;
        let id = if self.ts_is_contextual(T::TsGlobal) {
            global = true;
            ModuleExportName::Identifier(self.parse_ident(false)?)
        } else if self.type_ == T::String {
            ModuleExportName::Literal(self.parse_literal_token()?)
        } else {
            return self.unexpected(None);
        };
        let body = if self.type_ == T::BraceL {
            self.enter_scope(TS_SCOPE_TS_MODULE);
            let b = self.ts_parse_module_block()?;
            self.exit_scope_base();
            Some(TSModuleBody::TSModuleBlock(b))
        } else {
            self.semicolon()?;
            None
        };
        Ok(Statement::TSModuleDeclaration(TSModuleDeclaration { start, end: self.last_tok_end, declare: false, global, id, body }))
    }

    /// `tsParseModuleOrNamespaceDeclaration(node, nested)`
    fn ts_parse_module_or_namespace_declaration(&mut self, start: u32, nested: bool) -> PResult<TSModuleDeclaration> {
        let id = self.parse_ident(false)?;
        if !nested {
            self.check_lval_identifier(&id, BIND_TS_NAMESPACE, None)?;
        }
        let body = if self.eat(T::Dot)? {
            let inner_start = self.start;
            let inner = self.ts_parse_module_or_namespace_declaration(inner_start, true)?;
            TSModuleBody::TSModuleDeclaration(Box::new(inner))
        } else {
            self.enter_scope(TS_SCOPE_TS_MODULE);
            let b = self.ts_parse_module_block()?;
            self.exit_scope_base();
            TSModuleBody::TSModuleBlock(b)
        };
        Ok(TSModuleDeclaration {
            start,
            end: self.last_tok_end,
            declare: false,
            global: false,
            id: ModuleExportName::Identifier(id),
            body: Some(body),
        })
    }

    /// `tsParseTypeAliasDeclaration(node)`
    fn ts_parse_type_alias_declaration(&mut self, start: u32) -> PResult<Statement> {
        let id = self.parse_ident(false)?;
        self.check_lval_identifier(&id, BIND_TS_TYPE, None)?;
        self.maybe_export_defined(&id.name.clone());
        let mut type_parameters = None;
        let type_annotation = self.ts_in_type(|p| {
            type_parameters = p.ts_try_parse_type_parameters(TypeParamModifiers::InOut)?;
            p.expect(T::Eq)?;
            if p.ts_is_contextual(T::TsInterface) && p.lookahead()?.type_ != T::Dot {
                let node2_start = p.start;
                p.next()?;
                return Ok(TSType::TSIntrinsicKeyword(TSKeyword { start: node2_start, end: p.last_tok_end }));
            }
            p.ts_parse_type()
        })?;
        self.semicolon()?;
        Ok(Statement::TSTypeAliasDeclaration(TSTypeAliasDeclaration {
            start,
            end: self.last_tok_end,
            declare: false,
            id,
            type_parameters,
            type_annotation: Box::new(type_annotation),
        }))
    }

    /// `tsParseInterfaceDeclaration(node, properties)`
    fn ts_parse_interface_declaration(&mut self, start: u32, declare: bool, is_abstract: bool) -> PResult<Option<Statement>> {
        if self.has_following_line_break() {
            return Ok(None);
        }
        self.expect_contextual("interface")?;
        let id = if token_is_identifier(self.type_) {
            let id = self.parse_ident(false)?;
            self.check_lval_identifier(&id, BIND_TS_INTERFACE, None)?;
            self.maybe_export_defined(&id.name.clone());
            Some(id)
        } else {
            return self.raise(self.start, err::MISSING_INTERFACE_NAME);
        };
        let type_parameters = self.ts_try_parse_type_parameters(TypeParamModifiers::InOut)?;
        let extends = if self.eat(T::Extends)? { Some(self.ts_parse_heritage_clause("extends")?) } else { None };
        let body_start = self.start;
        let body = self.ts_parse_interface_body()?;
        let body = TSInterfaceBody { start: body_start, end: self.last_tok_end, body };
        Ok(Some(Statement::TSInterfaceDeclaration(TSInterfaceDeclaration {
            start,
            end: self.last_tok_end,
            declare,
            is_abstract,
            id,
            type_parameters,
            extends,
            body,
        })))
    }

    /// `tsParseImportEqualsDeclaration(node, isExport)`
    fn ts_parse_import_equals_declaration(&mut self, start: u32, import_kind: &'static str, is_export: bool) -> PResult<Statement> {
        let id = self.parse_ident(false)?;
        self.check_lval_identifier(&id, BIND_LEXICAL, None)?;
        self.expect(T::Eq)?;
        let module_reference = self.ts_parse_module_reference()?;
        if import_kind == "type" && !matches!(module_reference, TSModuleReference::TSExternalModuleReference(_)) {
            let s = match &module_reference {
                TSModuleReference::Identifier(i) => i.start,
                TSModuleReference::TSQualifiedName(q) => q.start,
                TSModuleReference::TSExternalModuleReference(e) => e.start,
            };
            return self.raise(s, err::IMPORT_ALIAS_HAS_IMPORT_TYPE);
        }
        self.semicolon()?;
        Ok(Statement::TSImportEqualsDeclaration(TSImportEqualsDeclaration {
            start,
            end: self.last_tok_end,
            import_kind,
            is_export,
            id,
            module_reference,
        }))
    }

    /// `tsParseModuleReference()`
    fn ts_parse_module_reference(&mut self) -> PResult<TSModuleReference> {
        if self.is_contextual("require") && self.lookahead_char_code() == 40 {
            // tsParseExternalModuleReference()
            let start = self.start;
            self.expect_contextual("require")?;
            self.expect(T::ParenL)?;
            if self.type_ != T::String {
                return self.unexpected(None);
            }
            let expression = self.parse_expr_atom(None, ForInit::No, false)?;
            self.expect(T::ParenR)?;
            return Ok(TSModuleReference::TSExternalModuleReference(TSExternalModuleReference {
                start,
                end: self.last_tok_end,
                expression,
            }));
        }
        Ok(match self.ts_parse_entity_name(/* allowReservedWords */ false)? {
            TSEntityName::Identifier(i) => TSModuleReference::Identifier(i),
            TSEntityName::TSQualifiedName(q) => TSModuleReference::TSQualifiedName(q),
        })
    }

    // --- classes

    /// `isAbstractClass()`
    fn is_abstract_class(&mut self) -> PResult<bool> {
        Ok(self.ts_is_contextual(T::TsAbstract) && self.lookahead()?.type_ == T::Class)
    }

    /// `isDeclareClass()`
    fn is_declare_class(&self) -> bool {
        if !self.ts_is_contextual(T::TsDeclare) {
            return false;
        }
        let after_declare = self.next_token_start();
        if self.is_unparsed_contextual(after_declare, "class") {
            return true;
        }
        if !self.is_unparsed_contextual(after_declare, "abstract") {
            return false;
        }
        let after_abstract = self.next_token_start_since(after_declare + "abstract".len() as u32);
        self.is_unparsed_contextual(after_abstract, "class")
    }

    /// `canHaveLeadingDecorator()`
    fn can_have_leading_decorator(&mut self) -> PResult<bool> {
        Ok(self.type_ == T::Class || self.is_abstract_class()? || self.is_declare_class())
    }

    /// `parseClass(node, isStatement)`; `is_abstract` / `declare`: set on the node by the caller.
    pub(crate) fn parse_class_ts(
        &mut self,
        start: u32,
        is_statement: IsStatement,
        is_abstract: bool,
        declare: bool,
    ) -> PResult<Class> {
        let old_in_abstract_class = self.tss().in_abstract_class;
        self.tsm().in_abstract_class = is_abstract;
        let r = self.parse_class_ts_inner(start, is_statement, is_abstract, declare);
        self.tsm().in_abstract_class = old_in_abstract_class;
        r
    }

    fn parse_class_ts_inner(&mut self, mut start: u32, is_statement: IsStatement, is_abstract: bool, declare: bool) -> PResult<Class> {
        self.next()?;
        let mut extra = TsClassExtra { is_abstract, declare, ..TsClassExtra::default() };
        // takeDecorators(node)
        let decorators = std::mem::take(self.tsm().decorator_stack.last_mut().unwrap());
        if !decorators.is_empty() {
            start = decorators[0].start;
            extra.decorators = Some(decorators);
        }
        let old_strict = self.strict;
        self.strict = true;
        // parseClassId(node, isStatement)
        let id = if is_statement == IsStatement::No && self.is_contextual("implements") {
            None
        } else {
            let id = self.parse_class_id(is_statement)?;
            extra.type_parameters = self.ts_try_parse_type_parameters(TypeParamModifiers::Class)?;
            id
        };
        // parseClassSuper(node)
        let super_class = if self.eat(T::Extends)? { Some(Box::new(self.parse_expr_subscripts(None, ForInit::No)?)) } else { None };
        if super_class.is_some() && (self.ts_match_left_relational() || self.type_ == T::BitShift) {
            extra.super_type_parameters = self.ts_parse_type_arguments_in_expression()?;
        }
        if self.eat_contextual("implements")? {
            extra.implements = Some(self.ts_parse_heritage_clause("implements")?);
        }
        self.enter_class_body();
        let class_body_start = self.start;
        let mut had_constructor = false;
        let mut body = Vec::new();
        let mut decorators: Vec<Decorator> = Vec::new();
        self.expect(T::BraceL)?;
        while self.type_ != T::BraceR {
            if self.type_ == T::At {
                decorators.push(self.parse_decorator()?);
                continue;
            }
            let element = self.parse_class_element_ts(super_class.is_some())?;
            let Some(mut element) = element else {
                if !decorators.is_empty() {
                    // `element.decorators = ...` on `null` throws a TypeError in the plugin
                    return self.unexpected(None);
                }
                continue;
            };
            let mut has_decorators = false;
            if !decorators.is_empty() {
                let first = decorators[0].start;
                let decs = std::mem::take(&mut decorators);
                has_decorators = true;
                match &mut element {
                    ClassElement::MethodDefinition(m) => {
                        m.start = first;
                        m.ts.get_or_insert_with(Box::default).decorators = Some(decs);
                    }
                    ClassElement::PropertyDefinition(p) => {
                        p.start = first;
                        p.ts.get_or_insert_with(Box::default).decorators = Some(decs);
                    }
                    ClassElement::TSIndexSignature(s) => {
                        s.start = first;
                        s.decorators = Some(decs);
                    }
                    ClassElement::StaticBlock(s) => s.start = first,
                }
            }
            match &element {
                ClassElement::MethodDefinition(m)
                    if m.kind == MethodKind::Constructor && matches!(m.value, FunctionNode::FunctionExpression(_)) =>
                {
                    if had_constructor {
                        return self.raise_recoverable(m.start, "Duplicate constructor in the same class");
                    }
                    had_constructor = true;
                    if has_decorators {
                        return self.raise(m.start, err::DECORATOR_CONSTRUCTOR);
                    }
                }
                ClassElement::MethodDefinition(MethodDefinition { key: PropertyKey::Private(p), value, .. })
                    if !matches!(value, FunctionNode::TSDeclareMethod(_)) =>
                {
                    let (name, key_start) = (p_name(p), p_start(p));
                    if self.is_private_name_conflicted_pub(&element) {
                        return self.raise_recoverable(key_start, format!("Identifier '#{}' has already been declared", name));
                    }
                }
                ClassElement::PropertyDefinition(PropertyDefinition { key: PropertyKey::Private(p), .. }) => {
                    let (name, key_start) = (p_name(p), p_start(p));
                    if self.is_private_name_conflicted_pub(&element) {
                        return self.raise_recoverable(key_start, format!("Identifier '#{}' has already been declared", name));
                    }
                }
                _ => {}
            }
            body.push(element);
        }
        self.strict = old_strict;
        self.next()?;
        if !decorators.is_empty() {
            return self.raise(self.start, err::TRAILING_DECORATOR);
        }
        let class_body = ClassBody { start: class_body_start, end: self.last_tok_end, body };
        self.exit_class_body()?;
        Ok(Class {
            start,
            end: self.last_tok_end,
            id,
            super_class,
            body: class_body,
            ts: if extra == TsClassExtra::default() { None } else { Some(Box::new(extra)) },
        })
    }

    /// `parsePostMemberNameModifiers(methodOrProp)`: returns `optional`.
    fn parse_post_member_name_modifiers(&mut self, modified: &Modified, start: u32) -> PResult<bool> {
        let optional = self.eat(T::Question)?;
        if modified.readonly && self.type_ == T::ParenL {
            return self.raise(start, err::CLASS_METHOD_HAS_READONLY);
        }
        if modified.declare && self.type_ == T::ParenL {
            return self.raise(start, err::CLASS_METHOD_HAS_DECLARE);
        }
        Ok(optional)
    }

    /// `parseClassElement(constructorAllowsSuper)`
    fn parse_class_element_ts(&mut self, constructor_allows_super: bool) -> PResult<Option<ClassElement>> {
        if self.eat(T::Semi)? {
            return Ok(None);
        }
        let start = self.start;
        const MODIFIERS: [&str; 9] =
            ["declare", "private", "public", "protected", "accessor", "override", "abstract", "readonly", "static"];
        let mut node = Modified::default();
        let modifier_map = self.ts_parse_modifiers(&mut node, &MODIFIERS, &["in", "out"], true, ModifierError::OnTypeParameterPositions)?;
        let is_static = modifier_map.is_static;
        let body = |p: &mut Self| p.parse_class_member_with_is_static(start, node, is_static, constructor_allows_super);
        if node.declare { self.ts_in_ambient_context(body) } else { body(self) }
    }

    /// `callParseClassMemberWithIsStatic` of `parseClassElement`.
    fn parse_class_member_with_is_static(
        &mut self,
        start: u32,
        node: Modified,
        is_static: bool,
        constructor_allows_super: bool,
    ) -> PResult<Option<ClassElement>> {
        const MODIFIERS: [&str; 9] =
            ["declare", "private", "public", "protected", "accessor", "override", "abstract", "readonly", "static"];
        if self.ts_is_start_of_static_blocks() {
            self.next()?;
            self.next()?;
            if node.has_some(&MODIFIERS) {
                return self.raise(self.start, err::STATIC_BLOCK_CANNOT_HAVE_MODIFIER);
            }
            if self.options.ecma_version >= 13 {
                return Ok(Some(ClassElement::StaticBlock(self.parse_class_static_block(start)?)));
            }
            return self.unexpected(None);
        }
        if let Some(mut idx) = self.ts_try_parse_index_signature(start)? {
            if node.is_abstract {
                return self.raise(start, err::INDEX_SIGNATURE_HAS_ABSTRACT);
            }
            if let Some(m) = node.accessibility {
                return self.raise(start, err::INDEX_SIGNATURE_HAS_ACCESSIBILITY(m));
            }
            if node.declare {
                return self.raise(start, err::INDEX_SIGNATURE_HAS_DECLARE);
            }
            if node.is_override {
                return self.raise(start, err::INDEX_SIGNATURE_HAS_OVERRIDE);
            }
            idx.readonly = node.readonly;
            idx.is_static = node.is_static;
            return Ok(Some(ClassElement::TSIndexSignature(idx)));
        }
        if !self.tss().in_abstract_class && node.is_abstract {
            return self.raise(start, err::NON_ABSTRACT_CLASS_HAS_ABSTRACT_METHOD);
        }
        if node.is_override && !constructor_allows_super {
            return self.raise(start, err::OVERRIDE_NOT_IN_SUB_CLASS);
        }
        let ecma_version = self.options.ecma_version;
        let mut key_name = "";
        let mut is_generator = false;
        let mut is_async = false;
        let mut kind = MethodKind::Method;
        if is_static && !(self.is_class_element_name_start() || self.type_ == T::Star) {
            key_name = "static";
        }
        if key_name.is_empty() && ecma_version >= 8 && self.eat_contextual("async")? {
            if (self.is_class_element_name_start() || self.type_ == T::Star) && !self.can_insert_semicolon() {
                is_async = true;
            } else {
                key_name = "async";
            }
        }
        if key_name.is_empty() && (ecma_version >= 9 || !is_async) && self.eat(T::Star)? {
            is_generator = true;
        }
        if key_name.is_empty() && !is_async && !is_generator {
            let last_value = match &self.value {
                TokVal::Str(s) if self.type_ == T::Name => s.clone(),
                _ => String::new(),
            };
            if self.eat_contextual("get")? || self.eat_contextual("set")? {
                if self.is_class_element_name_start() {
                    kind = if last_value == "get" { MethodKind::Get } else { MethodKind::Set };
                } else {
                    key_name = if last_value == "get" { "get" } else { "set" };
                }
            }
        }
        let (computed, key) = if !key_name.is_empty() {
            (
                false,
                PropertyKey::Expression(Box::new(Expression::Identifier(Identifier {
                    start: self.last_tok_start,
                    end: self.last_tok_end,
                    name: key_name.to_string(),
                    ts: None,
                }))),
            )
        } else {
            self.parse_class_element_name()?
        };
        let optional = self.parse_post_member_name_modifiers(&node, start)?;
        let mut extra = TsClassMemberExtra {
            accessibility: node.accessibility,
            declare: node.declare,
            is_abstract: node.is_abstract,
            readonly: node.readonly,
            is_override: node.is_override,
            accessor: node.accessor,
            optional,
            ..TsClassMemberExtra::default()
        };
        if self.ts_match_left_relational()
            || self.type_ == T::Relational
            || ecma_version < 13
            || self.type_ == T::ParenL
            || kind != MethodKind::Method
            || is_generator
            || is_async
        {
            let is_constructor = !is_static && key_is(computed, &key, "constructor");
            let allows_direct_super = is_constructor && constructor_allows_super;
            if is_constructor && kind != MethodKind::Method {
                return self.raise(crate::acorn::lval::property_key_start(&key), "Constructor can't have get/set modifier");
            }
            let kind = if is_constructor { MethodKind::Constructor } else { kind };
            let m = self.parse_class_method_ts(start, is_static, computed, key, kind, is_generator, is_async, allows_direct_super, extra)?;
            Ok(Some(ClassElement::MethodDefinition(m)))
        } else {
            // parseClassField(node)
            let is_private = matches!(key, PropertyKey::Private(_));
            if is_private {
                if node.is_abstract {
                    return self.raise(start, err::PRIVATE_ELEMENT_HAS_ABSTRACT);
                }
                if let Some(m) = node.accessibility {
                    return self.raise(start, err::PRIVATE_ELEMENT_HAS_ACCESSIBILITY(m));
                }
                self.parse_class_property_annotation(&mut extra)?;
            } else {
                self.parse_class_property_annotation(&mut extra)?;
                if self.is_ambient_context() && !(extra.readonly && extra.type_annotation.is_none()) && self.type_ == T::Eq {
                    return self.raise(self.start, err::DECLARE_CLASS_FIELD_HAS_INITIALIZER);
                }
                if extra.is_abstract && self.type_ == T::Eq {
                    let property_name = match &key {
                        PropertyKey::Expression(e) if !computed && matches!(&**e, Expression::Identifier(_)) => {
                            let Expression::Identifier(i) = &**e else { unreachable!() };
                            i.name.clone()
                        }
                        _ => {
                            let s = match &key {
                                PropertyKey::Expression(e) => e.span(),
                                PropertyKey::Private(PrivateIdentifierNode::PrivateIdentifier(p)) => p.span(),
                            };
                            format!("[{}]", &self.input[s.start as usize..s.end as usize])
                        }
                    };
                    return self.raise(self.start, err::ABSTRACT_PROPERTY_HAS_INITIALIZER(&property_name));
                }
            }
            let mut field = self.parse_class_field(start, is_static, computed, key)?;
            field.ts = Some(Box::new(extra));
            Ok(Some(ClassElement::PropertyDefinition(field)))
        }
    }

    /// `parseClassPropertyAnnotation(node)`
    fn parse_class_property_annotation(&mut self, node: &mut TsClassMemberExtra) -> PResult<()> {
        if !node.optional {
            if self.type_ == T::Prefix && self.tok_text() == "!" {
                self.next()?;
                node.definite = true;
            } else if self.eat(T::Question)? {
                node.optional = true;
            }
        }
        if let Some(type_) = self.ts_try_parse_type_annotation()? {
            node.type_annotation = Some(type_);
        }
        Ok(())
    }

    /// `parseClassMethod(method, isGenerator, isAsync, allowsDirectSuper)`
    #[allow(clippy::too_many_arguments)]
    fn parse_class_method_ts(
        &mut self,
        start: u32,
        is_static: bool,
        computed: bool,
        key: PropertyKey,
        kind: MethodKind,
        is_generator: bool,
        is_async: bool,
        allows_direct_super: bool,
        mut extra: TsClassMemberExtra,
    ) -> PResult<MethodDefinition> {
        let is_constructor = kind == MethodKind::Constructor;
        let is_private = matches!(key, PropertyKey::Private(_));
        let type_parameters = self.ts_try_parse_type_parameters(TypeParamModifiers::Const)?;
        if is_private {
            if let Some(m) = extra.accessibility {
                return self.raise(start, err::PRIVATE_METHODS_HAS_ACCESSIBILITY(m));
            }
        } else if let Some(tp) = &type_parameters {
            if is_constructor {
                return self.raise(tp.start, err::CONSTRUCTOR_HAS_TYPE_PARAMETERS);
            }
        }
        if extra.declare && (kind == MethodKind::Get || kind == MethodKind::Set) {
            return self.raise(start, err::DECLARE_ACCESSOR(if kind == MethodKind::Get { "get" } else { "set" }));
        }
        if type_parameters.is_some() {
            extra.type_parameters = type_parameters;
        }
        let key_start = crate::acorn::lval::property_key_start(&key);
        if kind == MethodKind::Constructor {
            if is_generator {
                return self.raise(key_start, "Constructor can't be a generator");
            }
            if is_async {
                return self.raise(key_start, "Constructor can't be an async method");
            }
        } else if is_static && key_is(computed, &key, "prototype") {
            return self.raise(key_start, "Classes may not have a static property named prototype");
        }
        let abstract_method = if extra.is_abstract {
            let name = match &key {
                PropertyKey::Expression(e) if !computed && matches!(&**e, Expression::Identifier(_)) => {
                    let Expression::Identifier(i) = &**e else { unreachable!() };
                    i.name.clone()
                }
                _ => {
                    let s = match &key {
                        PropertyKey::Expression(e) => e.span(),
                        PropertyKey::Private(PrivateIdentifierNode::PrivateIdentifier(p)) => p.span(),
                    };
                    format!("[{}]", &self.input[s.start as usize..s.end as usize])
                }
            };
            Some((start, name))
        } else {
            None
        };
        let value = self.parse_method_ts(is_generator, is_async, allows_direct_super, true, abstract_method)?;
        let (value_start, params) = match &value {
            FunctionNode::FunctionExpression(f) => (f.start, &f.params),
            FunctionNode::TSDeclareMethod(f) => (f.start, &f.params),
        };
        if kind == MethodKind::Get && !params.is_empty() {
            return self.raise_recoverable(value_start, "getter should have no params");
        }
        if kind == MethodKind::Set && params.len() != 1 {
            return self.raise_recoverable(value_start, "setter should have exactly one param");
        }
        if kind == MethodKind::Set {
            if let Pattern::RestElement(r) = &params[0] {
                return self.raise_recoverable(r.start, "Setter cannot use rest params");
            }
        }
        Ok(MethodDefinition {
            start,
            end: self.last_tok_end,
            is_static,
            computed,
            key,
            kind,
            value,
            ts: if extra == TsClassMemberExtra::default() { None } else { Some(Box::new(extra)) },
        })
    }

    // --- decorators

    /// `parseDecorators(allowExport)`
    pub(crate) fn parse_decorators(&mut self, allow_export: bool) -> PResult<()> {
        while self.type_ == T::At {
            let decorator = self.parse_decorator()?;
            self.tsm().decorator_stack.last_mut().unwrap().push(decorator);
        }
        if self.type_ == T::Export {
            if !allow_export {
                return self.unexpected(None);
            }
        } else if !self.can_have_leading_decorator()? {
            return self.raise(self.start, err::UNEXPECTED_LEADING_DECORATOR);
        }
        Ok(())
    }

    /// `parseDecorator()`
    pub(crate) fn parse_decorator(&mut self) -> PResult<Decorator> {
        let start = self.start;
        self.next()?;
        self.tsm().decorator_stack.push(Vec::new());
        let start_pos = self.start;
        let mut expr;
        if self.type_ == T::ParenL {
            let start_pos2 = self.start;
            self.next()?;
            expr = self.parse_expression(ForInit::No, None)?;
            self.expect(T::ParenR)?;
            if self.options.preserve_parens {
                expr = Expression::ParenthesizedExpression(ParenthesizedExpression {
                    start: start_pos2,
                    end: self.last_tok_end,
                    expression: Box::new(expr),
                });
            }
        } else {
            expr = Expression::Identifier(self.parse_ident(false)?);
            while self.eat(T::Dot)? {
                let property = self.parse_ident(true)?;
                // SVELTERS: the plugin's member expression has no `optional` key.
                expr = Expression::MemberExpression(MemberExpression {
                    start: start_pos,
                    end: self.last_tok_end,
                    object: Box::new(expr),
                    property: PropertyKey::Expression(Box::new(Expression::Identifier(property))),
                    computed: false,
                    optional: false,
                });
            }
        }
        let expression = self.parse_maybe_decorator_arguments(expr)?;
        self.tsm().decorator_stack.pop();
        Ok(Decorator { start, end: self.last_tok_end, expression })
    }

    /// `parseMaybeDecoratorArguments(expr)`
    fn parse_maybe_decorator_arguments(&mut self, expr: Expression) -> PResult<Expression> {
        let type_arguments = if self.ts_match_left_relational() || self.type_ == T::BitShift {
            self.ts_parse_type_arguments_in_expression()?
        } else {
            None
        };
        let start = expr.span().start;
        if self.eat(T::ParenL)? {
            let arguments = self.parse_expr_list(T::ParenR, false, false, None)?.into_iter().flatten().collect();
            return Ok(Expression::CallExpression(CallExpression {
                start,
                end: self.last_tok_end,
                callee: Box::new(expr),
                arguments,
                optional: false,
                ts: Some(Box::new(TsTypeArgumentsExtra { type_arguments, omit_optional: true })),
            }));
        }
        if let Some(type_arguments) = type_arguments {
            return Ok(Expression::TSInstantiationExpression(TSInstantiationExpression {
                start,
                end: self.last_tok_end,
                expression: Box::new(expr),
                type_arguments,
            }));
        }
        Ok(expr)
    }

    // --- modules

    /// `parseMaybeImportAttributes(node)`: `Some(attributes)` after `with` / `assert`.
    fn parse_maybe_import_attributes(&mut self) -> PResult<Option<Vec<ImportAttribute>>> {
        if self.type_ == T::With || self.type_ == T::TsAssert {
            self.next()?;
            // parseImportAttributes()
            self.expect(T::BraceL)?;
            let attrs = self.parse_with_entries()?;
            self.expect(T::BraceR)?;
            return Ok(Some(attrs));
        }
        Ok(None)
    }

    /// `parseWithEntries()`
    fn parse_with_entries(&mut self) -> PResult<Vec<ImportAttribute>> {
        let mut attrs = Vec::new();
        let mut attr_names: HashSet<Option<String>> = HashSet::new();
        loop {
            if self.type_ == T::BraceR {
                break;
            }
            let start = self.start;
            let withion_key_node = if self.type_ == T::String {
                ModuleExportName::Literal(self.parse_literal_token()?)
            } else {
                ModuleExportName::Identifier(self.parse_ident(true)?)
            };
            self.next()?;
            // `node.key.name` (undefined for a string key)
            let name = match &withion_key_node {
                ModuleExportName::Identifier(i) => Some(i.name.clone()),
                ModuleExportName::Literal(_) => None,
            };
            if attr_names.contains(&name) {
                return self.raise(self.pos, "Duplicated key in attributes");
            }
            attr_names.insert(name);
            if self.type_ != T::String {
                return self.raise(self.pos, "Only string is supported as an attribute value");
            }
            let value = LiteralNode::Literal(self.parse_literal_token()?);
            attrs.push(ImportAttribute { start, end: self.last_tok_end, key: withion_key_node, value });
            if !self.eat(T::Comma)? {
                break;
            }
        }
        Ok(attrs)
    }

    /// `parseImport(node)`
    pub fn parse_import_ts(&mut self, start: u32) -> PResult<Statement> {
        let mut enter_head = self.lookahead()?;
        let mut import_kind = "value";
        self.tsm().import_or_export_outer_kind = Some("value");
        if token_is_identifier(enter_head.type_) || self.type_ == T::Star || self.type_ == T::BraceL {
            let mut ahead = self.lookahead_n(2)?;
            if ahead.type_ != T::Comma
                && !Self::is_contextual_with_state("from", &ahead)
                && ahead.type_ != T::Eq
                && self.ts_eat_contextual_with_state("type", 1, &enter_head)?
            {
                self.tsm().import_or_export_outer_kind = Some("type");
                import_kind = "type";
                enter_head = self.lookahead()?;
                ahead = self.lookahead_n(2)?;
            }
            if token_is_identifier(enter_head.type_) && ahead.type_ == T::Eq {
                self.next()?;
                let import_node = self.ts_parse_import_equals_declaration(start, import_kind, false)?;
                self.tsm().import_or_export_outer_kind = Some("value");
                return Ok(import_node);
            }
        }
        self.next()?;
        let specifiers;
        let source;
        if self.type_ == T::String {
            specifiers = Vec::new();
            source = self.parse_literal_token()?;
        } else {
            specifiers = self.parse_import_specifiers_ts()?;
            self.expect_contextual("from")?;
            if self.type_ != T::String {
                return self.unexpected(None);
            }
            source = self.parse_literal_token()?;
        }
        let attributes = self.parse_maybe_import_attributes()?;
        self.semicolon()?;
        self.tsm().import_or_export_outer_kind = Some("value");
        if import_kind == "type"
            && specifiers.len() > 1
            && matches!(specifiers[0], ImportSpecifierKind::ImportDefaultSpecifier(_))
        {
            return self.raise(start, err::TYPE_IMPORT_CANNOT_SPECIFY_DEFAULT_AND_NAMED);
        }
        let has_attributes = attributes.is_some();
        Ok(Statement::ImportDeclaration(ImportDeclaration {
            start,
            end: self.last_tok_end,
            specifiers,
            source: LiteralNode::Literal(source),
            attributes: attributes.unwrap_or_default(),
            ts: Some(Box::new(TsImportKindExtra { import_kind, has_attributes })),
        }))
    }

    /// `parseImportSpecifiers()`
    fn parse_import_specifiers_ts(&mut self) -> PResult<Vec<ImportSpecifierKind>> {
        let mut nodes = Vec::new();
        let mut first = true;
        if token_is_identifier(self.type_) {
            nodes.push(ImportSpecifierKind::ImportDefaultSpecifier(self.parse_import_default_specifier()?));
            if !self.eat(T::Comma)? {
                return Ok(nodes);
            }
        }
        if self.type_ == T::Star {
            nodes.push(ImportSpecifierKind::ImportNamespaceSpecifier(self.parse_import_namespace_specifier()?));
            return Ok(nodes);
        }
        self.expect(T::BraceL)?;
        while !self.eat(T::BraceR)? {
            if !first {
                self.expect(T::Comma)?;
                if self.after_trailing_comma(T::BraceR, false)? {
                    break;
                }
            } else {
                first = false;
            }
            nodes.push(ImportSpecifierKind::ImportSpecifier(self.parse_import_specifier_ts()?));
        }
        Ok(nodes)
    }

    /// `parseImportSpecifier()`
    fn parse_import_specifier_ts(&mut self) -> PResult<ImportSpecifier> {
        let is_maybe_type_only = self.ts_is_contextual(T::TsType);
        if is_maybe_type_only {
            let start = self.start;
            let imported = self.parse_module_export_name()?;
            let in_type_only = self.tss().import_or_export_outer_kind == Some("type");
            let (left, right, kind) = self.parse_type_only_import_export_specifier(imported, true, in_type_only)?;
            let local = match right {
                ModuleExportName::Identifier(i) => i,
                ModuleExportName::Literal(l) => return self.raise(l.start, "Binding rvalue"),
            };
            return Ok(ImportSpecifier {
                start,
                end: self.last_tok_end,
                imported: left,
                local,
                ts: Some(Box::new(TsImportKindExtra { import_kind: kind, has_attributes: false })),
            });
        }
        let mut node = self.parse_import_specifier()?;
        node.ts = Some(Box::new(TsImportKindExtra { import_kind: "value", has_attributes: false }));
        Ok(node)
    }

    /// `parseExportSpecifier(exports)`
    fn parse_export_specifier_ts(&mut self, exports: Option<&mut Exports>) -> PResult<ExportSpecifier> {
        let is_maybe_type_only = self.ts_is_contextual(T::TsType);
        let is_string = self.type_ == T::String;
        if !is_string && is_maybe_type_only {
            let start = self.start;
            let local = self.parse_module_export_name()?;
            let in_type_only = self.tss().import_or_export_outer_kind == Some("type");
            let (left, right, kind) = self.parse_type_only_import_export_specifier(local, false, in_type_only)?;
            let (name, pos) = module_name_of(&right);
            self.check_export_ts(exports, &name, pos);
            return Ok(ExportSpecifier {
                start,
                end: self.last_tok_end,
                local: left,
                exported: right,
                ts: Some(Box::new(TsExportKindExtra { export_kind: Some(kind), has_attributes: false })),
            });
        }
        let mut node = self.parse_export_specifier(exports)?;
        node.ts = Some(Box::new(TsExportKindExtra { export_kind: Some("value"), has_attributes: false }));
        Ok(node)
    }

    /// `parseTypeOnlyImportExportSpecifier(node, isImport, isInTypeOnlyImportExport)`:
    /// `left` is `node[leftOfAsKey]` (imported / local); returns (left, right, kind).
    fn parse_type_only_import_export_specifier(
        &mut self,
        left: ModuleExportName,
        is_import: bool,
        is_in_type_only_import_export: bool,
    ) -> PResult<(ModuleExportName, ModuleExportName, &'static str)> {
        let mut left_of_as = left;
        let mut right_of_as: Option<ModuleExportName> = None;
        let mut has_type_specifier = false;
        let mut can_parse_as_keyword = true;
        let loc = match &left_of_as {
            ModuleExportName::Identifier(i) => i.start,
            ModuleExportName::Literal(l) => l.start,
        };
        let parse_right = |p: &mut Self| -> PResult<ModuleExportName> {
            if is_import { Ok(ModuleExportName::Identifier(p.parse_ident(false)?)) } else { p.parse_module_export_name() }
        };
        if self.is_contextual("as") {
            let first_as = self.parse_ident(false)?;
            if self.is_contextual("as") {
                let second_as = self.parse_ident(false)?;
                if token_is_keyword_or_identifier(self.type_) {
                    has_type_specifier = true;
                    left_of_as = ModuleExportName::Identifier(first_as);
                    right_of_as = Some(parse_right(self)?);
                    can_parse_as_keyword = false;
                } else {
                    right_of_as = Some(ModuleExportName::Identifier(second_as));
                    can_parse_as_keyword = false;
                }
            } else if token_is_keyword_or_identifier(self.type_) {
                can_parse_as_keyword = false;
                right_of_as = Some(parse_right(self)?);
            } else {
                has_type_specifier = true;
                left_of_as = ModuleExportName::Identifier(first_as);
            }
        } else if token_is_keyword_or_identifier(self.type_) {
            has_type_specifier = true;
            if is_import {
                let l = self.parse_ident(true)?;
                if !self.is_contextual("as") {
                    self.check_unreserved(l.start, l.end, &l.name)?;
                }
                left_of_as = ModuleExportName::Identifier(l);
            } else {
                left_of_as = self.parse_module_export_name()?;
            }
        }
        if has_type_specifier && is_in_type_only_import_export {
            return self.raise(
                loc,
                if is_import { err::TYPE_MODIFIER_IS_USED_IN_TYPE_IMPORTS } else { err::TYPE_MODIFIER_IS_USED_IN_TYPE_EXPORTS },
            );
        }
        let kind = if has_type_specifier { "type" } else { "value" };
        if can_parse_as_keyword && self.eat_contextual("as")? {
            right_of_as = Some(parse_right(self)?);
        }
        let right = match right_of_as {
            Some(r) => r,
            None => left_of_as.clone(),
        };
        if is_import {
            match &right {
                ModuleExportName::Identifier(i) => {
                    let i = i.clone();
                    self.check_lval_identifier(&i, BIND_LEXICAL, None)?;
                }
                ModuleExportName::Literal(l) => return self.raise(l.start, "Binding rvalue"),
            }
        }
        Ok((left_of_as, right, kind))
    }

    /// `checkExport(exports, name, _)`: the plugin records the name without the duplicate check.
    pub(crate) fn check_export_ts(&self, exports: Option<&mut Exports>, name: &str, _pos: u32) {
        if let Some(exports) = exports {
            exports.insert(name.to_string());
        }
    }

    /// `parseExport(node, exports)`
    pub fn parse_export_ts(&mut self, start: u32, mut exports: Option<&mut Exports>) -> PResult<Statement> {
        let enter_head = self.lookahead()?;
        if self.ts_eat_with_state(T::Import, 2, &enter_head)? {
            let import_kind;
            if self.ts_is_contextual(T::TsType) && self.lookahead_char_code() != 61 {
                import_kind = "type";
                self.tsm().import_or_export_outer_kind = Some("type");
                self.next()?;
            } else {
                import_kind = "value";
                self.tsm().import_or_export_outer_kind = Some("value");
            }
            let export_equals_node = self.ts_parse_import_equals_declaration(start, import_kind, /* isExport */ true)?;
            self.tsm().import_or_export_outer_kind = None;
            return Ok(export_equals_node);
        } else if self.ts_eat_with_state(T::Eq, 2, &enter_head)? {
            let expression = self.parse_expression(ForInit::No, None)?;
            self.semicolon()?;
            self.tsm().import_or_export_outer_kind = None;
            return Ok(Statement::TSExportAssignment(TSExportAssignment { start, end: self.last_tok_end, expression }));
        } else if self.ts_eat_contextual_with_state("as", 2, &enter_head)? {
            self.expect_contextual("namespace")?;
            let id = self.parse_ident(false)?;
            self.semicolon()?;
            self.tsm().import_or_export_outer_kind = None;
            return Ok(Statement::TSNamespaceExportDeclaration(TSNamespaceExportDeclaration { start, end: self.last_tok_end, id }));
        }
        let lookahead2 = self.lookahead_n(2)?.type_;
        let mut export_kind;
        if Self::ts_is_contextual_with_state(&enter_head, T::TsType) && (lookahead2 == T::BraceL || lookahead2 == T::Star) {
            self.next()?;
            self.tsm().import_or_export_outer_kind = Some("type");
            export_kind = "type";
        } else {
            self.tsm().import_or_export_outer_kind = Some("value");
            export_kind = "value";
        }
        self.next()?;
        if self.eat(T::Star)? {
            return self.parse_export_all_declaration_ts(start, exports, export_kind);
        }
        if self.eat(T::Default)? {
            self.check_export_ts(exports, "default", self.last_tok_start);
            let declaration = self.parse_export_default_declaration()?;
            return Ok(Statement::ExportDefaultDeclaration(ExportDefaultDeclaration {
                start,
                end: self.last_tok_end,
                declaration,
                ts: Some(Box::new(TsExportKindExtra { export_kind: Some(export_kind), has_attributes: false })),
            }));
        }
        let declaration;
        let specifiers;
        let mut source = None;
        let mut attributes = None;
        if self.should_parse_export_statement() {
            let (decl, type_kind) = self.parse_export_declaration_ts()?;
            if type_kind {
                export_kind = "type";
            }
            match &decl {
                Statement::VariableDeclaration(v) => {
                    for d in &v.declarations {
                        self.check_pattern_export_ts(exports.as_deref_mut(), &d.id);
                    }
                }
                other => {
                    if let Some((name, pos)) = declaration_id(other) {
                        self.check_export_ts(exports.as_deref_mut(), &name, pos);
                    }
                }
            }
            declaration = Some(Box::new(decl));
            specifiers = Vec::new();
        } else {
            declaration = None;
            specifiers = self.parse_export_specifiers_ts(exports.as_deref_mut())?;
            if self.eat_contextual("from")? {
                if self.type_ != T::String {
                    return self.unexpected(None);
                }
                source = Some(LiteralNode::Literal(self.parse_literal_token()?));
                attributes = self.parse_maybe_import_attributes()?;
            } else {
                for spec in &specifiers {
                    match &spec.local {
                        ModuleExportName::Identifier(local) => {
                            self.check_unreserved(local.start, local.end, &local.name)?;
                            self.check_local_export(&local.name, local.start);
                        }
                        ModuleExportName::Literal(l) => {
                            return self.raise(l.start, "A string literal cannot be used as an exported binding without `from`.");
                        }
                    }
                }
            }
            self.semicolon()?;
        }
        let has_attributes = attributes.is_some();
        Ok(Statement::ExportNamedDeclaration(ExportNamedDeclaration {
            start,
            end: self.last_tok_end,
            declaration,
            specifiers,
            source,
            attributes: attributes.unwrap_or_default(),
            ts: Some(Box::new(TsExportKindExtra { export_kind: Some(export_kind), has_attributes })),
        }))
    }

    fn check_pattern_export_ts(&self, mut exports: Option<&mut Exports>, pat: &Pattern) {
        match pat {
            Pattern::Identifier(i) => self.check_export_ts(exports, &i.name, i.start),
            Pattern::ObjectPattern(o) => {
                for prop in &o.properties {
                    match prop {
                        ObjectPatternMember::Property(p) => self.check_pattern_export_ts(exports.as_deref_mut(), &p.value),
                        ObjectPatternMember::RestElement(r) => self.check_pattern_export_ts(exports.as_deref_mut(), &r.argument),
                    }
                }
            }
            Pattern::ArrayPattern(a) => {
                for elt in a.elements.iter().flatten() {
                    self.check_pattern_export_ts(exports.as_deref_mut(), elt);
                }
            }
            Pattern::AssignmentPattern(a) => self.check_pattern_export_ts(exports, &a.left),
            Pattern::RestElement(r) => self.check_pattern_export_ts(exports, &r.argument),
            _ => {}
        }
    }

    fn parse_export_specifiers_ts(&mut self, mut exports: Option<&mut Exports>) -> PResult<Vec<ExportSpecifier>> {
        let mut nodes = Vec::new();
        let mut first = true;
        self.expect(T::BraceL)?;
        while !self.eat(T::BraceR)? {
            if !first {
                self.expect(T::Comma)?;
                if self.after_trailing_comma(T::BraceR, false)? {
                    break;
                }
            } else {
                first = false;
            }
            nodes.push(self.parse_export_specifier_ts(exports.as_deref_mut())?);
        }
        Ok(nodes)
    }

    /// `parseExportAllDeclaration(node, exports)`
    fn parse_export_all_declaration_ts(
        &mut self,
        start: u32,
        exports: Option<&mut Exports>,
        export_kind: &'static str,
    ) -> PResult<Statement> {
        let mut exported = None;
        if self.options.ecma_version >= 11 {
            if self.eat_contextual("as")? {
                let e = self.parse_module_export_name()?;
                let (name, _) = module_name_of(&e);
                self.check_export_ts(exports, &name, self.last_tok_start);
                exported = Some(e);
            }
        }
        self.expect_contextual("from")?;
        if self.type_ != T::String {
            return self.unexpected(None);
        }
        let source = self.parse_literal_token()?;
        let attributes = self.parse_maybe_import_attributes()?;
        self.semicolon()?;
        let has_attributes = attributes.is_some();
        Ok(Statement::ExportAllDeclaration(ExportAllDeclaration {
            start,
            end: self.last_tok_end,
            exported,
            source: LiteralNode::Literal(source),
            attributes: attributes.unwrap_or_default(),
            ts: Some(Box::new(TsExportKindExtra { export_kind: Some(export_kind), has_attributes })),
        }))
    }

    /// The `parseExportDefaultDeclaration` override (before acorn's).
    pub(crate) fn parse_export_default_declaration_ts_prefix(&mut self) -> PResult<Option<ExportDefaultKind>> {
        if self.is_abstract_class()? {
            let cls_start = self.start;
            self.next()?;
            let c = self.parse_class_ts(cls_start, IsStatement::Yes, true, false)?;
            return Ok(Some(ExportDefaultKind::Declaration(Box::new(Statement::ClassDeclaration(c)))));
        }
        if self.type_ == T::TsInterface {
            let start = self.start;
            if let Some(result) = self.ts_parse_interface_declaration(start, false, false)? {
                return Ok(Some(ExportDefaultKind::Declaration(Box::new(result))));
            }
        }
        Ok(None)
    }

    /// `shouldParseExportStatement()` (the TypeScript part)
    pub(crate) fn should_parse_export_statement_ts(&self) -> bool {
        token_is_ts_declaration_start(self.type_) || self.type_ == T::At
    }

    /// `parseExportDeclaration(node)`: the declaration, and whether it makes the export a type
    /// export (`node.exportKind = "type"`).
    fn parse_export_declaration_ts(&mut self) -> PResult<(Statement, bool)> {
        if !self.is_ambient_context() && self.ts_is_contextual(T::TsDeclare) {
            return self.ts_in_ambient_context(|p| p.parse_export_declaration_ts());
        }
        let start_pos = self.start;
        let is_declare = self.eat_contextual("declare")?;
        if is_declare && (self.ts_is_contextual(T::TsDeclare) || !self.should_parse_export_statement()) {
            return self.raise(self.start, err::EXPECTED_AMBIENT_AFTER_EXPORT_DECLARE);
        }
        let is_identifier = token_is_identifier(self.type_);
        let declaration = if is_identifier {
            // tsTryParseExportDeclaration()
            let s = self.start;
            let value = self.value_str().to_string();
            self.ts_parse_declaration(s, &value, true)?
        } else {
            None
        };
        let mut declaration = match declaration {
            Some(d) => d,
            None => self.parse_statement(None, false, None)?,
        };
        let type_kind = matches!(declaration, Statement::TSInterfaceDeclaration(_) | Statement::TSTypeAliasDeclaration(_)) || is_declare;
        if is_declare {
            set_statement_start(&mut declaration, start_pos);
            set_declare(&mut declaration);
        }
        Ok((declaration, type_kind))
    }

    /// `isClassElementNameStart()` (the TypeScript part)
    pub(crate) fn is_class_element_name_start_ts(&self) -> bool {
        self.ts_is_identifier()
    }

    /// Re-export of acorn's private-name bookkeeping for `parseClass`.
    fn is_private_name_conflicted_pub(&mut self, element: &ClassElement) -> bool {
        self.is_private_name_conflicted(element)
    }

    /// Unused by the parser itself; kept for parity with the plugin's helpers.
    #[allow(dead_code)]
    fn ts_lookahead_is(&self, state: &LookaheadState, t: T) -> bool {
        state.type_ == t
    }
}

fn p_name(p: &PrivateIdentifierNode) -> String {
    let PrivateIdentifierNode::PrivateIdentifier(p) = p;
    p.name.clone()
}

fn p_start(p: &PrivateIdentifierNode) -> u32 {
    let PrivateIdentifierNode::PrivateIdentifier(p) = p;
    p.start
}

/// `node.declaration.id` of an export declaration (name, start).
fn declaration_id(s: &Statement) -> Option<(String, u32)> {
    match s {
        Statement::FunctionDeclaration(Function { id: Some(id), .. })
        | Statement::ClassDeclaration(Class { id: Some(id), .. }) => Some((id.name.clone(), id.start)),
        Statement::TSDeclareFunction(TSDeclareFunction { id: Some(id), .. }) => Some((id.name.clone(), id.start)),
        Statement::TSInterfaceDeclaration(TSInterfaceDeclaration { id: Some(id), .. }) => Some((id.name.clone(), id.start)),
        Statement::TSTypeAliasDeclaration(n) => Some((n.id.name.clone(), n.id.start)),
        Statement::TSEnumDeclaration(n) => Some((n.id.name.clone(), n.id.start)),
        Statement::TSModuleDeclaration(n) => Some(module_name_of(&n.id)),
        _ => None,
    }
}
