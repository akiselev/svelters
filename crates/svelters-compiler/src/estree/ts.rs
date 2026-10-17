//! TypeScript nodes, as @sveltejs/acorn-typescript 1.0.13 emits them (owner: agent TS).
//!
//! acorn-typescript adds properties to ordinary ESTree nodes (`typeAnnotation` on binding
//! patterns, `typeParameters`/`returnType` on functions, member modifiers on class elements, ...)
//! and only when they apply: plain JS keeps exactly acorn's key set. Those properties live in a
//! `ts: Option<Box<...Extra>>` field of the node (flattened into its JSON, `None` for plain JS);
//! TypeScript-only nodes are the `TS*` types below. Keys acorn-typescript leaves `undefined` are
//! skipped when unset; keys it sets to `null` are serialized as `null`.

use super::*;
use serde::ser::SerializeMap;

fn is_false(b: &bool) -> bool {
    !*b
}

/// `typeAnnotation` / `optional` / `decorators` of a binding pattern (Identifier, ObjectPattern,
/// ArrayPattern, RestElement, AssignmentPattern) or of the expression it is converted from.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct TsPatternExtra {
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation>,
    /// `x?: T` (only ever `true`).
    #[serde(skip_serializing_if = "is_false")]
    pub optional: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorators: Option<Vec<Decorator>>,
}

/// `typeParameters` / `returnType` of a function or arrow; `declare` (only ever `true`).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct TsFunctionExtra {
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    #[serde(rename = "returnType", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation>,
    #[serde(skip_serializing_if = "is_false")]
    pub declare: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct TsClassExtra {
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    #[serde(rename = "superTypeParameters", skip_serializing_if = "Option::is_none")]
    pub super_type_parameters: Option<TSTypeParameterInstantiation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implements: Option<Vec<TSExpressionWithTypeArguments>>,
    #[serde(rename = "abstract", skip_serializing_if = "is_false")]
    pub is_abstract: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub declare: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorators: Option<Vec<Decorator>>,
}

/// Modifiers of a class member (`MethodDefinition`, `PropertyDefinition`). `tsParseModifiers`
/// only ever sets them to `true` (`accessibility` to the keyword).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct TsClassMemberExtra {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility: Option<&'static str>,
    #[serde(skip_serializing_if = "is_false")]
    pub declare: bool,
    #[serde(rename = "abstract", skip_serializing_if = "is_false")]
    pub is_abstract: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub readonly: bool,
    #[serde(rename = "override", skip_serializing_if = "is_false")]
    pub is_override: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub accessor: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub optional: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub definite: bool,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation>,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorators: Option<Vec<Decorator>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct TsDeclareExtra {
    #[serde(skip_serializing_if = "is_false")]
    pub declare: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct TsDefiniteExtra {
    #[serde(skip_serializing_if = "is_false")]
    pub definite: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct TsTypeArgumentsExtra {
    #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    /// CallExpression only: acorn-typescript's generic call sets `optional` only inside an
    /// optional chain.
    #[serde(skip)]
    pub omit_optional: bool,
}

impl Serialize for CallExpression {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(None)?;
        m.serialize_entry("start", &self.start)?;
        m.serialize_entry("end", &self.end)?;
        m.serialize_entry("callee", &self.callee)?;
        m.serialize_entry("arguments", &self.arguments)?;
        if !self.ts.as_ref().is_some_and(|t| t.omit_optional) {
            m.serialize_entry("optional", &self.optional)?;
        }
        if let Some(t) = self.ts.as_ref().and_then(|t| t.type_arguments.as_ref()) {
            m.serialize_entry("typeArguments", t)?;
        }
        m.end()
    }
}

/// `importKind` (`"value"` / `"type"`); `has_attributes`: a `with`/`assert` clause was read
/// (acorn-typescript only sets `attributes` then).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TsImportKindExtra {
    #[serde(rename = "importKind")]
    pub import_kind: &'static str,
    #[serde(skip)]
    pub has_attributes: bool,
}

/// `exportKind`; `has_attributes` as in [`TsImportKindExtra`].
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TsExportKindExtra {
    #[serde(rename = "exportKind", skip_serializing_if = "Option::is_none")]
    pub export_kind: Option<&'static str>,
    #[serde(skip)]
    pub has_attributes: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TsImportExpressionExtra {
    /// `import(source, expr)`: `arguments: [expr]`.
    pub arguments: Option<Vec<Expression>>,
}

// --- hand-written Serialize for the nodes whose key set depends on the parser

impl Serialize for ImportDeclaration {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(None)?;
        m.serialize_entry("start", &self.start)?;
        m.serialize_entry("end", &self.end)?;
        m.serialize_entry("specifiers", &self.specifiers)?;
        m.serialize_entry("source", &self.source)?;
        match &self.ts {
            None => m.serialize_entry("attributes", &self.attributes)?,
            Some(t) => {
                if t.has_attributes {
                    m.serialize_entry("attributes", &self.attributes)?;
                }
                m.serialize_entry("importKind", t.import_kind)?;
            }
        }
        m.end()
    }
}

impl Serialize for ExportNamedDeclaration {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(None)?;
        m.serialize_entry("start", &self.start)?;
        m.serialize_entry("end", &self.end)?;
        m.serialize_entry("declaration", &self.declaration)?;
        m.serialize_entry("specifiers", &self.specifiers)?;
        m.serialize_entry("source", &self.source)?;
        match &self.ts {
            None => m.serialize_entry("attributes", &self.attributes)?,
            Some(t) => {
                if t.has_attributes {
                    m.serialize_entry("attributes", &self.attributes)?;
                }
                if let Some(k) = t.export_kind {
                    m.serialize_entry("exportKind", k)?;
                }
            }
        }
        m.end()
    }
}

impl Serialize for ExportAllDeclaration {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(None)?;
        m.serialize_entry("start", &self.start)?;
        m.serialize_entry("end", &self.end)?;
        m.serialize_entry("exported", &self.exported)?;
        m.serialize_entry("source", &self.source)?;
        match &self.ts {
            None => m.serialize_entry("attributes", &self.attributes)?,
            Some(t) => {
                if t.has_attributes {
                    m.serialize_entry("attributes", &self.attributes)?;
                }
                if let Some(k) = t.export_kind {
                    m.serialize_entry("exportKind", k)?;
                }
            }
        }
        m.end()
    }
}

impl Serialize for ImportExpression {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(None)?;
        m.serialize_entry("start", &self.start)?;
        m.serialize_entry("end", &self.end)?;
        m.serialize_entry("source", &self.source)?;
        match &self.ts {
            None => m.serialize_entry("options", &self.options)?,
            Some(t) => {
                if let Some(a) = &t.arguments {
                    m.serialize_entry("arguments", a)?;
                }
            }
        }
        m.end()
    }
}

// --- annotations, type parameters

node! {
    #[serde(tag = "type", rename = "TSTypeAnnotation")]
    pub struct TSTypeAnnotation {
        #[serde(rename = "typeAnnotation")]
        pub type_annotation: Box<TSType>,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TsTrailingComma {
    /// BYTE offset of the trailing comma.
    #[serde(rename = "trailingComma")]
    pub trailing_comma: u32,
}

node! {
    #[serde(tag = "type", rename = "TSTypeParameterDeclaration")]
    pub struct TSTypeParameterDeclaration {
        pub params: Vec<TSTypeParameter>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub extra: Option<TsTrailingComma>,
    }
}

node! {
    #[serde(tag = "type", rename = "TSTypeParameter")]
    pub struct TSTypeParameter {
        pub name: String,
        #[serde(rename = "in", skip_serializing_if = "is_false")]
        pub is_in: bool,
        #[serde(rename = "out", skip_serializing_if = "is_false")]
        pub is_out: bool,
        #[serde(rename = "const", skip_serializing_if = "is_false")]
        pub is_const: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub constraint: Option<Box<TSType>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub default: Option<Box<TSType>>,
    }
}

node! {
    #[serde(tag = "type", rename = "TSTypeParameterInstantiation")]
    pub struct TSTypeParameterInstantiation { pub params: Vec<TSType> }
}

// --- types

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum TSType {
    TSAnyKeyword(TSKeyword),
    TSBooleanKeyword(TSKeyword),
    TSBigIntKeyword(TSKeyword),
    TSNeverKeyword(TSKeyword),
    TSNumberKeyword(TSKeyword),
    TSObjectKeyword(TSKeyword),
    TSStringKeyword(TSKeyword),
    TSSymbolKeyword(TSKeyword),
    TSUndefinedKeyword(TSKeyword),
    TSUnknownKeyword(TSKeyword),
    TSVoidKeyword(TSKeyword),
    TSNullKeyword(TSKeyword),
    TSIntrinsicKeyword(TSKeyword),
    TSThisType(TSKeyword),
    TSTypeReference(TSTypeReference),
    TSFunctionType(TSFunctionType),
    TSConstructorType(TSFunctionType),
    TSUnionType(TSUnionType),
    TSIntersectionType(TSUnionType),
    TSTypeOperator(TSTypeOperator),
    TSInferType(TSInferType),
    TSLiteralType(TSLiteralType),
    TSImportType(TSImportType),
    TSTypeQuery(TSTypeQuery),
    TSMappedType(TSMappedType),
    TSTypeLiteral(TSTypeLiteral),
    TSTupleType(TSTupleType),
    TSNamedTupleMember(TSNamedTupleMember),
    TSOptionalType(TSWrappedType),
    TSRestType(TSWrappedType),
    TSParenthesizedType(TSWrappedType),
    TSArrayType(TSArrayType),
    TSIndexedAccessType(TSIndexedAccessType),
    TSConditionalType(TSConditionalType),
    TSTypePredicate(TSTypePredicate),
}
spanned_enum!(TSType {
    TSAnyKeyword, TSBooleanKeyword, TSBigIntKeyword, TSNeverKeyword, TSNumberKeyword, TSObjectKeyword,
    TSStringKeyword, TSSymbolKeyword, TSUndefinedKeyword, TSUnknownKeyword, TSVoidKeyword, TSNullKeyword,
    TSIntrinsicKeyword, TSThisType, TSTypeReference, TSFunctionType, TSConstructorType, TSUnionType,
    TSIntersectionType, TSTypeOperator, TSInferType, TSLiteralType, TSImportType, TSTypeQuery,
    TSMappedType, TSTypeLiteral, TSTupleType, TSNamedTupleMember, TSOptionalType, TSRestType,
    TSParenthesizedType, TSArrayType, TSIndexedAccessType, TSConditionalType, TSTypePredicate,
});

impl TSType {
    /// The node's `type`.
    pub fn type_name(&self) -> &'static str {
        match self {
            TSType::TSAnyKeyword(_) => "TSAnyKeyword",
            TSType::TSBooleanKeyword(_) => "TSBooleanKeyword",
            TSType::TSBigIntKeyword(_) => "TSBigIntKeyword",
            TSType::TSNeverKeyword(_) => "TSNeverKeyword",
            TSType::TSNumberKeyword(_) => "TSNumberKeyword",
            TSType::TSObjectKeyword(_) => "TSObjectKeyword",
            TSType::TSStringKeyword(_) => "TSStringKeyword",
            TSType::TSSymbolKeyword(_) => "TSSymbolKeyword",
            TSType::TSUndefinedKeyword(_) => "TSUndefinedKeyword",
            TSType::TSUnknownKeyword(_) => "TSUnknownKeyword",
            TSType::TSVoidKeyword(_) => "TSVoidKeyword",
            TSType::TSNullKeyword(_) => "TSNullKeyword",
            TSType::TSIntrinsicKeyword(_) => "TSIntrinsicKeyword",
            TSType::TSThisType(_) => "TSThisType",
            TSType::TSTypeReference(_) => "TSTypeReference",
            TSType::TSFunctionType(_) => "TSFunctionType",
            TSType::TSConstructorType(_) => "TSConstructorType",
            TSType::TSUnionType(_) => "TSUnionType",
            TSType::TSIntersectionType(_) => "TSIntersectionType",
            TSType::TSTypeOperator(_) => "TSTypeOperator",
            TSType::TSInferType(_) => "TSInferType",
            TSType::TSLiteralType(_) => "TSLiteralType",
            TSType::TSImportType(_) => "TSImportType",
            TSType::TSTypeQuery(_) => "TSTypeQuery",
            TSType::TSMappedType(_) => "TSMappedType",
            TSType::TSTypeLiteral(_) => "TSTypeLiteral",
            TSType::TSTupleType(_) => "TSTupleType",
            TSType::TSNamedTupleMember(_) => "TSNamedTupleMember",
            TSType::TSOptionalType(_) => "TSOptionalType",
            TSType::TSRestType(_) => "TSRestType",
            TSType::TSParenthesizedType(_) => "TSParenthesizedType",
            TSType::TSArrayType(_) => "TSArrayType",
            TSType::TSIndexedAccessType(_) => "TSIndexedAccessType",
            TSType::TSConditionalType(_) => "TSConditionalType",
            TSType::TSTypePredicate(_) => "TSTypePredicate",
        }
    }

    /// Moves the start (acorn-typescript's `resetStartLocation`).
    pub fn set_start(&mut self, start: u32) {
        match self {
            TSType::TSAnyKeyword(n)
            | TSType::TSBooleanKeyword(n)
            | TSType::TSBigIntKeyword(n)
            | TSType::TSNeverKeyword(n)
            | TSType::TSNumberKeyword(n)
            | TSType::TSObjectKeyword(n)
            | TSType::TSStringKeyword(n)
            | TSType::TSSymbolKeyword(n)
            | TSType::TSUndefinedKeyword(n)
            | TSType::TSUnknownKeyword(n)
            | TSType::TSVoidKeyword(n)
            | TSType::TSNullKeyword(n)
            | TSType::TSIntrinsicKeyword(n)
            | TSType::TSThisType(n) => n.start = start,
            TSType::TSTypeReference(n) => n.start = start,
            TSType::TSFunctionType(n) | TSType::TSConstructorType(n) => n.start = start,
            TSType::TSUnionType(n) | TSType::TSIntersectionType(n) => n.start = start,
            TSType::TSTypeOperator(n) => n.start = start,
            TSType::TSInferType(n) => n.start = start,
            TSType::TSLiteralType(n) => n.start = start,
            TSType::TSImportType(n) => n.start = start,
            TSType::TSTypeQuery(n) => n.start = start,
            TSType::TSMappedType(n) => n.start = start,
            TSType::TSTypeLiteral(n) => n.start = start,
            TSType::TSTupleType(n) => n.start = start,
            TSType::TSNamedTupleMember(n) => n.start = start,
            TSType::TSOptionalType(n) | TSType::TSRestType(n) | TSType::TSParenthesizedType(n) => n.start = start,
            TSType::TSArrayType(n) => n.start = start,
            TSType::TSIndexedAccessType(n) => n.start = start,
            TSType::TSConditionalType(n) => n.start = start,
            TSType::TSTypePredicate(n) => n.start = start,
        }
    }
}

node! {
    /// Keyword types and `this` (no properties besides the span).
    pub struct TSKeyword {}
}

/// `Identifier` or `TSQualifiedName`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum TSEntityName {
    Identifier(Identifier),
    TSQualifiedName(Box<TSQualifiedName>),
}
spanned_enum!(TSEntityName { Identifier, TSQualifiedName });

node! { pub struct TSQualifiedName { pub left: TSEntityName, pub right: Identifier } }

node! {
    pub struct TSTypeReference {
        #[serde(rename = "typeName")]
        pub type_name: TSEntityName,
        #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
        pub type_arguments: Option<TSTypeParameterInstantiation>,
    }
}

node! {
    /// TSFunctionType / TSConstructorType (`abstract` is set on constructor types only).
    pub struct TSFunctionType {
        #[serde(rename = "abstract", skip_serializing_if = "Option::is_none")]
        pub is_abstract: Option<bool>,
        #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
        pub type_parameters: Option<TSTypeParameterDeclaration>,
        pub parameters: Vec<Pattern>,
        #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
        pub type_annotation: Option<TSTypeAnnotation>,
    }
}

node! {
    /// TSUnionType / TSIntersectionType.
    pub struct TSUnionType { pub types: Vec<TSType> }
}

node! {
    pub struct TSTypeOperator {
        pub operator: String,
        #[serde(rename = "typeAnnotation")]
        pub type_annotation: Box<TSType>,
    }
}

node! {
    pub struct TSInferType {
        #[serde(rename = "typeParameter")]
        pub type_parameter: TSTypeParameter,
    }
}

node! { pub struct TSLiteralType { pub literal: TSLiteralTypeLiteral } }

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum TSLiteralTypeLiteral {
    Literal(Literal),
    /// `-1`
    UnaryExpression(UnaryExpression),
    /// A template literal type: its `expressions` are types.
    TemplateLiteral(TSTemplateLiteralType),
}

node! {
    pub struct TSTemplateLiteralType {
        pub expressions: Vec<TSType>,
        pub quasis: Vec<TemplateElement>,
    }
}

node! {
    pub struct TSImportType {
        pub argument: Box<Expression>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub qualifier: Option<TSEntityName>,
        #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
        pub type_arguments: Option<TSTypeParameterInstantiation>,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum TSTypeQueryName {
    Identifier(Identifier),
    TSQualifiedName(Box<TSQualifiedName>),
    TSImportType(TSImportType),
}

node! {
    pub struct TSTypeQuery {
        #[serde(rename = "exprName")]
        pub expr_name: TSTypeQueryName,
        #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
        pub type_arguments: Option<TSTypeParameterInstantiation>,
    }
}

/// `readonly` / `optional` of a mapped type: `true`, `"+"` or `"-"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TsModifierSign {
    True,
    Plus,
    Minus,
}

impl Serialize for TsModifierSign {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            TsModifierSign::True => s.serialize_bool(true),
            TsModifierSign::Plus => s.serialize_str("+"),
            TsModifierSign::Minus => s.serialize_str("-"),
        }
    }
}

node! {
    pub struct TSMappedType {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub readonly: Option<TsModifierSign>,
        #[serde(rename = "typeParameter")]
        pub type_parameter: TSTypeParameter,
        #[serde(rename = "nameType")]
        pub name_type: Option<Box<TSType>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub optional: Option<TsModifierSign>,
        #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
        pub type_annotation: Option<Box<TSType>>,
    }
}

node! { pub struct TSTypeLiteral { pub members: Vec<TSTypeElement> } }

node! {
    pub struct TSTupleType {
        #[serde(rename = "elementTypes")]
        pub element_types: Vec<TSType>,
    }
}

node! {
    pub struct TSNamedTupleMember {
        pub label: Identifier,
        pub optional: bool,
        #[serde(rename = "elementType")]
        pub element_type: Box<TSType>,
    }
}

node! {
    /// TSOptionalType / TSRestType / TSParenthesizedType.
    pub struct TSWrappedType {
        #[serde(rename = "typeAnnotation")]
        pub type_annotation: Box<TSType>,
    }
}

node! {
    pub struct TSArrayType {
        #[serde(rename = "elementType")]
        pub element_type: Box<TSType>,
    }
}

node! {
    pub struct TSIndexedAccessType {
        #[serde(rename = "objectType")]
        pub object_type: Box<TSType>,
        #[serde(rename = "indexType")]
        pub index_type: Box<TSType>,
    }
}

node! {
    pub struct TSConditionalType {
        #[serde(rename = "checkType")]
        pub check_type: Box<TSType>,
        #[serde(rename = "extendsType")]
        pub extends_type: Box<TSType>,
        #[serde(rename = "trueType")]
        pub true_type: Box<TSType>,
        #[serde(rename = "falseType")]
        pub false_type: Box<TSType>,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum TSTypePredicateName {
    Identifier(Identifier),
    TSThisType(TSKeyword),
}

node! {
    pub struct TSTypePredicate {
        #[serde(rename = "parameterName")]
        pub parameter_name: TSTypePredicateName,
        /// `null` for `asserts x`.
        #[serde(rename = "typeAnnotation")]
        pub type_annotation: Option<TSTypeAnnotation>,
        pub asserts: bool,
    }
}

// --- type members (interface bodies, type literals)

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum TSTypeElement {
    TSCallSignatureDeclaration(TSSignatureDeclaration),
    TSConstructSignatureDeclaration(TSSignatureDeclaration),
    TSPropertySignature(TSPropertySignature),
    TSMethodSignature(TSMethodSignature),
    TSIndexSignature(TSIndexSignature),
}
spanned_enum!(TSTypeElement {
    TSCallSignatureDeclaration, TSConstructSignatureDeclaration, TSPropertySignature, TSMethodSignature,
    TSIndexSignature,
});

node! {
    /// TSCallSignatureDeclaration / TSConstructSignatureDeclaration.
    pub struct TSSignatureDeclaration {
        #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
        pub type_parameters: Option<TSTypeParameterDeclaration>,
        pub parameters: Vec<Pattern>,
        #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
        pub type_annotation: Option<TSTypeAnnotation>,
    }
}

node! {
    pub struct TSPropertySignature {
        #[serde(skip_serializing_if = "is_false")]
        pub readonly: bool,
        pub computed: bool,
        pub key: PropertyKey,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub kind: Option<&'static str>,
        #[serde(skip_serializing_if = "is_false")]
        pub optional: bool,
        #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
        pub type_annotation: Option<TSTypeAnnotation>,
    }
}

node! {
    pub struct TSMethodSignature {
        #[serde(skip_serializing_if = "is_false")]
        pub readonly: bool,
        pub computed: bool,
        pub key: PropertyKey,
        /// `"method"`, `"get"` or `"set"`.
        pub kind: &'static str,
        #[serde(skip_serializing_if = "is_false")]
        pub optional: bool,
        #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
        pub type_parameters: Option<TSTypeParameterDeclaration>,
        pub parameters: Vec<Pattern>,
        #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
        pub type_annotation: Option<TSTypeAnnotation>,
    }
}

node! {
    pub struct TSIndexSignature {
        #[serde(skip_serializing_if = "is_false")]
        pub readonly: bool,
        #[serde(rename = "static", skip_serializing_if = "is_false")]
        pub is_static: bool,
        pub parameters: Vec<Identifier>,
        #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
        pub type_annotation: Option<TSTypeAnnotation>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub decorators: Option<Vec<Decorator>>,
    }
}

// --- declarations

node! {
    pub struct TSInterfaceDeclaration {
        #[serde(skip_serializing_if = "is_false")]
        pub declare: bool,
        #[serde(rename = "abstract", skip_serializing_if = "is_false")]
        pub is_abstract: bool,
        /// `null` only after the `MissingInterfaceName` error.
        pub id: Option<Identifier>,
        #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
        pub type_parameters: Option<TSTypeParameterDeclaration>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub extends: Option<Vec<TSExpressionWithTypeArguments>>,
        pub body: TSInterfaceBody,
    }
}

node! {
    #[serde(tag = "type", rename = "TSInterfaceBody")]
    pub struct TSInterfaceBody { pub body: Vec<TSTypeElement> }
}

node! {
    #[serde(tag = "type", rename = "TSExpressionWithTypeArguments")]
    pub struct TSExpressionWithTypeArguments {
        pub expression: TSEntityName,
        #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
        pub type_parameters: Option<TSTypeParameterInstantiation>,
    }
}

node! {
    pub struct TSTypeAliasDeclaration {
        #[serde(skip_serializing_if = "is_false")]
        pub declare: bool,
        pub id: Identifier,
        #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
        pub type_parameters: Option<TSTypeParameterDeclaration>,
        #[serde(rename = "typeAnnotation")]
        pub type_annotation: Box<TSType>,
    }
}

node! {
    pub struct TSEnumDeclaration {
        #[serde(rename = "const", skip_serializing_if = "is_false")]
        pub is_const: bool,
        #[serde(skip_serializing_if = "is_false")]
        pub declare: bool,
        pub id: Identifier,
        pub members: Vec<TSEnumMember>,
    }
}

node! {
    #[serde(tag = "type", rename = "TSEnumMember")]
    pub struct TSEnumMember {
        /// Identifier or string Literal.
        pub id: ModuleExportName,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub initializer: Option<Expression>,
    }
}

node! {
    pub struct TSModuleDeclaration {
        #[serde(skip_serializing_if = "is_false")]
        pub declare: bool,
        #[serde(skip_serializing_if = "is_false")]
        pub global: bool,
        /// Identifier or string Literal.
        pub id: ModuleExportName,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<TSModuleBody>,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum TSModuleBody {
    TSModuleBlock(TSModuleBlock),
    /// `namespace a.b {}`: the inner declaration.
    TSModuleDeclaration(Box<TSModuleDeclaration>),
}

node! { pub struct TSModuleBlock { pub body: Vec<Statement> } }

node! {
    /// TSDeclareFunction / TSDeclareMethod: a function without a body.
    pub struct TSDeclareFunction {
        pub id: Option<Identifier>,
        pub expression: bool,
        pub generator: bool,
        #[serde(rename = "async")]
        pub is_async: bool,
        pub params: Vec<Pattern>,
        #[serde(flatten)]
        pub ts: TsFunctionExtra,
    }
}

node! {
    pub struct TSImportEqualsDeclaration {
        #[serde(rename = "importKind")]
        pub import_kind: &'static str,
        #[serde(rename = "isExport")]
        pub is_export: bool,
        pub id: Identifier,
        #[serde(rename = "moduleReference")]
        pub module_reference: TSModuleReference,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum TSModuleReference {
    Identifier(Identifier),
    TSQualifiedName(Box<TSQualifiedName>),
    TSExternalModuleReference(TSExternalModuleReference),
}

node! { pub struct TSExternalModuleReference { pub expression: Expression } }
node! { pub struct TSExportAssignment { pub expression: Expression } }
node! { pub struct TSNamespaceExportDeclaration { pub id: Identifier } }

// --- expressions, patterns

node! {
    /// TSAsExpression / TSSatisfiesExpression.
    pub struct TSAsExpression {
        pub expression: BoxExpr,
        #[serde(rename = "typeAnnotation")]
        pub type_annotation: Box<TSType>,
    }
}

node! {
    pub struct TSTypeAssertion {
        #[serde(rename = "typeAnnotation")]
        pub type_annotation: Box<TSType>,
        pub expression: BoxExpr,
    }
}

node! { pub struct TSNonNullExpression { pub expression: BoxExpr } }

node! {
    pub struct TSInstantiationExpression {
        pub expression: BoxExpr,
        #[serde(rename = "typeArguments")]
        pub type_arguments: TSTypeParameterInstantiation,
    }
}

node! {
    pub struct TSTypeCastExpression {
        pub expression: BoxExpr,
        #[serde(rename = "typeAnnotation")]
        pub type_annotation: TSTypeAnnotation,
    }
}

node! {
    /// An assignment target wrapped in `as` / `satisfies` / `<T>` / `!` (no `typeAnnotation`
    /// for TSNonNullExpression).
    pub struct TSPatternWrapper {
        pub expression: Box<Pattern>,
        #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
        pub type_annotation: Option<Box<TSType>>,
    }
}

node! {
    pub struct TSParameterProperty {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub accessibility: Option<&'static str>,
        #[serde(skip_serializing_if = "is_false")]
        pub readonly: bool,
        #[serde(rename = "override", skip_serializing_if = "is_false")]
        pub is_override: bool,
        /// Identifier or AssignmentPattern.
        pub parameter: Box<Pattern>,
    }
}

node! {
    #[serde(tag = "type", rename = "Decorator")]
    pub struct Decorator { pub expression: Expression }
}
