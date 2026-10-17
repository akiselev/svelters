//! Port of @sveltejs/acorn-typescript 1.0.13 (MIT, tyrealhu and the Svelte team), the acorn
//! plugin Svelte uses for `lang="ts"` (`acorn.Parser.extend(tsPlugin())`, default options: no
//! JSX, no `dts`). Owner: agent TS. Source: `node_modules/@sveltejs/acorn-typescript/index.js`
//! (the published bundle of `src/index.ts`, `src/extentions/decorators.ts`,
//! `src/extentions/import-assertions.ts`, `src/effects*.ts`, `src/tokenType.ts`, `src/error.ts`).
//!
//! The plugin is a subclass of acorn's `Parser` overriding ~90 methods. Here the subclass state
//! is [`TsState`] (`Parser::ts`, `Some` when `AcornOptions::typescript`), and every overridden
//! acorn method starts with a dispatch to its `*_ts` counterpart in this module (methods named
//! `ts_*` are the plugin's own). `super.method()` calls go to the acorn port directly (a
//! `*_base` function where the acorn method itself dispatches). Plain JS parsing never takes a
//! TypeScript path, so its behaviour is unchanged.
//!
//! Files: `mod.rs` (state, speculative parsing, tokenizer and scope overrides), `types.rs`
//! (type grammar), `expression.rs`, `lval.rs`, `statement.rs` (overrides of the corresponding
//! acorn files; `statement.rs` also holds the declaration parsers and decorators).
//!
//! SVELTERS (see docs/notes/native-compiler/deviations.md, "acorn-typescript"):
//! - `ParseEffects` (the plugin's undo log for speculative parsing) is a [`Checkpoint`]: a copy
//!   of every piece of parser state its write barriers cover, restored on rollback. Comments
//!   reported inside a speculative branch are dropped on rollback (the plugin buffers and
//!   discards them) and re-reported by `selectTryParseResult`.
//! - Typed nodes: the plugin mutates nodes in place (`node.typeAnnotation = ...`); here the
//!   properties go to the node's `ts` field or the TS node types of `estree::ts`.

mod expression;
mod lval;
mod statement;
pub use statement::FnOrDeclare;
mod types;

use super::scope::Scope;
use super::scopeflags::*;
use super::state::{Label, PErr, PResult, Parser, PrivateNameEntry, TokVal, UndefinedExports};
use super::tokencontext::TokContext;
use super::tokentype::TokenType as T;
use super::whitespace::{has_line_break, is_js_space, skip_white_space};
use crate::estree::{Comment, Decorator, TSTypeAnnotation};

// src/scopeflags.ts
pub const TS_SCOPE_OTHER: u32 = 1 << 20;
pub const TS_SCOPE_TS_MODULE: u32 = 1 << 21;

// `acornScope` of src/index.ts (acorn's values plus the TypeScript binding kinds)
pub const BIND_TS_TYPE: u32 = 6;
pub const BIND_TS_INTERFACE: u32 = 7;
pub const BIND_FLAGS_TS_EXPORT_ONLY: u32 = 1024;
pub const BIND_TS_NAMESPACE: u32 = 8 | 1024;
pub const BIND_FLAGS_TS_IMPORT: u32 = 4096;
pub const BIND_FLAGS_TS_ENUM: u32 = 256;
pub const BIND_FLAGS_TS_CONST_ENUM: u32 = 512;
pub const BIND_TS_ENUM: u32 = 2 | 256;
pub const BIND_TS_CONST_ENUM: u32 = 2 | 256 | 512;
pub const BIND_FLAGS_CLASS: u32 = 128;

/// The fields `TypeScriptParser`'s constructor adds (plus `decoratorStack`, `importsStack`).
#[derive(Clone, Debug)]
pub struct TsState {
    pub is_lookahead: bool,
    pub is_ambient_context: bool,
    pub in_abstract_class: bool,
    pub in_type: bool,
    pub in_disallow_conditional_types_context: bool,
    pub maybe_in_arrow_parameters: bool,
    pub should_parse_arrow_return_type: Option<TSTypeAnnotation>,
    pub should_parse_async_arrow_return_type: Option<TSTypeAnnotation>,
    pub decorator_stack: Vec<Vec<Decorator>>,
    pub imports_stack: Vec<Vec<String>>,
    /// We will only parse one import node or export node at same time. Default kind is undefined.
    pub import_or_export_outer_kind: Option<&'static str>,
}

impl Default for TsState {
    fn default() -> TsState {
        TsState {
            is_lookahead: false,
            is_ambient_context: false,
            in_abstract_class: false,
            in_type: false,
            in_disallow_conditional_types_context: false,
            maybe_in_arrow_parameters: false,
            should_parse_arrow_return_type: None,
            should_parse_async_arrow_return_type: None,
            decorator_stack: vec![Vec::new()],
            imports_stack: vec![Vec::new()],
            import_or_export_outer_kind: None,
        }
    }
}

/// The arrays `enterScope` adds to each scope.
#[derive(Clone, Debug, Default)]
pub struct TsScope {
    pub types: Vec<String>,
    pub enums: Vec<String>,
    pub const_enums: Vec<String>,
    pub classes: Vec<String>,
    pub export_only_bindings: Vec<String>,
}

/// `getCurLookaheadState()`: the token state `lookahead` returns.
#[derive(Clone, Debug)]
pub struct LookaheadState {
    pub type_: T,
    pub value: TokVal,
    pub start: u32,
    pub end: u32,
    pub pos: u32,
    pub contains_esc: bool,
}

impl LookaheadState {
    pub fn value_str(&self) -> &str {
        match &self.value {
            TokVal::Str(s) => s,
            _ => "",
        }
    }
}

/// Everything a speculative parse may change (`captureParserCheckpoint` + the state covered by
/// the plugin's write barriers: scopes, labels, private names, undefined exports, comments).
pub struct Checkpoint {
    pos: u32,
    type_: T,
    value: TokVal,
    value_lone_surrogate: bool,
    start: u32,
    end: u32,
    last_tok_start: u32,
    last_tok_end: u32,
    context: Vec<TokContext>,
    expr_allowed: bool,
    contains_esc: bool,
    strict: bool,
    potential_arrow_at: i64,
    potential_arrow_in_for_await: bool,
    yield_pos: u32,
    await_pos: u32,
    await_ident_pos: u32,
    labels: Vec<Label>,
    undefined_exports: UndefinedExports,
    scope_stack: Vec<Scope>,
    private_name_stack: Vec<PrivateNameEntry>,
    in_template_element: bool,
    comments_len: usize,
    depth: u32,
    ts: TsState,
}

/// Result of `tryParse`.
pub enum TryParse<N> {
    Ok(N),
    /// `abort()` was called; `events` are the comments the branch reported.
    Aborted(Vec<Comment>),
    /// A SyntaxError was thrown.
    Thrown(Box<PErr>, Vec<Comment>),
}

// src/error.ts
pub mod err {
    pub const ABSTRACT_METHOD_HAS_IMPLEMENTATION: fn(&str) -> String =
        |m| format!("Method '{m}' cannot have an implementation because it is marked abstract.");
    pub const ABSTRACT_PROPERTY_HAS_INITIALIZER: fn(&str) -> String =
        |p| format!("Property '{p}' cannot have an initializer because it is marked abstract.");
    pub const ACCESOR_CANNOT_DECLARE_THIS_PARAMETER: &str = "'get' and 'set' accessors cannot declare 'this' parameters.";
    pub const ACCESOR_CANNOT_HAVE_TYPE_PARAMETERS: &str = "An accessor cannot have type parameters.";
    pub const CANNOT_FIND_NAME: fn(&str) -> String = |n| format!("Cannot find name '{n}'.");
    pub const CLASS_METHOD_HAS_DECLARE: &str = "Class methods cannot have the 'declare' modifier.";
    pub const CLASS_METHOD_HAS_READONLY: &str = "Class methods cannot have the 'readonly' modifier.";
    pub const CONST_INITIALIZER_MUST_BE_STRING_OR_NUMERIC_LITERAL_OR_LITERAL_ENUM_REFERENCE: &str = "A 'const' initializer in an ambient context must be a string or numeric literal or literal enum reference.";
    pub const CONSTRUCTOR_HAS_TYPE_PARAMETERS: &str = "Type parameters cannot appear on a constructor declaration.";
    pub const DECLARE_ACCESSOR: fn(&str) -> String = |k| format!("'declare' is not allowed in {k}ters.");
    pub const DECLARE_CLASS_FIELD_HAS_INITIALIZER: &str = "Initializers are not allowed in ambient contexts.";
    pub const DECLARE_FUNCTION_HAS_IMPLEMENTATION: &str = "An implementation cannot be declared in ambient contexts.";
    pub const DUPLICATE_ACCESSIBILITY_MODIFIER: &str = "Accessibility modifier already seen.";
    pub const DUPLICATE_MODIFIER: fn(&str) -> String = |m| format!("Duplicate modifier: '{m}'.");
    pub const EMPTY_HERITAGE_CLAUSE_TYPE: fn(&str) -> String = |t| format!("'{t}' list cannot be empty.");
    pub const EMPTY_TYPE_ARGUMENTS: &str = "Type argument list cannot be empty.";
    pub const EMPTY_TYPE_PARAMETERS: &str = "Type parameter list cannot be empty.";
    pub const EXPECTED_AMBIENT_AFTER_EXPORT_DECLARE: &str = "'export declare' must be followed by an ambient declaration.";
    pub const IMPORT_ALIAS_HAS_IMPORT_TYPE: &str = "An import alias can not use 'import type'.";
    pub const INCOMPATIBLE_MODIFIERS: fn(&str, &str) -> String =
        |a, b| format!("'{a}' modifier cannot be used with '{b}' modifier.");
    pub const INDEX_SIGNATURE_HAS_ABSTRACT: &str = "Index signatures cannot have the 'abstract' modifier.";
    pub const INDEX_SIGNATURE_HAS_ACCESSIBILITY: fn(&str) -> String =
        |m| format!("Index signatures cannot have an accessibility modifier ('{m}').");
    pub const INDEX_SIGNATURE_HAS_DECLARE: &str = "Index signatures cannot have the 'declare' modifier.";
    pub const INDEX_SIGNATURE_HAS_OVERRIDE: &str = "'override' modifier cannot appear on an index signature.";
    pub const INITIALIZER_NOT_ALLOWED_IN_AMBIENT_CONTEXT: &str = "Initializers are not allowed in ambient contexts.";
    pub const INVALID_MODIFIER_ON_TYPE_MEMBER: fn(&str) -> String =
        |m| format!("'{m}' modifier cannot appear on a type member.");
    pub const INVALID_MODIFIER_ON_TYPE_PARAMETER: fn(&str) -> String =
        |m| format!("'{m}' modifier cannot appear on a type parameter.");
    pub const INVALID_MODIFIER_ON_TYPE_PARAMETER_POSITIONS: fn(&str) -> String =
        |m| format!("'{m}' modifier can only appear on a type parameter of a class, interface or type alias.");
    pub const INVALID_MODIFIERS_ORDER: fn(&str, &str) -> String =
        |a, b| format!("'{a}' modifier must precede '{b}' modifier.");
    pub const INVALID_PROPERTY_ACCESS_AFTER_INSTANTIATION_EXPRESSION: &str = "Invalid property access after an instantiation expression. You can either wrap the instantiation expression in parentheses, or delete the type arguments.";
    pub const INVALID_TUPLE_MEMBER_LABEL: &str = "Tuple members must be labeled with a simple identifier.";
    pub const MISSING_INTERFACE_NAME: &str = "'interface' declarations must be followed by an identifier.";
    pub const NON_ABSTRACT_CLASS_HAS_ABSTRACT_METHOD: &str = "Abstract methods can only appear within an abstract class.";
    pub const OPTIONAL_TYPE_BEFORE_REQUIRED: &str = "A required element cannot follow an optional element.";
    pub const OVERRIDE_NOT_IN_SUB_CLASS: &str = "This member cannot have an 'override' modifier because its containing class does not extend another class.";
    pub const PATTERN_IS_OPTIONAL: &str = "A binding pattern parameter cannot be optional in an implementation signature.";
    pub const PRIVATE_ELEMENT_HAS_ABSTRACT: &str = "Private elements cannot have the 'abstract' modifier.";
    pub const PRIVATE_ELEMENT_HAS_ACCESSIBILITY: fn(&str) -> String =
        |m| format!("Private elements cannot have an accessibility modifier ('{m}').");
    pub const PRIVATE_METHODS_HAS_ACCESSIBILITY: fn(&str) -> String =
        |m| format!("Private methods cannot have an accessibility modifier ('{m}').");
    pub const READONLY_FOR_METHOD_SIGNATURE: &str = "'readonly' modifier can only appear on a property declaration or index signature.";
    pub const SET_ACCESOR_CANNOT_HAVE_OPTIONAL_PARAMETER: &str = "A 'set' accessor cannot have an optional parameter.";
    pub const SET_ACCESOR_CANNOT_HAVE_REST_PARAMETER: &str = "A 'set' accessor cannot have rest parameter.";
    pub const SET_ACCESOR_CANNOT_HAVE_RETURN_TYPE: &str = "A 'set' accessor cannot have a return type annotation.";
    pub const STATIC_BLOCK_CANNOT_HAVE_MODIFIER: &str = "Static class blocks cannot have any modifier.";
    pub const TYPE_ANNOTATION_AFTER_ASSIGN: &str = "Type annotations must come before default assignments, e.g. instead of `age = 25: number` use `age: number = 25`.";
    pub const TYPE_IMPORT_CANNOT_SPECIFY_DEFAULT_AND_NAMED: &str = "A type-only import can specify a default import or named bindings, but not both.";
    pub const TYPE_MODIFIER_IS_USED_IN_TYPE_EXPORTS: &str = "The 'type' modifier cannot be used on a named export when 'export type' is used on its export statement.";
    pub const TYPE_MODIFIER_IS_USED_IN_TYPE_IMPORTS: &str = "The 'type' modifier cannot be used on a named import when 'import type' is used on its import statement.";
    pub const UNEXPECTED_PARAMETER_MODIFIER: &str = "A parameter property is only allowed in a constructor implementation.";
    pub const UNEXPECTED_READONLY: &str = "'readonly' type modifier is only permitted on array and tuple literal types.";
    pub const UNEXPECTED_TYPE_ANNOTATION: &str = "Did not expect a type annotation here.";
    pub const UNEXPECTED_TYPE_CAST_IN_PARAMETER: &str = "Unexpected type cast in parameter position.";
    pub const UNSUPPORTED_IMPORT_TYPE_ARGUMENT: &str = "Argument in a type import must be a string literal.";
    pub const UNSUPPORTED_PARAMETER_PROPERTY_KIND: &str = "A parameter property may not be declared using a binding pattern.";
    pub const UNSUPPORTED_SIGNATURE_PARAMETER_KIND: fn(&str) -> String =
        |t| format!("Name in a signature must be an Identifier, ObjectPattern or ArrayPattern, instead got {t}.");

    // DecoratorsError
    pub const UNEXPECTED_LEADING_DECORATOR: &str = "Leading decorators must be attached to a class declaration.";
    pub const DECORATOR_CONSTRUCTOR: &str = "Decorators can't be used with a constructor. Did you mean '@dec class { ... }'?";
    pub const TRAILING_DECORATOR: &str = "Decorators must be attached to a class element.";
    pub const SPREAD_ELEMENT_DECORATOR: &str = "Decorators can't be used with SpreadElement";
}

/// `tokenIsIdentifier`
pub fn token_is_identifier(t: T) -> bool {
    t == T::Name || t.is_ts_keyword()
}

/// `tokenIsKeywordOrIdentifier`
pub fn token_is_keyword_or_identifier(t: T) -> bool {
    t == T::Name || t.is_keyword() || t.is_ts_keyword()
}

/// `tokenIsLiteralPropertyName`
pub fn token_is_literal_property_name(t: T) -> bool {
    t == T::Name || t == T::String || t == T::Num || t.is_keyword() || t.is_ts_keyword()
}

/// `tokenIsTSDeclarationStart`
pub fn token_is_ts_declaration_start(t: T) -> bool {
    matches!(t, T::TsAbstract | T::TsDeclare | T::TsEnum | T::TsModule | T::TsNamespace | T::TsInterface | T::TsType)
}

/// `tokenIsTSTypeOperator`
pub fn token_is_ts_type_operator(t: T) -> bool {
    matches!(t, T::TsKeyof | T::TsReadonly | T::TsUnique)
}

/// `tsIsAccessModifier`
pub fn ts_is_access_modifier(m: &str) -> bool {
    m == "private" || m == "public" || m == "protected"
}

impl<'a> Parser<'a> {
    // --- state access

    pub(crate) fn tss(&self) -> &TsState {
        self.ts.as_deref().expect("TypeScript parser state")
    }

    pub(crate) fn tsm(&mut self) -> &mut TsState {
        self.ts.as_deref_mut().expect("TypeScript parser state")
    }

    /// `this.isLookahead` (false for plain JS).
    pub fn is_ts_lookahead(&self) -> bool {
        self.ts.as_ref().is_some_and(|t| t.is_lookahead)
    }

    pub(crate) fn in_type(&self) -> bool {
        self.tss().in_type
    }

    pub(crate) fn is_ambient_context(&self) -> bool {
        self.tss().is_ambient_context
    }

    /// `this.value` of a name-like token.
    pub(crate) fn value_str(&self) -> &str {
        match &self.value {
            TokVal::Str(s) => s,
            _ => "",
        }
    }

    // --- speculative parsing (src/effects.ts, src/effects-adapter.ts)

    pub(crate) fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            pos: self.pos,
            type_: self.type_,
            value: self.value.clone(),
            value_lone_surrogate: self.value_lone_surrogate,
            start: self.start,
            end: self.end,
            last_tok_start: self.last_tok_start,
            last_tok_end: self.last_tok_end,
            context: self.context.clone(),
            expr_allowed: self.expr_allowed,
            contains_esc: self.contains_esc,
            strict: self.strict,
            potential_arrow_at: self.potential_arrow_at,
            potential_arrow_in_for_await: self.potential_arrow_in_for_await,
            yield_pos: self.yield_pos,
            await_pos: self.await_pos,
            await_ident_pos: self.await_ident_pos,
            labels: self.labels.clone(),
            undefined_exports: self.undefined_exports.clone(),
            scope_stack: self.scope_stack.clone(),
            private_name_stack: self.private_name_stack.clone(),
            in_template_element: self.in_template_element,
            comments_len: self.comments.len(),
            depth: self.depth,
            ts: self.tss().clone(),
        }
    }

    /// Restores `c`; returns the comments reported since (the branch's events).
    pub(crate) fn rollback(&mut self, c: Checkpoint) -> Vec<Comment> {
        self.pos = c.pos;
        self.type_ = c.type_;
        self.value = c.value;
        self.value_lone_surrogate = c.value_lone_surrogate;
        self.start = c.start;
        self.end = c.end;
        self.last_tok_start = c.last_tok_start;
        self.last_tok_end = c.last_tok_end;
        self.context = c.context;
        self.expr_allowed = c.expr_allowed;
        self.contains_esc = c.contains_esc;
        self.strict = c.strict;
        self.potential_arrow_at = c.potential_arrow_at;
        self.potential_arrow_in_for_await = c.potential_arrow_in_for_await;
        self.yield_pos = c.yield_pos;
        self.await_pos = c.await_pos;
        self.await_ident_pos = c.await_ident_pos;
        self.labels = c.labels;
        self.undefined_exports = c.undefined_exports;
        self.scope_stack = c.scope_stack;
        self.private_name_stack = c.private_name_stack;
        self.in_template_element = c.in_template_element;
        self.depth = c.depth;
        **self.ts.as_mut().unwrap() = c.ts;
        self.comments.split_off(c.comments_len)
    }

    /// `tryParse(fn)`: `f` returns `Ok(None)` to `abort()`.
    pub(crate) fn try_parse<N>(&mut self, f: impl FnOnce(&mut Self) -> PResult<Option<N>>) -> PResult<TryParse<N>> {
        let frame = self.checkpoint();
        match f(self) {
            Ok(Some(node)) => Ok(TryParse::Ok(node)),
            Ok(None) => {
                let events = self.rollback(frame);
                Ok(TryParse::Aborted(events))
            }
            Err(e) => {
                let events = self.rollback(frame);
                match *e {
                    PErr::Syntax(_) => Ok(TryParse::Thrown(e, events)),
                    _ => Err(e),
                }
            }
        }
    }

    /// `selectTryParseResult(result)`: replays a failed branch's events.
    pub(crate) fn select_try_parse_result(&mut self, events: Vec<Comment>) {
        self.comments.extend(events);
    }

    /// `tsLookAhead(f)`: runs `f` and always rolls back.
    pub(crate) fn ts_look_ahead<R>(&mut self, f: impl FnOnce(&mut Self) -> PResult<R>) -> PResult<R> {
        let frame = self.checkpoint();
        let r = f(self);
        self.rollback(frame);
        r
    }

    /// `tsTryParse(f)`: keeps the branch when `f` returns a value (`Some`), rolls back otherwise.
    pub(crate) fn ts_try_parse<R>(&mut self, f: impl FnOnce(&mut Self) -> PResult<Option<R>>) -> PResult<Option<R>> {
        let frame = self.checkpoint();
        match f(self) {
            Ok(Some(r)) => Ok(Some(r)),
            Ok(None) => {
                self.rollback(frame);
                Ok(None)
            }
            Err(e) => {
                self.rollback(frame);
                Err(e)
            }
        }
    }

    /// `tsTryParseAndCatch(f)`: `f()` or `undefined` (errors and aborts are swallowed).
    pub(crate) fn ts_try_parse_and_catch<N>(
        &mut self,
        f: impl FnOnce(&mut Self) -> PResult<Option<N>>,
    ) -> PResult<Option<N>> {
        Ok(match self.try_parse(f)? {
            TryParse::Ok(n) => Some(n),
            _ => None,
        })
    }

    /// `lookahead(number)`: the token state after `number` (default 1) more tokens.
    pub(crate) fn lookahead_n(&mut self, number: u32) -> PResult<LookaheadState> {
        let old = (
            self.pos,
            self.type_,
            std::mem::take(&mut self.value),
            self.value_lone_surrogate,
            self.start,
            self.end,
            self.last_tok_start,
            self.last_tok_end,
            self.expr_allowed,
            self.contains_esc,
        );
        let cur = self.cur_context();
        let old_context = std::mem::replace(&mut self.context, cur.into_iter().collect());
        let old_is_lookahead = self.tss().is_lookahead;
        // `createLookaheadState()`: `this.value = null; this.context = [this.curContext()]`
        self.tsm().is_lookahead = true;
        let mut r = Ok(());
        for _ in 0..number.max(1) {
            r = self.next_token();
            if r.is_err() {
                break;
            }
        }
        let state = LookaheadState {
            type_: self.type_,
            value: std::mem::take(&mut self.value),
            start: self.start,
            end: self.end,
            pos: self.pos,
            contains_esc: self.contains_esc,
        };
        (
            self.pos,
            self.type_,
            self.value,
            self.value_lone_surrogate,
            self.start,
            self.end,
            self.last_tok_start,
            self.last_tok_end,
            self.expr_allowed,
            self.contains_esc,
        ) = old;
        self.context = old_context;
        self.tsm().is_lookahead = old_is_lookahead;
        r.map(|_| state)
    }

    pub(crate) fn lookahead(&mut self) -> PResult<LookaheadState> {
        self.lookahead_n(1)
    }

    // --- token helpers

    /// `ts_isContextual(token)`
    pub(crate) fn ts_is_contextual(&self, token: T) -> bool {
        self.type_ == token && !self.contains_esc
    }

    /// `isContextualWithState(keyword, state)`
    pub(crate) fn is_contextual_with_state(keyword: &str, state: &LookaheadState) -> bool {
        state.type_ == T::Name && state.value_str() == keyword && !state.contains_esc
    }

    /// `ts_isContextualWithState(state, token)`
    pub(crate) fn ts_is_contextual_with_state(state: &LookaheadState, token: T) -> bool {
        state.type_ == token && !state.contains_esc
    }

    /// `ts_eatWithState(type, nextCount, state)`
    pub(crate) fn ts_eat_with_state(&mut self, type_: T, next_count: u32, state: &LookaheadState) -> PResult<bool> {
        if type_ == state.type_ {
            for _ in 0..next_count {
                self.next()?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// `ts_eatContextualWithState(name, nextCount, state)`
    pub(crate) fn ts_eat_contextual_with_state(
        &mut self,
        name: &str,
        next_count: u32,
        state: &LookaheadState,
    ) -> PResult<bool> {
        if let Some(t) = super::tokentype::ts_keyword_type(name) {
            if Self::ts_is_contextual_with_state(state, t) {
                for _ in 0..next_count {
                    self.next()?;
                }
                return Ok(true);
            }
            Ok(false)
        } else {
            if !Self::is_contextual_with_state(name, state) {
                return Ok(false);
            }
            for _ in 0..next_count {
                self.next()?;
            }
            Ok(true)
        }
    }

    /// `tsMatchLeftRelational()`
    pub(crate) fn ts_match_left_relational(&self) -> bool {
        self.type_ == T::Relational && self.tok_text() == "<"
    }

    /// `tsMatchRightRelational()`
    pub(crate) fn ts_match_right_relational(&self) -> bool {
        self.type_ == T::Relational && self.tok_text() == ">"
    }

    /// `tsIsIdentifier()`
    pub(crate) fn ts_is_identifier(&self) -> bool {
        token_is_identifier(self.type_)
    }

    /// `isLiteralPropertyName()`
    pub(crate) fn is_literal_property_name(&self) -> bool {
        token_is_literal_property_name(self.type_)
    }

    /// `hasPrecedingLineBreak()`
    pub(crate) fn has_preceding_line_break(&self) -> bool {
        has_line_break(&self.input[self.last_tok_end as usize..self.start as usize])
    }

    /// `hasFollowingLineBreak()`: `skipWhiteSpaceToLineBreak` (sticky) at `this.end`.
    pub(crate) fn has_following_line_break(&self) -> bool {
        // skipWhiteSpaceInLine = /(?:[^\S\n\r  ]|\/\/.*|\/\*.*?\*\/)*/ (atomic), then
        // /(?=[\n\r  ]|\/\*(?!.*?\*\/)|$)/
        let s = &self.input[self.end as usize..];
        let mut i = 0;
        let line_term = |c: char| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}');
        loop {
            let rest = &s[i..];
            let Some(c) = rest.chars().next() else { break };
            if is_js_space(c) && !line_term(c) {
                i += c.len_utf8();
            } else if rest.starts_with("//") {
                let n = rest.find(line_term).unwrap_or(rest.len());
                i += n;
            } else if rest.starts_with("/*") {
                let line_end = rest.find(line_term).unwrap_or(rest.len());
                match rest[2..line_end].find("*/") {
                    Some(k) => i += k + 4,
                    None => break,
                }
            } else {
                break;
            }
        }
        let rest = &s[i..];
        match rest.chars().next() {
            None => true,
            Some(c) if line_term(c) => true,
            _ => {
                if let Some(after) = rest.strip_prefix("/*") {
                    let line_end = after.find(line_term).unwrap_or(after.len());
                    !after[..line_end].contains("*/")
                } else {
                    false
                }
            }
        }
    }

    /// `nextTokenStartSince(pos)`
    pub(crate) fn next_token_start_since(&self, pos: u32) -> u32 {
        skip_white_space(self.input, pos as usize) as u32
    }

    /// `nextTokenStart()`
    pub(crate) fn next_token_start(&self) -> u32 {
        self.next_token_start_since(self.pos)
    }

    /// `lookaheadCharCode()`
    pub(crate) fn lookahead_char_code(&self) -> u32 {
        self.code_at(self.next_token_start())
    }

    /// `isUnparsedContextual(nameStart, name)`
    pub(crate) fn is_unparsed_contextual(&self, name_start: u32, name: &str) -> bool {
        let name_end = name_start as usize + name.len();
        if self.input.get(name_start as usize..name_end) == Some(name) {
            let next_ch = self.code_at(name_end as u32);
            if next_ch == super::tokenize::EOF_CODE {
                return true;
            }
            // `isIdentifierChar(nextCh)` (BMP only) or a high surrogate (astral char)
            return !(super::identifier::is_identifier_char(next_ch, false) || next_ch > 0xFFFF);
        }
        false
    }

    /// `isLookaheadContextual(name)`
    pub(crate) fn is_lookahead_contextual(&self, name: &str) -> bool {
        let next = self.next_token_start();
        self.is_unparsed_contextual(next, name)
    }

    /// `isLineTerminator()`
    pub(crate) fn is_line_terminator(&mut self) -> PResult<bool> {
        Ok(self.eat(T::Semi)? || self.can_insert_semicolon())
    }

    /// `reScan_lt_gt()`: used after we have finished parsing types
    pub(crate) fn re_scan_lt_gt(&mut self) -> PResult<()> {
        if self.type_ == T::Relational {
            self.pos -= 1;
            let code = self.full_char_code_at_pos();
            self.read_token_lt_gt(code)?;
        }
        Ok(())
    }

    /// `reScan_lt()`
    pub(crate) fn re_scan_lt(&mut self) -> PResult<T> {
        let type_ = self.type_;
        if type_ == T::BitShift {
            self.pos -= 2;
            self.finish_op(T::Relational, 1)?;
            return Ok(T::Relational);
        }
        Ok(type_)
    }

    /// The UTF-16 column of `pos` (`loc.column`; the plugin raises at `startLoc.column`).
    pub(crate) fn column_of(&self, pos: u32) -> u32 {
        super::locutil::get_line_info(self.input, pos as usize).column
    }

    // --- tokenizer overrides

    /// `readToken(code)` (the `!inType` prefix; JSX contexts never occur without `jsx`).
    /// Returns true when it produced the token.
    pub fn read_token_ts(&mut self, code: u32) -> bool {
        if !self.in_type() && code == 60 && self.expr_allowed && self.code_at(self.pos + 1) != 33 {
            self.pos += 1;
            // options.jsx is off: `this.finishToken(tt.relational, "<")`
            self.finish_token(T::Relational, TokVal::None);
            return true;
        }
        false
    }

    /// `getTokenFromCode(code)` / `getTokenFromCodeInType(code)`; `None`: fall through to acorn.
    pub fn get_token_from_code_ts(&mut self, code: u32) -> Option<PResult<()>> {
        if self.in_type() {
            if code == 62 || code == 60 {
                return Some(self.finish_op(T::Relational, 1));
            }
            return None;
        }
        if code == 64 {
            self.pos += 1;
            self.finish_token(T::At, TokVal::None);
            return Some(Ok(()));
        }
        None
    }

    // --- scope overrides

    /// `enterScope(flags)`
    pub fn enter_scope_ts(&mut self, flags: u32) {
        let is_type_script_module = flags == TS_SCOPE_TS_MODULE;
        if is_type_script_module {
            self.tsm().imports_stack.push(Vec::new());
        }
        let acorn_flags = if is_type_script_module { flags | SCOPE_CLASS_STATIC_BLOCK } else { flags };
        self.enter_scope_base(acorn_flags);
        self.scope_stack.last_mut().unwrap().ts = Some(Box::default());
    }

    /// `exitScope()`
    pub fn exit_scope_ts(&mut self) {
        if self.current_scope().flags & TS_SCOPE_TS_MODULE != 0 {
            self.tsm().imports_stack.pop();
        }
        self.exit_scope_base();
    }

    /// `hasImport(name, allowShadow)`
    fn has_import(&self, name: &str, allow_shadow: bool) -> bool {
        let stack = &self.tss().imports_stack;
        let len = stack.len();
        if len > 0 && stack[len - 1].iter().any(|n| n == name) {
            return true;
        }
        if !allow_shadow && len > 1 {
            for s in &stack[..len - 1] {
                if s.iter().any(|n| n == name) {
                    return true;
                }
            }
        }
        false
    }

    /// `maybeExportDefined(scope, name)` (on the current scope)
    pub(crate) fn maybe_export_defined(&mut self, name: &str) {
        if self.in_module && self.current_scope().flags & SCOPE_TOP != 0 {
            self.undefined_exports.delete(name);
        }
    }

    fn ts_scope_mut(&mut self) -> &mut TsScope {
        self.scope_stack.last_mut().unwrap().ts.get_or_insert_with(Box::default)
    }

    /// `declareName(name, bindingType, pos)`
    pub fn declare_name_ts(&mut self, name: &str, binding_type: u32, pos: u32) -> PResult<()> {
        if binding_type & BIND_FLAGS_TS_IMPORT != 0 {
            if self.has_import(name, true) {
                return self.raise(pos, format!("Identifier '{name}' has already been declared."));
            }
            if let Some(top) = self.tsm().imports_stack.last_mut() {
                top.push(name.to_string());
            }
            return Ok(());
        }
        if binding_type & BIND_FLAGS_TS_EXPORT_ONLY != 0 {
            self.maybe_export_defined(name);
            self.ts_scope_mut().export_only_bindings.push(name.to_string());
            return Ok(());
        }
        if binding_type == BIND_TS_TYPE || binding_type == BIND_TS_INTERFACE {
            if binding_type == BIND_TS_TYPE && self.ts_scope_mut().types.iter().any(|n| n == name) {
                return self.raise(pos, format!("type '{name}' has already been declared."));
            }
            self.ts_scope_mut().types.push(name.to_string());
        } else if binding_type & BIND_FLAGS_TS_ENUM != 0 {
            if self.ts_scope_mut().enums.iter().any(|n| n == name) {
                return Ok(());
            }
            self.declare_name_base(name, BIND_LEXICAL, pos)?;
        } else {
            self.declare_name_base(name, binding_type, pos)?;
        }
        if binding_type & BIND_FLAGS_TS_ENUM != 0 {
            self.ts_scope_mut().enums.push(name.to_string());
        }
        if binding_type & BIND_FLAGS_TS_CONST_ENUM != 0 {
            self.ts_scope_mut().const_enums.push(name.to_string());
        }
        if binding_type & BIND_FLAGS_CLASS != 0 {
            self.ts_scope_mut().classes.push(name.to_string());
        }
        Ok(())
    }

    /// `checkLocalExport(id)`: true when the TypeScript override returns early.
    pub fn check_local_export_ts(&self, name: &str) -> bool {
        if self.has_import(name, false) {
            return true;
        }
        for scope in self.scope_stack.iter().rev() {
            if let Some(ts) = &scope.ts {
                if ts.types.iter().any(|n| n == name) || ts.export_only_bindings.iter().any(|n| n == name) {
                    return true;
                }
            }
        }
        false
    }

    /// `raise(pos, message)` for "Comma is not permitted after the rest element"
    /// (`raiseCommonCheck`): in an ambient context `(...rest,)` is accepted.
    pub(crate) fn raise_rest_comma(&mut self, pos: u32) -> PResult<()> {
        if self.ts.is_some()
            && self.is_ambient_context()
            && self.type_ == T::Comma
            && self.lookahead_char_code() == 41
        {
            self.next()?;
            return Ok(());
        }
        self.raise_recoverable(pos, "Comma is not permitted after the rest element")
    }
}
