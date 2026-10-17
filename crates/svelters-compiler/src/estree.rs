//! Typed ESTree, the JavaScript AST shared by the acorn port (`acorn/`), the Svelte template
//! parser (`svelte/parse/`), scope analysis (`svelte/scope.rs`) and lowering.
//!
//! Shape contract: node and field names follow ESTree exactly as acorn 8.19 emits them with
//! `ecmaVersion: 16, sourceType: "module"` (plus `ParenthesizedExpression` when the parser runs
//! with `preserve_parens`, as Svelte does for template expressions). `serde::Serialize` produces
//! acorn's JSON so the oracle tests can diff against real acorn / `svelte/compiler` output.
//!
//! Positions: `start`/`end` are BYTE offsets into the UTF-8 source. acorn and Svelte use UTF-16
//! offsets; the oracle harness converts before comparing. `loc` is not stored (derive it from
//! offsets with `diag::LineIndex`).
//!
//! Ownership: plain `Box`/`Vec`/`String`, no arena. Good enough for component-sized inputs.

use serde::Serialize;

pub type BoxExpr = Box<Expression>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: u32, end: u32) -> Span {
        Span { start, end }
    }
}

/// Every node type implements this so generic code (walkers, diagnostics) can get its span.
pub trait Spanned {
    fn span(&self) -> Span;
}

macro_rules! node {
    ($(#[$m:meta])* pub struct $name:ident { $($(#[$fm:meta])* pub $f:ident : $t:ty),* $(,)? }) => {
        #[derive(Clone, Debug, PartialEq, Serialize)]
        $(#[$m])*
        pub struct $name {
            pub start: u32,
            pub end: u32,
            $($(#[$fm])* pub $f: $t,)*
        }
        impl Spanned for $name {
            fn span(&self) -> Span { Span { start: self.start, end: self.end } }
        }
    };
}

/// A node whose `Serialize` is written by hand (key sets that depend on the parser mode).
macro_rules! manual_node {
    ($(#[$m:meta])* pub struct $name:ident { $($(#[$fm:meta])* pub $f:ident : $t:ty),* $(,)? }) => {
        #[derive(Clone, Debug, PartialEq)]
        $(#[$m])*
        pub struct $name {
            pub start: u32,
            pub end: u32,
            $($(#[$fm])* pub $f: $t,)*
        }
        impl Spanned for $name {
            fn span(&self) -> Span { Span { start: self.start, end: self.end } }
        }
    };
}

macro_rules! spanned_enum {
    ($name:ident { $($v:ident),* $(,)? }) => {
        impl Spanned for $name {
            fn span(&self) -> Span {
                match self { $($name::$v(n) => n.span(),)* }
            }
        }
    };
}

// ---------------------------------------------------------------------------------------------
// program, comments

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum SourceType {
    #[serde(rename = "module")]
    Module,
    #[serde(rename = "script")]
    Script,
}

node! {
    #[serde(tag = "type", rename = "Program")]
    pub struct Program {
        pub body: Vec<Statement>,
        #[serde(rename = "sourceType")]
        pub source_type: SourceType,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum CommentKind {
    Line,
    Block,
}

/// `onComment` record; `value` excludes the delimiters (`//`, `/* */`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Comment {
    #[serde(rename = "type")]
    pub kind: CommentKind,
    pub value: String,
    pub start: u32,
    pub end: u32,
}

// ---------------------------------------------------------------------------------------------
// statements

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum Statement {
    ExpressionStatement(ExpressionStatement),
    BlockStatement(BlockStatement),
    StaticBlock(StaticBlock),
    EmptyStatement(EmptyStatement),
    DebuggerStatement(DebuggerStatement),
    WithStatement(WithStatement),
    ReturnStatement(ReturnStatement),
    LabeledStatement(LabeledStatement),
    BreakStatement(BreakStatement),
    ContinueStatement(ContinueStatement),
    IfStatement(IfStatement),
    SwitchStatement(SwitchStatement),
    ThrowStatement(ThrowStatement),
    TryStatement(TryStatement),
    WhileStatement(WhileStatement),
    DoWhileStatement(DoWhileStatement),
    ForStatement(ForStatement),
    ForInStatement(ForInStatement),
    ForOfStatement(ForOfStatement),
    FunctionDeclaration(Function),
    VariableDeclaration(VariableDeclaration),
    ClassDeclaration(Class),
    ImportDeclaration(ImportDeclaration),
    ExportNamedDeclaration(ExportNamedDeclaration),
    ExportDefaultDeclaration(ExportDefaultDeclaration),
    ExportAllDeclaration(ExportAllDeclaration),
    // acorn-typescript (see the TypeScript section below)
    TSInterfaceDeclaration(TSInterfaceDeclaration),
    TSTypeAliasDeclaration(TSTypeAliasDeclaration),
    TSEnumDeclaration(TSEnumDeclaration),
    TSModuleDeclaration(TSModuleDeclaration),
    TSDeclareFunction(TSDeclareFunction),
    TSImportEqualsDeclaration(TSImportEqualsDeclaration),
    TSExportAssignment(TSExportAssignment),
    TSNamespaceExportDeclaration(TSNamespaceExportDeclaration),
}
spanned_enum!(Statement {
    ExpressionStatement, BlockStatement, StaticBlock, EmptyStatement, DebuggerStatement, WithStatement,
    ReturnStatement, LabeledStatement, BreakStatement, ContinueStatement, IfStatement, SwitchStatement,
    ThrowStatement, TryStatement, WhileStatement, DoWhileStatement, ForStatement, ForInStatement,
    ForOfStatement, FunctionDeclaration, VariableDeclaration, ClassDeclaration, ImportDeclaration,
    ExportNamedDeclaration, ExportDefaultDeclaration, ExportAllDeclaration, TSInterfaceDeclaration,
    TSTypeAliasDeclaration, TSEnumDeclaration, TSModuleDeclaration, TSDeclareFunction,
    TSImportEqualsDeclaration, TSExportAssignment, TSNamespaceExportDeclaration,
});

node! {
    pub struct ExpressionStatement {
        pub expression: Expression,
        /// Set for directive prologue entries (`"use strict"`): the raw string without quotes.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub directive: Option<String>,
    }
}
node! { pub struct BlockStatement { pub body: Vec<Statement> } }
node! { pub struct StaticBlock { pub body: Vec<Statement> } }
node! { pub struct EmptyStatement {} }
node! { pub struct DebuggerStatement {} }
node! { pub struct WithStatement { pub object: Expression, pub body: Box<Statement> } }
node! { pub struct ReturnStatement { pub argument: Option<Expression> } }
node! { pub struct LabeledStatement { pub label: Identifier, pub body: Box<Statement> } }
node! { pub struct BreakStatement { pub label: Option<Identifier> } }
node! { pub struct ContinueStatement { pub label: Option<Identifier> } }
node! {
    pub struct IfStatement {
        pub test: Expression,
        pub consequent: Box<Statement>,
        pub alternate: Option<Box<Statement>>,
    }
}
node! { pub struct SwitchStatement { pub discriminant: Expression, pub cases: Vec<SwitchCase> } }
node! {
    #[serde(tag = "type", rename = "SwitchCase")]
    pub struct SwitchCase { pub test: Option<Expression>, pub consequent: Vec<Statement> }
}
node! { pub struct ThrowStatement { pub argument: Expression } }
node! {
    pub struct TryStatement {
        #[serde(serialize_with = "ser_block")]
        pub block: BlockStatement,
        pub handler: Option<CatchClause>,
        #[serde(serialize_with = "ser_opt_block")]
        pub finalizer: Option<BlockStatement>,
    }
}
node! {
    #[serde(tag = "type", rename = "CatchClause")]
    pub struct CatchClause {
        pub param: Option<Pattern>,
        #[serde(serialize_with = "ser_block")]
        pub body: BlockStatement,
    }
}

/// Serializes a `BlockStatement` held in a field position with its `type` tag.
#[derive(Serialize)]
#[serde(tag = "type")]
enum BlockStatementRef<'a> {
    BlockStatement(&'a BlockStatement),
}

fn ser_block<S: serde::Serializer>(b: &BlockStatement, s: S) -> Result<S::Ok, S::Error> {
    BlockStatementRef::BlockStatement(b).serialize(s)
}

fn ser_opt_block<S: serde::Serializer>(b: &Option<BlockStatement>, s: S) -> Result<S::Ok, S::Error> {
    match b {
        Some(b) => s.serialize_some(&BlockStatementRef::BlockStatement(b)),
        None => s.serialize_none(),
    }
}
node! { pub struct WhileStatement { pub test: Expression, pub body: Box<Statement> } }
node! { pub struct DoWhileStatement { pub body: Box<Statement>, pub test: Expression } }
node! {
    pub struct ForStatement {
        pub init: Option<ForInit>,
        pub test: Option<Expression>,
        pub update: Option<Expression>,
        pub body: Box<Statement>,
    }
}
node! { pub struct ForInStatement { pub left: ForLeft, pub right: Expression, pub body: Box<Statement> } }
node! {
    pub struct ForOfStatement {
        #[serde(rename = "await")]
        pub is_await: bool,
        pub left: ForLeft,
        pub right: Expression,
        pub body: Box<Statement>,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ForInit {
    VariableDeclaration(VariableDeclarationNode),
    Expression(Expression),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ForLeft {
    VariableDeclaration(VariableDeclarationNode),
    Pattern(Pattern),
}

/// A `VariableDeclaration` in a position where the JSON needs its own `type` tag
/// (outside the `Statement` enum).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum VariableDeclarationNode {
    VariableDeclaration(VariableDeclaration),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum VariableKind {
    #[serde(rename = "var")]
    Var,
    #[serde(rename = "let")]
    Let,
    #[serde(rename = "const")]
    Const,
    #[serde(rename = "using")]
    Using,
    #[serde(rename = "await using")]
    AwaitUsing,
}

node! {
    pub struct VariableDeclaration {
        pub declarations: Vec<VariableDeclarator>,
        pub kind: VariableKind,
        /// acorn-typescript: `declare`.
        #[serde(flatten)]
        pub ts: Option<Box<TsDeclareExtra>>,
    }
}
node! {
    #[serde(tag = "type", rename = "VariableDeclarator")]
    pub struct VariableDeclarator {
        pub id: Pattern,
        pub init: Option<Expression>,
        /// acorn-typescript: `definite` (`let x!: T`).
        #[serde(flatten)]
        pub ts: Option<Box<TsDefiniteExtra>>,
    }
}

// ---------------------------------------------------------------------------------------------
// functions, classes

node! {
    /// FunctionDeclaration / FunctionExpression (the enum variant carries the type name).
    pub struct Function {
        pub id: Option<Identifier>,
        /// Always false for non-arrow functions (acorn emits it anyway).
        pub expression: bool,
        pub generator: bool,
        #[serde(rename = "async")]
        pub is_async: bool,
        pub params: Vec<Pattern>,
        pub body: BlockStatementNode,
        /// acorn-typescript: `typeParameters` / `returnType` / `declare`.
        #[serde(flatten)]
        pub ts: Option<Box<TsFunctionExtra>>,
    }
}

/// A `BlockStatement` in a field position (needs its own `type` tag in JSON).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum BlockStatementNode {
    BlockStatement(BlockStatement),
}

node! {
    pub struct ArrowFunctionExpression {
        pub id: Option<Identifier>,
        /// True when `body` is an expression.
        pub expression: bool,
        pub generator: bool,
        #[serde(rename = "async")]
        pub is_async: bool,
        pub params: Vec<Pattern>,
        pub body: ArrowBody,
        /// acorn-typescript: `typeParameters` / `returnType`.
        #[serde(flatten)]
        pub ts: Option<Box<TsFunctionExtra>>,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ArrowBody {
    Block(BlockStatementNode),
    Expression(BoxExpr),
}

node! {
    /// ClassDeclaration / ClassExpression.
    pub struct Class {
        pub id: Option<Identifier>,
        #[serde(rename = "superClass")]
        pub super_class: Option<BoxExpr>,
        pub body: ClassBody,
        /// acorn-typescript: `typeParameters`, `superTypeParameters`, `implements`, `abstract`,
        /// `declare`, `decorators`.
        #[serde(flatten)]
        pub ts: Option<Box<TsClassExtra>>,
    }
}
node! {
    #[serde(tag = "type", rename = "ClassBody")]
    pub struct ClassBody { pub body: Vec<ClassElement> }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum ClassElement {
    MethodDefinition(MethodDefinition),
    PropertyDefinition(PropertyDefinition),
    StaticBlock(StaticBlock),
    /// acorn-typescript: `[key: string]: T` in a class body.
    TSIndexSignature(TSIndexSignature),
}
spanned_enum!(ClassElement { MethodDefinition, PropertyDefinition, StaticBlock, TSIndexSignature });

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MethodKind {
    Constructor,
    Method,
    Get,
    Set,
}

node! {
    pub struct MethodDefinition {
        #[serde(rename = "static")]
        pub is_static: bool,
        pub computed: bool,
        pub key: PropertyKey,
        pub kind: MethodKind,
        pub value: FunctionNode,
        /// acorn-typescript member modifiers.
        #[serde(flatten)]
        pub ts: Option<Box<TsClassMemberExtra>>,
    }
}
node! {
    pub struct PropertyDefinition {
        #[serde(rename = "static")]
        pub is_static: bool,
        pub computed: bool,
        pub key: PropertyKey,
        pub value: Option<Expression>,
        /// acorn-typescript member modifiers and `typeAnnotation`.
        #[serde(flatten)]
        pub ts: Option<Box<TsClassMemberExtra>>,
    }
}

/// A `FunctionExpression` in a field position.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum FunctionNode {
    FunctionExpression(Function),
    /// acorn-typescript: a method overload without a body.
    TSDeclareMethod(TSDeclareFunction),
}

/// Object/class keys: an expression (Identifier, Literal, or anything when computed) or a
/// `PrivateIdentifier` (class members only).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum PropertyKey {
    Private(PrivateIdentifierNode),
    Expression(BoxExpr),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum PrivateIdentifierNode {
    PrivateIdentifier(PrivateIdentifier),
}

// ---------------------------------------------------------------------------------------------
// modules

manual_node! {
    pub struct ImportDeclaration {
        pub specifiers: Vec<ImportSpecifierKind>,
        pub source: LiteralNode,
        pub attributes: Vec<ImportAttribute>,
        /// acorn-typescript: `importKind`; `attributes` is only present after `with`/`assert`.
        pub ts: Option<Box<TsImportKindExtra>>,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum ImportSpecifierKind {
    ImportSpecifier(ImportSpecifier),
    ImportDefaultSpecifier(ImportDefaultSpecifier),
    ImportNamespaceSpecifier(ImportNamespaceSpecifier),
}
spanned_enum!(ImportSpecifierKind { ImportSpecifier, ImportDefaultSpecifier, ImportNamespaceSpecifier });

node! {
    pub struct ImportSpecifier {
        pub imported: ModuleExportName,
        pub local: Identifier,
        /// acorn-typescript: `importKind`.
        #[serde(flatten)]
        pub ts: Option<Box<TsImportKindExtra>>,
    }
}
node! { pub struct ImportDefaultSpecifier { pub local: Identifier } }
node! { pub struct ImportNamespaceSpecifier { pub local: Identifier } }
node! {
    #[serde(tag = "type", rename = "ImportAttribute")]
    pub struct ImportAttribute { pub key: ModuleExportName, pub value: LiteralNode }
}

/// `Identifier` or string `Literal` (ES2022 arbitrary module namespace names).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum ModuleExportName {
    Identifier(Identifier),
    Literal(Literal),
}

manual_node! {
    pub struct ExportNamedDeclaration {
        pub declaration: Option<Box<Statement>>,
        pub specifiers: Vec<ExportSpecifier>,
        pub source: Option<LiteralNode>,
        pub attributes: Vec<ImportAttribute>,
        /// acorn-typescript: `exportKind`; `attributes` is only present after `with`/`assert`.
        pub ts: Option<Box<TsExportKindExtra>>,
    }
}
node! {
    #[serde(tag = "type", rename = "ExportSpecifier")]
    pub struct ExportSpecifier {
        pub local: ModuleExportName,
        pub exported: ModuleExportName,
        /// acorn-typescript: `exportKind`.
        #[serde(flatten)]
        pub ts: Option<Box<TsExportKindExtra>>,
    }
}
node! {
    pub struct ExportDefaultDeclaration {
        pub declaration: ExportDefaultKind,
        /// acorn-typescript: `exportKind`.
        #[serde(flatten)]
        pub ts: Option<Box<TsExportKindExtra>>,
    }
}

/// `export default function/class` (declaration form, id optional) or an expression.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ExportDefaultKind {
    Declaration(Box<Statement>),
    Expression(Expression),
}

manual_node! {
    pub struct ExportAllDeclaration {
        pub exported: Option<ModuleExportName>,
        pub source: LiteralNode,
        pub attributes: Vec<ImportAttribute>,
        /// acorn-typescript: `exportKind`; `attributes` is only present after `with`/`assert`.
        pub ts: Option<Box<TsExportKindExtra>>,
    }
}

// ---------------------------------------------------------------------------------------------
// expressions

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum Expression {
    Identifier(Identifier),
    Literal(Literal),
    ThisExpression(ThisExpression),
    Super(Super),
    ArrayExpression(ArrayExpression),
    ObjectExpression(ObjectExpression),
    FunctionExpression(Function),
    ArrowFunctionExpression(ArrowFunctionExpression),
    ClassExpression(Class),
    TemplateLiteral(TemplateLiteral),
    TaggedTemplateExpression(TaggedTemplateExpression),
    UnaryExpression(UnaryExpression),
    UpdateExpression(UpdateExpression),
    BinaryExpression(BinaryExpression),
    LogicalExpression(LogicalExpression),
    AssignmentExpression(AssignmentExpression),
    ConditionalExpression(ConditionalExpression),
    CallExpression(CallExpression),
    NewExpression(NewExpression),
    MemberExpression(MemberExpression),
    ChainExpression(ChainExpression),
    SequenceExpression(SequenceExpression),
    YieldExpression(YieldExpression),
    AwaitExpression(AwaitExpression),
    ImportExpression(ImportExpression),
    MetaProperty(MetaProperty),
    ParenthesizedExpression(ParenthesizedExpression),
    // acorn-typescript (see the TypeScript section below)
    TSAsExpression(TSAsExpression),
    TSSatisfiesExpression(TSAsExpression),
    TSTypeAssertion(TSTypeAssertion),
    TSNonNullExpression(TSNonNullExpression),
    TSInstantiationExpression(TSInstantiationExpression),
    /// `(x: T)` inside parentheses that may turn out to be arrow parameters.
    TSTypeCastExpression(TSTypeCastExpression),
}
spanned_enum!(Expression {
    Identifier, Literal, ThisExpression, Super, ArrayExpression, ObjectExpression, FunctionExpression,
    ArrowFunctionExpression, ClassExpression, TemplateLiteral, TaggedTemplateExpression, UnaryExpression,
    UpdateExpression, BinaryExpression, LogicalExpression, AssignmentExpression, ConditionalExpression,
    CallExpression, NewExpression, MemberExpression, ChainExpression, SequenceExpression, YieldExpression,
    AwaitExpression, ImportExpression, MetaProperty, ParenthesizedExpression, TSAsExpression,
    TSSatisfiesExpression, TSTypeAssertion, TSNonNullExpression, TSInstantiationExpression,
    TSTypeCastExpression,
});

node! {
    #[serde(tag = "type", rename = "Identifier")]
    pub struct Identifier {
        pub name: String,
        /// acorn-typescript: `typeAnnotation` / `optional` / `decorators` (None for plain JS).
        #[serde(flatten)]
        pub ts: Option<Box<TsPatternExtra>>,
    }
}
node! {
    #[serde(tag = "type", rename = "PrivateIdentifier")]
    pub struct PrivateIdentifier { pub name: String }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LiteralValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
    /// `value` is a RegExp object in acorn (serialized as `{}` by JSON.stringify); see `regex`.
    RegExp,
    /// `value` is a BigInt in acorn (not JSON-serializable); see `bigint`.
    BigInt,
}

impl Serialize for LiteralValue {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            LiteralValue::String(v) => s.serialize_str(v),
            LiteralValue::Number(v) => s.serialize_f64(*v),
            LiteralValue::Boolean(v) => s.serialize_bool(*v),
            LiteralValue::Null | LiteralValue::BigInt => s.serialize_none(),
            LiteralValue::RegExp => {
                use serde::ser::SerializeMap;
                s.serialize_map(Some(0))?.end()
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RegExpLiteral {
    pub pattern: String,
    pub flags: String,
}

node! {
    #[serde(tag = "type", rename = "Literal")]
    pub struct Literal {
        pub value: LiteralValue,
        pub raw: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub regex: Option<RegExpLiteral>,
        /// Decimal digits of a BigInt literal, without the `n` suffix and separators.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub bigint: Option<String>,
    }
}

/// A string `Literal` in a field position (import sources etc.).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum LiteralNode {
    Literal(Literal),
}

node! { pub struct ThisExpression {} }
node! { pub struct Super {} }
node! {
    pub struct ArrayExpression {
        pub elements: Vec<Option<ArrayElement>>,
        /// acorn-typescript: `typeAnnotation` / `optional` / `decorators` (None for plain JS).
        #[serde(flatten)]
        pub ts: Option<Box<TsPatternExtra>>,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ArrayElement {
    Spread(SpreadElementNode),
    Expression(Expression),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum SpreadElementNode {
    SpreadElement(SpreadElement),
}

node! {
    pub struct SpreadElement {
        pub argument: Expression,
        /// acorn-typescript: `typeAnnotation` / `optional` / `decorators` (None for plain JS).
        #[serde(flatten)]
        pub ts: Option<Box<TsPatternExtra>>,
    }
}
node! {
    pub struct ObjectExpression {
        pub properties: Vec<ObjectMember>,
        /// acorn-typescript: `typeAnnotation` / `optional` / `decorators` (None for plain JS).
        #[serde(flatten)]
        pub ts: Option<Box<TsPatternExtra>>,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum ObjectMember {
    Property(Property),
    SpreadElement(SpreadElement),
}
spanned_enum!(ObjectMember { Property, SpreadElement });

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PropertyKind {
    Init,
    Get,
    Set,
}

node! {
    pub struct Property {
        pub method: bool,
        pub shorthand: bool,
        pub computed: bool,
        pub key: PropertyKey,
        pub value: Expression,
        pub kind: PropertyKind,
    }
}
node! {
    pub struct TemplateLiteral {
        pub expressions: Vec<Expression>,
        pub quasis: Vec<TemplateElement>,
    }
}
node! {
    #[serde(tag = "type", rename = "TemplateElement")]
    pub struct TemplateElement { pub value: TemplateValue, pub tail: bool }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TemplateValue {
    pub raw: String,
    /// None for an invalid escape in a tagged template.
    pub cooked: Option<String>,
}

node! {
    pub struct TaggedTemplateExpression {
        pub tag: BoxExpr,
        pub quasi: TemplateLiteralNode,
        /// acorn-typescript: `typeArguments`.
        #[serde(flatten)]
        pub ts: Option<Box<TsTypeArgumentsExtra>>,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum TemplateLiteralNode {
    TemplateLiteral(TemplateLiteral),
}

node! {
    pub struct UnaryExpression {
        pub operator: UnaryOperator,
        /// Always true (ESTree).
        pub prefix: bool,
        pub argument: BoxExpr,
    }
}
node! {
    pub struct UpdateExpression {
        pub operator: UpdateOperator,
        pub prefix: bool,
        /// Identifier or MemberExpression (or a parenthesized one).
        pub argument: BoxExpr,
    }
}
node! {
    pub struct BinaryExpression {
        pub left: BinaryLeft,
        pub operator: BinaryOperator,
        pub right: BoxExpr,
    }
}

/// `#x in obj` puts a PrivateIdentifier on the left.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum BinaryLeft {
    Private(PrivateIdentifierNode),
    Expression(BoxExpr),
}

node! {
    pub struct LogicalExpression {
        pub left: BoxExpr,
        pub operator: LogicalOperator,
        pub right: BoxExpr,
    }
}
node! {
    pub struct AssignmentExpression {
        pub operator: AssignmentOperator,
        pub left: Pattern,
        pub right: BoxExpr,
    }
}
node! {
    pub struct ConditionalExpression {
        pub test: BoxExpr,
        pub consequent: BoxExpr,
        pub alternate: BoxExpr,
    }
}
manual_node! {
    pub struct CallExpression {
        pub callee: BoxExpr,
        pub arguments: Vec<ArrayElement>,
        pub optional: bool,
        /// acorn-typescript: `typeArguments` (a generic call has no `optional` key unless it is
        /// part of an optional chain).
        pub ts: Option<Box<TsTypeArgumentsExtra>>,
    }
}
node! {
    pub struct NewExpression {
        pub callee: BoxExpr,
        pub arguments: Vec<ArrayElement>,
        /// acorn-typescript: `typeArguments`.
        #[serde(flatten)]
        pub ts: Option<Box<TsTypeArgumentsExtra>>,
    }
}
node! {
    pub struct MemberExpression {
        pub object: BoxExpr,
        pub property: PropertyKey,
        pub computed: bool,
        pub optional: bool,
    }
}
node! { pub struct ChainExpression { pub expression: BoxExpr } }
node! { pub struct SequenceExpression { pub expressions: Vec<Expression> } }
node! {
    pub struct YieldExpression {
        pub delegate: bool,
        pub argument: Option<BoxExpr>,
    }
}
node! { pub struct AwaitExpression { pub argument: BoxExpr } }
manual_node! {
    pub struct ImportExpression {
        pub source: BoxExpr,
        /// The second argument (import attributes), or null.
        pub options: Option<BoxExpr>,
        /// acorn-typescript's `parseDynamicImport`: no `options`; `arguments` after a comma.
        pub ts: Option<Box<TsImportExpressionExtra>>,
    }
}
node! { pub struct MetaProperty { pub meta: Identifier, pub property: Identifier } }
node! { pub struct ParenthesizedExpression { pub expression: BoxExpr } }

// ---------------------------------------------------------------------------------------------
// patterns

/// Binding and assignment targets. `Expression` holds MemberExpression (assignment targets only)
/// and, in non-strict legacy positions, ParenthesizedExpression around a valid target.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum Pattern {
    Identifier(Identifier),
    ObjectPattern(ObjectPattern),
    ArrayPattern(ArrayPattern),
    RestElement(RestElement),
    AssignmentPattern(AssignmentPattern),
    MemberExpression(Box<MemberExpression>),
    ParenthesizedExpression(ParenthesizedExpression),
    // acorn-typescript: parameter properties, and assignment targets that keep their TypeScript
    // wrapper (`(x as T) = v`, `x! = v`).
    TSParameterProperty(TSParameterProperty),
    TSAsExpression(TSPatternWrapper),
    TSSatisfiesExpression(TSPatternWrapper),
    TSTypeAssertion(TSPatternWrapper),
    TSNonNullExpression(TSPatternWrapper),
}
spanned_enum!(Pattern {
    Identifier, ObjectPattern, ArrayPattern, RestElement, AssignmentPattern, MemberExpression,
    ParenthesizedExpression, TSParameterProperty, TSAsExpression, TSSatisfiesExpression, TSTypeAssertion,
    TSNonNullExpression,
});

node! {
    pub struct ObjectPattern {
        pub properties: Vec<ObjectPatternMember>,
        /// acorn-typescript: `typeAnnotation` / `optional` / `decorators` (None for plain JS).
        #[serde(flatten)]
        pub ts: Option<Box<TsPatternExtra>>,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum ObjectPatternMember {
    /// `kind` is always `init`, `method` false; `value` is the target pattern.
    Property(PatternProperty),
    RestElement(RestElement),
}
spanned_enum!(ObjectPatternMember { Property, RestElement });

node! {
    pub struct PatternProperty {
        pub method: bool,
        pub shorthand: bool,
        pub computed: bool,
        pub key: PropertyKey,
        pub value: Pattern,
        pub kind: PropertyKind,
    }
}
node! {
    pub struct ArrayPattern {
        pub elements: Vec<Option<Pattern>>,
        /// acorn-typescript: `typeAnnotation` / `optional` / `decorators` (None for plain JS).
        #[serde(flatten)]
        pub ts: Option<Box<TsPatternExtra>>,
    }
}
node! {
    pub struct RestElement {
        pub argument: Box<Pattern>,
        /// acorn-typescript: `typeAnnotation` / `optional` / `decorators` (None for plain JS).
        #[serde(flatten)]
        pub ts: Option<Box<TsPatternExtra>>,
    }
}
node! {
    pub struct AssignmentPattern {
        pub left: Box<Pattern>,
        pub right: BoxExpr,
        /// acorn-typescript: `typeAnnotation` / `optional` / `decorators` (None for plain JS).
        #[serde(flatten)]
        pub ts: Option<Box<TsPatternExtra>>,
    }
}

// ---------------------------------------------------------------------------------------------
// operators (serialized as their source spelling)

macro_rules! ops {
    ($name:ident { $($v:ident = $s:literal),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
        pub enum $name { $(#[serde(rename = $s)] $v,)* }
        impl $name {
            pub fn as_str(self) -> &'static str { match self { $($name::$v => $s,)* } }
            pub fn from_str(s: &str) -> Option<$name> { match s { $($s => Some($name::$v),)* _ => None } }
        }
    };
}

ops!(UnaryOperator { Minus = "-", Plus = "+", Not = "!", BitNot = "~", Typeof = "typeof", Void = "void", Delete = "delete" });
ops!(UpdateOperator { Increment = "++", Decrement = "--" });
ops!(BinaryOperator {
    Eq = "==", NotEq = "!=", StrictEq = "===", StrictNotEq = "!==", Lt = "<", LtEq = "<=", Gt = ">", GtEq = ">=",
    Shl = "<<", Shr = ">>", UShr = ">>>", Add = "+", Sub = "-", Mul = "*", Div = "/", Rem = "%", Exp = "**",
    BitOr = "|", BitXor = "^", BitAnd = "&", In = "in", Instanceof = "instanceof",
});
ops!(LogicalOperator { Or = "||", And = "&&", Nullish = "??" });
ops!(AssignmentOperator {
    Assign = "=", AddAssign = "+=", SubAssign = "-=", MulAssign = "*=", DivAssign = "/=", RemAssign = "%=",
    ExpAssign = "**=", ShlAssign = "<<=", ShrAssign = ">>=", UShrAssign = ">>>=", BitOrAssign = "|=",
    BitXorAssign = "^=", BitAndAssign = "&=", OrAssign = "||=", AndAssign = "&&=", NullishAssign = "??=",
});

// TypeScript (acorn-typescript) nodes and node properties.
mod ts;
pub use ts::*;
