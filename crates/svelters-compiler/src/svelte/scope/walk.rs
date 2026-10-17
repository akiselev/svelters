//! Port of `zimmerframe` 1.1.5 `src/walk.js` (the walker `phases/scope.js` relies on), as a typed
//! visitor over both template (`svelte::ast`) and ESTree (`estree`) nodes.
//!
//! zimmerframe visits every own enumerable key of a node, in insertion order, recursing into any
//! object or array element that has a `type`. Here [`NodeRef::children`] lists the child nodes in
//! the same order (struct field order, which follows the upstream parsers' key order). Serde-only
//! wrapper enums (`BlockStatementNode`, `LiteralNode`, ...) are transparent: they are not nodes.
//!
//! Upstream visitors receive `{ path, state, next, visit, stop }`. Here a [`Visitor`] gets the node,
//! the state and the [`Walker`], whose `path` is zimmerframe's `path` and whose methods are
//! `next` / `visit` / `stop`. State is passed by reference; a visitor passes a new value to
//! `next`/`visit` to change it for the subtree (zimmerframe's `next(state)` / `visit(node, state)`).
//!
//! SVELTERS: zimmerframe's mutation support (visitors returning replacement nodes) is not ported:
//! the AST is immutable after parsing and scope analysis never transforms it.

use crate::estree::*;
use crate::svelte::ast::{self, AttributeLike, AttributeValue, ExpressionTagNode, FragmentNode, TextOrExpressionTag};

/// A borrowed reference to any node (template or ESTree). The variant is the node's `type`.
#[derive(Clone, Copy, Debug)]
pub enum NodeRef<'a> {
    // ---- ESTree
    Program(&'a Program),
    ExpressionStatement(&'a ExpressionStatement),
    BlockStatement(&'a BlockStatement),
    StaticBlock(&'a StaticBlock),
    EmptyStatement(&'a EmptyStatement),
    DebuggerStatement(&'a DebuggerStatement),
    WithStatement(&'a WithStatement),
    ReturnStatement(&'a ReturnStatement),
    LabeledStatement(&'a LabeledStatement),
    BreakStatement(&'a BreakStatement),
    ContinueStatement(&'a ContinueStatement),
    IfStatement(&'a IfStatement),
    SwitchStatement(&'a SwitchStatement),
    SwitchCase(&'a SwitchCase),
    ThrowStatement(&'a ThrowStatement),
    TryStatement(&'a TryStatement),
    CatchClause(&'a CatchClause),
    WhileStatement(&'a WhileStatement),
    DoWhileStatement(&'a DoWhileStatement),
    ForStatement(&'a ForStatement),
    ForInStatement(&'a ForInStatement),
    ForOfStatement(&'a ForOfStatement),
    FunctionDeclaration(&'a Function),
    FunctionExpression(&'a Function),
    ArrowFunctionExpression(&'a ArrowFunctionExpression),
    VariableDeclaration(&'a VariableDeclaration),
    VariableDeclarator(&'a VariableDeclarator),
    ClassDeclaration(&'a Class),
    ClassExpression(&'a Class),
    ClassBody(&'a ClassBody),
    MethodDefinition(&'a MethodDefinition),
    PropertyDefinition(&'a PropertyDefinition),
    ImportDeclaration(&'a ImportDeclaration),
    ImportSpecifier(&'a ImportSpecifier),
    ImportDefaultSpecifier(&'a ImportDefaultSpecifier),
    ImportNamespaceSpecifier(&'a ImportNamespaceSpecifier),
    ImportAttribute(&'a ImportAttribute),
    ExportNamedDeclaration(&'a ExportNamedDeclaration),
    ExportSpecifier(&'a ExportSpecifier),
    ExportDefaultDeclaration(&'a ExportDefaultDeclaration),
    ExportAllDeclaration(&'a ExportAllDeclaration),
    Identifier(&'a Identifier),
    PrivateIdentifier(&'a PrivateIdentifier),
    Literal(&'a Literal),
    ThisExpression(&'a ThisExpression),
    Super(&'a Super),
    ArrayExpression(&'a ArrayExpression),
    ObjectExpression(&'a ObjectExpression),
    /// `Property` inside an ObjectExpression.
    Property(&'a Property),
    /// `Property` inside an ObjectPattern (same `type` as `Property`).
    PatternProperty(&'a PatternProperty),
    SpreadElement(&'a SpreadElement),
    TemplateLiteral(&'a TemplateLiteral),
    TemplateElement(&'a TemplateElement),
    TaggedTemplateExpression(&'a TaggedTemplateExpression),
    UnaryExpression(&'a UnaryExpression),
    UpdateExpression(&'a UpdateExpression),
    BinaryExpression(&'a BinaryExpression),
    LogicalExpression(&'a LogicalExpression),
    AssignmentExpression(&'a AssignmentExpression),
    ConditionalExpression(&'a ConditionalExpression),
    CallExpression(&'a CallExpression),
    NewExpression(&'a NewExpression),
    MemberExpression(&'a MemberExpression),
    ChainExpression(&'a ChainExpression),
    SequenceExpression(&'a SequenceExpression),
    YieldExpression(&'a YieldExpression),
    AwaitExpression(&'a AwaitExpression),
    ImportExpression(&'a ImportExpression),
    MetaProperty(&'a MetaProperty),
    ParenthesizedExpression(&'a ParenthesizedExpression),
    ObjectPattern(&'a ObjectPattern),
    ArrayPattern(&'a ArrayPattern),
    RestElement(&'a RestElement),
    AssignmentPattern(&'a AssignmentPattern),
    // ---- template
    Root(&'a ast::Root),
    Script(&'a ast::Script),
    Fragment(&'a ast::Fragment),
    Text(&'a ast::Text),
    Comment(&'a ast::Comment),
    ExpressionTag(&'a ast::ExpressionTag),
    HtmlTag(&'a ast::HtmlTag),
    ConstTag(&'a ast::ConstTag),
    DeclarationTag(&'a ast::DeclarationTag),
    DebugTag(&'a ast::DebugTag),
    RenderTag(&'a ast::RenderTag),
    AttachTag(&'a ast::AttachTag),
    Component(&'a ast::Element),
    TitleElement(&'a ast::Element),
    SlotElement(&'a ast::Element),
    RegularElement(&'a ast::Element),
    SvelteBody(&'a ast::Element),
    SvelteComponent(&'a ast::SvelteComponent),
    SvelteDocument(&'a ast::Element),
    SvelteElement(&'a ast::SvelteElement),
    SvelteFragment(&'a ast::Element),
    SvelteBoundary(&'a ast::Element),
    SvelteHead(&'a ast::Element),
    SvelteOptions(&'a ast::Element),
    SvelteSelf(&'a ast::Element),
    SvelteWindow(&'a ast::Element),
    EachBlock(&'a ast::EachBlock),
    IfBlock(&'a ast::IfBlock),
    AwaitBlock(&'a ast::AwaitBlock),
    KeyBlock(&'a ast::KeyBlock),
    SnippetBlock(&'a ast::SnippetBlock),
    Attribute(&'a ast::Attribute),
    SpreadAttribute(&'a ast::SpreadAttribute),
    AnimateDirective(&'a ast::Directive),
    BindDirective(&'a ast::Directive),
    ClassDirective(&'a ast::Directive),
    LetDirective(&'a ast::Directive),
    OnDirective(&'a ast::Directive),
    UseDirective(&'a ast::Directive),
    StyleDirective(&'a ast::StyleDirective),
    TransitionDirective(&'a ast::TransitionDirective),
}

/// Stable identity of a node inside an immutable AST: the address of the node struct plus the
/// node type (two nodes of different types can share an address when one is the first field of
/// the other; `Property`/`PatternProperty` are distinct nodes that never share one). Valid as long as the AST is borrowed, which the `'a` lifetimes guarantee.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeKey {
    addr: usize,
    kind: &'static str,
}

fn addr<T>(r: &T) -> usize {
    r as *const T as usize
}

macro_rules! each_variant {
    ($self:expr, $n:ident => $e:expr, $f:ident => $fe:expr) => {
        match $self {
            NodeRef::Program($n) => $e,
            NodeRef::ExpressionStatement($n) => $e,
            NodeRef::BlockStatement($n) => $e,
            NodeRef::StaticBlock($n) => $e,
            NodeRef::EmptyStatement($n) => $e,
            NodeRef::DebuggerStatement($n) => $e,
            NodeRef::WithStatement($n) => $e,
            NodeRef::ReturnStatement($n) => $e,
            NodeRef::LabeledStatement($n) => $e,
            NodeRef::BreakStatement($n) => $e,
            NodeRef::ContinueStatement($n) => $e,
            NodeRef::IfStatement($n) => $e,
            NodeRef::SwitchStatement($n) => $e,
            NodeRef::SwitchCase($n) => $e,
            NodeRef::ThrowStatement($n) => $e,
            NodeRef::TryStatement($n) => $e,
            NodeRef::CatchClause($n) => $e,
            NodeRef::WhileStatement($n) => $e,
            NodeRef::DoWhileStatement($n) => $e,
            NodeRef::ForStatement($n) => $e,
            NodeRef::ForInStatement($n) => $e,
            NodeRef::ForOfStatement($n) => $e,
            NodeRef::FunctionDeclaration($n) => $e,
            NodeRef::FunctionExpression($n) => $e,
            NodeRef::ArrowFunctionExpression($n) => $e,
            NodeRef::VariableDeclaration($n) => $e,
            NodeRef::VariableDeclarator($n) => $e,
            NodeRef::ClassDeclaration($n) => $e,
            NodeRef::ClassExpression($n) => $e,
            NodeRef::ClassBody($n) => $e,
            NodeRef::MethodDefinition($n) => $e,
            NodeRef::PropertyDefinition($n) => $e,
            NodeRef::ImportDeclaration($n) => $e,
            NodeRef::ImportSpecifier($n) => $e,
            NodeRef::ImportDefaultSpecifier($n) => $e,
            NodeRef::ImportNamespaceSpecifier($n) => $e,
            NodeRef::ImportAttribute($n) => $e,
            NodeRef::ExportNamedDeclaration($n) => $e,
            NodeRef::ExportSpecifier($n) => $e,
            NodeRef::ExportDefaultDeclaration($n) => $e,
            NodeRef::ExportAllDeclaration($n) => $e,
            NodeRef::Identifier($n) => $e,
            NodeRef::PrivateIdentifier($n) => $e,
            NodeRef::Literal($n) => $e,
            NodeRef::ThisExpression($n) => $e,
            NodeRef::Super($n) => $e,
            NodeRef::ArrayExpression($n) => $e,
            NodeRef::ObjectExpression($n) => $e,
            NodeRef::Property($n) => $e,
            NodeRef::PatternProperty($n) => $e,
            NodeRef::SpreadElement($n) => $e,
            NodeRef::TemplateLiteral($n) => $e,
            NodeRef::TemplateElement($n) => $e,
            NodeRef::TaggedTemplateExpression($n) => $e,
            NodeRef::UnaryExpression($n) => $e,
            NodeRef::UpdateExpression($n) => $e,
            NodeRef::BinaryExpression($n) => $e,
            NodeRef::LogicalExpression($n) => $e,
            NodeRef::AssignmentExpression($n) => $e,
            NodeRef::ConditionalExpression($n) => $e,
            NodeRef::CallExpression($n) => $e,
            NodeRef::NewExpression($n) => $e,
            NodeRef::MemberExpression($n) => $e,
            NodeRef::ChainExpression($n) => $e,
            NodeRef::SequenceExpression($n) => $e,
            NodeRef::YieldExpression($n) => $e,
            NodeRef::AwaitExpression($n) => $e,
            NodeRef::ImportExpression($n) => $e,
            NodeRef::MetaProperty($n) => $e,
            NodeRef::ParenthesizedExpression($n) => $e,
            NodeRef::ObjectPattern($n) => $e,
            NodeRef::ArrayPattern($n) => $e,
            NodeRef::RestElement($n) => $e,
            NodeRef::AssignmentPattern($n) => $e,
            NodeRef::Root($n) => $e,
            NodeRef::Script($n) => $e,
            NodeRef::Fragment($f) => $fe,
            NodeRef::Text($n) => $e,
            NodeRef::Comment($n) => $e,
            NodeRef::ExpressionTag($n) => $e,
            NodeRef::HtmlTag($n) => $e,
            NodeRef::ConstTag($n) => $e,
            NodeRef::DeclarationTag($n) => $e,
            NodeRef::DebugTag($n) => $e,
            NodeRef::RenderTag($n) => $e,
            NodeRef::AttachTag($n) => $e,
            NodeRef::Component($n)
            | NodeRef::TitleElement($n)
            | NodeRef::SlotElement($n)
            | NodeRef::RegularElement($n)
            | NodeRef::SvelteBody($n)
            | NodeRef::SvelteDocument($n)
            | NodeRef::SvelteFragment($n)
            | NodeRef::SvelteBoundary($n)
            | NodeRef::SvelteHead($n)
            | NodeRef::SvelteOptions($n)
            | NodeRef::SvelteSelf($n)
            | NodeRef::SvelteWindow($n) => $e,
            NodeRef::SvelteComponent($n) => $e,
            NodeRef::SvelteElement($n) => $e,
            NodeRef::EachBlock($n) => $e,
            NodeRef::IfBlock($n) => $e,
            NodeRef::AwaitBlock($n) => $e,
            NodeRef::KeyBlock($n) => $e,
            NodeRef::SnippetBlock($n) => $e,
            NodeRef::Attribute($n) => $e,
            NodeRef::SpreadAttribute($n) => $e,
            NodeRef::AnimateDirective($n)
            | NodeRef::BindDirective($n)
            | NodeRef::ClassDirective($n)
            | NodeRef::LetDirective($n)
            | NodeRef::OnDirective($n)
            | NodeRef::UseDirective($n) => $e,
            NodeRef::StyleDirective($n) => $e,
            NodeRef::TransitionDirective($n) => $e,
        }
    };
}

impl<'a> NodeRef<'a> {
    /// The node's identity key (see [`NodeKey`]).
    pub fn key(self) -> NodeKey {
        NodeKey {
            addr: each_variant!(self, n => addr(n), f => addr(f)),
            kind: self.type_name(),
        }
    }

    /// Byte span of the node (a `Fragment` has none upstream; its nodes' extent is used).
    pub fn span(self) -> Span {
        use crate::estree::Spanned;
        each_variant!(self, n => n.span(), f => match (f.nodes.first(), f.nodes.last()) {
            (Some(a), Some(b)) => Span::new(a.span().start, b.span().end),
            _ => Span::default(),
        })
    }

    /// The node's `type` as upstream spells it.
    pub fn type_name(self) -> &'static str {
        use NodeRef::*;
        match self {
            Program(_) => "Program",
            ExpressionStatement(_) => "ExpressionStatement",
            BlockStatement(_) => "BlockStatement",
            StaticBlock(_) => "StaticBlock",
            EmptyStatement(_) => "EmptyStatement",
            DebuggerStatement(_) => "DebuggerStatement",
            WithStatement(_) => "WithStatement",
            ReturnStatement(_) => "ReturnStatement",
            LabeledStatement(_) => "LabeledStatement",
            BreakStatement(_) => "BreakStatement",
            ContinueStatement(_) => "ContinueStatement",
            IfStatement(_) => "IfStatement",
            SwitchStatement(_) => "SwitchStatement",
            SwitchCase(_) => "SwitchCase",
            ThrowStatement(_) => "ThrowStatement",
            TryStatement(_) => "TryStatement",
            CatchClause(_) => "CatchClause",
            WhileStatement(_) => "WhileStatement",
            DoWhileStatement(_) => "DoWhileStatement",
            ForStatement(_) => "ForStatement",
            ForInStatement(_) => "ForInStatement",
            ForOfStatement(_) => "ForOfStatement",
            FunctionDeclaration(_) => "FunctionDeclaration",
            FunctionExpression(_) => "FunctionExpression",
            ArrowFunctionExpression(_) => "ArrowFunctionExpression",
            VariableDeclaration(_) => "VariableDeclaration",
            VariableDeclarator(_) => "VariableDeclarator",
            ClassDeclaration(_) => "ClassDeclaration",
            ClassExpression(_) => "ClassExpression",
            ClassBody(_) => "ClassBody",
            MethodDefinition(_) => "MethodDefinition",
            PropertyDefinition(_) => "PropertyDefinition",
            ImportDeclaration(_) => "ImportDeclaration",
            ImportSpecifier(_) => "ImportSpecifier",
            ImportDefaultSpecifier(_) => "ImportDefaultSpecifier",
            ImportNamespaceSpecifier(_) => "ImportNamespaceSpecifier",
            ImportAttribute(_) => "ImportAttribute",
            ExportNamedDeclaration(_) => "ExportNamedDeclaration",
            ExportSpecifier(_) => "ExportSpecifier",
            ExportDefaultDeclaration(_) => "ExportDefaultDeclaration",
            ExportAllDeclaration(_) => "ExportAllDeclaration",
            Identifier(_) => "Identifier",
            PrivateIdentifier(_) => "PrivateIdentifier",
            Literal(_) => "Literal",
            ThisExpression(_) => "ThisExpression",
            Super(_) => "Super",
            ArrayExpression(_) => "ArrayExpression",
            ObjectExpression(_) => "ObjectExpression",
            Property(_) | PatternProperty(_) => "Property",
            SpreadElement(_) => "SpreadElement",
            TemplateLiteral(_) => "TemplateLiteral",
            TemplateElement(_) => "TemplateElement",
            TaggedTemplateExpression(_) => "TaggedTemplateExpression",
            UnaryExpression(_) => "UnaryExpression",
            UpdateExpression(_) => "UpdateExpression",
            BinaryExpression(_) => "BinaryExpression",
            LogicalExpression(_) => "LogicalExpression",
            AssignmentExpression(_) => "AssignmentExpression",
            ConditionalExpression(_) => "ConditionalExpression",
            CallExpression(_) => "CallExpression",
            NewExpression(_) => "NewExpression",
            MemberExpression(_) => "MemberExpression",
            ChainExpression(_) => "ChainExpression",
            SequenceExpression(_) => "SequenceExpression",
            YieldExpression(_) => "YieldExpression",
            AwaitExpression(_) => "AwaitExpression",
            ImportExpression(_) => "ImportExpression",
            MetaProperty(_) => "MetaProperty",
            ParenthesizedExpression(_) => "ParenthesizedExpression",
            ObjectPattern(_) => "ObjectPattern",
            ArrayPattern(_) => "ArrayPattern",
            RestElement(_) => "RestElement",
            AssignmentPattern(_) => "AssignmentPattern",
            Root(_) => "Root",
            Script(_) => "Script",
            Fragment(_) => "Fragment",
            Text(_) => "Text",
            Comment(_) => "Comment",
            ExpressionTag(_) => "ExpressionTag",
            HtmlTag(_) => "HtmlTag",
            ConstTag(_) => "ConstTag",
            DeclarationTag(_) => "DeclarationTag",
            DebugTag(_) => "DebugTag",
            RenderTag(_) => "RenderTag",
            AttachTag(_) => "AttachTag",
            Component(_) => "Component",
            TitleElement(_) => "TitleElement",
            SlotElement(_) => "SlotElement",
            RegularElement(_) => "RegularElement",
            SvelteBody(_) => "SvelteBody",
            SvelteComponent(_) => "SvelteComponent",
            SvelteDocument(_) => "SvelteDocument",
            SvelteElement(_) => "SvelteElement",
            SvelteFragment(_) => "SvelteFragment",
            SvelteBoundary(_) => "SvelteBoundary",
            SvelteHead(_) => "SvelteHead",
            SvelteOptions(_) => "SvelteOptions",
            SvelteSelf(_) => "SvelteSelf",
            SvelteWindow(_) => "SvelteWindow",
            EachBlock(_) => "EachBlock",
            IfBlock(_) => "IfBlock",
            AwaitBlock(_) => "AwaitBlock",
            KeyBlock(_) => "KeyBlock",
            SnippetBlock(_) => "SnippetBlock",
            Attribute(_) => "Attribute",
            SpreadAttribute(_) => "SpreadAttribute",
            AnimateDirective(_) => "AnimateDirective",
            BindDirective(_) => "BindDirective",
            ClassDirective(_) => "ClassDirective",
            LetDirective(_) => "LetDirective",
            OnDirective(_) => "OnDirective",
            UseDirective(_) => "UseDirective",
            StyleDirective(_) => "StyleDirective",
            TransitionDirective(_) => "TransitionDirective",
        }
    }

    /// Is this a function node (`FunctionDeclaration`, `FunctionExpression`, `ArrowFunctionExpression`)?
    pub fn is_function(self) -> bool {
        matches!(
            self,
            NodeRef::FunctionDeclaration(_) | NodeRef::FunctionExpression(_) | NodeRef::ArrowFunctionExpression(_)
        )
    }

    /// The child nodes in zimmerframe visiting order.
    pub fn children(self) -> Vec<NodeRef<'a>> {
        let mut c = Children(Vec::new());
        push_children(self, &mut c);
        c.0
    }
}

/// Collects children (transparent over serde wrapper enums).
struct Children<'a>(Vec<NodeRef<'a>>);

impl<'a> Children<'a> {
    fn n(&mut self, n: NodeRef<'a>) {
        self.0.push(n);
    }
    fn ident(&mut self, i: &'a Identifier) {
        self.n(NodeRef::Identifier(i));
    }
    fn expr(&mut self, e: &'a Expression) {
        self.n(expr_ref(e));
    }
    fn opt_expr(&mut self, e: &'a Option<Expression>) {
        if let Some(e) = e {
            self.expr(e);
        }
    }
    fn opt_box_expr(&mut self, e: &'a Option<BoxExpr>) {
        if let Some(e) = e {
            self.expr(e);
        }
    }
    fn stmt(&mut self, s: &'a Statement) {
        self.n(stmt_ref(s));
    }
    fn stmts(&mut self, s: &'a [Statement]) {
        for s in s {
            self.stmt(s);
        }
    }
    fn pat(&mut self, p: &'a Pattern) {
        self.n(pattern_ref(p));
    }
    fn key(&mut self, k: &'a PropertyKey) {
        self.n(property_key_ref(k));
    }
    fn args(&mut self, a: &'a [ArrayElement]) {
        for a in a {
            self.n(array_element_ref(a));
        }
    }
    fn literal(&mut self, l: &'a LiteralNode) {
        let LiteralNode::Literal(l) = l;
        self.n(NodeRef::Literal(l));
    }
    fn module_name(&mut self, m: &'a ModuleExportName) {
        self.n(module_export_name_ref(m));
    }
    fn attributes(&mut self, a: &'a [ImportAttribute]) {
        for a in a {
            self.n(NodeRef::ImportAttribute(a));
        }
    }
    fn fragment(&mut self, f: &'a ast::Fragment) {
        self.n(NodeRef::Fragment(f));
    }
    fn attr_value(&mut self, v: &'a AttributeValue) {
        match v {
            AttributeValue::True(_) => {}
            AttributeValue::Expression(ExpressionTagNode::ExpressionTag(t)) => self.n(NodeRef::ExpressionTag(t)),
            AttributeValue::Sequence(seq) => {
                for part in seq {
                    match part {
                        TextOrExpressionTag::Text(t) => self.n(NodeRef::Text(t)),
                        TextOrExpressionTag::ExpressionTag(t) => self.n(NodeRef::ExpressionTag(t)),
                    }
                }
            }
        }
    }
    fn attrs(&mut self, a: &'a [AttributeLike]) {
        for a in a {
            self.n(attribute_like_ref(a));
        }
    }
    fn var_decl_node(&mut self, d: &'a VariableDeclarationNode) {
        let VariableDeclarationNode::VariableDeclaration(d) = d;
        self.n(NodeRef::VariableDeclaration(d));
    }
    fn block_node(&mut self, b: &'a BlockStatementNode) {
        let BlockStatementNode::BlockStatement(b) = b;
        self.n(NodeRef::BlockStatement(b));
    }
}

pub fn expr_ref(e: &Expression) -> NodeRef<'_> {
    match e {
        Expression::Identifier(n) => NodeRef::Identifier(n),
        Expression::Literal(n) => NodeRef::Literal(n),
        Expression::ThisExpression(n) => NodeRef::ThisExpression(n),
        Expression::Super(n) => NodeRef::Super(n),
        Expression::ArrayExpression(n) => NodeRef::ArrayExpression(n),
        Expression::ObjectExpression(n) => NodeRef::ObjectExpression(n),
        Expression::FunctionExpression(n) => NodeRef::FunctionExpression(n),
        Expression::ArrowFunctionExpression(n) => NodeRef::ArrowFunctionExpression(n),
        Expression::ClassExpression(n) => NodeRef::ClassExpression(n),
        Expression::TemplateLiteral(n) => NodeRef::TemplateLiteral(n),
        Expression::TaggedTemplateExpression(n) => NodeRef::TaggedTemplateExpression(n),
        Expression::UnaryExpression(n) => NodeRef::UnaryExpression(n),
        Expression::UpdateExpression(n) => NodeRef::UpdateExpression(n),
        Expression::BinaryExpression(n) => NodeRef::BinaryExpression(n),
        Expression::LogicalExpression(n) => NodeRef::LogicalExpression(n),
        Expression::AssignmentExpression(n) => NodeRef::AssignmentExpression(n),
        Expression::ConditionalExpression(n) => NodeRef::ConditionalExpression(n),
        Expression::CallExpression(n) => NodeRef::CallExpression(n),
        Expression::NewExpression(n) => NodeRef::NewExpression(n),
        Expression::MemberExpression(n) => NodeRef::MemberExpression(n),
        Expression::ChainExpression(n) => NodeRef::ChainExpression(n),
        Expression::SequenceExpression(n) => NodeRef::SequenceExpression(n),
        Expression::YieldExpression(n) => NodeRef::YieldExpression(n),
        Expression::AwaitExpression(n) => NodeRef::AwaitExpression(n),
        Expression::ImportExpression(n) => NodeRef::ImportExpression(n),
        Expression::MetaProperty(n) => NodeRef::MetaProperty(n),
        Expression::ParenthesizedExpression(n) => NodeRef::ParenthesizedExpression(n),
        // SVELTERS: TypeScript nodes are removed (`remove_typescript_nodes`) before analysis;
        // a walk over an unstripped tree sees through the wrappers.
        Expression::TSAsExpression(n) | Expression::TSSatisfiesExpression(n) => expr_ref(&n.expression),
        Expression::TSTypeAssertion(n) => expr_ref(&n.expression),
        Expression::TSNonNullExpression(n) => expr_ref(&n.expression),
        Expression::TSInstantiationExpression(n) => expr_ref(&n.expression),
        Expression::TSTypeCastExpression(n) => expr_ref(&n.expression),
    }
}

/// Stands in for TypeScript-only statements (see [`stmt_ref`]).
static TS_EMPTY_STATEMENT: EmptyStatement = EmptyStatement { start: 0, end: 0 };

pub fn stmt_ref(s: &Statement) -> NodeRef<'_> {
    match s {
        Statement::ExpressionStatement(n) => NodeRef::ExpressionStatement(n),
        Statement::BlockStatement(n) => NodeRef::BlockStatement(n),
        Statement::StaticBlock(n) => NodeRef::StaticBlock(n),
        Statement::EmptyStatement(n) => NodeRef::EmptyStatement(n),
        Statement::DebuggerStatement(n) => NodeRef::DebuggerStatement(n),
        Statement::WithStatement(n) => NodeRef::WithStatement(n),
        Statement::ReturnStatement(n) => NodeRef::ReturnStatement(n),
        Statement::LabeledStatement(n) => NodeRef::LabeledStatement(n),
        Statement::BreakStatement(n) => NodeRef::BreakStatement(n),
        Statement::ContinueStatement(n) => NodeRef::ContinueStatement(n),
        Statement::IfStatement(n) => NodeRef::IfStatement(n),
        Statement::SwitchStatement(n) => NodeRef::SwitchStatement(n),
        Statement::ThrowStatement(n) => NodeRef::ThrowStatement(n),
        Statement::TryStatement(n) => NodeRef::TryStatement(n),
        Statement::WhileStatement(n) => NodeRef::WhileStatement(n),
        Statement::DoWhileStatement(n) => NodeRef::DoWhileStatement(n),
        Statement::ForStatement(n) => NodeRef::ForStatement(n),
        Statement::ForInStatement(n) => NodeRef::ForInStatement(n),
        Statement::ForOfStatement(n) => NodeRef::ForOfStatement(n),
        Statement::FunctionDeclaration(n) => NodeRef::FunctionDeclaration(n),
        Statement::VariableDeclaration(n) => NodeRef::VariableDeclaration(n),
        Statement::ClassDeclaration(n) => NodeRef::ClassDeclaration(n),
        Statement::ImportDeclaration(n) => NodeRef::ImportDeclaration(n),
        Statement::ExportNamedDeclaration(n) => NodeRef::ExportNamedDeclaration(n),
        Statement::ExportDefaultDeclaration(n) => NodeRef::ExportDefaultDeclaration(n),
        Statement::ExportAllDeclaration(n) => NodeRef::ExportAllDeclaration(n),
        // SVELTERS: TypeScript-only declarations declare no runtime bindings the walk visits.
        Statement::TSInterfaceDeclaration(_)
        | Statement::TSTypeAliasDeclaration(_)
        | Statement::TSEnumDeclaration(_)
        | Statement::TSModuleDeclaration(_)
        | Statement::TSDeclareFunction(_)
        | Statement::TSImportEqualsDeclaration(_)
        | Statement::TSExportAssignment(_)
        | Statement::TSNamespaceExportDeclaration(_) => NodeRef::EmptyStatement(&TS_EMPTY_STATEMENT),
    }
}

pub fn pattern_ref(p: &Pattern) -> NodeRef<'_> {
    match p {
        Pattern::Identifier(n) => NodeRef::Identifier(n),
        Pattern::ObjectPattern(n) => NodeRef::ObjectPattern(n),
        Pattern::ArrayPattern(n) => NodeRef::ArrayPattern(n),
        Pattern::RestElement(n) => NodeRef::RestElement(n),
        Pattern::AssignmentPattern(n) => NodeRef::AssignmentPattern(n),
        Pattern::MemberExpression(n) => NodeRef::MemberExpression(n),
        Pattern::ParenthesizedExpression(n) => NodeRef::ParenthesizedExpression(n),
        // SVELTERS: see `expr_ref`.
        Pattern::TSParameterProperty(n) => pattern_ref(&n.parameter),
        Pattern::TSAsExpression(n)
        | Pattern::TSSatisfiesExpression(n)
        | Pattern::TSTypeAssertion(n)
        | Pattern::TSNonNullExpression(n) => pattern_ref(&n.expression),
    }
}

pub fn property_key_ref(k: &PropertyKey) -> NodeRef<'_> {
    match k {
        PropertyKey::Private(PrivateIdentifierNode::PrivateIdentifier(p)) => NodeRef::PrivateIdentifier(p),
        PropertyKey::Expression(e) => expr_ref(e),
    }
}

pub fn array_element_ref(a: &ArrayElement) -> NodeRef<'_> {
    match a {
        ArrayElement::Spread(SpreadElementNode::SpreadElement(s)) => NodeRef::SpreadElement(s),
        ArrayElement::Expression(e) => expr_ref(e),
    }
}

pub fn module_export_name_ref(m: &ModuleExportName) -> NodeRef<'_> {
    match m {
        ModuleExportName::Identifier(i) => NodeRef::Identifier(i),
        ModuleExportName::Literal(l) => NodeRef::Literal(l),
    }
}

pub fn fragment_node_ref(n: &FragmentNode) -> NodeRef<'_> {
    match n {
        FragmentNode::Text(n) => NodeRef::Text(n),
        FragmentNode::Comment(n) => NodeRef::Comment(n),
        FragmentNode::ExpressionTag(n) => NodeRef::ExpressionTag(n),
        FragmentNode::HtmlTag(n) => NodeRef::HtmlTag(n),
        FragmentNode::ConstTag(n) => NodeRef::ConstTag(n),
        FragmentNode::DeclarationTag(n) => NodeRef::DeclarationTag(n),
        FragmentNode::DebugTag(n) => NodeRef::DebugTag(n),
        FragmentNode::RenderTag(n) => NodeRef::RenderTag(n),
        FragmentNode::AttachTag(n) => NodeRef::AttachTag(n),
        FragmentNode::Component(n) => NodeRef::Component(n),
        FragmentNode::TitleElement(n) => NodeRef::TitleElement(n),
        FragmentNode::SlotElement(n) => NodeRef::SlotElement(n),
        FragmentNode::RegularElement(n) => NodeRef::RegularElement(n),
        FragmentNode::SvelteBody(n) => NodeRef::SvelteBody(n),
        FragmentNode::SvelteComponent(n) => NodeRef::SvelteComponent(n),
        FragmentNode::SvelteDocument(n) => NodeRef::SvelteDocument(n),
        FragmentNode::SvelteElement(n) => NodeRef::SvelteElement(n),
        FragmentNode::SvelteFragment(n) => NodeRef::SvelteFragment(n),
        FragmentNode::SvelteBoundary(n) => NodeRef::SvelteBoundary(n),
        FragmentNode::SvelteHead(n) => NodeRef::SvelteHead(n),
        FragmentNode::SvelteOptionsRaw(n) => NodeRef::SvelteOptions(n),
        FragmentNode::SvelteSelf(n) => NodeRef::SvelteSelf(n),
        FragmentNode::SvelteWindow(n) => NodeRef::SvelteWindow(n),
        FragmentNode::EachBlock(n) => NodeRef::EachBlock(n),
        FragmentNode::IfBlock(n) => NodeRef::IfBlock(n),
        FragmentNode::AwaitBlock(n) => NodeRef::AwaitBlock(n),
        FragmentNode::KeyBlock(n) => NodeRef::KeyBlock(n),
        FragmentNode::SnippetBlock(n) => NodeRef::SnippetBlock(n),
    }
}

pub fn attribute_like_ref(a: &AttributeLike) -> NodeRef<'_> {
    match a {
        AttributeLike::Attribute(n) => NodeRef::Attribute(n),
        AttributeLike::SpreadAttribute(n) => NodeRef::SpreadAttribute(n),
        AttributeLike::AttachTag(n) => NodeRef::AttachTag(n),
        AttributeLike::AnimateDirective(n) => NodeRef::AnimateDirective(n),
        AttributeLike::BindDirective(n) => NodeRef::BindDirective(n),
        AttributeLike::ClassDirective(n) => NodeRef::ClassDirective(n),
        AttributeLike::LetDirective(n) => NodeRef::LetDirective(n),
        AttributeLike::OnDirective(n) => NodeRef::OnDirective(n),
        AttributeLike::StyleDirective(n) => NodeRef::StyleDirective(n),
        AttributeLike::TransitionDirective(n) => NodeRef::TransitionDirective(n),
        AttributeLike::UseDirective(n) => NodeRef::UseDirective(n),
    }
}

fn push_children<'a>(node: NodeRef<'a>, c: &mut Children<'a>) {
    use NodeRef as N;
    match node {
        N::Program(n) => c.stmts(&n.body),
        N::ExpressionStatement(n) => c.expr(&n.expression),
        N::BlockStatement(n) => c.stmts(&n.body),
        N::StaticBlock(n) => c.stmts(&n.body),
        N::EmptyStatement(_) | N::DebuggerStatement(_) => {}
        N::WithStatement(n) => {
            c.expr(&n.object);
            c.stmt(&n.body);
        }
        N::ReturnStatement(n) => c.opt_expr(&n.argument),
        N::LabeledStatement(n) => {
            c.ident(&n.label);
            c.stmt(&n.body);
        }
        N::BreakStatement(n) => {
            if let Some(l) = &n.label {
                c.ident(l);
            }
        }
        N::ContinueStatement(n) => {
            if let Some(l) = &n.label {
                c.ident(l);
            }
        }
        N::IfStatement(n) => {
            c.expr(&n.test);
            c.stmt(&n.consequent);
            if let Some(a) = &n.alternate {
                c.stmt(a);
            }
        }
        N::SwitchStatement(n) => {
            c.expr(&n.discriminant);
            for case in &n.cases {
                c.n(N::SwitchCase(case));
            }
        }
        N::SwitchCase(n) => {
            c.opt_expr(&n.test);
            c.stmts(&n.consequent);
        }
        N::ThrowStatement(n) => c.expr(&n.argument),
        N::TryStatement(n) => {
            c.n(N::BlockStatement(&n.block));
            if let Some(h) = &n.handler {
                c.n(N::CatchClause(h));
            }
            if let Some(f) = &n.finalizer {
                c.n(N::BlockStatement(f));
            }
        }
        N::CatchClause(n) => {
            if let Some(p) = &n.param {
                c.pat(p);
            }
            c.n(N::BlockStatement(&n.body));
        }
        N::WhileStatement(n) => {
            c.expr(&n.test);
            c.stmt(&n.body);
        }
        N::DoWhileStatement(n) => {
            c.stmt(&n.body);
            c.expr(&n.test);
        }
        N::ForStatement(n) => {
            match &n.init {
                Some(ForInit::VariableDeclaration(d)) => c.var_decl_node(d),
                Some(ForInit::Expression(e)) => c.expr(e),
                None => {}
            }
            c.opt_expr(&n.test);
            c.opt_expr(&n.update);
            c.stmt(&n.body);
        }
        N::ForInStatement(n) => {
            for_left(c, &n.left);
            c.expr(&n.right);
            c.stmt(&n.body);
        }
        N::ForOfStatement(n) => {
            for_left(c, &n.left);
            c.expr(&n.right);
            c.stmt(&n.body);
        }
        N::FunctionDeclaration(n) | N::FunctionExpression(n) => {
            if let Some(id) = &n.id {
                c.ident(id);
            }
            for p in &n.params {
                c.pat(p);
            }
            c.block_node(&n.body);
        }
        N::ArrowFunctionExpression(n) => {
            if let Some(id) = &n.id {
                c.ident(id);
            }
            for p in &n.params {
                c.pat(p);
            }
            match &n.body {
                ArrowBody::Block(b) => c.block_node(b),
                ArrowBody::Expression(e) => c.expr(e),
            }
        }
        N::VariableDeclaration(n) => {
            for d in &n.declarations {
                c.n(N::VariableDeclarator(d));
            }
        }
        N::VariableDeclarator(n) => {
            c.pat(&n.id);
            c.opt_expr(&n.init);
        }
        N::ClassDeclaration(n) | N::ClassExpression(n) => {
            if let Some(id) = &n.id {
                c.ident(id);
            }
            c.opt_box_expr(&n.super_class);
            c.n(N::ClassBody(&n.body));
        }
        N::ClassBody(n) => {
            for el in &n.body {
                match el {
                    ClassElement::MethodDefinition(m) => c.n(N::MethodDefinition(m)),
                    ClassElement::PropertyDefinition(p) => c.n(N::PropertyDefinition(p)),
                    ClassElement::StaticBlock(s) => c.n(N::StaticBlock(s)),
                    ClassElement::TSIndexSignature(_) => {}
                }
            }
        }
        N::MethodDefinition(n) => {
            c.key(&n.key);
            if let FunctionNode::FunctionExpression(f) = &n.value {
                c.n(N::FunctionExpression(f));
            }
        }
        N::PropertyDefinition(n) => {
            c.key(&n.key);
            c.opt_expr(&n.value);
        }
        N::ImportDeclaration(n) => {
            for s in &n.specifiers {
                match s {
                    ImportSpecifierKind::ImportSpecifier(s) => c.n(N::ImportSpecifier(s)),
                    ImportSpecifierKind::ImportDefaultSpecifier(s) => c.n(N::ImportDefaultSpecifier(s)),
                    ImportSpecifierKind::ImportNamespaceSpecifier(s) => c.n(N::ImportNamespaceSpecifier(s)),
                }
            }
            c.literal(&n.source);
            c.attributes(&n.attributes);
        }
        N::ImportSpecifier(n) => {
            c.module_name(&n.imported);
            c.ident(&n.local);
        }
        N::ImportDefaultSpecifier(n) => c.ident(&n.local),
        N::ImportNamespaceSpecifier(n) => c.ident(&n.local),
        N::ImportAttribute(n) => {
            c.module_name(&n.key);
            c.literal(&n.value);
        }
        N::ExportNamedDeclaration(n) => {
            if let Some(d) = &n.declaration {
                c.stmt(d);
            }
            for s in &n.specifiers {
                c.n(N::ExportSpecifier(s));
            }
            if let Some(s) = &n.source {
                c.literal(s);
            }
            c.attributes(&n.attributes);
        }
        N::ExportSpecifier(n) => {
            c.module_name(&n.local);
            c.module_name(&n.exported);
        }
        N::ExportDefaultDeclaration(n) => match &n.declaration {
            ExportDefaultKind::Declaration(s) => c.stmt(s),
            ExportDefaultKind::Expression(e) => c.expr(e),
        },
        N::ExportAllDeclaration(n) => {
            if let Some(e) = &n.exported {
                c.module_name(e);
            }
            c.literal(&n.source);
            c.attributes(&n.attributes);
        }
        N::Identifier(_) | N::PrivateIdentifier(_) | N::Literal(_) | N::ThisExpression(_) | N::Super(_) => {}
        N::ArrayExpression(n) => {
            for el in n.elements.iter().flatten() {
                c.n(array_element_ref(el));
            }
        }
        N::ObjectExpression(n) => {
            for p in &n.properties {
                match p {
                    ObjectMember::Property(p) => c.n(N::Property(p)),
                    ObjectMember::SpreadElement(s) => c.n(N::SpreadElement(s)),
                }
            }
        }
        N::Property(n) => {
            c.key(&n.key);
            c.expr(&n.value);
        }
        N::PatternProperty(n) => {
            c.key(&n.key);
            c.pat(&n.value);
        }
        N::SpreadElement(n) => c.expr(&n.argument),
        N::TemplateLiteral(n) => {
            // acorn creates `expressions` before `quasis`
            for e in &n.expressions {
                c.expr(e);
            }
            for q in &n.quasis {
                c.n(N::TemplateElement(q));
            }
        }
        N::TemplateElement(_) => {}
        N::TaggedTemplateExpression(n) => {
            c.expr(&n.tag);
            let TemplateLiteralNode::TemplateLiteral(q) = &n.quasi;
            c.n(N::TemplateLiteral(q));
        }
        N::UnaryExpression(n) => c.expr(&n.argument),
        N::UpdateExpression(n) => c.expr(&n.argument),
        N::BinaryExpression(n) => {
            match &n.left {
                BinaryLeft::Private(PrivateIdentifierNode::PrivateIdentifier(p)) => c.n(N::PrivateIdentifier(p)),
                BinaryLeft::Expression(e) => c.expr(e),
            }
            c.expr(&n.right);
        }
        N::LogicalExpression(n) => {
            c.expr(&n.left);
            c.expr(&n.right);
        }
        N::AssignmentExpression(n) => {
            c.pat(&n.left);
            c.expr(&n.right);
        }
        N::ConditionalExpression(n) => {
            c.expr(&n.test);
            c.expr(&n.consequent);
            c.expr(&n.alternate);
        }
        N::CallExpression(n) => {
            c.expr(&n.callee);
            c.args(&n.arguments);
        }
        N::NewExpression(n) => {
            c.expr(&n.callee);
            c.args(&n.arguments);
        }
        N::MemberExpression(n) => {
            c.expr(&n.object);
            c.key(&n.property);
        }
        N::ChainExpression(n) => c.expr(&n.expression),
        N::SequenceExpression(n) => {
            for e in &n.expressions {
                c.expr(e);
            }
        }
        N::YieldExpression(n) => c.opt_box_expr(&n.argument),
        N::AwaitExpression(n) => c.expr(&n.argument),
        N::ImportExpression(n) => {
            c.expr(&n.source);
            c.opt_box_expr(&n.options);
        }
        N::MetaProperty(n) => {
            c.ident(&n.meta);
            c.ident(&n.property);
        }
        N::ParenthesizedExpression(n) => c.expr(&n.expression),
        N::ObjectPattern(n) => {
            for p in &n.properties {
                match p {
                    ObjectPatternMember::Property(p) => c.n(N::PatternProperty(p)),
                    ObjectPatternMember::RestElement(r) => c.n(N::RestElement(r)),
                }
            }
        }
        N::ArrayPattern(n) => {
            for el in n.elements.iter().flatten() {
                c.pat(el);
            }
        }
        N::RestElement(n) => c.pat(&n.argument),
        N::AssignmentPattern(n) => {
            c.pat(&n.left);
            c.expr(&n.right);
        }
        // ---- template
        N::Root(n) => {
            // modern AST key order: css, js (absent), start, end, type, fragment, options, ...
            c.fragment(&n.fragment);
            if let Some(o) = &n.options {
                for a in &o.attributes {
                    c.n(N::Attribute(a));
                }
            }
            if let Some(s) = &n.instance {
                c.n(N::Script(s));
            }
            if let Some(s) = &n.module {
                c.n(N::Script(s));
            }
        }
        N::Script(n) => {
            c.n(N::Program(&n.content));
            for a in &n.attributes {
                c.n(N::Attribute(a));
            }
        }
        N::Fragment(n) => {
            for child in &n.nodes {
                c.n(fragment_node_ref(child));
            }
        }
        N::Text(_) | N::Comment(_) => {}
        N::ExpressionTag(n) => c.expr(&n.expression),
        N::HtmlTag(n) => c.expr(&n.expression),
        N::ConstTag(n) => c.n(N::VariableDeclaration(&n.declaration)),
        N::DeclarationTag(n) => c.n(N::VariableDeclaration(&n.declaration)),
        N::DebugTag(n) => {
            for i in &n.identifiers {
                c.ident(i);
            }
        }
        N::RenderTag(n) => c.expr(&n.expression),
        N::AttachTag(n) => c.expr(&n.expression),
        N::Component(n)
        | N::TitleElement(n)
        | N::SlotElement(n)
        | N::RegularElement(n)
        | N::SvelteBody(n)
        | N::SvelteDocument(n)
        | N::SvelteFragment(n)
        | N::SvelteBoundary(n)
        | N::SvelteHead(n)
        | N::SvelteOptions(n)
        | N::SvelteSelf(n)
        | N::SvelteWindow(n) => {
            c.attrs(&n.attributes);
            c.fragment(&n.fragment);
        }
        N::SvelteComponent(n) => {
            c.attrs(&n.attributes);
            c.fragment(&n.fragment);
            // `expression` is assigned after the element object is created
            c.expr(&n.expression);
        }
        N::SvelteElement(n) => {
            c.attrs(&n.attributes);
            c.fragment(&n.fragment);
            // `tag` is assigned after the element object is created
            c.expr(&n.tag);
        }
        N::EachBlock(n) => {
            // parser key order: expression, body, context, index, key (fallback added later)
            c.expr(&n.expression);
            c.fragment(&n.body);
            if let Some(p) = &n.context {
                c.pat(p);
            }
            c.opt_expr(&n.key);
            if let Some(f) = &n.fallback {
                c.fragment(f);
            }
        }
        N::IfBlock(n) => {
            c.expr(&n.test);
            c.fragment(&n.consequent);
            if let Some(a) = &n.alternate {
                c.fragment(a);
            }
        }
        N::AwaitBlock(n) => {
            c.expr(&n.expression);
            if let Some(p) = &n.value {
                c.pat(p);
            }
            if let Some(p) = &n.error {
                c.pat(p);
            }
            for f in [&n.pending, &n.then, &n.catch].into_iter().flatten() {
                c.fragment(f);
            }
        }
        N::KeyBlock(n) => {
            c.expr(&n.expression);
            c.fragment(&n.fragment);
        }
        N::SnippetBlock(n) => {
            c.ident(&n.expression);
            for p in &n.parameters {
                c.pat(p);
            }
            c.fragment(&n.body);
        }
        N::Attribute(n) => c.attr_value(&n.value),
        N::SpreadAttribute(n) => c.expr(&n.expression),
        N::AnimateDirective(n)
        | N::BindDirective(n)
        | N::ClassDirective(n)
        | N::LetDirective(n)
        | N::OnDirective(n)
        | N::UseDirective(n) => c.opt_expr(&n.expression),
        N::StyleDirective(n) => c.attr_value(&n.value),
        N::TransitionDirective(n) => c.opt_expr(&n.expression),
    }
}

fn for_left<'a>(c: &mut Children<'a>, l: &'a ForLeft) {
    match l {
        ForLeft::VariableDeclaration(d) => c.var_decl_node(d),
        ForLeft::Pattern(p) => c.pat(p),
    }
}

// ---------------------------------------------------------------------------------------------
// walk

/// zimmerframe's visitor object. `visit` is called for every node; the default visits the
/// children with the same state (a node without a visitor).
pub trait Visitor<'a> {
    type State;

    fn visit(&mut self, node: NodeRef<'a>, state: &Self::State, w: &mut Walker<'a>)
    where
        Self: Sized,
    {
        w.next(self, node, state);
    }
}

/// Walk context: `path` holds the ancestors of the node being visited (zimmerframe's `path`).
#[derive(Default)]
pub struct Walker<'a> {
    pub path: Vec<NodeRef<'a>>,
    stopped: bool,
}

impl<'a> Walker<'a> {
    pub fn new() -> Walker<'a> {
        Walker {
            path: Vec::new(),
            stopped: false,
        }
    }

    /// `walk(node, state, visitors)`.
    pub fn walk<V: Visitor<'a>>(node: NodeRef<'a>, state: &V::State, v: &mut V) {
        let mut w = Walker::new();
        w.visit_node(v, node, state);
    }

    /// Visits `node` (the internal `visit` of zimmerframe).
    pub fn visit_node<V: Visitor<'a>>(&mut self, v: &mut V, node: NodeRef<'a>, state: &V::State) {
        if self.stopped {
            return;
        }
        v.visit(node, state, self);
    }

    /// `context.next(state)`: visits the children of `node` (the node currently being visited).
    pub fn next<V: Visitor<'a>>(&mut self, v: &mut V, node: NodeRef<'a>, state: &V::State) {
        self.path.push(node);
        for child in node.children() {
            self.visit_node(v, child, state);
        }
        self.path.pop();
    }

    /// `context.visit(child, state)`: visits `child` from inside the visitor of `node`.
    pub fn visit<V: Visitor<'a>>(&mut self, v: &mut V, node: NodeRef<'a>, child: NodeRef<'a>, state: &V::State) {
        self.path.push(node);
        self.visit_node(v, child, state);
        self.path.pop();
    }

    /// `context.stop()`.
    pub fn stop(&mut self) {
        self.stopped = true;
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    /// `path.at(-1)`.
    pub fn parent(&self) -> Option<NodeRef<'a>> {
        self.path.last().copied()
    }
}
