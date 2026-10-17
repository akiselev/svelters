//! Port of acorn 8.19.0 `src/statement.js`.
//!
//! SVELTERS: statement `context` strings (`"if"`, `"label"`, `"iflabel"`, ...) are
//! `&'static str`; the `exports` object is a `HashSet` passed down from the top level.

use std::collections::{HashMap, HashSet};

use super::expression::ForInit;
use super::identifier::{is_identifier_char, is_identifier_start, is_keyword_relational_operator};
use super::lval::property_key_start;
use super::options::AllowReserved;
use super::parseutil::DestructuringErrors;
use super::scopeflags::*;
use super::state::{Label, LabelKind, PResult, Parser, PrivateNameEntry, TokVal};
use super::tokentype::TokenType as T;
use super::whitespace::{has_line_break, skip_white_space};
use crate::estree::*;

pub type Exports = HashSet<String>;

/// `parseClass`'s `isStatement`: `false`, `true` or `"nullableID"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IsStatement {
    No,
    Yes,
    NullableId,
}

const FUNC_STATEMENT: u32 = 1;
const FUNC_HANGING_STATEMENT: u32 = 2;
const FUNC_NULLABLE_ID: u32 = 4;

/// `context + "label"`
fn add_label(context: &'static str) -> &'static str {
    match context {
        "if" => "iflabel",
        "do" => "dolabel",
        "for" => "forlabel",
        "while" => "whilelabel",
        "with" => "withlabel",
        other => other,
    }
}

fn var_kind_str(kind: VariableKind) -> &'static str {
    match kind {
        VariableKind::Var => "var",
        VariableKind::Let => "let",
        VariableKind::Const => "const",
        VariableKind::Using => "using",
        VariableKind::AwaitUsing => "await using",
    }
}

fn module_export_name_str(n: &ModuleExportName) -> String {
    match n {
        ModuleExportName::Identifier(i) => i.name.clone(),
        ModuleExportName::Literal(l) => match &l.value {
            LiteralValue::String(s) => s.clone(),
            _ => l.raw.clone(),
        },
    }
}

fn module_export_name_start(n: &ModuleExportName) -> u32 {
    match n {
        ModuleExportName::Identifier(i) => i.start,
        ModuleExportName::Literal(l) => l.start,
    }
}

impl<'a> Parser<'a> {
    // ### Statement parsing

    // Parse a program. Initializes the parser, reads any number of
    // statements, and wraps them in a Program node.  Optionally takes a
    // `program` argument.  If present, the statements will be appended
    // to its body instead of creating a new node.

    pub fn parse_top_level(&mut self, start: u32) -> PResult<Program> {
        let mut exports = Exports::new();
        let mut body = Vec::new();
        while self.type_ != T::Eof {
            let stmt = self.parse_statement(None, true, Some(&mut exports))?;
            body.push(stmt);
        }
        if self.in_module {
            if let Some((name, start)) = self.undefined_exports.first() {
                return self.raise_recoverable(start, format!("Export '{}' is not defined", name));
            }
        }
        self.adapt_directive_prologue(&mut body);
        self.next()?;
        let source_type = if self.options.source_type == super::options::AcornSourceType::Module {
            SourceType::Module
        } else {
            SourceType::Script
        };
        Ok(Program { start, end: self.last_tok_end, body, source_type })
    }

    pub fn is_let(&self, context: Option<&'static str>) -> bool {
        if self.options.ecma_version < 6 || !self.is_contextual("let") {
            return false;
        }
        let mut next = skip_white_space(self.input, self.pos as usize) as u32;
        let mut next_ch = self.code_at(next);
        // For ambiguous cases, determine if a LexicalDeclaration (or only a
        // Statement) is allowed here. If context is not empty then only a Statement
        // is allowed. However, `let [` is an explicit negative lookahead for
        // ExpressionStatement, so special-case it first.
        if next_ch == 91 || next_ch == 92 {
            return true; // '[', '\'
        }
        if context.is_some() {
            return false;
        }

        if next_ch == 123 {
            return true; // '{'
        }
        if is_identifier_start(next_ch, true) {
            let start = next;
            loop {
                next += self.char_len_at(next);
                next_ch = self.code_at(next);
                if !is_identifier_char(next_ch, true) {
                    break;
                }
            }
            if next_ch == 92 {
                return true;
            }
            let ident = &self.input[start as usize..next as usize];
            if !is_keyword_relational_operator(ident) {
                return true;
            }
        }
        false
    }

    // check 'async [no LineTerminator here] function'
    // - 'async /*foo*/ function' is OK.
    // - 'async /*\n*/ function' is invalid.
    pub fn is_async_function(&self) -> bool {
        if self.options.ecma_version < 8 || !self.is_contextual("async") {
            return false;
        }

        let next = skip_white_space(self.input, self.pos as usize);
        let after = next + 8;
        !has_line_break(&self.input[self.pos as usize..next])
            && self.input.get(next..after) == Some("function")
            && (after == self.input.len() || {
                let a = self.code_at(after as u32);
                !(is_identifier_char(a, true) || a == 92)
            })
    }

    pub fn is_using_keyword(&self, is_await_using: bool, is_for: bool) -> bool {
        if self.options.ecma_version < 17 || !self.is_contextual(if is_await_using { "await" } else { "using" }) {
            return false;
        }

        let mut next = skip_white_space(self.input, self.pos as usize);

        if has_line_break(&self.input[self.pos as usize..next]) {
            return false;
        }

        if is_await_using {
            let using_end_pos = next + 5; /* using */
            if self.input.get(next..using_end_pos) != Some("using") || using_end_pos == self.input.len() {
                return false;
            }
            let after = self.code_at(using_end_pos as u32);
            if is_identifier_char(after, true) || after == 92 {
                return false;
            }

            next = skip_white_space(self.input, using_end_pos);
            if has_line_break(&self.input[using_end_pos..next]) {
                return false;
            }
        }

        let mut ch = self.code_at(next as u32);
        if !is_identifier_start(ch, true) && ch != 92 {
            return false;
        }
        let id_start = next;
        loop {
            next += self.char_len_at(next as u32) as usize;
            ch = self.code_at(next as u32);
            if !is_identifier_char(ch, true) {
                break;
            }
        }
        if ch == 92 {
            return true;
        }
        let id = &self.input[id_start..next];
        if is_keyword_relational_operator(id) {
            return false;
        }
        if is_for && !is_await_using && id == "of" {
            // Look ahead for using declaration with initializer, i.e., `for (using of = ...)`
            next = skip_white_space(self.input, next);
            if self.code_at(next as u32) != 61 /* '=' */ || {
                // Check for ==, === and => operators
                let c = self.code_at(next as u32 + 1);
                c == 61 || c == 62
            } {
                return false;
            }
        }
        true
    }

    pub fn is_await_using(&self, is_for: bool) -> bool {
        self.is_using_keyword(true, is_for)
    }

    pub fn is_using(&self, is_for: bool) -> bool {
        self.is_using_keyword(false, is_for)
    }

    // Parse a single statement.
    //
    // If expecting a statement and finding a slash operator, parse a
    // regular expression literal. This is to handle cases like
    // `if (foo) /blah/.exec(foo)`, where looking at the previous token
    // does not help.

    pub fn parse_statement(
        &mut self,
        context: Option<&'static str>,
        top_level: bool,
        exports: Option<&mut Exports>,
    ) -> PResult<Statement> {
        self.enter_depth()?;
        let r = self.parse_statement_inner(context, top_level, exports);
        self.depth -= 1;
        let v = r?;
        // SVELTERS: Svelte's `<script>` hook (`this.undefinedExports = {}` after every statement).
        if self.options.allow_undefined_exports {
            self.undefined_exports.clear();
        }
        Ok(v)
    }

    fn parse_statement_inner(
        &mut self,
        context: Option<&'static str>,
        top_level: bool,
        exports: Option<&mut Exports>,
    ) -> PResult<Statement> {
        let mut starttype = self.type_;
        let start = self.start;
        let mut kind: Option<VariableKind> = None;

        if self.is_let(context) {
            starttype = T::Var;
            kind = Some(VariableKind::Let);
        }

        // Most types of statements are recognized by the keyword they
        // start with. Many are trivial to parse, some require a bit of
        // complexity.

        match starttype {
            T::Break | T::Continue => self.parse_break_continue_statement(start, starttype == T::Break),
            T::Debugger => self.parse_debugger_statement(start),
            T::Do => self.parse_do_statement(start),
            T::For => self.parse_for_statement(start),
            T::Function => {
                // Function as sole body of either an if statement or a labeled statement
                // works, but not when it is part of a labeled statement that is the sole
                // body of an if statement.
                if let Some(c) = context {
                    if (self.strict || c != "if" && c != "label") && self.options.ecma_version >= 6 {
                        return self.unexpected(None);
                    }
                }
                self.parse_function_statement(start, false, context.is_none())
            }
            T::Class => {
                if context.is_some() {
                    return self.unexpected(None);
                }
                Ok(Statement::ClassDeclaration(self.parse_class(start, IsStatement::Yes)?))
            }
            T::If => self.parse_if_statement(start),
            T::Return => self.parse_return_statement(start),
            T::Switch => self.parse_switch_statement(start),
            T::Throw => self.parse_throw_statement(start),
            T::Try => self.parse_try_statement(start),
            T::Const | T::Var => {
                let kind = kind.unwrap_or(if self.type_ == T::Const { VariableKind::Const } else { VariableKind::Var });
                if context.is_some() && kind != VariableKind::Var {
                    return self.unexpected(None);
                }
                self.parse_var_statement(start, kind, false)
            }
            T::While => self.parse_while_statement(start),
            T::With => self.parse_with_statement(start),
            T::BraceL => Ok(Statement::BlockStatement(self.parse_block(true, Some(start), false)?)),
            T::Semi => self.parse_empty_statement(start),
            T::Export | T::Import => {
                if self.options.ecma_version > 10 && starttype == T::Import {
                    let next = skip_white_space(self.input, self.pos as usize);
                    let next_ch = self.input.as_bytes().get(next).copied();
                    if next_ch == Some(b'(') || next_ch == Some(b'.') {
                        let expr = self.parse_expression(ForInit::No, None)?;
                        return self.parse_expression_statement(start, expr);
                    }
                }

                if !self.options.allow_import_export_everywhere {
                    if !top_level {
                        return self.raise(self.start, "'import' and 'export' may only appear at the top level");
                    }
                    if !self.in_module {
                        return self.raise(self.start, "'import' and 'export' may appear only with 'sourceType: module'");
                    }
                }
                if starttype == T::Import {
                    self.parse_import(start)
                } else {
                    self.parse_export(start, exports)
                }
            }

            // If the statement does not start with a statement keyword or a
            // brace, it's an ExpressionStatement or LabeledStatement. We
            // simply start parsing an expression, and afterwards, if the
            // next token is a colon and the expression was a simple
            // Identifier node, we switch to interpreting it as a label.
            _ => {
                if self.is_async_function() {
                    if context.is_some() {
                        return self.unexpected(None);
                    }
                    self.next()?;
                    return self.parse_function_statement(start, true, context.is_none());
                }

                let using_kind = if self.is_await_using(false) {
                    Some(VariableKind::AwaitUsing)
                } else if self.is_using(false) {
                    Some(VariableKind::Using)
                } else {
                    None
                };
                if let Some(using_kind) = using_kind {
                    if !self.allow_using() {
                        return self.raise(self.start, "Using declaration cannot appear in the top level when source type is `script` or in the bare case statement");
                    }
                    if context.is_some() {
                        // Cases like `for (;;) using x = ...;`, `if (true) await using x = ...;`, etc. are not allowed.
                        return self.raise(self.start, "Using declaration is not allowed in single-statement positions");
                    }
                    if using_kind == VariableKind::AwaitUsing {
                        if !self.can_await() {
                            return self.raise(self.start, "Await using cannot appear outside of async function");
                        }
                        self.next()?;
                    }
                    self.next()?;
                    let declarations = self.parse_var(false, using_kind, false)?;
                    self.semicolon()?;
                    return Ok(Statement::VariableDeclaration(VariableDeclaration {
                        ts: None,
                        start,
                        end: self.last_tok_end,
                        declarations,
                        kind: using_kind,
                    }));
                }

                let maybe_name = match &self.value {
                    TokVal::Str(s) if starttype == T::Name => Some(s.clone()),
                    _ => None,
                };
                let expr = self.parse_expression(ForInit::No, None)?;
                if starttype == T::Name && matches!(expr, Expression::Identifier(_)) && self.eat(T::Colon)? {
                    let Expression::Identifier(id) = expr else { unreachable!() };
                    self.parse_labeled_statement(start, maybe_name.unwrap_or_default(), id, context)
                } else {
                    self.parse_expression_statement(start, expr)
                }
            }
        }
    }

    pub fn parse_break_continue_statement(&mut self, start: u32, is_break: bool) -> PResult<Statement> {
        let keyword = if is_break { "break" } else { "continue" };
        self.next()?;
        let label;
        if self.eat(T::Semi)? || self.insert_semicolon() {
            label = None;
        } else if self.type_ != T::Name {
            return self.unexpected(None);
        } else {
            label = Some(self.parse_ident(false)?);
            self.semicolon()?;
        }

        // Verify that there is an actual destination to break or
        // continue to.
        let mut i = 0;
        while i < self.labels.len() {
            let lab = &self.labels[i];
            if label.is_none() || lab.name.as_deref() == label.as_ref().map(|l| l.name.as_str()) {
                if lab.kind.is_some() && (is_break || lab.kind == Some(LabelKind::Loop)) {
                    break;
                }
                if label.is_some() && is_break {
                    break;
                }
            }
            i += 1;
        }
        if i == self.labels.len() {
            return self.raise(start, format!("Unsyntactic {}", keyword));
        }
        let end = self.last_tok_end;
        Ok(if is_break {
            Statement::BreakStatement(BreakStatement { start, end, label })
        } else {
            Statement::ContinueStatement(ContinueStatement { start, end, label })
        })
    }

    pub fn parse_debugger_statement(&mut self, start: u32) -> PResult<Statement> {
        self.next()?;
        self.semicolon()?;
        Ok(Statement::DebuggerStatement(DebuggerStatement { start, end: self.last_tok_end }))
    }

    pub fn parse_do_statement(&mut self, start: u32) -> PResult<Statement> {
        self.next()?;
        self.labels.push(loop_label());
        let body = self.parse_statement(Some("do"), false, None)?;
        self.labels.pop();
        self.expect(T::While)?;
        let test = self.parse_paren_expression()?;
        if self.options.ecma_version >= 6 {
            self.eat(T::Semi)?;
        } else {
            self.semicolon()?;
        }
        Ok(Statement::DoWhileStatement(DoWhileStatement { start, end: self.last_tok_end, body: Box::new(body), test }))
    }

    // Disambiguating between a `for` and a `for`/`in` or `for`/`of`
    // loop is non-trivial. Basically, we have to parse the init `var`
    // statement or expression, disallowing the `in` operator (see
    // the second parameter to `parseExpression`), and then check
    // whether the next token is `in` or `of`. When there is no init
    // part (semicolon immediately after the opening parenthesis), it
    // is a regular `for` loop.

    pub fn parse_for_statement(&mut self, start: u32) -> PResult<Statement> {
        self.next()?;
        let await_at: i64 =
            if self.options.ecma_version >= 9 && self.can_await() && self.eat_contextual("await")? {
                self.last_tok_start as i64
            } else {
                -1
            };
        self.labels.push(loop_label());
        self.enter_scope(0);
        self.expect(T::ParenL)?;
        if self.type_ == T::Semi {
            if await_at > -1 {
                return self.unexpected(Some(await_at as u32));
            }
            return self.parse_for(start, None);
        }
        let is_let = self.is_let(None);
        if self.type_ == T::Var || self.type_ == T::Const || is_let {
            let init_start = self.start;
            let kind = if is_let {
                VariableKind::Let
            } else if self.type_ == T::Const {
                VariableKind::Const
            } else {
                VariableKind::Var
            };
            self.next()?;
            let declarations = self.parse_var(true, kind, false)?;
            let init = VariableDeclaration { start: init_start, end: self.last_tok_end, declarations, kind, ts: None };
            return self.parse_for_after_init(start, init, await_at);
        }
        let starts_with_let = self.is_contextual("let");
        let mut is_for_of = false;

        let using_kind = if self.is_using(true) {
            Some(VariableKind::Using)
        } else if self.is_await_using(true) {
            Some(VariableKind::AwaitUsing)
        } else {
            None
        };
        if let Some(using_kind) = using_kind {
            let init_start = self.start;
            self.next()?;
            if using_kind == VariableKind::AwaitUsing {
                if !self.can_await() {
                    return self.raise(self.start, "Await using cannot appear outside of async function");
                }
                self.next()?;
            }
            let declarations = self.parse_var(true, using_kind, false)?;
            let init = VariableDeclaration { start: init_start, end: self.last_tok_end, declarations, kind: using_kind, ts: None };
            return self.parse_for_after_init(start, init, await_at);
        }
        let contains_esc = self.contains_esc;
        let mut ref_destructuring_errors = DestructuringErrors::new();
        let init_pos = self.start;
        let init = if await_at > -1 {
            self.parse_expr_subscripts(Some(&mut ref_destructuring_errors), ForInit::Await)?
        } else {
            self.parse_expression(ForInit::Yes, Some(&mut ref_destructuring_errors))?
        };
        if self.type_ == T::In || {
            is_for_of = self.options.ecma_version >= 6 && self.is_contextual("of");
            is_for_of
        } {
            let mut is_await = false;
            if await_at > -1 {
                // implies `ecmaVersion >= 9` (see declaration of awaitAt)
                if self.type_ == T::In {
                    return self.unexpected(Some(await_at as u32));
                }
                is_await = true;
            } else if is_for_of && self.options.ecma_version >= 8 {
                if init.span().start == init_pos
                    && !contains_esc
                    && matches!(&init, Expression::Identifier(i) if i.name == "async")
                {
                    return self.unexpected(None);
                }
            }
            if starts_with_let && is_for_of {
                return self
                    .raise(init.span().start, "The left-hand side of a for-of loop may not start with 'let'.");
            }
            let init = self.to_assignable(init, false, Some(&mut ref_destructuring_errors))?;
            self.check_lval_pattern(&init, BIND_NONE, None)?;
            return self.parse_for_in(start, ForLeft::Pattern(init), is_await);
        } else {
            self.check_expression_errors(Some(&ref_destructuring_errors), true)?;
        }
        if await_at > -1 {
            return self.unexpected(Some(await_at as u32));
        }
        self.parse_for(start, Some(ForInit_::Expression(init)))
    }

    // Helper method to parse for loop after variable initialization
    pub fn parse_for_after_init(&mut self, start: u32, init: VariableDeclaration, await_at: i64) -> PResult<Statement> {
        if (self.type_ == T::In || (self.options.ecma_version >= 6 && self.is_contextual("of")))
            && init.declarations.len() == 1
        {
            let mut is_await = false;
            if self.type_ == T::In {
                if matches!(init.kind, VariableKind::Using | VariableKind::AwaitUsing) && init.declarations[0].init.is_none() {
                    return self.raise(self.start, "Using declaration is not allowed in for-in loops");
                }
                if self.options.ecma_version >= 9 && await_at > -1 {
                    return self.unexpected(Some(await_at as u32));
                }
            } else if self.options.ecma_version >= 9 {
                is_await = await_at > -1;
            }
            return self.parse_for_in(
                start,
                ForLeft::VariableDeclaration(VariableDeclarationNode::VariableDeclaration(init)),
                is_await,
            );
        }
        if await_at > -1 {
            return self.unexpected(Some(await_at as u32));
        }
        self.parse_for(start, Some(ForInit_::VariableDeclaration(VariableDeclarationNode::VariableDeclaration(init))))
    }

    pub fn parse_function_statement(&mut self, start: u32, is_async: bool, declaration_position: bool) -> PResult<Statement> {
        self.next()?;
        let f = self.parse_function(
            start,
            FUNC_STATEMENT | if declaration_position { 0 } else { FUNC_HANGING_STATEMENT },
            false,
            is_async,
            ForInit::No,
        )?;
        Ok(Statement::FunctionDeclaration(f))
    }

    pub fn parse_if_statement(&mut self, start: u32) -> PResult<Statement> {
        self.next()?;
        let test = self.parse_paren_expression()?;
        // allow function declarations in branches, but only in non-strict mode
        let consequent = self.parse_statement(Some("if"), false, None)?;
        let alternate = if self.eat(T::Else)? { Some(Box::new(self.parse_statement(Some("if"), false, None)?)) } else { None };
        Ok(Statement::IfStatement(IfStatement {
            start,
            end: self.last_tok_end,
            test,
            consequent: Box::new(consequent),
            alternate,
        }))
    }

    pub fn parse_return_statement(&mut self, start: u32) -> PResult<Statement> {
        if !self.allow_return() {
            return self.raise(self.start, "'return' outside of function");
        }
        self.next()?;

        // In `return` (and `break`/`continue`), the keywords with
        // optional arguments, we eagerly look for a semicolon or the
        // possibility to insert one.

        let argument = if self.eat(T::Semi)? || self.insert_semicolon() {
            None
        } else {
            let a = self.parse_expression(ForInit::No, None)?;
            self.semicolon()?;
            Some(a)
        };
        Ok(Statement::ReturnStatement(ReturnStatement { start, end: self.last_tok_end, argument }))
    }

    pub fn parse_switch_statement(&mut self, start: u32) -> PResult<Statement> {
        self.next()?;
        let discriminant = self.parse_paren_expression()?;
        let mut cases: Vec<SwitchCase> = Vec::new();
        self.expect(T::BraceL)?;
        self.labels.push(switch_label());
        self.enter_scope(SCOPE_SWITCH);

        // Statements under must be grouped (by label) in SwitchCase
        // nodes. `cur` is used to keep the node that we are currently
        // adding statements to.

        let mut cur: Option<SwitchCase> = None;
        let mut saw_default = false;
        while self.type_ != T::BraceR {
            if self.type_ == T::Case || self.type_ == T::Default {
                let is_case = self.type_ == T::Case;
                if let Some(mut c) = cur.take() {
                    c.end = self.last_tok_end;
                    cases.push(c);
                }
                let case_start = self.start;
                self.next()?;
                let test = if is_case {
                    Some(self.parse_expression(ForInit::No, None)?)
                } else {
                    if saw_default {
                        return self.raise_recoverable(self.last_tok_start, "Multiple default clauses");
                    }
                    saw_default = true;
                    None
                };
                cur = Some(SwitchCase { start: case_start, end: 0, test, consequent: Vec::new() });
                self.expect(T::Colon)?;
            } else {
                if cur.is_none() {
                    return self.unexpected(None);
                }
                let stmt = self.parse_statement(None, false, None)?;
                cur.as_mut().unwrap().consequent.push(stmt);
            }
        }
        self.exit_scope();
        if let Some(mut c) = cur.take() {
            c.end = self.last_tok_end;
            cases.push(c);
        }
        self.next()?; // Closing brace
        self.labels.pop();
        Ok(Statement::SwitchStatement(SwitchStatement { start, end: self.last_tok_end, discriminant, cases }))
    }

    pub fn parse_throw_statement(&mut self, start: u32) -> PResult<Statement> {
        self.next()?;
        if has_line_break(&self.input[self.last_tok_end as usize..self.start as usize]) {
            return self.raise(self.last_tok_end, "Illegal newline after throw");
        }
        let argument = self.parse_expression(ForInit::No, None)?;
        self.semicolon()?;
        Ok(Statement::ThrowStatement(ThrowStatement { start, end: self.last_tok_end, argument }))
    }

    pub fn parse_catch_clause_param(&mut self) -> PResult<Pattern> {
        let param = self.parse_binding_atom()?;
        let simple = matches!(param, Pattern::Identifier(_));
        self.enter_scope(if simple { SCOPE_SIMPLE_CATCH } else { 0 });
        self.check_lval_pattern(&param, if simple { BIND_SIMPLE_CATCH } else { BIND_LEXICAL }, None)?;
        self.expect(T::ParenR)?;

        Ok(param)
    }

    pub fn parse_try_statement(&mut self, start: u32) -> PResult<Statement> {
        self.next()?;
        let block = self.parse_block(true, None, false)?;
        let mut handler = None;
        if self.type_ == T::Catch {
            let clause_start = self.start;
            self.next()?;
            let param;
            if self.eat(T::ParenL)? {
                param = Some(self.parse_catch_clause_param()?);
            } else {
                if self.options.ecma_version < 10 {
                    return self.unexpected(None);
                }
                param = None;
                self.enter_scope(0);
            }
            let body = self.parse_block(false, None, false)?;
            self.exit_scope();
            handler = Some(CatchClause { start: clause_start, end: self.last_tok_end, param, body });
        }
        let finalizer = if self.eat(T::Finally)? { Some(self.parse_block(true, None, false)?) } else { None };
        if handler.is_none() && finalizer.is_none() {
            return self.raise(start, "Missing catch or finally clause");
        }
        Ok(Statement::TryStatement(TryStatement { start, end: self.last_tok_end, block, handler, finalizer }))
    }

    pub fn parse_var_statement(
        &mut self,
        start: u32,
        kind: VariableKind,
        allow_missing_initializer: bool,
    ) -> PResult<Statement> {
        self.next()?;
        let declarations = self.parse_var(false, kind, allow_missing_initializer)?;
        self.semicolon()?;
        Ok(Statement::VariableDeclaration(VariableDeclaration { start, end: self.last_tok_end, declarations, kind, ts: None }))
    }

    pub fn parse_while_statement(&mut self, start: u32) -> PResult<Statement> {
        self.next()?;
        let test = self.parse_paren_expression()?;
        self.labels.push(loop_label());
        let body = self.parse_statement(Some("while"), false, None)?;
        self.labels.pop();
        Ok(Statement::WhileStatement(WhileStatement { start, end: self.last_tok_end, test, body: Box::new(body) }))
    }

    pub fn parse_with_statement(&mut self, start: u32) -> PResult<Statement> {
        if self.strict {
            return self.raise(self.start, "'with' in strict mode");
        }
        self.next()?;
        let object = self.parse_paren_expression()?;
        let body = self.parse_statement(Some("with"), false, None)?;
        Ok(Statement::WithStatement(WithStatement { start, end: self.last_tok_end, object, body: Box::new(body) }))
    }

    pub fn parse_empty_statement(&mut self, start: u32) -> PResult<Statement> {
        self.next()?;
        Ok(Statement::EmptyStatement(EmptyStatement { start, end: self.last_tok_end }))
    }

    pub fn parse_labeled_statement(
        &mut self,
        start: u32,
        maybe_name: String,
        expr: Identifier,
        context: Option<&'static str>,
    ) -> PResult<Statement> {
        for label in &self.labels {
            if label.name.as_deref() == Some(maybe_name.as_str()) {
                return self.raise(expr.start, format!("Label '{}' is already declared", maybe_name));
            }
        }
        let kind = if self.type_.is_loop() {
            Some(LabelKind::Loop)
        } else if self.type_ == T::Switch {
            Some(LabelKind::Switch)
        } else {
            None
        };
        let cur_start = self.start;
        for label in self.labels.iter_mut().rev() {
            if label.statement_start == Some(start) {
                // Update information about previous labels on this node
                label.statement_start = Some(cur_start);
                label.kind = kind;
            } else {
                break;
            }
        }
        self.labels.push(Label { name: Some(maybe_name), kind, statement_start: Some(cur_start) });
        let ctx = match context {
            Some(c) => {
                if c.contains("label") {
                    c
                } else {
                    add_label(c)
                }
            }
            None => "label",
        };
        let body = self.parse_statement(Some(ctx), false, None)?;
        self.labels.pop();
        Ok(Statement::LabeledStatement(LabeledStatement { start, end: self.last_tok_end, label: expr, body: Box::new(body) }))
    }

    pub fn parse_expression_statement(&mut self, start: u32, expr: Expression) -> PResult<Statement> {
        self.semicolon()?;
        Ok(Statement::ExpressionStatement(ExpressionStatement {
            start,
            end: self.last_tok_end,
            expression: expr,
            directive: None,
        }))
    }

    // Parse a semicolon-enclosed block of statements, handling `"use
    // strict"` declarations when `allowStrict` is true (used for
    // function bodies).

    pub fn parse_block(
        &mut self,
        create_new_lexical_scope: bool,
        node_start: Option<u32>,
        exit_strict: bool,
    ) -> PResult<BlockStatement> {
        let start = node_start.unwrap_or(self.start);
        let mut body = Vec::new();
        self.expect(T::BraceL)?;
        if create_new_lexical_scope {
            self.enter_scope(0);
        }
        while self.type_ != T::BraceR {
            let stmt = self.parse_statement(None, false, None)?;
            body.push(stmt);
        }
        if exit_strict {
            self.strict = false;
        }
        self.next()?;
        if create_new_lexical_scope {
            self.exit_scope();
        }
        Ok(BlockStatement { start, end: self.last_tok_end, body })
    }

    // Parse a regular `for` loop. The disambiguation code in
    // `parseStatement` will already have parsed the init statement or
    // expression.

    pub fn parse_for(&mut self, start: u32, init: Option<ForInit_>) -> PResult<Statement> {
        self.expect(T::Semi)?;
        let test = if self.type_ == T::Semi { None } else { Some(self.parse_expression(ForInit::No, None)?) };
        self.expect(T::Semi)?;
        let update = if self.type_ == T::ParenR { None } else { Some(self.parse_expression(ForInit::No, None)?) };
        self.expect(T::ParenR)?;
        let body = self.parse_statement(Some("for"), false, None)?;
        self.exit_scope();
        self.labels.pop();
        Ok(Statement::ForStatement(ForStatement { start, end: self.last_tok_end, init, test, update, body: Box::new(body) }))
    }

    // Parse a `for`/`in` and `for`/`of` loop, which are almost
    // same from parser's perspective.

    pub fn parse_for_in(&mut self, start: u32, init: ForLeft, is_await: bool) -> PResult<Statement> {
        let is_for_in = self.type_ == T::In;
        self.next()?;

        if let ForLeft::VariableDeclaration(VariableDeclarationNode::VariableDeclaration(decl)) = &init {
            if decl.declarations[0].init.is_some()
                && (!is_for_in
                    || self.options.ecma_version < 8
                    || self.strict
                    || decl.kind != VariableKind::Var
                    || !matches!(decl.declarations[0].id, Pattern::Identifier(_)))
            {
                return self.raise(
                    decl.start,
                    format!(
                        "{} loop variable declaration may not have an initializer",
                        if is_for_in { "for-in" } else { "for-of" }
                    ),
                );
            }
        }
        let right =
            if is_for_in { self.parse_expression(ForInit::No, None)? } else { self.parse_maybe_assign(ForInit::No, None)? };
        self.expect(T::ParenR)?;
        let body = self.parse_statement(Some("for"), false, None)?;
        self.exit_scope();
        self.labels.pop();
        let end = self.last_tok_end;
        Ok(if is_for_in {
            Statement::ForInStatement(ForInStatement { start, end, left: init, right, body: Box::new(body) })
        } else {
            Statement::ForOfStatement(ForOfStatement { start, end, is_await, left: init, right, body: Box::new(body) })
        })
    }

    // Parse a list of variable declarations.

    pub fn parse_var(
        &mut self,
        is_for: bool,
        kind: VariableKind,
        allow_missing_initializer: bool,
    ) -> PResult<Vec<VariableDeclarator>> {
        let mut declarations = Vec::new();
        loop {
            let decl_start = self.start;
            let id = self.parse_var_id(kind)?;
            let init;
            if self.eat(T::Eq)? {
                init = Some(self.parse_maybe_assign(if is_for { ForInit::Yes } else { ForInit::No }, None)?);
            } else if !allow_missing_initializer
                && kind == VariableKind::Const
                && !(self.type_ == T::In || (self.options.ecma_version >= 6 && self.is_contextual("of")))
            {
                return self.unexpected(None);
            } else if !allow_missing_initializer
                && matches!(kind, VariableKind::Using | VariableKind::AwaitUsing)
                && self.options.ecma_version >= 17
                && self.type_ != T::In
                && !self.is_contextual("of")
            {
                return self
                    .raise(self.last_tok_end, format!("Missing initializer in {} declaration", var_kind_str(kind)));
            } else if !allow_missing_initializer
                && !matches!(id, Pattern::Identifier(_))
                && !(is_for && (self.type_ == T::In || self.is_contextual("of")))
            {
                return self.raise(self.last_tok_end, "Complex binding patterns require an initialization value");
            } else {
                init = None;
            }
            declarations.push(VariableDeclarator { start: decl_start, end: self.last_tok_end, id, init, ts: None });
            if !self.eat(T::Comma)? {
                break;
            }
        }
        Ok(declarations)
    }

    pub fn parse_var_id(&mut self, kind: VariableKind) -> PResult<Pattern> {
        let id = if matches!(kind, VariableKind::Using | VariableKind::AwaitUsing) {
            Pattern::Identifier(self.parse_ident(false)?)
        } else {
            self.parse_binding_atom()?
        };

        self.check_lval_pattern(&id, if kind == VariableKind::Var { BIND_VAR } else { BIND_LEXICAL }, None)?;
        Ok(id)
    }

    // Parse a function declaration or literal (depending on the
    // `statement & FUNC_STATEMENT`).

    // Remove `allowExpressionBody` for 7.0.0, as it is only called with false
    pub fn parse_function(
        &mut self,
        start: u32,
        statement: u32,
        _allow_expression_body: bool,
        is_async: bool,
        for_init: ForInit,
    ) -> PResult<Function> {
        // initFunction
        let mut id: Option<Identifier> = None;
        let mut generator = false;
        if self.options.ecma_version >= 9 || self.options.ecma_version >= 6 && !is_async {
            if self.type_ == T::Star && (statement & FUNC_HANGING_STATEMENT) != 0 {
                return self.unexpected(None);
            }
            generator = self.eat(T::Star)?;
        }
        let is_async_node = self.options.ecma_version >= 8 && is_async;

        if statement & FUNC_STATEMENT != 0 {
            id = if (statement & FUNC_NULLABLE_ID) != 0 && self.type_ != T::Name {
                None
            } else {
                Some(self.parse_ident(false)?)
            };
            if let Some(id) = &id {
                if statement & FUNC_HANGING_STATEMENT == 0 {
                    // If it is a regular function declaration in sloppy mode, then it is
                    // subject to Annex B semantics (BIND_FUNCTION). Otherwise, the binding
                    // mode depends on properties of the current scope (see
                    // treatFunctionsAsVar).
                    let binding = if self.strict || generator || is_async_node {
                        if self.treat_functions_as_var() {
                            BIND_VAR
                        } else {
                            BIND_LEXICAL
                        }
                    } else {
                        BIND_FUNCTION
                    };
                    let id = id.clone();
                    self.check_lval_identifier(&id, binding, None)?;
                }
            }
        }

        let old_yield_pos = self.yield_pos;
        let old_await_pos = self.await_pos;
        let old_await_ident_pos = self.await_ident_pos;
        self.yield_pos = 0;
        self.await_pos = 0;
        self.await_ident_pos = 0;
        self.enter_scope(function_flags(is_async_node, generator));

        if statement & FUNC_STATEMENT == 0 {
            id = if self.type_ == T::Name { Some(self.parse_ident(false)?) } else { None };
        }

        let params = self.parse_function_params()?;
        let body = self.parse_function_body(start, &params, id.as_ref(), false, false, for_init)?.into_block();

        self.yield_pos = old_yield_pos;
        self.await_pos = old_await_pos;
        self.await_ident_pos = old_await_ident_pos;
        Ok(Function {
            ts: None,
            start,
            end: self.last_tok_end,
            id,
            expression: false,
            generator,
            is_async: is_async_node,
            params,
            body: BlockStatementNode::BlockStatement(body),
        })
    }

    pub fn parse_function_params(&mut self) -> PResult<Vec<Pattern>> {
        self.expect(T::ParenL)?;
        let params =
            self.parse_binding_list(T::ParenR, false, self.options.ecma_version >= 8)?.into_iter().flatten().collect();
        self.check_yield_await_in_default_params()?;
        Ok(params)
    }

    // Parse a class declaration or literal (depending on the
    // `isStatement` parameter).

    pub fn parse_class(&mut self, start: u32, is_statement: IsStatement) -> PResult<Class> {
        self.next()?;

        // ecma-262 14.6 Class Definitions
        // A class definition is always strict mode code.
        let old_strict = self.strict;
        self.strict = true;

        let id = self.parse_class_id(is_statement)?;
        let super_class = self.parse_class_super()?;
        self.enter_class_body();
        let class_body_start = self.start;
        let mut had_constructor = false;
        let mut body = Vec::new();
        self.expect(T::BraceL)?;
        while self.type_ != T::BraceR {
            let element = self.parse_class_element(super_class.is_some())?;
            if let Some(element) = element {
                match &element {
                    ClassElement::MethodDefinition(m) if m.kind == MethodKind::Constructor => {
                        if had_constructor {
                            return self.raise_recoverable(m.start, "Duplicate constructor in the same class");
                        }
                        had_constructor = true;
                    }
                    ClassElement::MethodDefinition(MethodDefinition { key: PropertyKey::Private(_), .. })
                    | ClassElement::PropertyDefinition(PropertyDefinition { key: PropertyKey::Private(_), .. }) => {
                        if self.is_private_name_conflicted(&element) {
                            let (name, start) = private_key(&element);
                            return self
                                .raise_recoverable(start, format!("Identifier '#{}' has already been declared", name));
                        }
                    }
                    _ => {}
                }
                body.push(element);
            }
        }
        self.strict = old_strict;
        self.next()?;
        let class_body = ClassBody { start: class_body_start, end: self.last_tok_end, body };
        self.exit_class_body()?;
        Ok(Class { start, end: self.last_tok_end, id, super_class, body: class_body, ts: None })
    }

    pub fn parse_class_element(&mut self, constructor_allows_super: bool) -> PResult<Option<ClassElement>> {
        if self.eat(T::Semi)? {
            return Ok(None);
        }

        let ecma_version = self.options.ecma_version;
        let start = self.start;
        let mut key_name = "";
        let mut is_generator = false;
        let mut is_async = false;
        let mut kind = MethodKind::Method;
        let mut is_static = false;

        if self.eat_contextual("static")? {
            // Parse static init block
            if ecma_version >= 13 && self.eat(T::BraceL)? {
                return Ok(Some(ClassElement::StaticBlock(self.parse_class_static_block(start)?)));
            }
            if self.is_class_element_name_start() || self.type_ == T::Star {
                is_static = true;
            } else {
                key_name = "static";
            }
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

        // Parse element name
        let computed;
        let key;
        if !key_name.is_empty() {
            // 'async', 'get', 'set', or 'static' were not a keyword contextually.
            // The last token is any of those. Make it the element name.
            computed = false;
            key = PropertyKey::Expression(Box::new(Expression::Identifier(Identifier {
                ts: None,
                start: self.last_tok_start,
                end: self.last_tok_end,
                name: key_name.to_string(),
            })));
        } else {
            (computed, key) = self.parse_class_element_name()?;
        }

        // Parse element value
        if ecma_version < 13 || self.type_ == T::ParenL || kind != MethodKind::Method || is_generator || is_async {
            let is_constructor = !is_static && check_key_name(computed, &key, "constructor");
            let allows_direct_super = is_constructor && constructor_allows_super;
            // Couldn't move this check into the 'parseClassMethod' method for backward compatibility.
            if is_constructor && kind != MethodKind::Method {
                return self.raise(property_key_start(&key), "Constructor can't have get/set modifier");
            }
            let kind = if is_constructor { MethodKind::Constructor } else { kind };
            Ok(Some(ClassElement::MethodDefinition(self.parse_class_method(
                start,
                is_static,
                computed,
                key,
                kind,
                is_generator,
                is_async,
                allows_direct_super,
            )?)))
        } else {
            Ok(Some(ClassElement::PropertyDefinition(self.parse_class_field(start, is_static, computed, key)?)))
        }
    }

    pub fn is_class_element_name_start(&self) -> bool {
        matches!(self.type_, T::Name | T::PrivateId | T::Num | T::String | T::BracketL) || self.type_.is_keyword()
    }

    pub fn parse_class_element_name(&mut self) -> PResult<(bool, PropertyKey)> {
        if self.type_ == T::PrivateId {
            if matches!(&self.value, TokVal::Str(s) if s == "constructor") {
                return self.raise(self.start, "Classes can't have an element named '#constructor'");
            }
            Ok((false, PropertyKey::Private(PrivateIdentifierNode::PrivateIdentifier(self.parse_private_ident()?))))
        } else {
            self.parse_property_name()
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn parse_class_method(
        &mut self,
        start: u32,
        is_static: bool,
        computed: bool,
        key: PropertyKey,
        kind: MethodKind,
        is_generator: bool,
        is_async: bool,
        allows_direct_super: bool,
    ) -> PResult<MethodDefinition> {
        // Check key and flags
        let key_start = property_key_start(&key);
        if kind == MethodKind::Constructor {
            if is_generator {
                return self.raise(key_start, "Constructor can't be a generator");
            }
            if is_async {
                return self.raise(key_start, "Constructor can't be an async method");
            }
        } else if is_static && check_key_name(computed, &key, "prototype") {
            return self.raise(key_start, "Classes may not have a static property named prototype");
        }

        // Parse value
        let value = self.parse_method(is_generator, is_async, allows_direct_super)?;

        // Check value
        if kind == MethodKind::Get && !value.params.is_empty() {
            return self.raise_recoverable(value.start, "getter should have no params");
        }
        if kind == MethodKind::Set && value.params.len() != 1 {
            return self.raise_recoverable(value.start, "setter should have exactly one param");
        }
        if kind == MethodKind::Set {
            if let Pattern::RestElement(r) = &value.params[0] {
                return self.raise_recoverable(r.start, "Setter cannot use rest params");
            }
        }

        Ok(MethodDefinition {
            ts: None,
            start,
            end: self.last_tok_end,
            is_static,
            computed,
            key,
            kind,
            value: FunctionNode::FunctionExpression(value),
        })
    }

    pub fn parse_class_field(
        &mut self,
        start: u32,
        is_static: bool,
        computed: bool,
        key: PropertyKey,
    ) -> PResult<PropertyDefinition> {
        if check_key_name(computed, &key, "constructor") {
            return self.raise(property_key_start(&key), "Classes can't have a field named 'constructor'");
        } else if is_static && check_key_name(computed, &key, "prototype") {
            return self.raise(property_key_start(&key), "Classes can't have a static field named 'prototype'");
        }

        let value = if self.eat(T::Eq)? {
            // To raise SyntaxError if 'arguments' exists in the initializer.
            self.enter_scope(SCOPE_CLASS_FIELD_INIT | SCOPE_SUPER);
            let v = self.parse_maybe_assign(ForInit::No, None)?;
            self.exit_scope();
            Some(v)
        } else {
            None
        };
        self.semicolon()?;

        Ok(PropertyDefinition { start, end: self.last_tok_end, is_static, computed, key, value, ts: None })
    }

    pub fn parse_class_static_block(&mut self, start: u32) -> PResult<StaticBlock> {
        let mut body = Vec::new();

        let old_labels = std::mem::take(&mut self.labels);
        self.enter_scope(SCOPE_CLASS_STATIC_BLOCK | SCOPE_SUPER);
        while self.type_ != T::BraceR {
            let stmt = self.parse_statement(None, false, None)?;
            body.push(stmt);
        }
        self.next()?;
        self.exit_scope();
        self.labels = old_labels;

        Ok(StaticBlock { start, end: self.last_tok_end, body })
    }

    pub fn parse_class_id(&mut self, is_statement: IsStatement) -> PResult<Option<Identifier>> {
        if self.type_ == T::Name {
            let id = self.parse_ident(false)?;
            if is_statement != IsStatement::No {
                self.check_lval_identifier(&id, BIND_LEXICAL, None)?;
            }
            Ok(Some(id))
        } else {
            if is_statement == IsStatement::Yes {
                return self.unexpected(None);
            }
            Ok(None)
        }
    }

    pub fn parse_class_super(&mut self) -> PResult<Option<BoxExpr>> {
        Ok(if self.eat(T::Extends)? { Some(Box::new(self.parse_expr_subscripts(None, ForInit::No)?)) } else { None })
    }

    pub fn enter_class_body(&mut self) {
        self.private_name_stack.push(PrivateNameEntry::default());
    }

    pub fn exit_class_body(&mut self) -> PResult<()> {
        let PrivateNameEntry { declared, used } = self.private_name_stack.pop().unwrap();
        if !self.options.check_private_fields {
            return Ok(());
        }
        let has_parent = !self.private_name_stack.is_empty();
        for (name, start) in used {
            if !declared.contains_key(&name) {
                if has_parent {
                    self.private_name_stack.last_mut().unwrap().used.push((name, start));
                } else {
                    return self
                        .raise_recoverable(start, format!("Private field '#{}' must be declared in an enclosing class", name));
                }
            }
        }
        Ok(())
    }

    fn is_private_name_conflicted(&mut self, element: &ClassElement) -> bool {
        let (name, _) = private_key(element);
        let private_name_map: &mut HashMap<String, &'static str> =
            &mut self.private_name_stack.last_mut().unwrap().declared;
        let curr = private_name_map.get(&name).copied();

        let mut next = "true";
        if let ClassElement::MethodDefinition(m) = element {
            if m.kind == MethodKind::Get || m.kind == MethodKind::Set {
                next = match (m.is_static, m.kind) {
                    (true, MethodKind::Get) => "sget",
                    (true, _) => "sset",
                    (false, MethodKind::Get) => "iget",
                    (false, _) => "iset",
                };
            }
        }

        // `class { get #a(){}; static set #a(_){} }` is also conflict.
        if curr == Some("iget") && next == "iset"
            || curr == Some("iset") && next == "iget"
            || curr == Some("sget") && next == "sset"
            || curr == Some("sset") && next == "sget"
        {
            private_name_map.insert(name, "true");
            false
        } else if curr.is_none() {
            private_name_map.insert(name, next);
            false
        } else {
            true
        }
    }

    // Parses module export declaration.

    pub fn parse_export_all_declaration(&mut self, start: u32, exports: Option<&mut Exports>) -> PResult<Statement> {
        let mut exported = None;
        if self.options.ecma_version >= 11 && self.eat_contextual("as")? {
            let e = self.parse_module_export_name()?;
            self.check_export(exports, &module_export_name_str(&e), self.last_tok_start)?;
            exported = Some(e);
        }
        self.expect_contextual("from")?;
        if self.type_ != T::String {
            return self.unexpected(None);
        }
        let source = self.parse_string_literal()?;
        let attributes = if self.options.ecma_version >= 16 { self.parse_with_clause()? } else { Vec::new() };
        self.semicolon()?;
        Ok(Statement::ExportAllDeclaration(ExportAllDeclaration {
            ts: None,
            start,
            end: self.last_tok_end,
            exported,
            source: LiteralNode::Literal(source),
            attributes,
        }))
    }

    pub fn parse_export(&mut self, start: u32, exports: Option<&mut Exports>) -> PResult<Statement> {
        self.next()?;
        // export * from '...'
        if self.eat(T::Star)? {
            return self.parse_export_all_declaration(start, exports);
        }
        if self.eat(T::Default)? {
            // export default ...
            self.check_export(exports, "default", self.last_tok_start)?;
            let declaration = self.parse_export_default_declaration()?;
            return Ok(Statement::ExportDefaultDeclaration(ExportDefaultDeclaration {
                ts: None,
                start,
                end: self.last_tok_end,
                declaration,
            }));
        }
        // export var|const|let|function|class ...
        let declaration;
        let specifiers;
        let mut source = None;
        let mut attributes = Vec::new();
        if self.should_parse_export_statement() {
            let decl = self.parse_export_declaration()?;
            match &decl {
                Statement::VariableDeclaration(v) => self.check_variable_export(exports, &v.declarations)?,
                Statement::FunctionDeclaration(Function { id: Some(id), .. })
                | Statement::ClassDeclaration(Class { id: Some(id), .. }) => {
                    let (name, start) = (id.name.clone(), id.start);
                    self.check_export(exports, &name, start)?;
                }
                _ => {}
            }
            declaration = Some(Box::new(decl));
            specifiers = Vec::new();
        } else {
            // export { x, y as z } [from '...']
            declaration = None;
            specifiers = self.parse_export_specifiers(exports)?;
            if self.eat_contextual("from")? {
                if self.type_ != T::String {
                    return self.unexpected(None);
                }
                source = Some(LiteralNode::Literal(self.parse_string_literal()?));
                if self.options.ecma_version >= 16 {
                    attributes = self.parse_with_clause()?;
                }
            } else {
                for spec in &specifiers {
                    match &spec.local {
                        ModuleExportName::Identifier(local) => {
                            // check for keywords used as local names
                            self.check_unreserved(local.start, local.end, &local.name)?;
                            // check if export is defined
                            self.check_local_export(&local.name, local.start);
                        }
                        ModuleExportName::Literal(l) => {
                            return self.raise(
                                l.start,
                                "A string literal cannot be used as an exported binding without `from`.",
                            );
                        }
                    }
                }
            }
            self.semicolon()?;
        }
        Ok(Statement::ExportNamedDeclaration(ExportNamedDeclaration {
            ts: None,
            start,
            end: self.last_tok_end,
            declaration,
            specifiers,
            source,
            attributes,
        }))
    }

    pub fn parse_export_declaration(&mut self) -> PResult<Statement> {
        self.parse_statement(None, false, None)
    }

    pub fn parse_export_default_declaration(&mut self) -> PResult<ExportDefaultKind> {
        let mut is_async = false;
        if self.type_ == T::Function || {
            is_async = self.is_async_function();
            is_async
        } {
            let f_start = self.start;
            self.next()?;
            if is_async {
                self.next()?;
            }
            let f = self.parse_function(f_start, FUNC_STATEMENT | FUNC_NULLABLE_ID, false, is_async, ForInit::No)?;
            Ok(ExportDefaultKind::Declaration(Box::new(Statement::FunctionDeclaration(f))))
        } else if self.type_ == T::Class {
            let c_start = self.start;
            let c = self.parse_class(c_start, IsStatement::NullableId)?;
            Ok(ExportDefaultKind::Declaration(Box::new(Statement::ClassDeclaration(c))))
        } else {
            let declaration = self.parse_maybe_assign(ForInit::No, None)?;
            self.semicolon()?;
            Ok(ExportDefaultKind::Expression(declaration))
        }
    }

    pub fn check_export(&self, exports: Option<&mut Exports>, name: &str, pos: u32) -> PResult<()> {
        let Some(exports) = exports else { return Ok(()) };
        if exports.contains(name) {
            return self.raise_recoverable(pos, format!("Duplicate export '{}'", name));
        }
        exports.insert(name.to_string());
        Ok(())
    }

    pub fn check_pattern_export(&self, mut exports: Option<&mut Exports>, pat: &Pattern) -> PResult<()> {
        match pat {
            Pattern::Identifier(i) => self.check_export(exports, &i.name, i.start),
            Pattern::ObjectPattern(o) => {
                for prop in &o.properties {
                    match prop {
                        ObjectPatternMember::Property(p) => self.check_pattern_export(exports.as_deref_mut(), &p.value)?,
                        ObjectPatternMember::RestElement(r) => {
                            self.check_pattern_export(exports.as_deref_mut(), &r.argument)?
                        }
                    }
                }
                Ok(())
            }
            Pattern::ArrayPattern(a) => {
                for elt in a.elements.iter().flatten() {
                    self.check_pattern_export(exports.as_deref_mut(), elt)?;
                }
                Ok(())
            }
            Pattern::AssignmentPattern(a) => self.check_pattern_export(exports, &a.left),
            Pattern::RestElement(r) => self.check_pattern_export(exports, &r.argument),
            _ => Ok(()),
        }
    }

    pub fn check_variable_export(&self, mut exports: Option<&mut Exports>, decls: &[VariableDeclarator]) -> PResult<()> {
        if exports.is_none() {
            return Ok(());
        }
        for decl in decls {
            self.check_pattern_export(exports.as_deref_mut(), &decl.id)?;
        }
        Ok(())
    }

    pub fn should_parse_export_statement(&self) -> bool {
        let kw = self.type_.keyword();
        kw == Some("var")
            || kw == Some("const")
            || kw == Some("class")
            || kw == Some("function")
            || self.is_let(None)
            || self.is_async_function()
    }

    // Parses a comma-separated list of module exports.

    pub fn parse_export_specifier(&mut self, exports: Option<&mut Exports>) -> PResult<ExportSpecifier> {
        let start = self.start;
        let local = self.parse_module_export_name()?;

        let exported = if self.eat_contextual("as")? { self.parse_module_export_name()? } else { local.clone() };
        self.check_export(exports, &module_export_name_str(&exported), module_export_name_start(&exported))?;

        Ok(ExportSpecifier { start, end: self.last_tok_end, local, exported, ts: None })
    }

    pub fn parse_export_specifiers(&mut self, mut exports: Option<&mut Exports>) -> PResult<Vec<ExportSpecifier>> {
        let mut nodes = Vec::new();
        let mut first = true;
        // export { x, y as z } [from '...']
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

            nodes.push(self.parse_export_specifier(exports.as_deref_mut())?);
        }
        Ok(nodes)
    }

    // Parses import declaration.

    pub fn parse_import(&mut self, start: u32) -> PResult<Statement> {
        self.next()?;

        // import '...'
        let specifiers;
        let source;
        if self.type_ == T::String {
            specifiers = Vec::new();
            source = self.parse_string_literal()?;
        } else {
            specifiers = self.parse_import_specifiers()?;
            self.expect_contextual("from")?;
            if self.type_ != T::String {
                return self.unexpected(None);
            }
            source = self.parse_string_literal()?;
        }
        let attributes = if self.options.ecma_version >= 16 { self.parse_with_clause()? } else { Vec::new() };
        self.semicolon()?;
        Ok(Statement::ImportDeclaration(ImportDeclaration {
            ts: None,
            start,
            end: self.last_tok_end,
            specifiers,
            source: LiteralNode::Literal(source),
            attributes,
        }))
    }

    // Parses a comma-separated list of module imports.

    pub fn parse_import_specifier(&mut self) -> PResult<ImportSpecifier> {
        let start = self.start;
        let imported = self.parse_module_export_name()?;

        let local = if self.eat_contextual("as")? {
            self.parse_ident(false)?
        } else {
            match &imported {
                ModuleExportName::Identifier(i) => {
                    self.check_unreserved(i.start, i.end, &i.name)?;
                    i.clone()
                }
                // `checkLValSimple` on a Literal: "Binding rvalue"
                ModuleExportName::Literal(l) => return self.raise(l.start, "Binding rvalue"),
            }
        };
        self.check_lval_identifier(&local, BIND_LEXICAL, None)?;

        Ok(ImportSpecifier { start, end: self.last_tok_end, imported, local, ts: None })
    }

    pub fn parse_import_default_specifier(&mut self) -> PResult<ImportDefaultSpecifier> {
        // import defaultObj, { x, y as z } from '...'
        let start = self.start;
        let local = self.parse_ident(false)?;
        self.check_lval_identifier(&local, BIND_LEXICAL, None)?;
        Ok(ImportDefaultSpecifier { start, end: self.last_tok_end, local })
    }

    pub fn parse_import_namespace_specifier(&mut self) -> PResult<ImportNamespaceSpecifier> {
        let start = self.start;
        self.next()?;
        self.expect_contextual("as")?;
        let local = self.parse_ident(false)?;
        self.check_lval_identifier(&local, BIND_LEXICAL, None)?;
        Ok(ImportNamespaceSpecifier { start, end: self.last_tok_end, local })
    }

    pub fn parse_import_specifiers(&mut self) -> PResult<Vec<ImportSpecifierKind>> {
        let mut nodes = Vec::new();
        let mut first = true;
        if self.type_ == T::Name {
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

            nodes.push(ImportSpecifierKind::ImportSpecifier(self.parse_import_specifier()?));
        }
        Ok(nodes)
    }

    pub fn parse_with_clause(&mut self) -> PResult<Vec<ImportAttribute>> {
        let mut nodes = Vec::new();
        if !self.eat(T::With)? {
            return Ok(nodes);
        }
        self.expect(T::BraceL)?;
        let mut attribute_keys: HashSet<String> = HashSet::new();
        let mut first = true;
        while !self.eat(T::BraceR)? {
            if !first {
                self.expect(T::Comma)?;
                if self.after_trailing_comma(T::BraceR, false)? {
                    break;
                }
            } else {
                first = false;
            }

            let attr = self.parse_import_attribute()?;
            let key_name = module_export_name_str(&attr.key);
            if attribute_keys.contains(&key_name) {
                return self.raise_recoverable(
                    module_export_name_start(&attr.key),
                    format!("Duplicate attribute key '{}'", key_name),
                );
            }
            attribute_keys.insert(key_name);
            nodes.push(attr);
        }
        Ok(nodes)
    }

    pub fn parse_import_attribute(&mut self) -> PResult<ImportAttribute> {
        let start = self.start;
        let key = if self.type_ == T::String {
            ModuleExportName::Literal(self.parse_string_literal()?)
        } else {
            ModuleExportName::Identifier(self.parse_ident(self.options.allow_reserved != AllowReserved::Never)?)
        };
        self.expect(T::Colon)?;
        if self.type_ != T::String {
            return self.unexpected(None);
        }
        let value = LiteralNode::Literal(self.parse_string_literal()?);
        Ok(ImportAttribute { start, end: self.last_tok_end, key, value })
    }

    pub fn parse_module_export_name(&mut self) -> PResult<ModuleExportName> {
        if self.options.ecma_version >= 13 && self.type_ == T::String {
            let lone = self.value_lone_surrogate;
            let string_literal = self.parse_string_literal()?;
            if lone {
                return self.raise(string_literal.start, "An export name cannot include a lone surrogate.");
            }
            return Ok(ModuleExportName::Literal(string_literal));
        }
        Ok(ModuleExportName::Identifier(self.parse_ident(true)?))
    }

    /// `parseExprAtom()` / `parseLiteral(this.value)` for a string token.
    fn parse_string_literal(&mut self) -> PResult<Literal> {
        self.parse_literal_token()
    }

    // Set `ExpressionStatement#directive` property for directive prologues.
    pub fn adapt_directive_prologue(&self, statements: &mut [Statement]) {
        for s in statements.iter_mut() {
            if !self.is_directive_candidate(s) {
                break;
            }
            if let Statement::ExpressionStatement(es) = s {
                if let Expression::Literal(l) = &es.expression {
                    es.directive = Some(l.raw[1..l.raw.len() - 1].to_string());
                }
            }
        }
    }

    pub fn is_directive_candidate(&self, statement: &Statement) -> bool {
        self.options.ecma_version >= 5
            && match statement {
                Statement::ExpressionStatement(es) => {
                    matches!(&es.expression, Expression::Literal(Literal { value: LiteralValue::String(_), .. }))
                        // Reject parenthesized strings.
                        && matches!(self.input.as_bytes().get(es.start as usize), Some(b'"') | Some(b'\''))
                }
                _ => false,
            }
    }
}

type ForInit_ = crate::estree::ForInit;

fn loop_label() -> Label {
    Label { name: None, kind: Some(LabelKind::Loop), statement_start: None }
}

fn switch_label() -> Label {
    Label { name: None, kind: Some(LabelKind::Switch), statement_start: None }
}

fn private_key(element: &ClassElement) -> (String, u32) {
    let key = match element {
        ClassElement::MethodDefinition(m) => &m.key,
        ClassElement::PropertyDefinition(p) => &p.key,
        ClassElement::StaticBlock(_) | ClassElement::TSIndexSignature(_) => return (String::new(), 0),
    };
    match key {
        PropertyKey::Private(PrivateIdentifierNode::PrivateIdentifier(p)) => (p.name.clone(), p.start),
        _ => (String::new(), 0),
    }
}

fn check_key_name(computed: bool, key: &PropertyKey, name: &str) -> bool {
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
