import TS from "tree-sitter";

export interface CppParser {
    parse(
        input: string | Input,
        previousTree?: Tree,
        options?: { bufferSize?: number; includedRanges?: Range[] },
    ): Tree;
    getLanguage(): any;
    setLanguage(language: any): void;
    getLogger(): Logger;
    setLogger(logFunc: Logger): void;
}

export type Point = {
    row: number;
    column: number;
};

export type Range = {
    startIndex: number;
    endIndex: number;
    startPosition: Point;
    endPosition: Point;
};

export type Edit = {
    startIndex: number;
    oldEndIndex: number;
    newEndIndex: number;
    startPosition: Point;
    oldEndPosition: Point;
    newEndPosition: Point;
};

export type Logger = (
    message: string,
    params: { [param: string]: string },
    type: "parse" | "lex",
) => void;

export interface Input {
    seek(index: number): void;
    read(): any;
}

interface SyntaxNodeBase {
    tree: Tree;
    type: string;
    isNamed: boolean;
    text: string;
    startPosition: Point;
    endPosition: Point;
    startIndex: number;
    endIndex: number;
    parent: SyntaxNode | null;
    children: Array<SyntaxNode>;
    namedChildren: Array<SyntaxNode>;
    childCount: number;
    namedChildCount: number;
    firstChild: SyntaxNode | null;
    firstNamedChild: SyntaxNode | null;
    lastChild: SyntaxNode | null;
    lastNamedChild: SyntaxNode | null;
    nextSibling: SyntaxNode | null;
    nextNamedSibling: SyntaxNode | null;
    previousSibling: SyntaxNode | null;
    previousNamedSibling: SyntaxNode | null;

    hasChanges(): boolean;
    hasError(): boolean;
    isMissing(): boolean;
    toString(): string;
    child(index: number): SyntaxNode | null;
    namedChild(index: number): SyntaxNode | null;
    firstChildForIndex(index: number): SyntaxNode | null;
    firstNamedChildForIndex(index: number): SyntaxNode | null;

    descendantForIndex(index: number): SyntaxNode;
    descendantForIndex(startIndex: number, endIndex: number): SyntaxNode;
    namedDescendantForIndex(index: number): SyntaxNode;
    namedDescendantForIndex(startIndex: number, endIndex: number): SyntaxNode;
    descendantForPosition(position: Point): SyntaxNode;
    descendantForPosition(startPosition: Point, endPosition: Point): SyntaxNode;
    namedDescendantForPosition(position: Point): SyntaxNode;
    namedDescendantForPosition(
        startPosition: Point,
        endPosition: Point,
    ): SyntaxNode;
    descendantsOfType<T extends TypeString>(
        types: T | readonly T[],
        startPosition?: Point,
        endPosition?: Point,
    ): NodeOfType<T>[];

    closest<T extends SyntaxType>(types: T | readonly T[]): NamedNode<T> | null;
    walk(): TreeCursor;
}

export interface TreeCursor {
    nodeType: string;
    nodeText: string;
    nodeIsNamed: boolean;
    startPosition: Point;
    endPosition: Point;
    startIndex: number;
    endIndex: number;
    readonly currentNode: SyntaxNode;

    reset(node: SyntaxNode): void;
    gotoParent(): boolean;
    gotoFirstChild(): boolean;
    gotoFirstChildForIndex(index: number): boolean;
    gotoNextSibling(): boolean;
}

export interface Tree {
    readonly rootNode: SyntaxNode;

    edit(delta: Edit): Tree;
    walk(): TreeCursor;
    getChangedRanges(other: Tree): Range[];
    getEditedRange(other: Tree): Range;
}

interface NamedNodeBase extends SyntaxNodeBase {
    isNamed: true;
}

/** An unnamed node with the given type string. */
export interface UnnamedNode<T extends string = string> extends SyntaxNodeBase {
    type: T;
    isNamed: false;
}

type PickNamedType<Node, T extends string> = Node extends {
    type: T;
    isNamed: true;
}
    ? Node
    : never;

type PickType<Node, T extends string> = Node extends { type: T } ? Node : never;

/** A named node with the given `type` string. */
export type NamedNode<T extends SyntaxType = SyntaxType> = PickNamedType<
    SyntaxNode,
    T
>;

/**
 * A node with the given `type` string.
 *
 * Note that this matches both named and unnamed nodes. Use `NamedNode<T>` to pick only named nodes.
 */
export type NodeOfType<T extends string> = PickType<SyntaxNode, T>;

interface TreeCursorOfType<S extends string, T extends SyntaxNodeBase> {
    nodeType: S;
    currentNode: T;
}

type TreeCursorRecord = {
    [K in TypeString]: TreeCursorOfType<K, NodeOfType<K>>;
};

/**
 * A tree cursor whose `nodeType` correlates with `currentNode`.
 *
 * The typing becomes invalid once the underlying cursor is mutated.
 *
 * The intention is to cast a `TreeCursor` to `TypedTreeCursor` before
 * switching on `nodeType`.
 *
 * For example:
 * ```ts
 * let cursor = root.walk();
 * while (cursor.gotoNextSibling()) {
 *   const c = cursor as TypedTreeCursor;
 *   switch (c.nodeType) {
 *     case SyntaxType.Foo: {
 *       let node = c.currentNode; // Typed as FooNode.
 *       break;
 *     }
 *   }
 * }
 * ```
 */
export type TypedTreeCursor = TreeCursorRecord[keyof TreeCursorRecord];

export interface ErrorNode extends NamedNodeBase {
    type: SyntaxType.ERROR;
    hasError(): true;
}

export const enum SyntaxType {
    ERROR = "ERROR",
    AbstractArrayDeclarator = "abstract_array_declarator",
    AbstractFunctionDeclarator = "abstract_function_declarator",
    AbstractParenthesizedDeclarator = "abstract_parenthesized_declarator",
    AbstractPointerDeclarator = "abstract_pointer_declarator",
    AbstractReferenceDeclarator = "abstract_reference_declarator",
    AccessSpecifier = "access_specifier",
    AliasDeclaration = "alias_declaration",
    ArgumentList = "argument_list",
    ArrayDeclarator = "array_declarator",
    AssignmentExpression = "assignment_expression",
    Attribute = "attribute",
    AttributeSpecifier = "attribute_specifier",
    BaseClassClause = "base_class_clause",
    BinaryExpression = "binary_expression",
    BitfieldClause = "bitfield_clause",
    BreakStatement = "break_statement",
    CallExpression = "call_expression",
    CaseStatement = "case_statement",
    CastExpression = "cast_expression",
    CatchClause = "catch_clause",
    CharLiteral = "char_literal",
    ClassSpecifier = "class_specifier",
    CommaExpression = "comma_expression",
    CompoundLiteralExpression = "compound_literal_expression",
    CompoundStatement = "compound_statement",
    ConcatenatedString = "concatenated_string",
    ConditionClause = "condition_clause",
    ConditionalExpression = "conditional_expression",
    ContinueStatement = "continue_statement",
    Declaration = "declaration",
    DeclarationList = "declaration_list",
    Decltype = "decltype",
    DefaultMethodClause = "default_method_clause",
    DeleteExpression = "delete_expression",
    DeleteMethodClause = "delete_method_clause",
    DependentType = "dependent_type",
    DestructorName = "destructor_name",
    DoStatement = "do_statement",
    EnumSpecifier = "enum_specifier",
    Enumerator = "enumerator",
    EnumeratorList = "enumerator_list",
    ExplicitFunctionSpecifier = "explicit_function_specifier",
    ExpressionStatement = "expression_statement",
    FieldDeclaration = "field_declaration",
    FieldDeclarationList = "field_declaration_list",
    FieldDesignator = "field_designator",
    FieldExpression = "field_expression",
    FieldInitializer = "field_initializer",
    FieldInitializerList = "field_initializer_list",
    ForRangeLoop = "for_range_loop",
    ForStatement = "for_statement",
    FriendDeclaration = "friend_declaration",
    FunctionDeclarator = "function_declarator",
    FunctionDefinition = "function_definition",
    GotoStatement = "goto_statement",
    IfStatement = "if_statement",
    InitDeclarator = "init_declarator",
    InitializerList = "initializer_list",
    InitializerPair = "initializer_pair",
    LabeledStatement = "labeled_statement",
    LambdaCaptureSpecifier = "lambda_capture_specifier",
    LambdaDefaultCapture = "lambda_default_capture",
    LambdaExpression = "lambda_expression",
    LinkageSpecification = "linkage_specification",
    MsBasedModifier = "ms_based_modifier",
    MsCallModifier = "ms_call_modifier",
    MsDeclspecModifier = "ms_declspec_modifier",
    MsPointerModifier = "ms_pointer_modifier",
    MsUnalignedPtrModifier = "ms_unaligned_ptr_modifier",
    NamespaceDefinition = "namespace_definition",
    NewDeclarator = "new_declarator",
    NewExpression = "new_expression",
    Noexcept = "noexcept",
    OperatorCast = "operator_cast",
    OptionalParameterDeclaration = "optional_parameter_declaration",
    OptionalTypeParameterDeclaration = "optional_type_parameter_declaration",
    ParameterDeclaration = "parameter_declaration",
    ParameterList = "parameter_list",
    ParameterPackExpansion = "parameter_pack_expansion",
    ParenthesizedDeclarator = "parenthesized_declarator",
    ParenthesizedExpression = "parenthesized_expression",
    PointerDeclarator = "pointer_declarator",
    PointerExpression = "pointer_expression",
    PreprocCall = "preproc_call",
    PreprocDef = "preproc_def",
    PreprocDefined = "preproc_defined",
    PreprocElif = "preproc_elif",
    PreprocElse = "preproc_else",
    PreprocFunctionDef = "preproc_function_def",
    PreprocIf = "preproc_if",
    PreprocIfdef = "preproc_ifdef",
    PreprocInclude = "preproc_include",
    PreprocParams = "preproc_params",
    ReferenceDeclarator = "reference_declarator",
    ReturnStatement = "return_statement",
    ScopedFieldIdentifier = "scoped_field_identifier",
    ScopedIdentifier = "scoped_identifier",
    ScopedNamespaceIdentifier = "scoped_namespace_identifier",
    ScopedTypeIdentifier = "scoped_type_identifier",
    SizedTypeSpecifier = "sized_type_specifier",
    SizeofExpression = "sizeof_expression",
    StaticAssertDeclaration = "static_assert_declaration",
    StorageClassSpecifier = "storage_class_specifier",
    StringLiteral = "string_literal",
    StructSpecifier = "struct_specifier",
    StructuredBindingDeclarator = "structured_binding_declarator",
    SubscriptDesignator = "subscript_designator",
    SubscriptExpression = "subscript_expression",
    SwitchStatement = "switch_statement",
    TemplateArgumentList = "template_argument_list",
    TemplateDeclaration = "template_declaration",
    TemplateFunction = "template_function",
    TemplateInstantiation = "template_instantiation",
    TemplateMethod = "template_method",
    TemplateParameterList = "template_parameter_list",
    TemplateTemplateParameterDeclaration = "template_template_parameter_declaration",
    TemplateType = "template_type",
    ThrowSpecifier = "throw_specifier",
    ThrowStatement = "throw_statement",
    TrailingReturnType = "trailing_return_type",
    TranslationUnit = "translation_unit",
    TryStatement = "try_statement",
    TypeDefinition = "type_definition",
    TypeDescriptor = "type_descriptor",
    TypeParameterDeclaration = "type_parameter_declaration",
    TypeQualifier = "type_qualifier",
    UnaryExpression = "unary_expression",
    UnionSpecifier = "union_specifier",
    UpdateExpression = "update_expression",
    UsingDeclaration = "using_declaration",
    VariadicDeclarator = "variadic_declarator",
    VariadicParameterDeclaration = "variadic_parameter_declaration",
    VariadicTypeParameterDeclaration = "variadic_type_parameter_declaration",
    VirtualFunctionSpecifier = "virtual_function_specifier",
    VirtualSpecifier = "virtual_specifier",
    WhileStatement = "while_statement",
    Auto = "auto",
    Comment = "comment",
    EscapeSequence = "escape_sequence",
    False = "false",
    FieldIdentifier = "field_identifier",
    Identifier = "identifier",
    MsRestrictModifier = "ms_restrict_modifier",
    MsSignedPtrModifier = "ms_signed_ptr_modifier",
    MsUnsignedPtrModifier = "ms_unsigned_ptr_modifier",
    NamespaceIdentifier = "namespace_identifier",
    Null = "null",
    Nullptr = "nullptr",
    NumberLiteral = "number_literal",
    OperatorName = "operator_name",
    PreprocArg = "preproc_arg",
    PreprocDirective = "preproc_directive",
    PrimitiveType = "primitive_type",
    RawStringLiteral = "raw_string_literal",
    StatementIdentifier = "statement_identifier",
    SystemLibString = "system_lib_string",
    This = "this",
    True = "true",
    TypeIdentifier = "type_identifier",
}

export type UnnamedType =
    | "\n"
    | "!"
    | "!="
    | '"'
    | "#define"
    | "#elif"
    | "#else"
    | "#endif"
    | "#if"
    | "#ifdef"
    | "#ifndef"
    | "#include"
    | "%"
    | "%="
    | "&"
    | "&&"
    | "&="
    | "'"
    | "("
    | ")"
    | "*"
    | "*="
    | "+"
    | "++"
    | "+="
    | ","
    | "-"
    | "--"
    | "-="
    | "->"
    | "."
    | "..."
    | "/"
    | "/="
    | ":"
    | "::"
    | ";"
    | "<"
    | "<<"
    | "<<="
    | "<="
    | "="
    | "=="
    | ">"
    | ">="
    | ">>"
    | ">>="
    | "?"
    | 'L"'
    | "L'"
    | 'U"'
    | "U'"
    | "["
    | "[["
    | "]"
    | "]]"
    | "^"
    | "^="
    | "_Atomic"
    | "__attribute__"
    | "__based"
    | "__cdecl"
    | "__clrcall"
    | "__declspec"
    | "__fastcall"
    | "__stdcall"
    | "__thiscall"
    | "__unaligned"
    | "__vectorcall"
    | "_unaligned"
    | "break"
    | "case"
    | "catch"
    | "class"
    | "const"
    | "constexpr"
    | "continue"
    | SyntaxType.Decltype // both named and unnamed
    | "default"
    | "defined"
    | "delete"
    | "do"
    | "else"
    | "enum"
    | "explicit"
    | "extern"
    | "final"
    | "for"
    | "friend"
    | "goto"
    | "if"
    | "inline"
    | "long"
    | "mutable"
    | "namespace"
    | "new"
    | SyntaxType.Noexcept // both named and unnamed
    | "operator"
    | "override"
    | "private"
    | "protected"
    | "public"
    | "register"
    | "restrict"
    | "return"
    | "short"
    | "signed"
    | "sizeof"
    | "static"
    | "static_assert"
    | "struct"
    | "switch"
    | "template"
    | "throw"
    | "try"
    | "typedef"
    | "typename"
    | 'u"'
    | "u'"
    | 'u8"'
    | "u8'"
    | "union"
    | "unsigned"
    | "using"
    | "virtual"
    | "volatile"
    | "while"
    | "{"
    | "|"
    | "|="
    | "||"
    | "}"
    | "~";

export type TypeString = SyntaxType | UnnamedType;

export type SyntaxNode =
    | AbstractDeclaratorNode
    | DeclaratorNode
    | ExpressionNode
    | FieldDeclaratorNode
    | StatementNode
    | TypeDeclaratorNode
    | TypeSpecifierNode
    | AbstractArrayDeclaratorNode
    | AbstractFunctionDeclaratorNode
    | AbstractParenthesizedDeclaratorNode
    | AbstractPointerDeclaratorNode
    | AbstractReferenceDeclaratorNode
    | AccessSpecifierNode
    | AliasDeclarationNode
    | ArgumentListNode
    | ArrayDeclaratorNode
    | AssignmentExpressionNode
    | AttributeNode
    | AttributeSpecifierNode
    | BaseClassClauseNode
    | BinaryExpressionNode
    | BitfieldClauseNode
    | BreakStatementNode
    | CallExpressionNode
    | CaseStatementNode
    | CastExpressionNode
    | CatchClauseNode
    | CharLiteralNode
    | ClassSpecifierNode
    | CommaExpressionNode
    | CompoundLiteralExpressionNode
    | CompoundStatementNode
    | ConcatenatedStringNode
    | ConditionClauseNode
    | ConditionalExpressionNode
    | ContinueStatementNode
    | DeclarationNode
    | DeclarationListNode
    | DecltypeNode
    | DefaultMethodClauseNode
    | DeleteExpressionNode
    | DeleteMethodClauseNode
    | DependentTypeNode
    | DestructorNameNode
    | DoStatementNode
    | EnumSpecifierNode
    | EnumeratorNode
    | EnumeratorListNode
    | ExplicitFunctionSpecifierNode
    | ExpressionStatementNode
    | FieldDeclarationNode
    | FieldDeclarationListNode
    | FieldDesignatorNode
    | FieldExpressionNode
    | FieldInitializerNode
    | FieldInitializerListNode
    | ForRangeLoopNode
    | ForStatementNode
    | FriendDeclarationNode
    | FunctionDeclaratorNode
    | FunctionDefinitionNode
    | GotoStatementNode
    | IfStatementNode
    | InitDeclaratorNode
    | InitializerListNode
    | InitializerPairNode
    | LabeledStatementNode
    | LambdaCaptureSpecifierNode
    | LambdaDefaultCaptureNode
    | LambdaExpressionNode
    | LinkageSpecificationNode
    | MsBasedModifierNode
    | MsCallModifierNode
    | MsDeclspecModifierNode
    | MsPointerModifierNode
    | MsUnalignedPtrModifierNode
    | NamespaceDefinitionNode
    | NewDeclaratorNode
    | NewExpressionNode
    | NoexceptNode
    | OperatorCastNode
    | OptionalParameterDeclarationNode
    | OptionalTypeParameterDeclarationNode
    | ParameterDeclarationNode
    | ParameterListNode
    | ParameterPackExpansionNode
    | ParenthesizedDeclaratorNode
    | ParenthesizedExpressionNode
    | PointerDeclaratorNode
    | PointerExpressionNode
    | PreprocCallNode
    | PreprocDefNode
    | PreprocDefinedNode
    | PreprocElifNode
    | PreprocElseNode
    | PreprocFunctionDefNode
    | PreprocIfNode
    | PreprocIfdefNode
    | PreprocIncludeNode
    | PreprocParamsNode
    | ReferenceDeclaratorNode
    | ReturnStatementNode
    | ScopedFieldIdentifierNode
    | ScopedIdentifierNode
    | ScopedNamespaceIdentifierNode
    | ScopedTypeIdentifierNode
    | SizedTypeSpecifierNode
    | SizeofExpressionNode
    | StaticAssertDeclarationNode
    | StorageClassSpecifierNode
    | StringLiteralNode
    | StructSpecifierNode
    | StructuredBindingDeclaratorNode
    | SubscriptDesignatorNode
    | SubscriptExpressionNode
    | SwitchStatementNode
    | TemplateArgumentListNode
    | TemplateDeclarationNode
    | TemplateFunctionNode
    | TemplateInstantiationNode
    | TemplateMethodNode
    | TemplateParameterListNode
    | TemplateTemplateParameterDeclarationNode
    | TemplateTypeNode
    | ThrowSpecifierNode
    | ThrowStatementNode
    | TrailingReturnTypeNode
    | TranslationUnitNode
    | TryStatementNode
    | TypeDefinitionNode
    | TypeDescriptorNode
    | TypeParameterDeclarationNode
    | TypeQualifierNode
    | UnaryExpressionNode
    | UnionSpecifierNode
    | UpdateExpressionNode
    | UsingDeclarationNode
    | VariadicDeclaratorNode
    | VariadicParameterDeclarationNode
    | VariadicTypeParameterDeclarationNode
    | VirtualFunctionSpecifierNode
    | VirtualSpecifierNode
    | WhileStatementNode
    | UnnamedNode<"\n">
    | UnnamedNode<"!">
    | UnnamedNode<"!=">
    | UnnamedNode<'"'>
    | UnnamedNode<"#define">
    | UnnamedNode<"#elif">
    | UnnamedNode<"#else">
    | UnnamedNode<"#endif">
    | UnnamedNode<"#if">
    | UnnamedNode<"#ifdef">
    | UnnamedNode<"#ifndef">
    | UnnamedNode<"#include">
    | UnnamedNode<"%">
    | UnnamedNode<"%=">
    | UnnamedNode<"&">
    | UnnamedNode<"&&">
    | UnnamedNode<"&=">
    | UnnamedNode<"'">
    | UnnamedNode<"(">
    | UnnamedNode<")">
    | UnnamedNode<"*">
    | UnnamedNode<"*=">
    | UnnamedNode<"+">
    | UnnamedNode<"++">
    | UnnamedNode<"+=">
    | UnnamedNode<",">
    | UnnamedNode<"-">
    | UnnamedNode<"--">
    | UnnamedNode<"-=">
    | UnnamedNode<"->">
    | UnnamedNode<".">
    | UnnamedNode<"...">
    | UnnamedNode<"/">
    | UnnamedNode<"/=">
    | UnnamedNode<":">
    | UnnamedNode<"::">
    | UnnamedNode<";">
    | UnnamedNode<"<">
    | UnnamedNode<"<<">
    | UnnamedNode<"<<=">
    | UnnamedNode<"<=">
    | UnnamedNode<"=">
    | UnnamedNode<"==">
    | UnnamedNode<">">
    | UnnamedNode<">=">
    | UnnamedNode<">>">
    | UnnamedNode<">>=">
    | UnnamedNode<"?">
    | UnnamedNode<'L"'>
    | UnnamedNode<"L'">
    | UnnamedNode<'U"'>
    | UnnamedNode<"U'">
    | UnnamedNode<"[">
    | UnnamedNode<"[[">
    | UnnamedNode<"]">
    | UnnamedNode<"]]">
    | UnnamedNode<"^">
    | UnnamedNode<"^=">
    | UnnamedNode<"_Atomic">
    | UnnamedNode<"__attribute__">
    | UnnamedNode<"__based">
    | UnnamedNode<"__cdecl">
    | UnnamedNode<"__clrcall">
    | UnnamedNode<"__declspec">
    | UnnamedNode<"__fastcall">
    | UnnamedNode<"__stdcall">
    | UnnamedNode<"__thiscall">
    | UnnamedNode<"__unaligned">
    | UnnamedNode<"__vectorcall">
    | UnnamedNode<"_unaligned">
    | AutoNode
    | UnnamedNode<"break">
    | UnnamedNode<"case">
    | UnnamedNode<"catch">
    | UnnamedNode<"class">
    | CommentNode
    | UnnamedNode<"const">
    | UnnamedNode<"constexpr">
    | UnnamedNode<"continue">
    | UnnamedNode<SyntaxType.Decltype>
    | UnnamedNode<"default">
    | UnnamedNode<"defined">
    | UnnamedNode<"delete">
    | UnnamedNode<"do">
    | UnnamedNode<"else">
    | UnnamedNode<"enum">
    | EscapeSequenceNode
    | UnnamedNode<"explicit">
    | UnnamedNode<"extern">
    | FalseNode
    | FieldIdentifierNode
    | UnnamedNode<"final">
    | UnnamedNode<"for">
    | UnnamedNode<"friend">
    | UnnamedNode<"goto">
    | IdentifierNode
    | UnnamedNode<"if">
    | UnnamedNode<"inline">
    | UnnamedNode<"long">
    | MsRestrictModifierNode
    | MsSignedPtrModifierNode
    | MsUnsignedPtrModifierNode
    | UnnamedNode<"mutable">
    | UnnamedNode<"namespace">
    | NamespaceIdentifierNode
    | UnnamedNode<"new">
    | UnnamedNode<SyntaxType.Noexcept>
    | NullNode
    | NullptrNode
    | NumberLiteralNode
    | UnnamedNode<"operator">
    | OperatorNameNode
    | UnnamedNode<"override">
    | PreprocArgNode
    | PreprocDirectiveNode
    | PrimitiveTypeNode
    | UnnamedNode<"private">
    | UnnamedNode<"protected">
    | UnnamedNode<"public">
    | RawStringLiteralNode
    | UnnamedNode<"register">
    | UnnamedNode<"restrict">
    | UnnamedNode<"return">
    | UnnamedNode<"short">
    | UnnamedNode<"signed">
    | UnnamedNode<"sizeof">
    | StatementIdentifierNode
    | UnnamedNode<"static">
    | UnnamedNode<"static_assert">
    | UnnamedNode<"struct">
    | UnnamedNode<"switch">
    | SystemLibStringNode
    | UnnamedNode<"template">
    | ThisNode
    | UnnamedNode<"throw">
    | TrueNode
    | UnnamedNode<"try">
    | TypeIdentifierNode
    | UnnamedNode<"typedef">
    | UnnamedNode<"typename">
    | UnnamedNode<'u"'>
    | UnnamedNode<"u'">
    | UnnamedNode<'u8"'>
    | UnnamedNode<"u8'">
    | UnnamedNode<"union">
    | UnnamedNode<"unsigned">
    | UnnamedNode<"using">
    | UnnamedNode<"virtual">
    | UnnamedNode<"volatile">
    | UnnamedNode<"while">
    | UnnamedNode<"{">
    | UnnamedNode<"|">
    | UnnamedNode<"|=">
    | UnnamedNode<"||">
    | UnnamedNode<"}">
    | UnnamedNode<"~">
    | ErrorNode;

export type AbstractDeclaratorNode =
    | AbstractArrayDeclaratorNode
    | AbstractFunctionDeclaratorNode
    | AbstractParenthesizedDeclaratorNode
    | AbstractPointerDeclaratorNode
    | AbstractReferenceDeclaratorNode;

export type DeclaratorNode =
    | ArrayDeclaratorNode
    | DestructorNameNode
    | FunctionDeclaratorNode
    | IdentifierNode
    | OperatorNameNode
    | ParenthesizedDeclaratorNode
    | PointerDeclaratorNode
    | ReferenceDeclaratorNode
    | ScopedIdentifierNode
    | StructuredBindingDeclaratorNode
    | TemplateFunctionNode;

export type ExpressionNode =
    | AssignmentExpressionNode
    | BinaryExpressionNode
    | CallExpressionNode
    | CastExpressionNode
    | CharLiteralNode
    | CompoundLiteralExpressionNode
    | ConcatenatedStringNode
    | ConditionalExpressionNode
    | DeleteExpressionNode
    | FalseNode
    | FieldExpressionNode
    | IdentifierNode
    | LambdaExpressionNode
    | NewExpressionNode
    | NullNode
    | NullptrNode
    | NumberLiteralNode
    | ParameterPackExpansionNode
    | ParenthesizedExpressionNode
    | PointerExpressionNode
    | RawStringLiteralNode
    | ScopedIdentifierNode
    | SizeofExpressionNode
    | StringLiteralNode
    | SubscriptExpressionNode
    | TemplateFunctionNode
    | ThisNode
    | TrueNode
    | UnaryExpressionNode
    | UpdateExpressionNode;

export type FieldDeclaratorNode =
    | ArrayDeclaratorNode
    | FieldIdentifierNode
    | FunctionDeclaratorNode
    | OperatorNameNode
    | ParenthesizedDeclaratorNode
    | PointerDeclaratorNode
    | ReferenceDeclaratorNode
    | TemplateMethodNode;

export type StatementNode =
    | BreakStatementNode
    | CaseStatementNode
    | CompoundStatementNode
    | ContinueStatementNode
    | DoStatementNode
    | ExpressionStatementNode
    | ForRangeLoopNode
    | ForStatementNode
    | GotoStatementNode
    | IfStatementNode
    | LabeledStatementNode
    | ReturnStatementNode
    | SwitchStatementNode
    | ThrowStatementNode
    | TryStatementNode
    | WhileStatementNode;

export type TypeDeclaratorNode =
    | ArrayDeclaratorNode
    | FunctionDeclaratorNode
    | ParenthesizedDeclaratorNode
    | PointerDeclaratorNode
    | TypeIdentifierNode;

export type TypeSpecifierNode =
    | AutoNode
    | ClassSpecifierNode
    | DecltypeNode
    | DependentTypeNode
    | EnumSpecifierNode
    | PrimitiveTypeNode
    | ScopedTypeIdentifierNode
    | SizedTypeSpecifierNode
    | StructSpecifierNode
    | TemplateTypeNode
    | TypeIdentifierNode
    | UnionSpecifierNode;

export interface AbstractArrayDeclaratorNode extends NamedNodeBase {
    type: SyntaxType.AbstractArrayDeclarator;
    declaratorNode?: AbstractDeclaratorNode;
    sizeNode?: UnnamedNode<"*"> | ExpressionNode;
}

export interface AbstractFunctionDeclaratorNode extends NamedNodeBase {
    type: SyntaxType.AbstractFunctionDeclarator;
    declaratorNode?: AbstractDeclaratorNode;
    parametersNode: ParameterListNode;
}

export interface AbstractParenthesizedDeclaratorNode extends NamedNodeBase {
    type: SyntaxType.AbstractParenthesizedDeclarator;
}

export interface AbstractPointerDeclaratorNode extends NamedNodeBase {
    type: SyntaxType.AbstractPointerDeclarator;
    declaratorNode?: AbstractDeclaratorNode;
}

export interface AbstractReferenceDeclaratorNode extends NamedNodeBase {
    type: SyntaxType.AbstractReferenceDeclarator;
}

export interface AccessSpecifierNode extends NamedNodeBase {
    type: SyntaxType.AccessSpecifier;
}

export interface AliasDeclarationNode extends NamedNodeBase {
    type: SyntaxType.AliasDeclaration;
    nameNode: TypeIdentifierNode;
    typeNode: TypeDescriptorNode;
}

export interface ArgumentListNode extends NamedNodeBase {
    type: SyntaxType.ArgumentList;
}

export interface ArrayDeclaratorNode extends NamedNodeBase {
    type: SyntaxType.ArrayDeclarator;
    declaratorNode: DeclaratorNode | FieldDeclaratorNode | TypeDeclaratorNode;
    sizeNode?: UnnamedNode<"*"> | ExpressionNode;
}

export interface AssignmentExpressionNode extends NamedNodeBase {
    type: SyntaxType.AssignmentExpression;
    leftNode:
        | CallExpressionNode
        | FieldExpressionNode
        | IdentifierNode
        | ParenthesizedExpressionNode
        | PointerExpressionNode
        | ScopedNamespaceIdentifierNode
        | SubscriptExpressionNode;
    rightNode: ExpressionNode;
}

export interface AttributeNode extends NamedNodeBase {
    type: SyntaxType.Attribute;
}

export interface AttributeSpecifierNode extends NamedNodeBase {
    type: SyntaxType.AttributeSpecifier;
}

export interface BaseClassClauseNode extends NamedNodeBase {
    type: SyntaxType.BaseClassClause;
}

export interface BinaryExpressionNode extends NamedNodeBase {
    type: SyntaxType.BinaryExpression;
    leftNode: ExpressionNode | PreprocDefinedNode;
    operatorNode:
        | UnnamedNode<"!=">
        | UnnamedNode<"%">
        | UnnamedNode<"&">
        | UnnamedNode<"&&">
        | UnnamedNode<"*">
        | UnnamedNode<"+">
        | UnnamedNode<"-">
        | UnnamedNode<"/">
        | UnnamedNode<"<">
        | UnnamedNode<"<<">
        | UnnamedNode<"<=">
        | UnnamedNode<"==">
        | UnnamedNode<">">
        | UnnamedNode<">=">
        | UnnamedNode<">>">
        | UnnamedNode<"^">
        | UnnamedNode<"|">
        | UnnamedNode<"||">;
    rightNode: ExpressionNode | PreprocDefinedNode;
}

export interface BitfieldClauseNode extends NamedNodeBase {
    type: SyntaxType.BitfieldClause;
}

export interface BreakStatementNode extends NamedNodeBase {
    type: SyntaxType.BreakStatement;
}

export interface CallExpressionNode extends NamedNodeBase {
    type: SyntaxType.CallExpression;
    argumentsNode: ArgumentListNode;
    functionNode: ExpressionNode | PrimitiveTypeNode;
}

export interface CaseStatementNode extends NamedNodeBase {
    type: SyntaxType.CaseStatement;
    valueNode?: ExpressionNode;
}

export interface CastExpressionNode extends NamedNodeBase {
    type: SyntaxType.CastExpression;
    typeNode: TypeDescriptorNode;
    valueNode: ExpressionNode;
}

export interface CatchClauseNode extends NamedNodeBase {
    type: SyntaxType.CatchClause;
    bodyNode: CompoundStatementNode;
    parametersNode: ParameterListNode;
}

export interface CharLiteralNode extends NamedNodeBase {
    type: SyntaxType.CharLiteral;
}

export interface ClassSpecifierNode extends NamedNodeBase {
    type: SyntaxType.ClassSpecifier;
    bodyNode?: FieldDeclarationListNode;
    nameNode?: ScopedTypeIdentifierNode | TemplateTypeNode | TypeIdentifierNode;
}

export interface CommaExpressionNode extends NamedNodeBase {
    type: SyntaxType.CommaExpression;
    leftNode: ExpressionNode;
    rightNode: ExpressionNode | CommaExpressionNode;
}

export interface CompoundLiteralExpressionNode extends NamedNodeBase {
    type: SyntaxType.CompoundLiteralExpression;
    typeNode:
        | ScopedTypeIdentifierNode
        | TemplateTypeNode
        | TypeDescriptorNode
        | TypeIdentifierNode;
    valueNode: InitializerListNode;
}

export interface CompoundStatementNode extends NamedNodeBase {
    type: SyntaxType.CompoundStatement;
}

export interface ConcatenatedStringNode extends NamedNodeBase {
    type: SyntaxType.ConcatenatedString;
}

export interface ConditionClauseNode extends NamedNodeBase {
    type: SyntaxType.ConditionClause;
    initializerNode?: DeclarationNode | ExpressionStatementNode;
    valueNode: ExpressionNode | CommaExpressionNode | DeclarationNode;
}

export interface ConditionalExpressionNode extends NamedNodeBase {
    type: SyntaxType.ConditionalExpression;
    alternativeNode: ExpressionNode;
    conditionNode: ExpressionNode;
    consequenceNode: ExpressionNode;
}

export interface ContinueStatementNode extends NamedNodeBase {
    type: SyntaxType.ContinueStatement;
}

export interface DeclarationNode extends NamedNodeBase {
    type: SyntaxType.Declaration;
    declaratorNodes: (DeclaratorNode | InitDeclaratorNode | OperatorCastNode)[];
    default_valueNode?: ExpressionNode;
    typeNode?: TypeSpecifierNode;
    valueNode?: ExpressionNode | InitializerListNode;
}

export interface DeclarationListNode extends NamedNodeBase {
    type: SyntaxType.DeclarationList;
}

export interface DecltypeNode extends NamedNodeBase {
    type: SyntaxType.Decltype;
}

export interface DefaultMethodClauseNode extends NamedNodeBase {
    type: SyntaxType.DefaultMethodClause;
}

export interface DeleteExpressionNode extends NamedNodeBase {
    type: SyntaxType.DeleteExpression;
}

export interface DeleteMethodClauseNode extends NamedNodeBase {
    type: SyntaxType.DeleteMethodClause;
}

export interface DependentTypeNode extends NamedNodeBase {
    type: SyntaxType.DependentType;
}

export interface DestructorNameNode extends NamedNodeBase {
    type: SyntaxType.DestructorName;
}

export interface DoStatementNode extends NamedNodeBase {
    type: SyntaxType.DoStatement;
    bodyNode: StatementNode;
    conditionNode: ParenthesizedExpressionNode;
}

export interface EnumSpecifierNode extends NamedNodeBase {
    type: SyntaxType.EnumSpecifier;
    baseNode?:
        | ScopedTypeIdentifierNode
        | SizedTypeSpecifierNode
        | TypeIdentifierNode;
    bodyNode?: EnumeratorListNode;
    nameNode?: ScopedTypeIdentifierNode | TemplateTypeNode | TypeIdentifierNode;
}

export interface EnumeratorNode extends NamedNodeBase {
    type: SyntaxType.Enumerator;
    nameNode: IdentifierNode;
    valueNode?: ExpressionNode;
}

export interface EnumeratorListNode extends NamedNodeBase {
    type: SyntaxType.EnumeratorList;
}

export interface ExplicitFunctionSpecifierNode extends NamedNodeBase {
    type: SyntaxType.ExplicitFunctionSpecifier;
}

export interface ExpressionStatementNode extends NamedNodeBase {
    type: SyntaxType.ExpressionStatement;
}

export interface FieldDeclarationNode extends NamedNodeBase {
    type: SyntaxType.FieldDeclaration;
    declaratorNodes: FieldDeclaratorNode[];
    default_valueNode?: ExpressionNode | InitializerListNode;
    typeNode: TypeSpecifierNode;
}

export interface FieldDeclarationListNode extends NamedNodeBase {
    type: SyntaxType.FieldDeclarationList;
}

export interface FieldDesignatorNode extends NamedNodeBase {
    type: SyntaxType.FieldDesignator;
}

export interface FieldExpressionNode extends NamedNodeBase {
    type: SyntaxType.FieldExpression;
    argumentNode: ExpressionNode;
    fieldNode: DestructorNameNode | FieldIdentifierNode | TemplateMethodNode;
}

export interface FieldInitializerNode extends NamedNodeBase {
    type: SyntaxType.FieldInitializer;
}

export interface FieldInitializerListNode extends NamedNodeBase {
    type: SyntaxType.FieldInitializerList;
}

export interface ForRangeLoopNode extends NamedNodeBase {
    type: SyntaxType.ForRangeLoop;
    bodyNode: StatementNode;
    declaratorNode: DeclaratorNode;
    rightNode: ExpressionNode | InitializerListNode;
    typeNode: TypeSpecifierNode;
}

export interface ForStatementNode extends NamedNodeBase {
    type: SyntaxType.ForStatement;
    conditionNode?: ExpressionNode;
    initializerNode?: ExpressionNode | CommaExpressionNode | DeclarationNode;
    updateNode?: ExpressionNode | CommaExpressionNode;
}

export interface FriendDeclarationNode extends NamedNodeBase {
    type: SyntaxType.FriendDeclaration;
}

export interface FunctionDeclaratorNode extends NamedNodeBase {
    type: SyntaxType.FunctionDeclarator;
    declaratorNode: DeclaratorNode | FieldDeclaratorNode | TypeDeclaratorNode;
    parametersNode: ParameterListNode;
}

export interface FunctionDefinitionNode extends NamedNodeBase {
    type: SyntaxType.FunctionDefinition;
    bodyNode?: CompoundStatementNode;
    declaratorNode: DeclaratorNode | FieldDeclaratorNode | OperatorCastNode;
    typeNode?: TypeSpecifierNode;
}

export interface GotoStatementNode extends NamedNodeBase {
    type: SyntaxType.GotoStatement;
    labelNode: StatementIdentifierNode;
}

export interface IfStatementNode extends NamedNodeBase {
    type: SyntaxType.IfStatement;
    alternativeNode?: StatementNode;
    conditionNode: ConditionClauseNode;
    consequenceNode: StatementNode;
}

export interface InitDeclaratorNode extends NamedNodeBase {
    type: SyntaxType.InitDeclarator;
    declaratorNode: DeclaratorNode;
    valueNode: ExpressionNode | ArgumentListNode | InitializerListNode;
}

export interface InitializerListNode extends NamedNodeBase {
    type: SyntaxType.InitializerList;
}

export interface InitializerPairNode extends NamedNodeBase {
    type: SyntaxType.InitializerPair;
    designatorNodes: (FieldDesignatorNode | SubscriptDesignatorNode)[];
    valueNode: ExpressionNode | InitializerListNode;
}

export interface LabeledStatementNode extends NamedNodeBase {
    type: SyntaxType.LabeledStatement;
    labelNode: StatementIdentifierNode;
}

export interface LambdaCaptureSpecifierNode extends NamedNodeBase {
    type: SyntaxType.LambdaCaptureSpecifier;
}

export interface LambdaDefaultCaptureNode extends NamedNodeBase {
    type: SyntaxType.LambdaDefaultCapture;
}

export interface LambdaExpressionNode extends NamedNodeBase {
    type: SyntaxType.LambdaExpression;
    bodyNode: CompoundStatementNode;
    capturesNode: LambdaCaptureSpecifierNode;
    declaratorNode?: AbstractFunctionDeclaratorNode;
}

export interface LinkageSpecificationNode extends NamedNodeBase {
    type: SyntaxType.LinkageSpecification;
    bodyNode: DeclarationNode | DeclarationListNode | FunctionDefinitionNode;
    valueNode: StringLiteralNode;
}

export interface MsBasedModifierNode extends NamedNodeBase {
    type: SyntaxType.MsBasedModifier;
}

export interface MsCallModifierNode extends NamedNodeBase {
    type: SyntaxType.MsCallModifier;
}

export interface MsDeclspecModifierNode extends NamedNodeBase {
    type: SyntaxType.MsDeclspecModifier;
}

export interface MsPointerModifierNode extends NamedNodeBase {
    type: SyntaxType.MsPointerModifier;
}

export interface MsUnalignedPtrModifierNode extends NamedNodeBase {
    type: SyntaxType.MsUnalignedPtrModifier;
}

export interface NamespaceDefinitionNode extends NamedNodeBase {
    type: SyntaxType.NamespaceDefinition;
    bodyNode: DeclarationListNode;
    nameNode?: IdentifierNode;
}

export interface NewDeclaratorNode extends NamedNodeBase {
    type: SyntaxType.NewDeclarator;
    lengthNode: ExpressionNode;
}

export interface NewExpressionNode extends NamedNodeBase {
    type: SyntaxType.NewExpression;
    argumentsNode?: ArgumentListNode | InitializerListNode;
    declaratorNode?: NewDeclaratorNode;
    placementNode?: ArgumentListNode;
    typeNode: TypeSpecifierNode;
}

export interface NoexceptNode extends NamedNodeBase {
    type: SyntaxType.Noexcept;
}

export interface OperatorCastNode extends NamedNodeBase {
    type: SyntaxType.OperatorCast;
    declaratorNode: AbstractDeclaratorNode;
    namespaceNode?:
        | NamespaceIdentifierNode
        | ScopedNamespaceIdentifierNode
        | TemplateTypeNode;
    typeNode: TypeSpecifierNode;
}

export interface OptionalParameterDeclarationNode extends NamedNodeBase {
    type: SyntaxType.OptionalParameterDeclaration;
    declaratorNode?: DeclaratorNode;
    default_valueNode: ExpressionNode;
    typeNode: TypeSpecifierNode;
}

export interface OptionalTypeParameterDeclarationNode extends NamedNodeBase {
    type: SyntaxType.OptionalTypeParameterDeclaration;
    default_typeNode: TypeSpecifierNode;
    nameNode?: TypeIdentifierNode;
}

export interface ParameterDeclarationNode extends NamedNodeBase {
    type: SyntaxType.ParameterDeclaration;
    declaratorNode?: AbstractDeclaratorNode | DeclaratorNode;
    typeNode: TypeSpecifierNode;
}

export interface ParameterListNode extends NamedNodeBase {
    type: SyntaxType.ParameterList;
}

export interface ParameterPackExpansionNode extends NamedNodeBase {
    type: SyntaxType.ParameterPackExpansion;
    patternNode: ExpressionNode | TypeDescriptorNode;
}

export interface ParenthesizedDeclaratorNode extends NamedNodeBase {
    type: SyntaxType.ParenthesizedDeclarator;
}

export interface ParenthesizedExpressionNode extends NamedNodeBase {
    type: SyntaxType.ParenthesizedExpression;
}

export interface PointerDeclaratorNode extends NamedNodeBase {
    type: SyntaxType.PointerDeclarator;
    declaratorNode: DeclaratorNode | FieldDeclaratorNode | TypeDeclaratorNode;
}

export interface PointerExpressionNode extends NamedNodeBase {
    type: SyntaxType.PointerExpression;
    argumentNode: ExpressionNode;
    operatorNode: UnnamedNode<"&"> | UnnamedNode<"*">;
}

export interface PreprocCallNode extends NamedNodeBase {
    type: SyntaxType.PreprocCall;
    argumentNode?: PreprocArgNode;
    directiveNode: PreprocDirectiveNode;
}

export interface PreprocDefNode extends NamedNodeBase {
    type: SyntaxType.PreprocDef;
    nameNode: IdentifierNode;
    valueNode?: PreprocArgNode;
}

export interface PreprocDefinedNode extends NamedNodeBase {
    type: SyntaxType.PreprocDefined;
}

export interface PreprocElifNode extends NamedNodeBase {
    type: SyntaxType.PreprocElif;
    alternativeNode?: PreprocElifNode | PreprocElseNode;
    conditionNode:
        | BinaryExpressionNode
        | CallExpressionNode
        | CharLiteralNode
        | IdentifierNode
        | NumberLiteralNode
        | ParenthesizedExpressionNode
        | PreprocDefinedNode
        | UnaryExpressionNode;
}

export interface PreprocElseNode extends NamedNodeBase {
    type: SyntaxType.PreprocElse;
}

export interface PreprocFunctionDefNode extends NamedNodeBase {
    type: SyntaxType.PreprocFunctionDef;
    nameNode: IdentifierNode;
    parametersNode: PreprocParamsNode;
    valueNode?: PreprocArgNode;
}

export interface PreprocIfNode extends NamedNodeBase {
    type: SyntaxType.PreprocIf;
    alternativeNode?: PreprocElifNode | PreprocElseNode;
    conditionNode:
        | BinaryExpressionNode
        | CallExpressionNode
        | CharLiteralNode
        | IdentifierNode
        | NumberLiteralNode
        | ParenthesizedExpressionNode
        | PreprocDefinedNode
        | UnaryExpressionNode;
}

export interface PreprocIfdefNode extends NamedNodeBase {
    type: SyntaxType.PreprocIfdef;
    alternativeNode?: PreprocElifNode | PreprocElseNode;
    nameNode: IdentifierNode;
}

export interface PreprocIncludeNode extends NamedNodeBase {
    type: SyntaxType.PreprocInclude;
    pathNode:
        | CallExpressionNode
        | IdentifierNode
        | StringLiteralNode
        | SystemLibStringNode;
}

export interface PreprocParamsNode extends NamedNodeBase {
    type: SyntaxType.PreprocParams;
}

export interface ReferenceDeclaratorNode extends NamedNodeBase {
    type: SyntaxType.ReferenceDeclarator;
}

export interface ReturnStatementNode extends NamedNodeBase {
    type: SyntaxType.ReturnStatement;
}

export interface ScopedFieldIdentifierNode extends NamedNodeBase {
    type: SyntaxType.ScopedFieldIdentifier;
    nameNode: DestructorNameNode | FieldIdentifierNode | OperatorNameNode;
    namespaceNode?:
        | NamespaceIdentifierNode
        | ScopedNamespaceIdentifierNode
        | TemplateTypeNode;
}

export interface ScopedIdentifierNode extends NamedNodeBase {
    type: SyntaxType.ScopedIdentifier;
    nameNode: DestructorNameNode | IdentifierNode | OperatorNameNode;
    namespaceNode?:
        | NamespaceIdentifierNode
        | ScopedNamespaceIdentifierNode
        | TemplateTypeNode;
}

export interface ScopedNamespaceIdentifierNode extends NamedNodeBase {
    type: SyntaxType.ScopedNamespaceIdentifier;
    nameNode: NamespaceIdentifierNode;
    namespaceNode?:
        | NamespaceIdentifierNode
        | ScopedNamespaceIdentifierNode
        | TemplateTypeNode;
}

export interface ScopedTypeIdentifierNode extends NamedNodeBase {
    type: SyntaxType.ScopedTypeIdentifier;
    nameNode: TypeIdentifierNode;
    namespaceNode?:
        | NamespaceIdentifierNode
        | ScopedNamespaceIdentifierNode
        | TemplateTypeNode;
}

export interface SizedTypeSpecifierNode extends NamedNodeBase {
    type: SyntaxType.SizedTypeSpecifier;
    typeNode?: PrimitiveTypeNode | TypeIdentifierNode;
}

export interface SizeofExpressionNode extends NamedNodeBase {
    type: SyntaxType.SizeofExpression;
    typeNode?: TypeDescriptorNode;
    valueNode?: ExpressionNode;
}

export interface StaticAssertDeclarationNode extends NamedNodeBase {
    type: SyntaxType.StaticAssertDeclaration;
    conditionNode: ExpressionNode;
    messageNode?:
        | ConcatenatedStringNode
        | RawStringLiteralNode
        | StringLiteralNode;
}

export interface StorageClassSpecifierNode extends NamedNodeBase {
    type: SyntaxType.StorageClassSpecifier;
}

export interface StringLiteralNode extends NamedNodeBase {
    type: SyntaxType.StringLiteral;
}

export interface StructSpecifierNode extends NamedNodeBase {
    type: SyntaxType.StructSpecifier;
    bodyNode?: FieldDeclarationListNode;
    nameNode?: ScopedTypeIdentifierNode | TemplateTypeNode | TypeIdentifierNode;
}

export interface StructuredBindingDeclaratorNode extends NamedNodeBase {
    type: SyntaxType.StructuredBindingDeclarator;
}

export interface SubscriptDesignatorNode extends NamedNodeBase {
    type: SyntaxType.SubscriptDesignator;
}

export interface SubscriptExpressionNode extends NamedNodeBase {
    type: SyntaxType.SubscriptExpression;
    argumentNode: ExpressionNode;
    indexNode: ExpressionNode;
}

export interface SwitchStatementNode extends NamedNodeBase {
    type: SyntaxType.SwitchStatement;
    bodyNode: CompoundStatementNode;
    conditionNode: ConditionClauseNode;
}

export interface TemplateArgumentListNode extends NamedNodeBase {
    type: SyntaxType.TemplateArgumentList;
}

export interface TemplateDeclarationNode extends NamedNodeBase {
    type: SyntaxType.TemplateDeclaration;
    parametersNode: TemplateParameterListNode;
}

export interface TemplateFunctionNode extends NamedNodeBase {
    type: SyntaxType.TemplateFunction;
    argumentsNode: TemplateArgumentListNode;
    nameNode: IdentifierNode | ScopedIdentifierNode;
}

export interface TemplateInstantiationNode extends NamedNodeBase {
    type: SyntaxType.TemplateInstantiation;
    declaratorNode: DeclaratorNode;
    typeNode?: TypeSpecifierNode;
}

export interface TemplateMethodNode extends NamedNodeBase {
    type: SyntaxType.TemplateMethod;
    argumentsNode: TemplateArgumentListNode;
    nameNode: FieldIdentifierNode | ScopedFieldIdentifierNode;
}

export interface TemplateParameterListNode extends NamedNodeBase {
    type: SyntaxType.TemplateParameterList;
}

export interface TemplateTemplateParameterDeclarationNode
    extends NamedNodeBase {
    type: SyntaxType.TemplateTemplateParameterDeclaration;
    parametersNode: TemplateParameterListNode;
}

export interface TemplateTypeNode extends NamedNodeBase {
    type: SyntaxType.TemplateType;
    argumentsNode: TemplateArgumentListNode;
    nameNode: ScopedTypeIdentifierNode | TypeIdentifierNode;
}

export interface ThrowSpecifierNode extends NamedNodeBase {
    type: SyntaxType.ThrowSpecifier;
}

export interface ThrowStatementNode extends NamedNodeBase {
    type: SyntaxType.ThrowStatement;
}

export interface TrailingReturnTypeNode extends NamedNodeBase {
    type: SyntaxType.TrailingReturnType;
}

export interface TranslationUnitNode extends NamedNodeBase {
    type: SyntaxType.TranslationUnit;
}

export interface TryStatementNode extends NamedNodeBase {
    type: SyntaxType.TryStatement;
    bodyNode: CompoundStatementNode;
}

export interface TypeDefinitionNode extends NamedNodeBase {
    type: SyntaxType.TypeDefinition;
    declaratorNodes: TypeDeclaratorNode[];
    typeNode: TypeSpecifierNode;
}

export interface TypeDescriptorNode extends NamedNodeBase {
    type: SyntaxType.TypeDescriptor;
    declaratorNode?: AbstractDeclaratorNode;
    typeNode: TypeSpecifierNode;
}

export interface TypeParameterDeclarationNode extends NamedNodeBase {
    type: SyntaxType.TypeParameterDeclaration;
}

export interface TypeQualifierNode extends NamedNodeBase {
    type: SyntaxType.TypeQualifier;
}

export interface UnaryExpressionNode extends NamedNodeBase {
    type: SyntaxType.UnaryExpression;
    argumentNode: ExpressionNode | PreprocDefinedNode;
    operatorNode:
        | UnnamedNode<"!">
        | UnnamedNode<"+">
        | UnnamedNode<"-">
        | UnnamedNode<"~">;
}

export interface UnionSpecifierNode extends NamedNodeBase {
    type: SyntaxType.UnionSpecifier;
    bodyNode?: FieldDeclarationListNode;
    nameNode?: ScopedTypeIdentifierNode | TemplateTypeNode | TypeIdentifierNode;
}

export interface UpdateExpressionNode extends NamedNodeBase {
    type: SyntaxType.UpdateExpression;
    argumentNode: ExpressionNode;
    operatorNode: UnnamedNode<"++"> | UnnamedNode<"--">;
}

export interface UsingDeclarationNode extends NamedNodeBase {
    type: SyntaxType.UsingDeclaration;
}

export interface VariadicDeclaratorNode extends NamedNodeBase {
    type: SyntaxType.VariadicDeclarator;
}

export interface VariadicParameterDeclarationNode extends NamedNodeBase {
    type: SyntaxType.VariadicParameterDeclaration;
    declaratorNode: ReferenceDeclaratorNode | VariadicDeclaratorNode;
    typeNode: TypeSpecifierNode;
}

export interface VariadicTypeParameterDeclarationNode extends NamedNodeBase {
    type: SyntaxType.VariadicTypeParameterDeclaration;
}

export interface VirtualFunctionSpecifierNode extends NamedNodeBase {
    type: SyntaxType.VirtualFunctionSpecifier;
}

export interface VirtualSpecifierNode extends NamedNodeBase {
    type: SyntaxType.VirtualSpecifier;
}

export interface WhileStatementNode extends NamedNodeBase {
    type: SyntaxType.WhileStatement;
    bodyNode: StatementNode;
    conditionNode: ConditionClauseNode;
}

export interface AutoNode extends NamedNodeBase {
    type: SyntaxType.Auto;
}

export interface CommentNode extends NamedNodeBase {
    type: SyntaxType.Comment;
}

export interface EscapeSequenceNode extends NamedNodeBase {
    type: SyntaxType.EscapeSequence;
}

export interface FalseNode extends NamedNodeBase {
    type: SyntaxType.False;
}

export interface FieldIdentifierNode extends NamedNodeBase {
    type: SyntaxType.FieldIdentifier;
}

export interface IdentifierNode extends NamedNodeBase {
    type: SyntaxType.Identifier;
}

export interface MsRestrictModifierNode extends NamedNodeBase {
    type: SyntaxType.MsRestrictModifier;
}

export interface MsSignedPtrModifierNode extends NamedNodeBase {
    type: SyntaxType.MsSignedPtrModifier;
}

export interface MsUnsignedPtrModifierNode extends NamedNodeBase {
    type: SyntaxType.MsUnsignedPtrModifier;
}

export interface NamespaceIdentifierNode extends NamedNodeBase {
    type: SyntaxType.NamespaceIdentifier;
}

export interface NullNode extends NamedNodeBase {
    type: SyntaxType.Null;
}

export interface NullptrNode extends NamedNodeBase {
    type: SyntaxType.Nullptr;
}

export interface NumberLiteralNode extends NamedNodeBase {
    type: SyntaxType.NumberLiteral;
}

export interface OperatorNameNode extends NamedNodeBase {
    type: SyntaxType.OperatorName;
}

export interface PreprocArgNode extends NamedNodeBase {
    type: SyntaxType.PreprocArg;
}

export interface PreprocDirectiveNode extends NamedNodeBase {
    type: SyntaxType.PreprocDirective;
}

export interface PrimitiveTypeNode extends NamedNodeBase {
    type: SyntaxType.PrimitiveType;
}

export interface RawStringLiteralNode extends NamedNodeBase {
    type: SyntaxType.RawStringLiteral;
}

export interface StatementIdentifierNode extends NamedNodeBase {
    type: SyntaxType.StatementIdentifier;
}

export interface SystemLibStringNode extends NamedNodeBase {
    type: SyntaxType.SystemLibString;
}

export interface ThisNode extends NamedNodeBase {
    type: SyntaxType.This;
}

export interface TrueNode extends NamedNodeBase {
    type: SyntaxType.True;
}

export interface TypeIdentifierNode extends NamedNodeBase {
    type: SyntaxType.TypeIdentifier;
}
