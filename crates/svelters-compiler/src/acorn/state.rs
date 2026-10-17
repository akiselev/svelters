//! Port of acorn 8.19.0 `src/state.js` (the `Parser` class: constructor, state, getters).

use std::collections::HashMap;

use super::identifier::{
    is_reserved_3, is_reserved_5, is_reserved_6, is_reserved_strict, is_reserved_strict_bind, KeywordSet,
};
use super::options::{get_options, AcornOptions, AcornSourceType, AllowReserved, NormalizedOptions};
use super::scope::Scope;
use super::scopeflags::*;
use super::tokencontext::TokContext;
use super::tokentype::TokenType;
use super::ParseError;
use crate::estree::Comment;

/// Value of the current token (`this.value`).
#[derive(Clone, Debug, Default)]
pub enum TokVal {
    #[default]
    None,
    /// Names, keywords-as-names, strings (cooked), private names.
    Str(String),
    Num(f64),
    /// Decimal digits of a BigInt literal.
    BigInt(String),
    Regex { pattern: String, flags: String },
    /// Cooked template chunk.
    Template(String),
    /// Raw source of an invalid template chunk.
    InvalidTemplate(String),
}

/// acorn throws; the port returns errors. `InvalidTemplateEscape` is acorn's
/// `INVALID_TEMPLATE_ESCAPE_ERROR` sentinel, caught by `tryReadTemplateToken`.
#[derive(Debug)]
pub enum PErr {
    Syntax(ParseError),
    InvalidTemplateEscape,
}

pub type PResult<T> = Result<T, Box<PErr>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelKind {
    Loop,
    Switch,
}

#[derive(Clone, Debug)]
pub struct Label {
    pub name: Option<String>,
    pub kind: Option<LabelKind>,
    pub statement_start: Option<u32>,
}

/// An entry of `privateNameStack`.
#[derive(Clone, Default)]
pub struct PrivateNameEntry {
    pub declared: HashMap<String, &'static str>,
    pub used: Vec<(String, u32)>,
}

/// `this.undefinedExports`: insertion-ordered map name -> (order, start).
#[derive(Clone, Default)]
pub struct UndefinedExports {
    pub map: HashMap<String, (u64, u32)>,
    pub seq: u64,
}

impl UndefinedExports {
    pub fn set(&mut self, name: &str, start: u32) {
        if let Some(e) = self.map.get_mut(name) {
            e.1 = start;
        } else {
            self.seq += 1;
            self.map.insert(name.to_string(), (self.seq, start));
        }
    }
    pub fn delete(&mut self, name: &str) {
        if !self.map.is_empty() {
            self.map.remove(name);
        }
    }
    pub fn clear(&mut self) {
        self.map.clear();
    }
    /// The first entry in insertion order.
    pub fn first(&self) -> Option<(&str, u32)> {
        self.map.iter().min_by_key(|(_, v)| v.0).map(|(k, v)| (k.as_str(), v.1))
    }
}

pub struct Parser<'a> {
    pub options: NormalizedOptions,
    pub input: &'a str,
    pub keywords: KeywordSet,
    /// `reserved` list selector: None when `allowReserved === true`.
    reserved_base: Option<u32>,
    reserved_await: bool,

    // Used to signal to callers of `readWord1` whether the word
    // contained any escape sequences. This is needed because words with
    // escape sequences must not be interpreted as keywords.
    pub contains_esc: bool,

    // Set up token state

    // The current position of the tokenizer in the input.
    pub pos: u32,

    // Properties of the current token:
    // Its type
    pub type_: TokenType,
    // For tokens that include more information than their type, the value
    pub value: TokVal,
    /// SVELTERS: set when a string token's value contained a lone surrogate escape.
    pub value_lone_surrogate: bool,
    // Its start and end offset
    pub start: u32,
    pub end: u32,

    // Position information for the previous token
    pub last_tok_start: u32,
    pub last_tok_end: u32,

    // The context stack is used to superficially track syntactic
    // context to predict whether a regular expression is allowed in a
    // given position.
    pub context: Vec<TokContext>,
    pub expr_allowed: bool,

    // Figure out if it's a module code.
    pub in_module: bool,
    pub strict: bool,

    // Used to signify the start of a potential arrow function
    pub potential_arrow_at: i64,
    pub potential_arrow_in_for_await: bool,

    // Positions to delayed-check that yield/await does not exist in default parameters.
    pub yield_pos: u32,
    pub await_pos: u32,
    pub await_ident_pos: u32,
    // Labels in scope.
    pub labels: Vec<Label>,
    // Thus-far undefined exports.
    pub undefined_exports: UndefinedExports,

    // Scope tracking for duplicate variable names (see scope.js)
    pub scope_stack: Vec<Scope>,

    // The stack of private names.
    // Each element has two properties: 'declared' and 'used'.
    // When it exited from the outermost class definition, all used private names must be declared.
    pub private_name_stack: Vec<PrivateNameEntry>,

    pub in_template_element: bool,

    /// `onComment` target.
    pub comments: &'a mut Vec<Comment>,

    /// SVELTERS: recursion depth guard standing in for acorn's `catchStackOverflow`.
    pub depth: u32,

    /// SVELTERS: the state acorn-typescript's `TypeScriptParser` adds; `Some` when parsing
    /// TypeScript (`options.typescript`), which also switches on its method overrides.
    pub ts: Option<Box<super::typescript::TsState>>,
}

impl<'a> Parser<'a> {
    pub fn new(
        options: &AcornOptions,
        input: &'a str,
        start_pos: u32,
        comments: &'a mut Vec<Comment>,
    ) -> Result<Parser<'a>, ParseError> {
        let options = get_options(options).map_err(|m| ParseError { message: m, pos: 0, raised_at: 0 })?;
        let keywords = if options.ecma_version >= 6 {
            KeywordSet::Es6
        } else if options.source_type == AcornSourceType::Module {
            KeywordSet::Es5Module
        } else {
            KeywordSet::Es5
        };
        let (reserved_base, reserved_await) = if options.allow_reserved != AllowReserved::Yes {
            (
                Some(if options.ecma_version >= 6 {
                    6
                } else if options.ecma_version == 5 {
                    5
                } else {
                    3
                }),
                options.source_type == AcornSourceType::Module,
            )
        } else {
            (None, false)
        };
        let in_module = options.source_type == AcornSourceType::Module;
        let top_flags = if options.source_type == AcornSourceType::CommonJs {
            // In commonjs, the top-level scope behaves like a function scope
            SCOPE_FUNCTION
        } else {
            SCOPE_TOP
        };
        let mut p = Parser {
            keywords,
            reserved_base,
            reserved_await,
            input,
            contains_esc: false,
            pos: start_pos,
            type_: TokenType::Eof,
            value: TokVal::None,
            value_lone_surrogate: false,
            start: start_pos,
            end: start_pos,
            last_tok_start: start_pos,
            last_tok_end: start_pos,
            context: Vec::new(),
            expr_allowed: true,
            in_module,
            strict: false,
            potential_arrow_at: -1,
            potential_arrow_in_for_await: false,
            yield_pos: 0,
            await_pos: 0,
            await_ident_pos: 0,
            labels: Vec::new(),
            undefined_exports: UndefinedExports::default(),
            scope_stack: Vec::new(),
            private_name_stack: Vec::new(),
            in_template_element: false,
            comments,
            depth: 0,
            ts: if options.typescript { Some(Box::default()) } else { None },
            options,
        };
        p.context = p.initial_context();
        p.strict = p.in_module || p.options.strict || p.strict_directive(p.pos);

        // If enabled, skip leading hashbang line.
        if p.pos == 0 && p.options.allow_hash_bang && p.input.starts_with("#!") {
            p.skip_line_comment(2);
        }

        p.enter_scope(top_flags);
        Ok(p)
    }

    // --- reserved word tests (`this.reservedWords`, `reservedWordsStrict`, `reservedWordsStrictBind`)

    fn is_reserved_base(&self, w: &str) -> bool {
        match self.reserved_base {
            None => false,
            Some(v) => {
                (match v {
                    6 => is_reserved_6(w),
                    5 => is_reserved_5(w),
                    _ => is_reserved_3(w),
                }) || (self.reserved_await && w == "await")
            }
        }
    }

    pub fn reserved_words(&self, w: &str) -> bool {
        self.is_reserved_base(w)
    }

    pub fn reserved_words_strict(&self, w: &str) -> bool {
        self.is_reserved_base(w) || is_reserved_strict(w)
    }

    pub fn reserved_words_strict_bind(&self, w: &str) -> bool {
        self.reserved_words_strict(w) || is_reserved_strict_bind(w)
    }

    pub fn is_keyword_word(&self, w: &str) -> bool {
        super::identifier::is_keyword(self.keywords, w)
    }

    // --- getters

    pub fn in_function(&self) -> bool {
        (self.current_var_scope().flags & SCOPE_FUNCTION) > 0
    }

    pub fn in_generator(&self) -> bool {
        (self.current_var_scope().flags & SCOPE_GENERATOR) > 0
    }

    pub fn in_async(&self) -> bool {
        (self.current_var_scope().flags & SCOPE_ASYNC) > 0
    }

    pub fn can_await(&self) -> bool {
        for scope in self.scope_stack.iter().rev() {
            let flags = scope.flags;
            if flags & (SCOPE_CLASS_STATIC_BLOCK | SCOPE_CLASS_FIELD_INIT) != 0 {
                return false;
            }
            if flags & SCOPE_FUNCTION != 0 {
                return (flags & SCOPE_ASYNC) > 0;
            }
        }
        (self.in_module && self.options.ecma_version >= 13) || self.options.allow_await_outside_function
    }

    pub fn allow_return(&self) -> bool {
        if self.in_function() {
            return true;
        }
        if self.options.allow_return_outside_function && self.current_var_scope().flags & SCOPE_TOP != 0 {
            return true;
        }
        false
    }

    pub fn allow_super(&self) -> bool {
        let flags = self.current_this_scope().flags;
        (flags & SCOPE_SUPER) > 0 || self.options.allow_super_outside_method
    }

    pub fn allow_direct_super(&self) -> bool {
        (self.current_this_scope().flags & SCOPE_DIRECT_SUPER) > 0
    }

    pub fn treat_functions_as_var(&self) -> bool {
        self.treat_functions_as_var_in_scope(self.current_scope())
    }

    pub fn allow_new_dot_target(&self) -> bool {
        for scope in self.scope_stack.iter().rev() {
            let flags = scope.flags;
            if flags & (SCOPE_CLASS_STATIC_BLOCK | SCOPE_CLASS_FIELD_INIT) != 0
                || ((flags & SCOPE_FUNCTION) != 0 && (flags & SCOPE_ARROW) == 0)
            {
                return true;
            }
        }
        false
    }

    pub fn allow_using(&self) -> bool {
        let flags = self.current_scope().flags;
        if flags & SCOPE_SWITCH != 0 {
            return false;
        }
        if !self.in_module && flags & SCOPE_TOP != 0 {
            return false;
        }
        true
    }

    pub fn in_class_static_block(&self) -> bool {
        (self.current_var_scope().flags & SCOPE_CLASS_STATIC_BLOCK) > 0
    }
}
