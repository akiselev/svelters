//! Port of acorn 8.19.0 `src/tokentype.js`.
//!
//! ## Token types
//!
//! The assignment of fine-grained, information-carrying type objects
//! allows the tokenizer to store the information it has about a
//! token in a way that is very cheap for the parser to look up.
//!
//! The `beforeExpr` property is used to disambiguate between regular
//! expressions and divisions. It is set on all token types that can
//! be followed by an expression (thus, a slash after them would be a
//! regular expression).
//!
//! The `startsExpr` property is used to check if the token ends a
//! `yield` expression. It is set on all token types that either can
//! directly start an expression (like a quotation mark) or can
//! continue an expression (like the body of a string).
//!
//! `isLoop` marks a keyword as starting a loop, which is important
//! to know when parsing a label, in order to allow or disallow
//! continue jumps to that label.
//!
//! SVELTERS: acorn's `TokenType` objects become a Rust enum; the per-type configuration
//! (`beforeExpr`, `binop`, ...) is answered by `match` tables. `updateContext` hooks live in
//! `tokencontext.rs`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenType {
    Num,
    Regexp,
    String,
    Name,
    PrivateId,
    Eof,

    // Punctuation token types.
    BracketL,
    BracketR,
    BraceL,
    BraceR,
    ParenL,
    ParenR,
    Comma,
    Semi,
    Colon,
    Dot,
    Question,
    QuestionDot,
    Arrow,
    Template,
    InvalidTemplate,
    Ellipsis,
    BackQuote,
    DollarBraceL,

    // Operators. These carry several kinds of properties to help the
    // parser use them properly (the presence of these properties is
    // what categorizes them as operators).
    //
    // `binop`, when present, specifies that this operator is a binary
    // operator, and will refer to its precedence.
    //
    // `prefix` and `postfix` mark the operator as a prefix or postfix
    // unary operator.
    //
    // `isAssign` marks all of `=`, `+=`, `-=` etcetera, which act as
    // binary operators with a very low precedence, that should result
    // in AssignmentExpression nodes.
    Eq,
    Assign,
    IncDec,
    Prefix,
    LogicalOR,
    LogicalAND,
    BitwiseOR,
    BitwiseXOR,
    BitwiseAND,
    Equality,
    Relational,
    BitShift,
    PlusMin,
    Modulo,
    Star,
    Slash,
    Starstar,
    Coalesce,

    // Keyword token types.
    Break,
    Case,
    Catch,
    Continue,
    Debugger,
    Default,
    Do,
    Else,
    Finally,
    For,
    Function,
    If,
    Return,
    Switch,
    Throw,
    Try,
    Var,
    Const,
    While,
    With,
    New,
    This,
    Super,
    Class,
    Extends,
    Export,
    Import,
    Null,
    True,
    False,
    In,
    Instanceof,
    Typeof,
    Void,
    Delete,

    // SVELTERS: the token types acorn-typescript adds (`generateTsKwTokenType`: TypeScript
    // keywords are distinct `TokenType("name", { startsExpr })` objects; `generateTsTokenType`:
    // `@`). Only the TypeScript parser produces them. The JSX token types are not needed (Svelte
    // uses the plugin without `jsx`).
    TsAssert,
    TsAsserts,
    TsGlobal,
    TsKeyof,
    TsReadonly,
    TsUnique,
    TsAbstract,
    TsDeclare,
    TsEnum,
    TsModule,
    TsNamespace,
    TsInterface,
    TsType,
    At,
}

use TokenType as T;

impl TokenType {
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            T::Num => "num",
            T::Regexp => "regexp",
            T::String => "string",
            T::Name => "name",
            T::PrivateId => "privateId",
            T::Eof => "eof",
            T::BracketL => "[",
            T::BracketR => "]",
            T::BraceL => "{",
            T::BraceR => "}",
            T::ParenL => "(",
            T::ParenR => ")",
            T::Comma => ",",
            T::Semi => ";",
            T::Colon => ":",
            T::Dot => ".",
            T::Question => "?",
            T::QuestionDot => "?.",
            T::Arrow => "=>",
            T::Template => "template",
            T::InvalidTemplate => "invalidTemplate",
            T::Ellipsis => "...",
            T::BackQuote => "`",
            T::DollarBraceL => "${",
            T::Eq => "=",
            T::Assign => "_=",
            T::IncDec => "++/--",
            T::Prefix => "!/~",
            T::LogicalOR => "||",
            T::LogicalAND => "&&",
            T::BitwiseOR => "|",
            T::BitwiseXOR => "^",
            T::BitwiseAND => "&",
            T::Equality => "==/!=/===/!==",
            T::Relational => "</>/<=/>=",
            T::BitShift => "<</>>/>>>",
            T::PlusMin => "+/-",
            T::Modulo => "%",
            T::Star => "*",
            T::Slash => "/",
            T::Starstar => "**",
            T::Coalesce => "??",
            T::At => "@",
            t if t.is_ts_keyword() => "name",
            _ => self.keyword().unwrap_or(""),
        }
    }

    /// `type.keyword`: the keyword name for keyword token types.
    pub fn keyword(self) -> Option<&'static str> {
        Some(match self {
            T::Break => "break",
            T::Case => "case",
            T::Catch => "catch",
            T::Continue => "continue",
            T::Debugger => "debugger",
            T::Default => "default",
            T::Do => "do",
            T::Else => "else",
            T::Finally => "finally",
            T::For => "for",
            T::Function => "function",
            T::If => "if",
            T::Return => "return",
            T::Switch => "switch",
            T::Throw => "throw",
            T::Try => "try",
            T::Var => "var",
            T::Const => "const",
            T::While => "while",
            T::With => "with",
            T::New => "new",
            T::This => "this",
            T::Super => "super",
            T::Class => "class",
            T::Extends => "extends",
            T::Export => "export",
            T::Import => "import",
            T::Null => "null",
            T::True => "true",
            T::False => "false",
            T::In => "in",
            T::Instanceof => "instanceof",
            T::Typeof => "typeof",
            T::Void => "void",
            T::Delete => "delete",
            _ => return None,
        })
    }

    pub fn is_keyword(self) -> bool {
        self.keyword().is_some()
    }

    /// One of acorn-typescript's keyword token types (`tsKwTokenTypeValues.includes(type)`).
    pub fn is_ts_keyword(self) -> bool {
        matches!(
            self,
            T::TsAssert
                | T::TsAsserts
                | T::TsGlobal
                | T::TsKeyof
                | T::TsReadonly
                | T::TsUnique
                | T::TsAbstract
                | T::TsDeclare
                | T::TsEnum
                | T::TsModule
                | T::TsNamespace
                | T::TsInterface
                | T::TsType
        )
    }

    pub fn before_expr(self) -> bool {
        matches!(
            self,
            T::BracketL
                | T::BraceL
                | T::ParenL
                | T::Comma
                | T::Semi
                | T::Colon
                | T::Question
                | T::Arrow
                | T::Ellipsis
                | T::DollarBraceL
                | T::Eq
                | T::Assign
                | T::Prefix
                | T::LogicalOR
                | T::LogicalAND
                | T::BitwiseOR
                | T::BitwiseXOR
                | T::BitwiseAND
                | T::Equality
                | T::Relational
                | T::BitShift
                | T::PlusMin
                | T::Modulo
                | T::Star
                | T::Slash
                | T::Starstar
                | T::Coalesce
                | T::Case
                | T::Default
                | T::Do
                | T::Else
                | T::Return
                | T::Throw
                | T::New
                | T::Extends
                | T::In
                | T::Instanceof
                | T::Typeof
                | T::Void
                | T::Delete
        )
    }

    pub fn starts_expr(self) -> bool {
        matches!(
            self,
            T::Num
                | T::Regexp
                | T::String
                | T::Name
                | T::PrivateId
                | T::BracketL
                | T::BraceL
                | T::ParenL
                | T::BackQuote
                | T::DollarBraceL
                | T::IncDec
                | T::Prefix
                | T::PlusMin
                | T::Function
                | T::New
                | T::This
                | T::Super
                | T::Class
                | T::Import
                | T::Null
                | T::True
                | T::False
                | T::Typeof
                | T::Void
                | T::Delete
        ) || self.is_ts_keyword()
    }

    pub fn is_loop(self) -> bool {
        matches!(self, T::Do | T::For | T::While)
    }

    pub fn is_assign(self) -> bool {
        matches!(self, T::Eq | T::Assign)
    }

    pub fn prefix(self) -> bool {
        matches!(self, T::IncDec | T::Prefix | T::PlusMin | T::Typeof | T::Void | T::Delete)
    }

    pub fn postfix(self) -> bool {
        matches!(self, T::IncDec)
    }

    pub fn binop(self) -> Option<i32> {
        Some(match self {
            T::LogicalOR => 1,
            T::LogicalAND => 2,
            T::BitwiseOR => 3,
            T::BitwiseXOR => 4,
            T::BitwiseAND => 5,
            T::Equality => 6,
            T::Relational => 7,
            T::BitShift => 8,
            T::PlusMin => 9,
            T::Modulo => 10,
            T::Star => 10,
            T::Slash => 10,
            T::Coalesce => 1,
            T::In => 7,
            T::Instanceof => 7,
            _ => return None,
        })
    }
}

/// acorn-typescript's `tokTypes[word]` for `keywordsRegExp` words.
pub fn ts_keyword_type(word: &str) -> Option<TokenType> {
    Some(match word {
        "assert" => T::TsAssert,
        "asserts" => T::TsAsserts,
        "global" => T::TsGlobal,
        "keyof" => T::TsKeyof,
        "readonly" => T::TsReadonly,
        "unique" => T::TsUnique,
        "abstract" => T::TsAbstract,
        "declare" => T::TsDeclare,
        "enum" => T::TsEnum,
        "module" => T::TsModule,
        "namespace" => T::TsNamespace,
        "interface" => T::TsInterface,
        "type" => T::TsType,
        _ => return None,
    })
}

/// `keywords[name]`: map keyword names to token types.
pub fn keyword_type(word: &str) -> Option<TokenType> {
    Some(match word {
        "break" => T::Break,
        "case" => T::Case,
        "catch" => T::Catch,
        "continue" => T::Continue,
        "debugger" => T::Debugger,
        "default" => T::Default,
        "do" => T::Do,
        "else" => T::Else,
        "finally" => T::Finally,
        "for" => T::For,
        "function" => T::Function,
        "if" => T::If,
        "return" => T::Return,
        "switch" => T::Switch,
        "throw" => T::Throw,
        "try" => T::Try,
        "var" => T::Var,
        "const" => T::Const,
        "while" => T::While,
        "with" => T::With,
        "new" => T::New,
        "this" => T::This,
        "super" => T::Super,
        "class" => T::Class,
        "extends" => T::Extends,
        "export" => T::Export,
        "import" => T::Import,
        "null" => T::Null,
        "true" => T::True,
        "false" => T::False,
        "in" => T::In,
        "instanceof" => T::Instanceof,
        "typeof" => T::Typeof,
        "void" => T::Void,
        "delete" => T::Delete,
        _ => return None,
    })
}
